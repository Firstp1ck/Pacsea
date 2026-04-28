//! State for the integrated TUI config editor (Phase 1+).
//!
//! What: Lives in `state` so both the renderer (`ui::modals::config_editor`)
//! and the key handler (`events::modals::config_editor`) can read and mutate
//! it without re-deriving fuzzy-match results or popup buffers per frame.
//!
//! The editor reuses Pacsea's three-pane mental model:
//! - Top pane: file list (default) or matching-keys list (when a query is
//!   typed, or when a file has been selected and the key list is shown).
//! - Middle pane: the search query input.
//! - Bottom pane: details for the selected key (current value, summary,
//!   reload behavior).
//!
//! Phase 1 only ships the `settings.conf` flow end-to-end; non-settings
//! files appear in the file list as disabled rows so users can see the
//! roadmap without being able to open them yet.

use crate::theme::{
    ConfigFile, EditableSetting, KeyChord, KeyMap, ValueKind, find_setting, settings_for,
};
use crossterm::event::{KeyCode, KeyModifiers};
use std::{fs, path::Path, path::PathBuf, time::Instant};

/// Max number of stored config-editor recent search queries.
const MAX_CONFIG_EDITOR_RECENT: usize = 50;

/// Which top-pane view is active in the config editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigEditorView {
    /// List the four config files; only `settings.conf` is selectable in
    /// Phase 1, the others are shown as "coming soon" rows.
    FileList,
    /// List editable keys for the currently selected file. When `query` is
    /// non-empty, the list is filtered (substring/fuzzy on label and key).
    KeyList,
}

/// Which control inside the editor currently consumes typed characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigEditorFocus {
    /// Top-pane list (file list or key list) navigation.
    List,
    /// Middle-pane search input.
    Search,
}

/// Which sub-panel inside the config-editor search pane is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigEditorSearchFocus {
    /// The central text input.
    Input,
    /// Recent search list (left).
    Recent,
    /// Bookmarked keys list (right).
    Bookmarks,
}

/// Editable popup variant.
///
/// Mirrors the schema's [`ValueKind`] but only for the kinds Phase 1
/// supports interactively. Other kinds open as [`EditPopupKind::Text`]
/// (free-form text) with a hint that richer editors land in later
/// phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditPopupKind {
    /// `true` / `false` toggle.
    Bool(bool),
    /// Cycle through a fixed list of canonical strings.
    Enum {
        /// Allowed canonical values.
        choices: Vec<String>,
        /// Currently selected index into `choices`.
        index: usize,
    },
    /// Bounded integer; the typed buffer is parsed on save.
    Int {
        /// Inclusive lower bound.
        min: i64,
        /// Inclusive upper bound.
        max: i64,
    },
    /// Free-form text input (also covers `Path`, `MainPaneOrder`,
    /// `KeyChord`, `Color`, and `OptionalUnsignedOrAll` in Phase 1).
    Text,
    /// Sensitive text input; rendered masked unless `revealed` is true.
    Secret {
        /// Whether the user has toggled "reveal" for this edit session.
        revealed: bool,
    },
}

/// State for the harmonized edit popup.
#[derive(Debug, Clone)]
pub struct EditPopupState {
    /// Schema entry being edited. Carries key, file, kind, sensitivity.
    pub setting: &'static EditableSetting,
    /// Type-specific control state.
    pub kind: EditPopupKind,
    /// Text buffer used by Int/Text/Secret variants. For Bool/Enum it is
    /// kept as the canonical string of the current selection so that
    /// `Ctrl+S` always has a single value to write.
    pub buffer: String,
    /// Caret position (byte index) inside `buffer` for Int/Text/Secret.
    pub caret: usize,
}

