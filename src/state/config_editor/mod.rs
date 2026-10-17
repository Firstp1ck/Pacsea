//! Configuration tabs, filtering, popup state, and persisted editor history.

use crate::theme::{ConfigFile, EditableSetting, KeyChord, ValueKind, find_setting, settings_for};
use crossterm::event::{KeyCode, KeyModifiers};
use std::{collections::HashMap, fs, path::Path, path::PathBuf, time::Instant};

/// Display grouping and ordering.
mod groups;
/// Deterministic dropdown candidates.
pub mod suggestions;
/// Shared popup validation.
pub mod validate;
/// Effective values and defaults.
mod values;
pub use groups::{SettingGroup, setting_group};
pub use suggestions::{Suggestion, SuggestionContext, SuggestionTag, suggestions_for};
pub use validate::validate_value;
pub use values::{
    ValueSnapshot, color_to_config_string, current_value_string, keybind_chords_for_key,
    theme_color_value_string,
};

/// Max number of stored config-editor recent search queries.
const MAX_CONFIG_EDITOR_RECENT: usize = 50;

/// Maximum suggestion rows visible at once and moved by popup page navigation.
pub const DROPDOWN_PAGE_ROWS: usize = 8;

/// What: Find the best choice for a stored enum value.
///
/// Inputs:
/// - `choices`: Canonical choices in display order.
/// - `current`: Trimmed stored value to preselect.
///
/// Output:
/// - Matching choice index, or zero when no choice matches.
///
/// Details:
/// - Exact spelling takes precedence; fallback ignores ASCII case and treats
///   underscores as hyphens, without changing the stored original value.
fn enum_choice_index(choices: &[String], current: &str) -> usize {
    if let Some(index) = choices.iter().position(|choice| choice == current) {
        return index;
    }
    let normalized = current.to_ascii_lowercase().replace('_', "-");
    choices
        .iter()
        .position(|choice| choice.to_ascii_lowercase().replace('_', "-") == normalized)
        .unwrap_or(0)
}

/// Which top-pane view is active in the config editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigEditorView {
    /// Bookmarked keys across all editable files.
    Starred,
    /// List editable keys for the currently selected file. When `query` is
    /// non-empty, the list is filtered (substring/fuzzy on label and key).
    KeyList,
}

