//! Static schema describing editable Pacsea config keys.
//!
//! What: Defines the value kinds, reload behavior, sensitivity, and aliases for
//! every setting the integrated config editor is allowed to change. Phases 1+
//! of `dev/IMPROVEMENTS/IMPLEMENTATION_PLAN_tui_integrated_config_editing.md`
//! consume this schema to render typed rows and a harmonized edit popup.
//!
//! Phase 0 establishes the types and a representative subset of entries. Later
//! phases extend [`EDITABLE_SETTINGS`] (and add `EDITABLE_KEYBINDS` /
//! `EDITABLE_THEME` slices) without changing the consumer-facing API.

use crate::theme::config::patch::ConfigFile;

/// What kind of value a setting holds, used to pick the editor popup variant
/// and to validate input before [`crate::theme::config::patch::patch_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKind {
    /// `true` / `false`. Renders as a toggle.
    Bool,
    /// One of a fixed list of canonical strings (e.g. `sort_mode`,
    /// `privilege_tool`). Editor renders a chooser.
    Enum {
        /// Allowed canonical values, in display order.
        choices: &'static [&'static str],
    },
    /// Free-form string (typically a short identifier or label).
    String,
    /// Filesystem path. Editor allows browsing and validates the input is
    /// non-empty (existence check is best-effort).
    Path,
    /// Secret string (e.g. API key). Editor masks the current value and
    /// requires explicit "reveal" before display.
    Secret,
    /// Integer constrained to `[min, max]` inclusive.
    IntRange {
        /// Minimum allowed value.
        min: i64,
        /// Maximum allowed value.
        max: i64,
    },
    /// Optional positive integer expressed as a decimal number or the literal
    /// `"all"`. Used for keys like `news_max_age_days`.
    OptionalUnsignedOrAll,
    /// Hex (`#RRGGBB`) or `R,G,B` triplet. Used for theme colors.
    Color,
    /// Comma-separated ordered list of pane identifiers, e.g.
    /// `package_info, search, results`. Validation lives next to the
    /// `MainVerticalPane` parser.
    MainPaneOrder,
    /// Single keychord string (modifiers + key) understood by the keybind
    /// parser. Used by `keybinds.conf` rows.
    KeyChord,
}

/// How a saved value reaches the running app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadBehavior {
    /// Existing background file watchers (or per-event reloads) pick the
    /// change up automatically; no extra action needed.
    AutoReload,
    /// Editor must call a specific in-process apply step after writing
    /// (e.g. `theme::reload_theme`). Display in the editor as "applies
    /// immediately".
    AppliesOnSave,
    /// Change becomes effective only after the next Pacsea launch. The
    /// editor must show a "needs restart" hint.
    RequiresRestart,
}

/// Whether the value should be redacted in UI/logs by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    /// Plain value, safe to render and log.
    Normal,
    /// Treat as a secret. The editor masks it in the bottom pane and never
    /// echoes it into log lines.
    Sensitive,
}

/// Schema entry for a single editable setting key.
#[derive(Debug, Clone)]
pub struct EditableSetting {
    /// Canonical key name, written verbatim to the config file.
    pub key: &'static str,
    /// Deprecated names recognized on disk; rewritten to `key` on save.
    pub aliases: &'static [&'static str],
    /// Which config file owns this key.
    pub file: ConfigFile,
    /// Value kind / editor variant.
    pub kind: ValueKind,
    /// How the saved value reaches the running app.
    pub reload: ReloadBehavior,
    /// Whether the value should be redacted in display/logs by default.
    pub sensitivity: Sensitivity,
}

impl EditableSetting {
    /// What: Test whether `name` (raw or normalized) refers to this entry.
    ///
    /// Inputs:
    /// - `name`: User-provided or on-disk key.
    ///
    /// Output:
    /// - `true` if `name` matches `self.key` or any alias after normalization.
    ///
    /// Details:
    /// - Normalization mirrors `theme::config::patch::patch_key`: lowercase and
    ///   collapse `.`, `-`, ` ` into `_`.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        let norm = normalize(name);
        if normalize(self.key) == norm {
            return true;
        }
        self.aliases.iter().any(|a| normalize(a) == norm)
    }

    /// What: Build the i18n key for this setting's translated label.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Dot-notation i18n key under `app.modals.config_editor.settings.*`.
    #[must_use]
    pub fn label_i18n_key(&self) -> String {
        format!("app.modals.config_editor.settings.{}.label", self.key)
    }

    /// What: Build the i18n key for this setting's translated summary.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Dot-notation i18n key under `app.modals.config_editor.settings.*`.
    #[must_use]
    pub fn summary_i18n_key(&self) -> String {
        format!("app.modals.config_editor.settings.{}.summary", self.key)
    }
}