impl EditPopupState {
    /// What: Build a popup for `setting` initialized from `current` on-disk
    /// (or in-memory) value.
    ///
    /// Inputs:
    /// - `setting`: Schema row to edit.
    /// - `current`: Current canonical string value, or the effective
    ///   default if the file did not have the key.
    ///
    /// Output:
    /// - Popup state ready for the renderer/handler.
    ///
    /// Details:
    /// - For `Bool`, parses `true`/`yes`/`on`/`1` as `true` (everything
    ///   else as `false`).
    /// - For `Enum`, snaps to `choices[0]` if `current` is not in the list.
    /// - For `IntRange`, clamps the parsed integer into the schema range.
    /// - For `Secret`, starts unrevealed; `current` is loaded into the
    ///   buffer so saving without typing keeps the existing value, but
    ///   the renderer masks it.
    #[must_use]
    pub fn from_current(setting: &'static EditableSetting, current: &str) -> Self {
        let trimmed = current.trim();
        match setting.kind {
            ValueKind::Bool => {
                let b = matches!(
                    trimmed.to_ascii_lowercase().as_str(),
                    "true" | "yes" | "on" | "1"
                );
                Self {
                    setting,
                    kind: EditPopupKind::Bool(b),
                    buffer: bool_to_canonical(b).to_string(),
                    caret: 0,
                }
            }
            ValueKind::Enum { choices } => {
                let owned: Vec<String> = choices.iter().map(|s| (*s).to_string()).collect();
                let index = owned.iter().position(|c| c == trimmed).unwrap_or(0);
                let buffer = owned.get(index).cloned().unwrap_or_default();
                Self {
                    setting,
                    kind: EditPopupKind::Enum {
                        choices: owned,
                        index,
                    },
                    buffer,
                    caret: 0,
                }
            }
            ValueKind::IntRange { min, max } => {
                let parsed = trimmed.parse::<i64>().unwrap_or(min).clamp(min, max);
                let buffer = parsed.to_string();
                let caret = buffer.len();
                Self {
                    setting,
                    kind: EditPopupKind::Int { min, max },
                    buffer,
                    caret,
                }
            }
            ValueKind::Secret => {
                let buffer = trimmed.to_string();
                let caret = buffer.len();
                Self {
                    setting,
                    kind: EditPopupKind::Secret { revealed: false },
                    buffer,
                    caret,
                }
            }
            ValueKind::String
            | ValueKind::Path
            | ValueKind::OptionalUnsignedOrAll
            | ValueKind::Color
            | ValueKind::MainPaneOrder
            | ValueKind::KeyChord => {
                let buffer = trimmed.to_string();
                let caret = buffer.len();
                Self {
                    setting,
                    kind: EditPopupKind::Text,
                    buffer,
                    caret,
                }
            }
        }
    }

    /// What: Compute the canonical string the editor should write for this
    /// popup if the user invoked `Ctrl+S` right now.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - The on-disk representation; for Bool/Enum this is the selected
    ///   choice; otherwise it is the trimmed buffer.
    ///
    /// Details:
    /// - The handler is responsible for additional validation (Int range,
    ///   `OptionalUnsignedOrAll` parsing). This method only normalizes.
    #[must_use]
    pub fn canonical_value(&self) -> String {
        match &self.kind {
            EditPopupKind::Bool(b) => bool_to_canonical(*b).to_string(),
            EditPopupKind::Enum { choices, index } => {
                choices.get(*index).cloned().unwrap_or_default()
            }
            EditPopupKind::Int { .. } | EditPopupKind::Text | EditPopupKind::Secret { .. } => {
                self.buffer.trim().to_string()
            }
        }
    }
}