/// Editable popup variant.
///
/// Mirrors the schema's [`ValueKind`] but only for the kinds the editor
/// supports interactively. Kinds without a dedicated control open as
/// [`EditPopupKind::Text`] (free-form text); `keybinds.conf` rows open as
/// [`EditPopupKind::KeyChord`] with optional chord capture.
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
    /// Free-form text input (also covers `Path`,
    /// `Color`, and `OptionalUnsignedOrAll` in Phase 1).
    Text,
    /// Sensitive text input; rendered masked unless `revealed` is true.
    Secret {
        /// Whether the user has toggled "reveal" for this edit session.
        revealed: bool,
    },
    /// Key-chord input for `keybinds.conf` rows. Supports free-form text
    /// editing plus a dedicated capture mode (`Ctrl+K`) that records the
    /// next pressed chord verbatim.
    KeyChord {
        /// When `true`, the next key event is swallowed and serialized into
        /// the buffer instead of being interpreted as popup input.
        capturing: bool,
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
    /// Canonical value the popup opened with; `Ctrl+Z` resets back to it.
    pub original: String,
    /// All deduplicated dropdown candidates.
    pub suggestions: Vec<Suggestion>,
    /// Indices of candidates matching the current token.
    pub visible: Vec<usize>,
    /// Position within visible; None saves the typed buffer.
    pub highlight: Option<usize>,
    /// First visible dropdown row.
    pub scroll: usize,
    /// Whether input filters the dropdown.
    pub filter_active: bool,
    /// Validation of the effective value, recomputed after edits.
    pub validation: Result<(), String>,
    /// Target file's modification time when the popup opened. Used to warn
    /// when the file changed on disk while the popup was editing.
    pub opened_mtime: Option<std::time::SystemTime>,
    /// Set after the first save attempt hit an mtime mismatch; the next
    /// save proceeds anyway (explicit user confirmation via repeat `Ctrl+S`).
    pub stale_overwrite_armed: bool,
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
    /// - For `Enum`, tries exact, then ASCII-case/underscore-tolerant matching;
    ///   snaps to `choices[0]` when no match is found.
    /// - For `IntRange`, clamps the parsed integer into the schema range.
    /// - For `Secret`, starts unrevealed; `current` is loaded into the
    ///   buffer so saving without typing keeps the existing value, but
    ///   the renderer masks it.
    #[must_use]
    pub fn from_current(setting: &'static EditableSetting, current: &str) -> Self {
        let trimmed = current.trim();
        let (kind, buffer, caret) = match setting.kind {
            ValueKind::Bool => {
                let b = matches!(
                    trimmed.to_ascii_lowercase().as_str(),
                    "true" | "yes" | "on" | "1"
                );
                (EditPopupKind::Bool(b), bool_to_canonical(b).to_string(), 0)
            }
            ValueKind::MainPaneOrder => {
                let owned = suggestions::main_pane_choices();
                let normalized = crate::state::parse_main_pane_order(trimmed)
                    .map(|order| crate::state::format_main_pane_order(&order))
                    .unwrap_or_default();
                let index = enum_choice_index(&owned, &normalized);
                let buffer = owned[index].clone();
                (
                    EditPopupKind::Enum {
                        choices: owned,
                        index,
                    },
                    buffer,
                    0,
                )
            }
            ValueKind::Enum { choices } => {
                let owned: Vec<String> = choices.iter().map(|s| (*s).to_string()).collect();
                let index = enum_choice_index(&owned, trimmed);
                let buffer = owned.get(index).cloned().unwrap_or_default();
                (
                    EditPopupKind::Enum {
                        choices: owned,
                        index,
                    },
                    buffer,
                    0,
                )
            }
            ValueKind::IntRange { min, max } => {
                let parsed = trimmed.parse::<i64>().unwrap_or(min).clamp(min, max);
                let buffer = parsed.to_string();
                let caret = buffer.len();
                (EditPopupKind::Int { min, max }, buffer, caret)
            }
            ValueKind::Secret => (
                EditPopupKind::Secret { revealed: false },
                trimmed.to_string(),
                trimmed.len(),
            ),
            ValueKind::KeyChord => (
                EditPopupKind::KeyChord { capturing: false },
                trimmed.to_string(),
                trimmed.len(),
            ),
            ValueKind::String
            | ValueKind::Path
            | ValueKind::OptionalUnsignedOrAll
            | ValueKind::Color => (EditPopupKind::Text, trimmed.to_string(), trimmed.len()),
        };
        let mut popup = Self {
            setting,
            kind,
            buffer,
            caret,
            original: trimmed.to_string(),
            suggestions: Vec::new(),
            visible: Vec::new(),
            highlight: None,
            scroll: 0,
            filter_active: false,
            validation: Ok(()),

            opened_mtime: None,
            stale_overwrite_armed: false,
        };
        popup.set_suggestions(suggestions_for(
            setting,
            &SuggestionContext {
                current: trimmed.to_string(),
                default: trimmed.to_string(),
                ..SuggestionContext::default()
            },
        ));
        popup
    }

    /// What: Select an enum choice and synchronize its canonical buffer.
    ///
    /// Inputs:
    /// - `index`: Index of the desired choice.
    ///
    /// Output:
    /// - `true` when the popup is an enum and the index is in range; otherwise
    ///   `false` without changing the popup.
    ///
    /// Details:
    /// - Both keyboard and mouse selection use this method so saves use the
    ///   same canonical string as the highlighted choice.
    pub fn select_enum_index(&mut self, index: usize) -> bool {
        let EditPopupKind::Enum {
            choices,
            index: selected,
        } = &mut self.kind
        else {
            return false;
        };
        let Some(choice) = choices.get(index) else {
            return false;
        };
        *selected = index;
        self.buffer.clone_from(choice);
        self.caret = self.buffer.len();
        self.filter_active = false;
        self.refresh_visible();
        true
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
            EditPopupKind::Int { .. }
            | EditPopupKind::Text
            | EditPopupKind::Secret { .. }
            | EditPopupKind::KeyChord { .. } => self.buffer.trim().to_string(),
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
    /// File selected by a regular tab; retained while Starred is active.
    pub selected_file: ConfigFile,
    /// Active top-pane view.
    pub view: ConfigEditorView,
    /// Cursor in the key-list view, indexing into the filtered list.
    pub key_cursor: usize,
    /// Search query; every whitespace-separated term must match.
    pub query: String,
    /// Caret position inside `query`.
    pub query_caret: usize,
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
    /// Bookmarked setting keys (canonical `EditableSetting::key`).
    pub bookmarked_keys: Vec<String>,
    /// Path where config-editor bookmarks are persisted.
    pub bookmarked_keys_path: PathBuf,
    /// Dirty flag indicating bookmarks should be flushed to disk.
    pub bookmarked_keys_dirty: bool,
    /// Active edit popup, if any.
    pub popup: Option<EditPopupState>,
    /// Dropdown hit rows; row i addresses `popup.visible[popup.scroll + i]`.
    pub popup_choice_rects: Vec<(u16, u16, u16, u16)>,
    /// Lowercased localized labels and summaries.
    pub search_text: HashMap<&'static str, String>,
    /// Selected recent query while the dropdown is open.
    pub recent_dropdown: Option<usize>,
    /// Number of selectable list rows rendered on this page.
    pub list_page_rows: u16,
    /// Tab hit rectangles.
    pub tab_rects: Vec<(ConfigEditorTab, (u16, u16, u16, u16))>,
    /// Filtered key indices and row hit rectangles.
    pub row_rects: Vec<(usize, (u16, u16, u16, u16))>,
    /// Recent query indices and hit rectangles.
    pub recent_rects: Vec<(usize, (u16, u16, u16, u16))>,
    /// Record action hit rectangle.
    pub record_button_rect: Option<(u16, u16, u16, u16)>,
    /// Scrollable key list bounds.
    pub list_rect: Option<(u16, u16, u16, u16)>,
    /// Popup bounds for wheel handling.
    pub popup_rect: Option<(u16, u16, u16, u16)>,
    /// Last save outcome / hint, shown in the footer.
    pub status: Option<String>,
}

impl Default for ConfigEditorState {
    fn default() -> Self {
        Self {
            selected_file: ConfigFile::Settings,
            view: ConfigEditorView::KeyList,
            key_cursor: 0,
            query: String::new(),
            query_caret: 0,
            recent_queries: Vec::new(),
            recent_queries_path: PathBuf::new(),
            recent_queries_dirty: false,
            query_last_input_change: Instant::now(),
            last_saved_query_value: None,
            bookmarked_keys: Vec::new(),
            bookmarked_keys_path: PathBuf::new(),
            bookmarked_keys_dirty: false,
            popup: None,
            popup_choice_rects: Vec::new(),
            search_text: HashMap::new(),
            recent_dropdown: None,
            list_page_rows: 10,
            tab_rects: Vec::new(),
            row_rects: Vec::new(),
            recent_rects: Vec::new(),
            record_button_rect: None,
            list_rect: None,
            popup_rect: None,
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
    /// What: Load the editor's persisted search history and bookmarks.
    ///
    /// Inputs: Pacsea's configured lists directory.
    ///
    /// Output: Fresh editor state with persistence paths and normalized lists.
    ///
    /// Details: Missing or malformed files yield empty lists; `Default` stays free of I/O.
    #[must_use]
    pub fn load_persisted() -> Self {
        let lists_dir = crate::theme::lists_dir();
        let recent_queries_path = lists_dir.join("config_editor_recent_searches.json");
        let bookmarked_keys_path = lists_dir.join("config_editor_bookmarks.json");
        Self {
            recent_queries: load_string_list_with_limit(
                &recent_queries_path,
                MAX_CONFIG_EDITOR_RECENT,
            ),
            bookmarked_keys: load_string_list_with_limit(&bookmarked_keys_path, usize::MAX),
            recent_queries_path,
            bookmarked_keys_path,
            ..Self::default()
        }
    }

    /// What: Build the filtered key list for the currently selected file
    /// and active query.
    ///
    /// Inputs: None (uses `self.selected_file` and `self.query`).
    ///
    /// Output:
    /// - Vector of schema entries that match the query (or all entries if
    ///   `query` is empty), in stable file/group/declaration order.
    ///
    /// Details:
    /// - Terms match canonical keys, aliases, and cached localized label/summary text.
    /// - Starred combines bookmarked keys from all three editable files.
    #[must_use]
    pub fn filtered_keys(&self) -> Vec<&'static EditableSetting> {
        let files = if self.view == ConfigEditorView::Starred {
            vec![
                ConfigFile::Settings,
                ConfigFile::Keybinds,
                ConfigFile::Theme,
            ]
        } else {
            vec![self.selected_file]
        };
        let terms: Vec<String> = self
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut entries: Vec<_> = files
            .into_iter()
            .flat_map(settings_for)
            .filter(|entry| self.view != ConfigEditorView::Starred || self.is_bookmarked(entry.key))
            .filter(|entry| {
                terms.iter().all(|term| {
                    entry.key.to_lowercase().contains(term)
                        || entry
                            .aliases
                            .iter()
                            .any(|alias| alias.to_lowercase().contains(term))
                        || self
                            .search_text
                            .get(entry.key)
                            .is_some_and(|text| text.contains(term))
                })
            })
            .collect();
        entries.sort_by_key(|entry| (groups::file_order(entry.file), setting_group(entry)));
        entries
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
    /// - Mutates `recent_queries` and its persistence metadata.
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
        self.last_saved_query_value = Some(trimmed.to_string());
        self.recent_queries_dirty = true;
    }

    /// Toggle bookmark for a setting key.
    pub fn toggle_bookmark_key(&mut self, key: &str) -> bool {
        if let Some(idx) = self.bookmarked_keys.iter().position(|k| k == key) {
            self.bookmarked_keys.remove(idx);
            self.bookmarked_keys_dirty = true;
            return false;
        }
        self.bookmarked_keys.push(key.to_string());
        self.bookmarked_keys_dirty = true;
        true
    }
}

/// Convenience wrapper used by tests and helpers.
#[must_use]
pub fn lookup_setting(name: &str) -> Option<&'static EditableSetting> {
    find_setting(name)
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

/// What: Serialize a raw key event into the canonical chord string used by
/// the keybind parser, for the popup's capture mode.
///
/// Inputs:
/// - `code`: Crossterm key code from the captured event.
/// - `mods`: Modifier flags from the captured event.
///
/// Output:
/// - `Some(chord)` such as `"Ctrl+r"` or `"Shift+Tab"` when the key is
///   representable in `keybinds.conf`; `None` for unsupported codes
///   (media keys, bare modifier presses, etc.), which capture mode should
///   ignore rather than record.
#[must_use]
pub fn key_event_to_canonical_chord_string(code: KeyCode, mods: KeyModifiers) -> Option<String> {
    if !matches!(code, KeyCode::BackTab) && chord_key_label(code).is_empty() {
        return None;
    }
    Some(chord_to_canonical_string(&KeyChord { code, mods }))
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

/// Existing state regression tests.
#[cfg(test)]
mod tests;

/// Configuration file tabs and the cross-file bookmark list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigEditorTab {
    /// General application configuration.
    Settings,
    /// Keyboard shortcuts.
    Keybinds,
    /// Color palette.
    Theme,
    /// Bookmarked keys from all files.
    Starred,
}

impl ConfigEditorTab {
    /// Tabs in their keyboard and visual order.
    pub const ALL: [Self; 4] = [Self::Settings, Self::Keybinds, Self::Theme, Self::Starred];
}

impl ConfigEditorState {
    /// Active tab derived from the compatibility file/view fields.
    #[must_use]
    pub const fn active_tab(&self) -> ConfigEditorTab {
        if matches!(self.view, ConfigEditorView::Starred) {
            return ConfigEditorTab::Starred;
        }
        match self.selected_file {
            ConfigFile::Keybinds => ConfigEditorTab::Keybinds,
            ConfigFile::Theme => ConfigEditorTab::Theme,
            ConfigFile::Settings | ConfigFile::Repos => ConfigEditorTab::Settings,
        }
    }

    /// What: Switch tabs while preserving the search query.
    ///
    /// Inputs: Destination tab.
    ///
    /// Output: Updates file/view and clamps selection.
    ///
    /// Details: Starred retains the last selected file for export compatibility.
    pub fn set_tab(&mut self, tab: ConfigEditorTab) {
        self.view = if tab == ConfigEditorTab::Starred {
            ConfigEditorView::Starred
        } else {
            ConfigEditorView::KeyList
        };
        match tab {
            ConfigEditorTab::Settings => self.selected_file = ConfigFile::Settings,
            ConfigEditorTab::Keybinds => self.selected_file = ConfigFile::Keybinds,
            ConfigEditorTab::Theme => self.selected_file = ConfigFile::Theme,
            ConfigEditorTab::Starred => {}
        }
        self.clamp_key_cursor();
    }

    /// What: Cycle through file tabs and Starred with wraparound.
    ///
    /// Inputs: Forward or backward direction.
    ///
    /// Output: Switches the active tab and clamps its cursor.
    ///
    /// Details: The search query survives every tab switch.
    pub fn cycle_tab(&mut self, forward: bool) {
        let index = ConfigEditorTab::ALL
            .iter()
            .position(|tab| *tab == self.active_tab())
            .unwrap_or(0);
        self.set_tab(ConfigEditorTab::ALL[(index + if forward { 1 } else { 3 }) % 4]);
    }

    /// Whether a canonical schema key is bookmarked.
    #[must_use]
    pub fn is_bookmarked(&self, key: &str) -> bool {
        self.bookmarked_keys.iter().any(|saved| saved == key)
    }

    /// What: Populate localized search text when entering the editor or handling its first key.
    ///
    /// Inputs: Application translation state.
    ///
    /// Output: Lowercased labels and summaries indexed by canonical schema key.
    ///
    /// Details: Untranslated identifiers are excluded so internal i18n paths never match.
    pub fn build_search_text(&mut self, app: &crate::state::AppState) {
        self.search_text = [
            ConfigFile::Settings,
            ConfigFile::Keybinds,
            ConfigFile::Theme,
        ]
        .into_iter()
        .flat_map(settings_for)
        .map(|entry| {
            let text = [entry.label_i18n_key(), entry.summary_i18n_key()]
                .into_iter()
                .filter_map(|key| {
                    let translated = crate::i18n::t(app, &key);
                    (translated != key).then_some(translated)
                })
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            (entry.key, text)
        })
        .collect();
    }
}

impl EditPopupState {
    /// Whether the popup accepts only predefined choices.
    #[must_use]
    pub const fn is_strict(&self) -> bool {
        matches!(
            self.kind,
            EditPopupKind::Bool(_) | EditPopupKind::Enum { .. }
        )
    }

    /// Whether suggestions replace only the last comma-separated token.
    #[must_use]
    pub fn comma_list(&self) -> bool {
        suggestions::comma_list(self.setting.key)
    }

    /// Number of Unicode scalar values before the byte-indexed caret.
    #[must_use]
    pub fn caret_column(&self) -> usize {
        self.buffer.get(..self.caret).unwrap_or("").chars().count()
    }

    /// What: Resolve exactly what Enter or Ctrl+S will save.
    ///
    /// Inputs: Current buffer and optional highlighted candidate.
    ///
    /// Output: Canonical complete value.
    ///
    /// Details: Comma-list suggestions preserve the prefix before the final token.
    #[must_use]
    pub fn effective_value(&self) -> String {
        let candidate = self
            .highlight
            .and_then(|position| self.visible.get(position))
            .and_then(|&index| self.suggestions.get(index));
        if let Some(candidate) = candidate {
            return self.completed_value(&candidate.value);
        }
        self.canonical_value()
    }

    /// Complete a token while retaining earlier comma-list selections.
    fn completed_value(&self, value: &str) -> String {
        if self.comma_list()
            && let Some((prefix, _)) = self.buffer.rsplit_once(',')
        {
            return format!("{prefix}, {value}");
        }
        value.to_string()
    }

    /// What: Install opening-time suggestions and initialize selection.
    ///
    /// Inputs: Deduplicated candidates generated from captured facts.
    ///
    /// Output: Replaces suggestions, resets filtering, and caches validation.
    ///
    /// Details: Strict controls highlight their selected value; free input stays unhighlighted.
    pub fn set_suggestions(&mut self, suggestions: Vec<Suggestion>) {
        self.suggestions = suggestions;
        self.filter_active = false;
        self.refresh_visible();
    }

    /// What: Refilter candidates after input changes and cache validation.
    ///
    /// Inputs: Current buffer and filter flag.
    ///
    /// Output: Updates visible indices, highlight, scroll, and validation.
    ///
    /// Details: Free input never automatically highlights a value.
    pub fn refresh_visible(&mut self) {
        let token = if self.comma_list() {
            self.buffer.rsplit(',').next().unwrap_or("")
        } else {
            &self.buffer
        };
        let needle = token.trim().to_lowercase();
        let completed: Vec<_> = if self.comma_list() {
            self.buffer
                .rsplit_once(',')
                .map_or_else(Vec::new, |(prefix, _)| {
                    prefix.split(',').map(str::trim).collect()
                })
        } else {
            Vec::new()
        };
        self.visible = self
            .suggestions
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                !completed.contains(&candidate.value.as_str())
                    && (!self.filter_active || candidate.value.to_lowercase().contains(&needle))
            })
            .map(|(index, _)| index)
            .collect();
        if self.is_strict() && self.visible.is_empty() {
            // Keep the effective strict choice visible even when a filter has no matches.
            let value = self.canonical_value();
            if let Some(index) = self
                .suggestions
                .iter()
                .position(|candidate| candidate.value == value)
            {
                self.visible.push(index);
            }
        }
        self.highlight = if self.is_strict() {
            let value = self.canonical_value();
            self.visible
                .iter()
                .position(|&index| self.suggestions[index].value == value)
                .or_else(|| (!self.visible.is_empty()).then_some(0))
        } else {
            None
        };
        self.scroll = 0;
        self.sync_highlight();
        self.refresh_validation();
    }

    /// What: Cache validation of the value the next save would write.
    ///
    /// Inputs: Schema entry, effective input, and current keymap for chord conflicts.
    ///
    /// Output: Updates `validation` with success or an actionable error.
    ///
    /// Details: Called on edits and selection changes, never by the renderer.
    pub fn refresh_validation(&mut self) {
        let value = self.effective_value();
        self.validation = validate_value(self.setting, &value).and_then(|()| {
            if matches!(self.kind, EditPopupKind::KeyChord { .. }) {
                validate::validate_keybind_conflict(self.setting, &value)
            } else {
                Ok(())
            }
        });
    }

    /// Synchronize strict control state with its highlighted suggestion.
    fn sync_highlight(&mut self) {
        let Some(value) = self
            .highlight
            .and_then(|position| self.visible.get(position))
            .and_then(|&index| self.suggestions.get(index))
            .map(|candidate| candidate.value.clone())
        else {
            return;
        };
        match &mut self.kind {
            EditPopupKind::Bool(selected) => *selected = value == "true",
            EditPopupKind::Enum { choices, index } => {
                if let Some(selected) = choices.iter().position(|choice| *choice == value) {
                    *index = selected;
                }
            }
            _ => {}
        }
    }

    /// What: Highlight a visible candidate without saving it.
    ///
    /// Inputs: Position within `visible`, independent of scroll offset.
    ///
    /// Output: Returns whether the position exists; updates selection and validation.
    ///
    /// Details: Free-input buffers remain intact; strict controls synchronize their canonical choice.
    pub fn select_highlight(&mut self, position: usize) -> bool {
        if position >= self.visible.len() {
            return false;
        }
        self.highlight = Some(position);
        self.sync_highlight();
        // While filtering, the buffer is the user's filter text; replacing it would
        // corrupt the next keystroke's filter.
        if self.is_strict() && !self.filter_active {
            self.buffer = self.canonical_value();
            self.caret = self.buffer.len();
        }
        if position < self.scroll {
            self.scroll = position;
        }
        if position >= self.scroll.saturating_add(DROPDOWN_PAGE_ROWS) {
            self.scroll = position.saturating_sub(DROPDOWN_PAGE_ROWS - 1);
        }
        self.refresh_validation();
        true
    }

    /// What: Move the dropdown highlight by a signed row count.
    ///
    /// Inputs: Relative movement, including page-sized deltas.
    ///
    /// Output: Updates highlight, scroll, and validation.
    ///
    /// Details: Moving up from the first free-input suggestion returns to typed input.
    pub fn move_highlight(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let next = match self.highlight {
            Some(0) if delta < 0 && !self.is_strict() => {
                self.highlight = None;
                self.refresh_validation();
                return;
            }
            Some(position) => position
                .saturating_add_signed(delta)
                .min(self.visible.len() - 1),
            None if delta < 0 => self.visible.len() - 1,
            None => 0,
        };
        self.select_highlight(next);
    }

    /// What: Complete the highlighted or first candidate into free input.
    ///
    /// Inputs: Current highlight and filtered candidates.
    ///
    /// Output: Replaces the final token or whole buffer, moves the caret, and refreshes validation.
    ///
    /// Details: Completion does not save and removes free-input highlighting.
    pub fn complete_suggestion(&mut self) {
        let Some(index) = self.visible.get(self.highlight.unwrap_or(0)).copied() else {
            return;
        };
        self.buffer = self.completed_value(&self.suggestions[index].value);
        self.caret = self.buffer.len();
        self.filter_active = false;
        self.refresh_visible();
    }

    /// What: Apply a caret-aware text edit on UTF-8 character boundaries.
    ///
    /// Inputs: Printable or editing key.
    ///
    /// Output: Mutates the buffer/caret and refreshes candidate filtering and validation.
    ///
    /// Details: Strict picker input begins a new filter on its first printable edit.
    pub fn edit_buffer(&mut self, code: KeyCode) {
        if self.is_strict()
            && !self.filter_active
            && matches!(
                code,
                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
            )
        {
            self.buffer.clear();
            self.caret = 0;
        }
        self.caret = self.caret.min(self.buffer.len());
        while !self.buffer.is_char_boundary(self.caret) {
            self.caret = self.caret.saturating_sub(1);
        }
        let before = self.buffer[..self.caret]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index);
        let after = self.buffer[self.caret..]
            .chars()
            .next()
            .map_or(self.caret, |ch| self.caret + ch.len_utf8());
        match code {
            KeyCode::Left => {
                self.caret = before;
                return;
            }
            KeyCode::Right => {
                self.caret = after;
                return;
            }
            KeyCode::Home => {
                self.caret = 0;
                return;
            }
            KeyCode::End => {
                self.caret = self.buffer.len();
                return;
            }
            KeyCode::Backspace => {
                self.buffer.replace_range(before..self.caret, "");
                self.caret = before;
            }
            KeyCode::Delete => {
                self.buffer.replace_range(self.caret..after, "");
            }
            KeyCode::Char(ch) => {
                self.buffer.insert(self.caret, ch);
                self.caret += ch.len_utf8();
            }
            _ => return,
        }
        self.filter_active = true;
        self.refresh_visible();
    }
}

#[cfg(test)]
mod ux_tests;