/// What: Normalize a key for case-/punctuation-insensitive comparison.
///
/// Inputs:
/// - `s`: Raw key string.
///
/// Output:
/// - Lowercased, underscore-normalized owned string.
fn normalize(s: &str) -> String {
    s.trim().to_lowercase().replace(['.', '-', ' '], "_")
}

/// What: Initial Phase-0 subset of editable settings.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Static slice of [`EditableSetting`] entries.
///
/// Details:
/// - Covers each [`ValueKind`] variant at least once so the editor's popup
///   variants can be exercised end-to-end before Phase 1 fills in the rest.
/// - Phase 1 extends this slice; consumers must treat it as append-only by
///   `key` name.
pub const EDITABLE_SETTINGS: &[EditableSetting] = &[
    EditableSetting {
        key: "sort_mode",
        aliases: &["results_sort"],
        file: ConfigFile::Settings,
        kind: ValueKind::Enum {
            choices: &[
                "best_matches",
                "alphabetical",
                "official_first",
                "aur_popularity",
            ],
        },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "show_install_pane",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "show_search_history_pane",
        aliases: &["show_recent_pane"],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "mirror_count",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 200 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_max_age_days",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::OptionalUnsignedOrAll,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "clipboard_suffix",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "preferred_terminal",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "privilege_mode",
        aliases: &["privilege_tool", "priv_tool"],
        file: ConfigFile::Settings,
        kind: ValueKind::Enum {
            choices: &["auto", "sudo", "doas"],
        },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "main_pane_order",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::MainPaneOrder,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "virustotal_api_key",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Secret,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Sensitive,
    },
    // ── Layout ───────────────────────────────────────────────────────
    EditableSetting {
        key: "layout_left_pct",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 0, max: 100 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "layout_center_pct",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 0, max: 100 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "layout_right_pct",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 0, max: 100 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    // ── Vertical row limits ──────────────────────────────────────────
    EditableSetting {
        key: "vertical_min_results",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 200 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "vertical_max_results",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 200 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "vertical_min_middle",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 200 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "vertical_max_middle",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 200 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "vertical_min_package_info",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 200 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    // ── Misc UI toggles ──────────────────────────────────────────────
    EditableSetting {
        key: "app_dry_run_default",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::RequiresRestart,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "show_keybinds_footer",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    // ── Search behavior ──────────────────────────────────────────────
    EditableSetting {
        key: "search_startup_mode",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Enum {
            choices: &["insert_mode", "normal_mode"],
        },
        reload: ReloadBehavior::RequiresRestart,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "fuzzy_search",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "installed_packages_mode",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Enum {
            choices: &["leaf", "all"],
        },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    // ── Preflight / privilege ────────────────────────────────────────
    EditableSetting {
        key: "skip_preflight",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "use_passwordless_sudo",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "auth_mode",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Enum {
            choices: &["prompt", "passwordless_only", "interactive"],
        },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    // ── Mirrors ──────────────────────────────────────────────────────
    EditableSetting {
        key: "selected_countries",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    // ── Scan defaults ────────────────────────────────────────────────
    EditableSetting {
        key: "scan_do_clamav",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "scan_do_trivy",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "scan_do_semgrep",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "scan_do_shellcheck",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "scan_do_virustotal",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "scan_do_custom",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "scan_do_sleuth",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    // ── PKGBUILD checks ──────────────────────────────────────────────
    EditableSetting {
        key: "pkgbuild_shellcheck_exclude",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "pkgbuild_checks_show_raw_output",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    // ── News symbols / filters ───────────────────────────────────────
    EditableSetting {
        key: "news_read_symbol",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_unread_symbol",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_filter_show_arch_news",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_filter_show_advisories",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_filter_show_pkg_updates",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_filter_show_aur_updates",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_filter_show_aur_comments",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "news_filter_installed_only",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    // ── Startup news popup ───────────────────────────────────────────
    EditableSetting {
        key: "startup_news_configured",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "startup_news_show_arch_news",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "startup_news_show_advisories",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "startup_news_show_aur_updates",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "startup_news_show_aur_comments",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "startup_news_show_pkg_updates",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "startup_news_max_age_days",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::OptionalUnsignedOrAll,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    // ── Misc ─────────────────────────────────────────────────────────
    EditableSetting {
        key: "package_marker",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Enum {
            choices: &["full_line", "front", "end"],
        },
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "locale",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::RequiresRestart,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "updates_refresh_interval",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 86400 },
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "use_terminal_theme",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::RequiresRestart,
        sensitivity: Sensitivity::Normal,
    },
    // ── AUR voting ───────────────────────────────────────────────────
    EditableSetting {
        key: "aur_vote_enabled",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::Bool,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "aur_vote_ssh_timeout_seconds",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::IntRange { min: 1, max: 600 },
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
    EditableSetting {
        key: "aur_vote_ssh_command",
        aliases: &[],
        file: ConfigFile::Settings,
        kind: ValueKind::String,
        reload: ReloadBehavior::AutoReload,
        sensitivity: Sensitivity::Normal,
    },
];

/// What: Phase-2 set of editable keybind rows backed by `keybinds.conf`.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Static slice of [`EditableSetting`] entries with `file = ConfigFile::Keybinds`.
///
/// Details:
/// - Canonical key names match the parser in `theme::settings::parse_keybinds`.
/// - Aliases mirror the alias sets the parser already accepts so existing
///   user-edited files migrate to canonical names on save.
/// - All entries reload via `AppliesOnSave` because [`crate::theme::settings`]
///   reparses `keybinds.conf` and `apply_settings_to_app_state` copies
///   `prefs.keymap` into `app.keymap` after each save.
pub const EDITABLE_KEYBINDS: &[EditableSetting] = &[
    // ── Global ───────────────────────────────────────────────────────
    keybind_entry("keybind_help", &["keybind_help_overlay"]),
    keybind_entry(
        "keybind_toggle_config",
        &["keybind_config_menu", "keybind_config_lists"],
    ),
    keybind_entry("keybind_toggle_options", &["keybind_options_menu"]),
    keybind_entry("keybind_toggle_panels", &["keybind_panels_menu"]),
    keybind_entry(
        "keybind_reload_config",
        &["keybind_reload_theme", "keybind_reload"],
    ),
    keybind_entry("keybind_exit", &["keybind_quit"]),
    keybind_entry(
        "keybind_show_pkgbuild",
        &["keybind_pkgbuild", "keybind_toggle_pkgbuild"],
    ),
    keybind_entry(
        "keybind_comments_toggle",
        &["keybind_show_comments", "keybind_toggle_comments"],
    ),
    keybind_entry(
        "keybind_run_pkgbuild_checks",
        &["keybind_pkgbuild_checks", "keybind_toggle_pkgbuild_checks"],
    ),
    keybind_entry(
        "keybind_cycle_pkgbuild_sections",
        &[
            "keybind_pkgbuild_section_cycle",
            "keybind_pkgbuild_next_section",
        ],
    ),
    keybind_entry("keybind_change_sort", &["keybind_sort"]),
    keybind_entry(
        "keybind_pane_next",
        &["keybind_next_pane", "keybind_switch_pane"],
    ),
    keybind_entry("keybind_pane_left", &[]),
    keybind_entry("keybind_pane_right", &[]),
    keybind_entry("keybind_toggle_fuzzy", &["keybind_fuzzy_toggle"]),
    // ── Search pane ──────────────────────────────────────────────────
    keybind_entry("keybind_search_move_up", &[]),
    keybind_entry("keybind_search_move_down", &[]),
    keybind_entry("keybind_search_page_up", &[]),
    keybind_entry("keybind_search_page_down", &[]),
    keybind_entry("keybind_search_add", &[]),
    keybind_entry("keybind_search_install", &[]),
    keybind_entry("keybind_search_focus_left", &[]),
    keybind_entry("keybind_search_focus_right", &[]),
    keybind_entry("keybind_search_backspace", &[]),
    keybind_entry("keybind_search_insert_clear", &[]),
    // ── Search normal mode ───────────────────────────────────────────
    keybind_entry("keybind_search_normal_toggle", &[]),
    keybind_entry("keybind_search_normal_insert", &[]),
    keybind_entry("keybind_search_normal_select_left", &[]),
    keybind_entry("keybind_search_normal_select_right", &[]),
    keybind_entry("keybind_search_normal_delete", &[]),
    keybind_entry("keybind_search_normal_clear", &[]),
    keybind_entry(
        "keybind_search_normal_open_status",
        &["keybind_normal_open_status", "keybind_open_status"],
    ),
    keybind_entry("keybind_search_normal_import", &[]),
    keybind_entry("keybind_search_normal_export", &[]),
    keybind_entry("keybind_search_normal_updates", &[]),
    // ── Recent pane ──────────────────────────────────────────────────
    keybind_entry("keybind_recent_move_up", &[]),
    keybind_entry("keybind_recent_move_down", &[]),
    keybind_entry("keybind_recent_find", &[]),
    keybind_entry("keybind_recent_use", &[]),
    keybind_entry("keybind_recent_add", &[]),
    keybind_entry("keybind_recent_to_search", &[]),
    keybind_entry("keybind_recent_focus_right", &[]),
    keybind_entry("keybind_recent_remove", &[]),
    keybind_entry("keybind_recent_clear", &[]),
    // ── Install pane ─────────────────────────────────────────────────
    keybind_entry("keybind_install_move_up", &[]),
    keybind_entry("keybind_install_move_down", &[]),
    keybind_entry("keybind_install_confirm", &[]),
    keybind_entry("keybind_install_remove", &[]),
    keybind_entry("keybind_install_clear", &[]),
    keybind_entry("keybind_install_find", &[]),
    keybind_entry("keybind_install_to_search", &[]),
    keybind_entry("keybind_install_focus_left", &[]),
    // ── News modal ───────────────────────────────────────────────────
    keybind_entry("keybind_news_mark_read", &[]),
    keybind_entry("keybind_news_mark_all_read", &[]),
    keybind_entry("keybind_news_feed_mark_read", &[]),
    keybind_entry("keybind_news_feed_mark_unread", &[]),
    keybind_entry("keybind_news_feed_toggle_read", &[]),
];

/// What: Construct an [`EditableSetting`] row for a keybind action.
///
/// Inputs:
/// - `key`: Canonical action key matching the parser in
///   `theme::settings::parse_keybinds`.
/// - `aliases`: Alternate names recognized on disk and rewritten on save.
///
/// Output:
/// - `EditableSetting` with `file = ConfigFile::Keybinds`,
///   `kind = ValueKind::KeyChord`, and `reload = AppliesOnSave`.
const fn keybind_entry(key: &'static str, aliases: &'static [&'static str]) -> EditableSetting {
    EditableSetting {
        key,
        aliases,
        file: ConfigFile::Keybinds,
        kind: ValueKind::KeyChord,
        reload: ReloadBehavior::AppliesOnSave,
        sensitivity: Sensitivity::Normal,
    }
}

/// What: Look up an editable setting by canonical key or alias.
///
/// Inputs:
/// - `name`: Canonical key or any alias (case-/punctuation-insensitive).
///
/// Output:
/// - `Some(&EditableSetting)` on match, `None` otherwise.
#[must_use]
pub fn find_setting(name: &str) -> Option<&'static EditableSetting> {
    EDITABLE_SETTINGS
        .iter()
        .chain(EDITABLE_KEYBINDS.iter())
        .find(|s| s.matches(name))
}

/// What: Return all editable settings registered for `file`.
///
/// Inputs:
/// - `file`: Config file kind.
///
/// Output:
/// - Vector of references in declaration order.
#[must_use]
pub fn settings_for(file: ConfigFile) -> Vec<&'static EditableSetting> {
    EDITABLE_SETTINGS
        .iter()
        .chain(EDITABLE_KEYBINDS.iter())
        .filter(|s| s.file == file)
        .collect()
}

/// What: Determine the conflict-detection scope for a keybind action.
///
/// Inputs:
/// - `key`: Canonical keybind action key.
///
/// Output:
/// - Static label identifying the scope (`global`, `search`, `search_normal`,
///   `recent`, `install`, or `news`).
///
/// Details:
/// - Two bindings collide only when they share both a chord and this scope.
/// - Same-chord rebinds across scopes (e.g. `Left` for `pane_left` and
///   `search_focus_left`) are considered intentional and do not conflict.
#[must_use]
pub fn keybind_scope(key: &str) -> &'static str {
    if key.starts_with("keybind_search_normal_") {
        "search_normal"
    } else if key.starts_with("keybind_search_") {
        "search"
    } else if key.starts_with("keybind_recent_") {
        "recent"
    } else if key.starts_with("keybind_install_") {
        "install"
    } else if key.starts_with("keybind_news_") {
        "news"
    } else {
        "global"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_after_normalization() {
        let mut seen = std::collections::HashSet::new();
        for entry in EDITABLE_SETTINGS {
            let norm = normalize(entry.key);
            assert!(
                seen.insert(norm.clone()),
                "duplicate key in EDITABLE_SETTINGS: {norm}"
            );
        }
    }

    #[test]
    fn aliases_resolve_to_primary() {
        let s = find_setting("show_recent_pane").expect("alias should resolve");
        assert_eq!(s.key, "show_search_history_pane");

        let s = find_setting("Results.Sort").expect("alias is normalized");
        assert_eq!(s.key, "sort_mode");
    }

    #[test]
    fn enum_choices_are_non_empty() {
        for entry in EDITABLE_SETTINGS {
            if let ValueKind::Enum { choices } = entry.kind {
                assert!(
                    !choices.is_empty(),
                    "{} declares Enum without choices",
                    entry.key
                );
            }
        }
    }

    #[test]
    fn int_range_is_well_formed() {
        for entry in EDITABLE_SETTINGS {
            if let ValueKind::IntRange { min, max } = entry.kind {
                assert!(
                    min <= max,
                    "{} declares inverted IntRange: {min}..{max}",
                    entry.key
                );
            }
        }
    }

    #[test]
    fn settings_for_filters_by_file() {
        let only_settings = settings_for(ConfigFile::Settings);
        assert!(!only_settings.is_empty());
        for s in only_settings {
            assert_eq!(s.file, ConfigFile::Settings);
        }
        // Phase 2 ships keybind rows; theme/repos still empty.
        let only_keybinds = settings_for(ConfigFile::Keybinds);
        assert!(!only_keybinds.is_empty());
        for s in only_keybinds {
            assert_eq!(s.file, ConfigFile::Keybinds);
            assert!(matches!(s.kind, ValueKind::KeyChord));
        }
        assert!(settings_for(ConfigFile::Theme).is_empty());
        assert!(settings_for(ConfigFile::Repos).is_empty());
    }

    #[test]
    fn keybind_keys_are_unique_after_normalization() {
        let mut seen = std::collections::HashSet::new();
        for entry in EDITABLE_KEYBINDS {
            let norm = normalize(entry.key);
            assert!(
                seen.insert(norm.clone()),
                "duplicate key in EDITABLE_KEYBINDS: {norm}"
            );
        }
    }

    #[test]
    fn keybind_aliases_resolve_to_primary() {
        let s = find_setting("keybind_help_overlay").expect("alias should resolve");
        assert_eq!(s.key, "keybind_help");
        let s = find_setting("keybind_open_status").expect("alias should resolve");
        assert_eq!(s.key, "keybind_search_normal_open_status");
    }

    #[test]
    fn keybind_scope_groups_by_prefix() {
        assert_eq!(keybind_scope("keybind_help"), "global");
        assert_eq!(keybind_scope("keybind_pane_next"), "global");
        assert_eq!(keybind_scope("keybind_search_move_up"), "search");
        assert_eq!(
            keybind_scope("keybind_search_normal_toggle"),
            "search_normal"
        );
        assert_eq!(keybind_scope("keybind_recent_remove"), "recent");
        assert_eq!(keybind_scope("keybind_install_remove"), "install");
        assert_eq!(keybind_scope("keybind_news_mark_read"), "news");
    }
}