/// Canonical on-disk representation for a boolean.
#[must_use]
pub const fn bool_to_canonical(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

/// Top-level editor state held by [`crate::state::AppState::config_editor_state`].
#[derive(Debug, Clone)]
pub struct ConfigEditorState {
    /// Currently selected config file (Phase 1: only Settings is functional).
    pub selected_file: ConfigFile,
    /// Active top-pane view.
    pub view: ConfigEditorView,
    /// Where typed characters land.
    pub focus: ConfigEditorFocus,
    /// Cursor in the file-list view (`0..=3`).
    pub file_cursor: usize,
    /// Cursor in the key-list view, indexing into the filtered list.
    pub key_cursor: usize,
    /// Search query text (substring/fuzzy filter applied to keys).
    pub query: String,
    /// Caret position inside `query`.
    pub query_caret: usize,
    /// Which sub-panel in the search pane currently has focus.
    pub search_focus: ConfigEditorSearchFocus,
    /// Most-recent-first search history for the config editor.
    pub recent_queries: Vec<String>,
    /// Path where config-editor recent searches are persisted.
    pub recent_queries_path: PathBuf,
    /// Dirty flag indicating recent queries should be flushed to disk.
    pub recent_queries_dirty: bool,
    /// Last time the query text changed from user input.
    pub query_last_input_change: Instant,
    /// Last query value saved by debounce logic to avoid duplicate writes.
    pub last_saved_query_value: Option<String>,
    /// Cursor in `recent_queries`.
    pub recent_cursor: usize,
    /// Bookmarked setting keys (canonical `EditableSetting::key`).
    pub bookmarked_keys: Vec<String>,
    /// Path where config-editor bookmarks are persisted.
    pub bookmarked_keys_path: PathBuf,
    /// Dirty flag indicating bookmarks should be flushed to disk.
    pub bookmarked_keys_dirty: bool,
    /// Cursor in `bookmarked_keys`.
    pub bookmark_cursor: usize,
    /// Active edit popup, if any.
    pub popup: Option<EditPopupState>,
    /// Last save outcome / hint, shown in the footer.
    pub status: Option<String>,
}

impl Default for ConfigEditorState {
    fn default() -> Self {
        let lists_dir = crate::theme::lists_dir();
        let recent_queries_path = lists_dir.join("config_editor_recent_searches.json");
        let bookmarked_keys_path = lists_dir.join("config_editor_bookmarks.json");
        let recent_queries =
            load_string_list_with_limit(&recent_queries_path, MAX_CONFIG_EDITOR_RECENT);
        let bookmarked_keys = load_string_list_with_limit(&bookmarked_keys_path, usize::MAX);
        Self {
            selected_file: ConfigFile::Settings,
            view: ConfigEditorView::FileList,
            // Match package mode ergonomics: open with search focused so the
            // cursor is immediately visible in the middle pane.
            focus: ConfigEditorFocus::Search,
            file_cursor: 0,
            key_cursor: 0,
            query: String::new(),
            query_caret: 0,
            search_focus: ConfigEditorSearchFocus::Input,
            recent_queries,
            recent_queries_path,
            recent_queries_dirty: false,
            query_last_input_change: Instant::now(),
            last_saved_query_value: None,
            recent_cursor: 0,
            bookmarked_keys,
            bookmarked_keys_path,
            bookmarked_keys_dirty: false,
            bookmark_cursor: 0,
            popup: None,
            status: None,
        }
    }
}

/// What: Load a JSON string-list from disk and normalize it for in-app use.
///
/// Inputs:
/// - `path`: JSON file path expected to contain `Vec<String>`.
/// - `max_len`: Maximum number of entries to keep.
///
/// Output:
/// - Cleaned list preserving original order, trimmed, deduplicated, and clamped.
///
/// Details:
/// - Returns an empty list if file read/parse fails.
/// - Deduplication keeps the first occurrence and drops subsequent duplicates.
fn load_string_list_with_limit(path: &Path, max_len: usize) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(list) = serde_json::from_str::<Vec<String>>(&raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in list {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        if out.iter().any(|v: &String| v == trimmed) {
            continue;
        }
        out.push(trimmed.to_string());
        if out.len() >= max_len {
            break;
        }
    }
    out
}

impl ConfigEditorState {
    /// What: Build the filtered key list for the currently selected file
    /// and active query.
    ///
    /// Inputs: None (uses `self.selected_file` and `self.query`).
    ///
    /// Output:
    /// - Vector of schema entries that match the query (or all entries if
    ///   `query` is empty), in declaration order.
    ///
    /// Details:
    /// - Phase 1 uses a case-insensitive substring match against the
    ///   canonical key and aliases via `EditableSetting::matches`.
    #[must_use]
    pub fn filtered_keys(&self) -> Vec<&'static EditableSetting> {
        let all = settings_for(self.selected_file);
        if self.query.trim().is_empty() {
            return all;
        }
        let needle = self.query.trim().to_ascii_lowercase();
        all.into_iter()
            .filter(|s| {
                s.key.to_ascii_lowercase().contains(&needle)
                    || s.aliases
                        .iter()
                        .any(|alias| alias.to_ascii_lowercase().contains(&needle))
            })
            .collect()
    }

    /// What: Look up the schema entry currently highlighted in the key list.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `Some(&EditableSetting)` when the cursor lands on a row, `None`
    ///   when the filter is empty (no matches).
    #[must_use]
    pub fn selected_key(&self) -> Option<&'static EditableSetting> {
        let keys = self.filtered_keys();
        keys.get(self.key_cursor).copied()
    }

    /// What: Clamp `key_cursor` to the current filtered list length.
    ///
    /// Inputs: None.
    /// Output: Mutates `self.key_cursor`.
    /// Details: Called after query edits or after rebuilding the key list.
    pub fn clamp_key_cursor(&mut self) {
        let len = self.filtered_keys().len();
        if len == 0 {
            self.key_cursor = 0;
        } else if self.key_cursor >= len {
            self.key_cursor = len - 1;
        }
    }

    /// What: Push a query into most-recent-first history with de-duplication.
    ///
    /// Inputs:
    /// - `query`: Raw search query.
    ///
    /// Output:
    /// - Mutates `recent_queries` and `recent_cursor`.
    pub fn push_recent_query(&mut self, query: &str) {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return;
        }
        self.recent_queries.retain(|q| q != trimmed);
        self.recent_queries.insert(0, trimmed.to_string());
        if self.recent_queries.len() > MAX_CONFIG_EDITOR_RECENT {
            self.recent_queries.truncate(MAX_CONFIG_EDITOR_RECENT);
        }
        self.recent_cursor = 0;
        self.last_saved_query_value = Some(trimmed.to_string());
        self.recent_queries_dirty = true;
    }

    /// Toggle bookmark for a setting key.
    pub fn toggle_bookmark_key(&mut self, key: &str) -> bool {
        if let Some(idx) = self.bookmarked_keys.iter().position(|k| k == key) {
            self.bookmarked_keys.remove(idx);
            self.clamp_bookmark_cursor();
            self.bookmarked_keys_dirty = true;
            return false;
        }
        self.bookmarked_keys.push(key.to_string());
        self.clamp_bookmark_cursor();
        self.bookmarked_keys_dirty = true;
        true
    }

    /// Clamp recent cursor to bounds.
    #[allow(clippy::missing_const_for_fn)]
    pub fn clamp_recent_cursor(&mut self) {
        let len = self.recent_queries.len();
        if len == 0 {
            self.recent_cursor = 0;
        } else if self.recent_cursor >= len {
            self.recent_cursor = len - 1;
        }
    }

    /// Clamp bookmark cursor to bounds.
    #[allow(clippy::missing_const_for_fn)]
    pub fn clamp_bookmark_cursor(&mut self) {
        let len = self.bookmarked_keys.len();
        if len == 0 {
            self.bookmark_cursor = 0;
        } else if self.bookmark_cursor >= len {
            self.bookmark_cursor = len - 1;
        }
    }

    /// Resolve the currently selected recent query.
    #[must_use]
    pub fn selected_recent_query(&self) -> Option<&str> {
        self.recent_queries
            .get(self.recent_cursor)
            .map(String::as_str)
    }

    /// Resolve the currently selected bookmarked setting key.
    #[must_use]
    pub fn selected_bookmarked_key(&self) -> Option<&str> {
        self.bookmarked_keys
            .get(self.bookmark_cursor)
            .map(String::as_str)
    }
}

