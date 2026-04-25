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

/// What: Look up an editable setting by canonical key or alias.
///
/// Inputs:
/// - `name`: Canonical key or any alias (case-/punctuation-insensitive).
///
/// Output:
/// - `Some(&EditableSetting)` on match, `None` otherwise.
#[must_use]
pub fn find_setting(name: &str) -> Option<&'static EditableSetting> {
    EDITABLE_SETTINGS.iter().find(|s| s.matches(name))
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
        .filter(|s| s.file == file)
        .collect()
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
        // Phase 0 has no theme/keybinds/repos entries yet.
        assert!(settings_for(ConfigFile::Theme).is_empty());
        assert!(settings_for(ConfigFile::Keybinds).is_empty());
        assert!(settings_for(ConfigFile::Repos).is_empty());
    }
}