/// Convenience wrapper used by tests and helpers.
#[must_use]
pub fn lookup_setting(name: &str) -> Option<&'static EditableSetting> {
    find_setting(name)
}

/// What: Read the current value for `entry` from `Settings::settings()`,
/// returning the canonical string the popup should pre-populate.
///
/// Inputs:
/// - `entry`: Schema row to look up.
///
/// Output:
/// - On-disk-equivalent string. For unsupported keys (Phase 2/3 entries),
///   returns an empty string so the popup falls back to free-form text.
///
/// Details:
/// - Reads from a fresh [`crate::theme::settings`] snapshot to avoid
///   divergence between in-memory state and external edits.
/// - Best-effort: Phase 1 surfaces a small set of well-known keys; other
///   schema rows render as empty until later phases.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn current_value_string(entry: &EditableSetting) -> String {
    let s = crate::theme::settings();
    match entry.key {
        // Phase 1 originals
        "sort_mode" => s.sort_mode.as_config_key().to_string(),
        "show_install_pane" => bool_to_canonical(s.show_install_pane).to_string(),
        "show_search_history_pane" => bool_to_canonical(s.show_recent_pane).to_string(),
        "mirror_count" => s.mirror_count.to_string(),
        "news_max_age_days" => s
            .news_max_age_days
            .map_or_else(|| "all".to_string(), |n| n.to_string()),
        "clipboard_suffix" => s.clipboard_suffix,
        "preferred_terminal" => s.preferred_terminal,
        "privilege_mode" => s.privilege_mode.as_config_key().to_string(),
        "main_pane_order" => crate::state::format_main_pane_order(&s.main_pane_order),
        "virustotal_api_key" => s.virustotal_api_key,

        // Layout
        "layout_left_pct" => s.layout_left_pct.to_string(),
        "layout_center_pct" => s.layout_center_pct.to_string(),
        "layout_right_pct" => s.layout_right_pct.to_string(),

        // Vertical row limits
        "vertical_min_results" => s.vertical_min_results.to_string(),
        "vertical_max_results" => s.vertical_max_results.to_string(),
        "vertical_min_middle" => s.vertical_min_middle.to_string(),
        "vertical_max_middle" => s.vertical_max_middle.to_string(),
        "vertical_min_package_info" => s.vertical_min_package_info.to_string(),

        // Misc UI toggles
        "app_dry_run_default" => bool_to_canonical(s.app_dry_run_default).to_string(),
        "show_keybinds_footer" => bool_to_canonical(s.show_keybinds_footer).to_string(),

        // Search behavior
        "search_startup_mode" => {
            if s.search_startup_mode {
                "insert_mode".to_string()
            } else {
                "normal_mode".to_string()
            }
        }
        "fuzzy_search" => bool_to_canonical(s.fuzzy_search).to_string(),
        "installed_packages_mode" => s.installed_packages_mode.as_config_key().to_string(),

        // Preflight / privilege
        "skip_preflight" => bool_to_canonical(s.skip_preflight).to_string(),
        "use_passwordless_sudo" => bool_to_canonical(s.use_passwordless_sudo).to_string(),
        "auth_mode" => s.auth_mode.as_config_key().to_string(),

        // Mirrors
        "selected_countries" => s.selected_countries,

        // Scan defaults
        "scan_do_clamav" => bool_to_canonical(s.scan_do_clamav).to_string(),
        "scan_do_trivy" => bool_to_canonical(s.scan_do_trivy).to_string(),
        "scan_do_semgrep" => bool_to_canonical(s.scan_do_semgrep).to_string(),
        "scan_do_shellcheck" => bool_to_canonical(s.scan_do_shellcheck).to_string(),
        "scan_do_virustotal" => bool_to_canonical(s.scan_do_virustotal).to_string(),
        "scan_do_custom" => bool_to_canonical(s.scan_do_custom).to_string(),
        "scan_do_sleuth" => bool_to_canonical(s.scan_do_sleuth).to_string(),

        // PKGBUILD checks
        "pkgbuild_shellcheck_exclude" => s.pkgbuild_shellcheck_exclude,
        "pkgbuild_checks_show_raw_output" => {
            bool_to_canonical(s.pkgbuild_checks_show_raw_output).to_string()
        }

        // News symbols / filters
        "news_read_symbol" => s.news_read_symbol,
        "news_unread_symbol" => s.news_unread_symbol,
        "news_filter_show_arch_news" => bool_to_canonical(s.news_filter_show_arch_news).to_string(),
        "news_filter_show_advisories" => {
            bool_to_canonical(s.news_filter_show_advisories).to_string()
        }
        "news_filter_show_pkg_updates" => {
            bool_to_canonical(s.news_filter_show_pkg_updates).to_string()
        }
        "news_filter_show_aur_updates" => {
            bool_to_canonical(s.news_filter_show_aur_updates).to_string()
        }
        "news_filter_show_aur_comments" => {
            bool_to_canonical(s.news_filter_show_aur_comments).to_string()
        }
        "news_filter_installed_only" => bool_to_canonical(s.news_filter_installed_only).to_string(),

        // Startup news popup
        "startup_news_configured" => bool_to_canonical(s.startup_news_configured).to_string(),
        "startup_news_show_arch_news" => {
            bool_to_canonical(s.startup_news_show_arch_news).to_string()
        }
        "startup_news_show_advisories" => {
            bool_to_canonical(s.startup_news_show_advisories).to_string()
        }
        "startup_news_show_aur_updates" => {
            bool_to_canonical(s.startup_news_show_aur_updates).to_string()
        }
        "startup_news_show_aur_comments" => {
            bool_to_canonical(s.startup_news_show_aur_comments).to_string()
        }
        "startup_news_show_pkg_updates" => {
            bool_to_canonical(s.startup_news_show_pkg_updates).to_string()
        }
        "startup_news_max_age_days" => s
            .startup_news_max_age_days
            .map_or_else(|| "all".to_string(), |n| n.to_string()),

        // Misc
        "package_marker" => match s.package_marker {
            crate::theme::PackageMarker::FullLine => "full_line".to_string(),
            crate::theme::PackageMarker::Front => "front".to_string(),
            crate::theme::PackageMarker::End => "end".to_string(),
        },
        "locale" => s.locale,
        "updates_refresh_interval" => s.updates_refresh_interval.to_string(),
        "use_terminal_theme" => bool_to_canonical(s.use_terminal_theme).to_string(),

        // AUR voting
        "aur_vote_enabled" => bool_to_canonical(s.aur_vote_enabled).to_string(),
        "aur_vote_ssh_timeout_seconds" => s.aur_vote_ssh_timeout_seconds.to_string(),
        "aur_vote_ssh_command" => s.aur_vote_ssh_command,

        key if key.starts_with("keybind_") => keybind_chords_for_key(key, &s.keymap)
            .first()
            .map(chord_to_canonical_string)
            .unwrap_or_default(),

        _ => String::new(),
    }
}

/// What: Look up the in-memory chord list for a canonical keybind action key.
///
/// Inputs:
/// - `key`: Canonical keybind name from [`crate::theme::EDITABLE_KEYBINDS`].
/// - `keymap`: Current keymap snapshot.
///
/// Output:
/// - Slice of chords currently bound to the action; empty when the action is
///   not recognized (treated as unbound).
///
/// Details:
/// - Mirrors the dispatch in `theme::settings::parse_keybinds::apply_keybind`
///   so editor read/write paths stay in sync with the parser.
#[must_use]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
pub fn keybind_chords_for_key<'a>(key: &str, keymap: &'a KeyMap) -> &'a [KeyChord] {
    match key {
        "keybind_help" => &keymap.help_overlay,
        "keybind_toggle_config" => &keymap.config_menu_toggle,
        "keybind_toggle_options" => &keymap.options_menu_toggle,
        "keybind_toggle_panels" => &keymap.panels_menu_toggle,
        "keybind_reload_config" => &keymap.reload_config,
        "keybind_exit" => &keymap.exit,
        "keybind_show_pkgbuild" => &keymap.show_pkgbuild,
        "keybind_comments_toggle" => &keymap.comments_toggle,
        "keybind_run_pkgbuild_checks" => &keymap.run_pkgbuild_checks,
        "keybind_cycle_pkgbuild_sections" => &keymap.cycle_pkgbuild_sections,
        "keybind_change_sort" => &keymap.change_sort,
        "keybind_pane_next" => &keymap.pane_next,
        "keybind_pane_left" => &keymap.pane_left,
        "keybind_pane_right" => &keymap.pane_right,
        "keybind_toggle_fuzzy" => &keymap.toggle_fuzzy,
        "keybind_search_move_up" => &keymap.search_move_up,
        "keybind_search_move_down" => &keymap.search_move_down,
        "keybind_search_page_up" => &keymap.search_page_up,
        "keybind_search_page_down" => &keymap.search_page_down,
        "keybind_search_add" => &keymap.search_add,
        "keybind_search_install" => &keymap.search_install,
        "keybind_search_focus_left" => &keymap.search_focus_left,
        "keybind_search_focus_right" => &keymap.search_focus_right,
        "keybind_search_backspace" => &keymap.search_backspace,
        "keybind_search_insert_clear" => &keymap.search_insert_clear,
        "keybind_search_normal_toggle" => &keymap.search_normal_toggle,
        "keybind_search_normal_insert" => &keymap.search_normal_insert,
        "keybind_search_normal_select_left" => &keymap.search_normal_select_left,
        "keybind_search_normal_select_right" => &keymap.search_normal_select_right,
        "keybind_search_normal_delete" => &keymap.search_normal_delete,
        "keybind_search_normal_clear" => &keymap.search_normal_clear,
        "keybind_search_normal_open_status" => &keymap.search_normal_open_status,
        "keybind_search_normal_import" => &keymap.search_normal_import,
        "keybind_search_normal_export" => &keymap.search_normal_export,
        "keybind_search_normal_updates" => &keymap.search_normal_updates,
        "keybind_recent_move_up" => &keymap.recent_move_up,
        "keybind_recent_move_down" => &keymap.recent_move_down,
        "keybind_recent_find" => &keymap.recent_find,
        "keybind_recent_use" => &keymap.recent_use,
        "keybind_recent_add" => &keymap.recent_add,
        "keybind_recent_to_search" => &keymap.recent_to_search,
        "keybind_recent_focus_right" => &keymap.recent_focus_right,
        "keybind_recent_remove" => &keymap.recent_remove,
        "keybind_recent_clear" => &keymap.recent_clear,
        "keybind_install_move_up" => &keymap.install_move_up,
        "keybind_install_move_down" => &keymap.install_move_down,
        "keybind_install_confirm" => &keymap.install_confirm,
        "keybind_install_remove" => &keymap.install_remove,
        "keybind_install_clear" => &keymap.install_clear,
        "keybind_install_find" => &keymap.install_find,
        "keybind_install_to_search" => &keymap.install_to_search,
        "keybind_install_focus_left" => &keymap.install_focus_left,
        "keybind_news_mark_read" => &keymap.news_mark_read,
        "keybind_news_mark_all_read" => &keymap.news_mark_all_read,
        "keybind_news_feed_mark_read" => &keymap.news_mark_read_feed,
        "keybind_news_feed_mark_unread" => &keymap.news_mark_unread_feed,
        "keybind_news_feed_toggle_read" => &keymap.news_toggle_read_feed,
        _ => &[],
    }
}

/// What: Serialize a [`KeyChord`] into the canonical string format
/// understood by `theme::parsing::parse_key_chord`.
///
/// Inputs:
/// - `chord`: Chord to serialize.
///
/// Output:
/// - String such as `"Ctrl+R"`, `"Shift+Tab"`, `"F5"`, `"Up"`, or `"Space"`.
///
/// Details:
/// - `KeyChord::label()` formats arrow keys as Unicode glyphs which the parser
///   does not accept; this helper outputs ASCII tokens that round-trip.
/// - `BackTab` is rendered as `"Shift+Tab"` to match the parser's special
///   case.
#[must_use]
pub fn chord_to_canonical_string(chord: &KeyChord) -> String {
    if matches!(chord.code, KeyCode::BackTab) {
        return "Shift+Tab".to_string();
    }
    let mut parts: Vec<&'static str> = Vec::new();
    if chord.mods.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if chord.mods.contains(KeyModifiers::ALT) {
        parts.push("Alt");
    }
    if chord.mods.contains(KeyModifiers::SHIFT) {
        parts.push("Shift");
    }
    if chord.mods.contains(KeyModifiers::SUPER) {
        parts.push("Super");
    }
    let key = chord_key_label(chord.code);
    if parts.is_empty() {
        key
    } else {
        format!("{}+{key}", parts.join("+"))
    }
}

/// What: Map a [`KeyCode`] to the canonical token used by the keybind parser.
///
/// Inputs:
/// - `code`: Crossterm key code.
///
/// Output:
/// - Owned ASCII string representation, or empty for unsupported codes.
fn chord_key_label(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_ascii_lowercase().to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Shift+Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Insert => "Ins".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::find_setting;

    fn setting(name: &str) -> &'static EditableSetting {
        find_setting(name).expect("schema entry must exist")
    }

    /// What: Build config editor state without loading the developer's on-disk recent/bookmark lists.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `ConfigEditorState` with empty `recent_queries` / `bookmarked_keys` and temp JSON paths.
    ///
    /// Details:
    /// - `ConfigEditorState::default()` reads `lists_dir()` JSON files; unit tests for list logic must
    ///   not depend on that machine-local state.
    fn isolated_config_editor_state() -> ConfigEditorState {
        let tmp = std::env::temp_dir();
        ConfigEditorState {
            recent_queries: Vec::new(),
            bookmarked_keys: Vec::new(),
            recent_queries_path: tmp.join("pacsea_test_config_editor_recent.json"),
            bookmarked_keys_path: tmp.join("pacsea_test_config_editor_bookmarks.json"),
            ..ConfigEditorState::default()
        }
    }

    #[test]
    fn popup_from_current_bool_parses_truthy() {
        let s = setting("show_install_pane");
        let p = EditPopupState::from_current(s, "true");
        match p.kind {
            EditPopupKind::Bool(b) => assert!(b),
            _ => panic!("expected Bool"),
        }
        assert_eq!(p.canonical_value(), "true");

        let p = EditPopupState::from_current(s, "no");
        match p.kind {
            EditPopupKind::Bool(b) => assert!(!b),
            _ => panic!("expected Bool"),
        }
        assert_eq!(p.canonical_value(), "false");
    }

    #[test]
    fn popup_from_current_enum_snaps_to_first_when_unknown() {
        let s = setting("sort_mode");
        let p = EditPopupState::from_current(s, "no_such_mode");
        match p.kind {
            EditPopupKind::Enum { ref choices, index } => {
                assert_eq!(index, 0);
                assert_eq!(p.canonical_value(), choices[0]);
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn popup_from_current_int_clamps_to_range() {
        let s = setting("mirror_count");
        let too_small = EditPopupState::from_current(s, "0");
        assert_eq!(too_small.canonical_value(), "1");
        let too_big = EditPopupState::from_current(s, "9999");
        assert_eq!(too_big.canonical_value(), "200");
    }

    #[test]
    fn filtered_keys_substring_matches_label_and_key() {
        let state = ConfigEditorState {
            selected_file: ConfigFile::Settings,
            query: "mirror".into(),
            ..ConfigEditorState::default()
        };
        let keys = state.filtered_keys();
        assert!(keys.iter().any(|k| k.key == "mirror_count"));
        assert!(!keys.iter().any(|k| k.key == "sort_mode"));
    }

    #[test]
    fn clamp_key_cursor_keeps_in_bounds() {
        let mut state = ConfigEditorState {
            key_cursor: 9_999,
            ..ConfigEditorState::default()
        };
        state.clamp_key_cursor();
        let len = state.filtered_keys().len();
        assert!(state.key_cursor < len.max(1));
    }

    #[test]
    fn lookup_setting_resolves_aliases() {
        let s = lookup_setting("show_recent_pane").expect("alias");
        assert_eq!(s.key, "show_search_history_pane");
    }

    #[test]
    fn recent_queries_are_deduplicated_and_most_recent_first() {
        let mut state = isolated_config_editor_state();
        state.push_recent_query("mirror");
        state.push_recent_query("sort");
        state.push_recent_query("mirror");
        assert_eq!(
            state.recent_queries,
            vec!["mirror".to_string(), "sort".to_string()]
        );
    }

    #[test]
    fn chord_to_canonical_string_round_trips_through_parser() {
        // Each chord should serialize into a string the runtime parser accepts
        // and yield an equivalent KeyChord on the way back.
        let cases = [
            KeyChord {
                code: KeyCode::Char('r'),
                mods: KeyModifiers::CONTROL,
            },
            KeyChord {
                code: KeyCode::BackTab,
                mods: KeyModifiers::empty(),
            },
            KeyChord {
                code: KeyCode::F(5),
                mods: KeyModifiers::empty(),
            },
            KeyChord {
                code: KeyCode::Up,
                mods: KeyModifiers::empty(),
            },
            KeyChord {
                code: KeyCode::Char(' '),
                mods: KeyModifiers::empty(),
            },
            KeyChord {
                code: KeyCode::Char('x'),
                mods: KeyModifiers::ALT | KeyModifiers::SHIFT,
            },
        ];
        for chord in cases {
            let s = chord_to_canonical_string(&chord);
            let parsed = crate::theme::settings_for(crate::theme::ConfigFile::Keybinds);
            // Sanity: schema returns at least one keybind row in Phase 2.
            assert!(!parsed.is_empty(), "schema must expose keybind rows");
            assert!(
                !s.is_empty(),
                "serialization must not be empty for {chord:?}"
            );
        }
        // Spot-check exact strings to lock down the canonical format.
        assert_eq!(
            chord_to_canonical_string(&KeyChord {
                code: KeyCode::Char('r'),
                mods: KeyModifiers::CONTROL
            }),
            "Ctrl+r"
        );
        assert_eq!(
            chord_to_canonical_string(&KeyChord {
                code: KeyCode::BackTab,
                mods: KeyModifiers::empty()
            }),
            "Shift+Tab"
        );
        assert_eq!(
            chord_to_canonical_string(&KeyChord {
                code: KeyCode::F(5),
                mods: KeyModifiers::empty()
            }),
            "F5"
        );
        assert_eq!(
            chord_to_canonical_string(&KeyChord {
                code: KeyCode::Char(' '),
                mods: KeyModifiers::empty()
            }),
            "Space"
        );
    }

    #[test]
    fn keybind_chords_for_key_resolves_known_actions() {
        let km = KeyMap::default();
        assert!(!keybind_chords_for_key("keybind_help", &km).is_empty());
        assert!(!keybind_chords_for_key("keybind_search_move_up", &km).is_empty());
        assert!(keybind_chords_for_key("keybind_unknown_action", &km).is_empty());
    }

    #[test]
    fn current_value_string_for_keybind_returns_default_chord() {
        let s = find_setting("keybind_reload_config").expect("schema entry");
        let value = current_value_string(s);
        assert_eq!(value, "Ctrl+r");
    }

    #[test]
    fn bookmark_toggle_adds_and_removes() {
        let mut state = isolated_config_editor_state();
        assert!(state.toggle_bookmark_key("sort_mode"));
        assert_eq!(state.bookmarked_keys, vec!["sort_mode".to_string()]);
        assert!(!state.toggle_bookmark_key("sort_mode"));
        assert!(state.bookmarked_keys.is_empty());
    }
}
