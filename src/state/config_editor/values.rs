//! Captured effective values and cached codebase defaults.
use super::{bool_to_canonical, chord_to_canonical_string};
use crate::theme::{EditableSetting, KeyChord, KeyMap};
/// What: Resolve a settings or keybind entry through one canonical value mapping.
///
/// Inputs: Schema entry and captured settings.
///
/// Output: Canonical string, or empty for an unsupported key.
///
/// Details: The same mapping serves effective values and codebase defaults.
#[must_use]
fn settings_value(entry: &EditableSetting, s: &crate::theme::Settings) -> String {
    if let Some(value) = pi_scan_current_value(entry.key, &s.pi_scan) {
        return value;
    }
    match entry.key {
        // Phase 1 originals
        "sort_mode" => s.sort_mode.as_config_key().to_string(),
        "show_install_pane" => bool_to_canonical(s.show_install_pane).to_string(),
        "show_search_history_pane" => bool_to_canonical(s.show_recent_pane).to_string(),
        "mirror_count" => s.mirror_count.to_string(),
        "news_max_age_days" => s
            .news_max_age_days
            .map_or_else(|| "all".to_string(), |n| n.to_string()),
        "clipboard_suffix" => s.clipboard_suffix.clone(),
        "preferred_terminal" => s.preferred_terminal.clone(),
        "privilege_mode" => s.privilege_mode.as_config_key().to_string(),
        "main_pane_order" => crate::state::format_main_pane_order(&s.main_pane_order),
        "virustotal_api_key" => s.virustotal_api_key.clone(),

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
        "selected_countries" => s.selected_countries.clone(),

        // Scan defaults
        "scan_do_clamav" => bool_to_canonical(s.scan_do_clamav).to_string(),
        "scan_do_trivy" => bool_to_canonical(s.scan_do_trivy).to_string(),
        "scan_do_semgrep" => bool_to_canonical(s.scan_do_semgrep).to_string(),
        "scan_do_shellcheck" => bool_to_canonical(s.scan_do_shellcheck).to_string(),
        "scan_do_virustotal" => bool_to_canonical(s.scan_do_virustotal).to_string(),
        "scan_do_custom" => bool_to_canonical(s.scan_do_custom).to_string(),
        "scan_do_sleuth" => bool_to_canonical(s.scan_do_sleuth).to_string(),

        // PKGBUILD checks
        "pkgbuild_shellcheck_exclude" => s.pkgbuild_shellcheck_exclude.clone(),
        "pkgbuild_checks_show_raw_output" => {
            bool_to_canonical(s.pkgbuild_checks_show_raw_output).to_string()
        }

        _ => settings_misc_value(entry, s),
    }
}

/// Resolve the news, startup, and other settings using the shared value mapping.
fn settings_misc_value(entry: &EditableSetting, s: &crate::theme::Settings) -> String {
    match entry.key {
        // News symbols / filters
        "news_read_symbol" => s.news_read_symbol.clone(),
        "news_unread_symbol" => s.news_unread_symbol.clone(),
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
        "locale" => s.locale.clone(),
        "updates_refresh_interval" => s.updates_refresh_interval.to_string(),
        "use_terminal_theme" => bool_to_canonical(s.use_terminal_theme).to_string(),

        // AUR voting
        "aur_vote_enabled" => bool_to_canonical(s.aur_vote_enabled).to_string(),
        "aur_vote_ssh_timeout_seconds" => s.aur_vote_ssh_timeout_seconds.to_string(),
        "aur_vote_ssh_command" => s.aur_vote_ssh_command.clone(),

        key if key.starts_with("keybind_") => keybind_chords_for_key(key, &s.keymap)
            .first()
            .map(chord_to_canonical_string)
            .unwrap_or_default(),

        _ => String::new(),
    }
}

/// Return the current canonical value for an optional Pi scanner key.
fn pi_scan_current_value(key: &str, settings: &crate::theme::PiScanSettings) -> Option<String> {
    let value = match key {
        "pi_scan_enabled" => bool_to_canonical(settings.enabled).to_string(),
        "pi_scan_background_enabled" => bool_to_canonical(settings.background_enabled).to_string(),
        "pi_scan_binary" => settings.binary.clone(),
        "pi_scan_provider" => settings.provider.clone(),
        "pi_scan_model" => settings.model.clone(),
        "pi_scan_fallback_models" => settings.fallback_models.clone(),
        "pi_scan_thinking" => settings.thinking.clone(),
        "pi_scan_observation_interval_seconds" => settings.observation_interval_seconds.to_string(),
        "pi_scan_head_query_timeout_seconds" => settings.head_query_timeout_seconds.to_string(),
        "pi_scan_observation_deadline_seconds" => settings.observation_deadline_seconds.to_string(),
        "pi_scan_model_attempt_timeout_seconds" => {
            settings.model_attempt_timeout_seconds.to_string()
        }
        "pi_scan_logical_timeout_seconds" => settings.logical_timeout_seconds.to_string(),
        "pi_scan_background_starts_per_hour" => settings.background_starts_per_hour.to_string(),
        "pi_scan_background_token_cap_24h" => settings.background_token_cap_24h.to_string(),
        "pi_scan_background_cost_cap_24h" => settings.background_cost_cap_24h.clone(),
        "pi_scan_result_retention_days" => settings.result_retention_days.to_string(),
        "pi_scan_show_raw_output" => bool_to_canonical(settings.show_raw_output).to_string(),
        "pi_scan_https_proxy" => settings.https_proxy.clone(),
        _ => return None,
    };
    Some(value)
}

/// What: Look up the effective in-memory color for a theme schema key.
///
/// Inputs:
/// - `key`: Preferred theme key from [`crate::theme::EDITABLE_THEME`]
///   (e.g. `background_base`).
///
/// Output:
/// - Config-file color literal (`#rrggbb`), or an empty string when the key
///   is not a theme color or the color has no RGB representation.
///
/// Details:
/// - Reads the live [`crate::theme::theme`] snapshot, so values reflect the
///   effective palette (including terminal-derived themes) rather than raw
///   file content.
#[must_use]
pub fn theme_color_value_string(key: &str) -> String {
    color_from_theme(key, &crate::theme::theme())
}

/// Resolve a color using an already captured palette.
fn color_from_theme(key: &str, th: &crate::theme::Theme) -> String {
    let color = match key {
        "background_base" => th.base,
        "background_mantle" => th.mantle,
        "background_crust" => th.crust,
        "surface_level1" => th.surface1,
        "surface_level2" => th.surface2,
        "overlay_primary" => th.overlay1,
        "overlay_secondary" => th.overlay2,
        "text_primary" => th.text,
        "text_secondary" => th.subtext0,
        "text_tertiary" => th.subtext1,
        "accent_interactive" => th.sapphire,
        "accent_heading" => th.mauve,
        "semantic_success" => th.green,
        "semantic_warning" => th.yellow,
        "semantic_error" => th.red,
        "accent_emphasis" => th.lavender,
        _ => return String::new(),
    };
    color_to_config_string(color)
}

/// What: Serialize a ratatui color into the `#rrggbb` form accepted by
/// `theme.conf` parsing.
///
/// Inputs:
/// - `color`: Color from the active [`crate::theme::Theme`].
///
/// Output:
/// - Lowercase hex literal for RGB colors; empty string for palette/named
///   variants that have no fixed RGB value.
#[must_use]
pub fn color_to_config_string(color: ratatui::style::Color) -> String {
    match color {
        ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
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
        "keybind_search_normal_pi_scan" => &keymap.search_normal_pi_scan,
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

/// Effective configuration captured once for a render or popup opening.
#[derive(Debug, Clone)]
pub struct ValueSnapshot {
    /// Settings including the effective keymap.
    settings: crate::theme::Settings,
    /// Effective palette, including terminal-derived colors.
    theme: crate::theme::Theme,
}

/// Codebase defaults shared across every snapshot.
static DEFAULTS: std::sync::OnceLock<ValueSnapshot> = std::sync::OnceLock::new();

impl ValueSnapshot {
    /// Capture effective settings and palette once.
    #[must_use]
    pub fn capture() -> Self {
        Self {
            settings: crate::theme::settings(),
            theme: crate::theme::theme(),
        }
    }

    /// Build a snapshot from explicit values so render tests ignore user configuration.
    #[cfg(test)]
    pub(crate) const fn from_parts(
        settings: crate::theme::Settings,
        theme: crate::theme::Theme,
    ) -> Self {
        Self { settings, theme }
    }

    /// What: Resolve an entry using this snapshot.
    ///
    /// Inputs: Schema entry.
    ///
    /// Output: Canonical configuration value.
    ///
    /// Details: No filesystem or global snapshot reads occur per entry.
    #[must_use]
    pub fn value_for(&self, entry: &EditableSetting) -> String {
        if entry.file == crate::theme::ConfigFile::Theme {
            color_from_theme(entry.key, &self.theme)
        } else {
            settings_value(entry, &self.settings)
        }
    }

    /// What: Resolve the codebase default for an entry.
    ///
    /// Inputs: Schema entry.
    ///
    /// Output: Default canonical value.
    ///
    /// Details: Defaults never depend on user configuration or terminal colors.
    #[must_use]
    pub fn default_for(&self, entry: &EditableSetting) -> String {
        DEFAULTS
            .get_or_init(|| Self {
                settings: crate::theme::Settings::default(),
                theme: crate::theme::default_theme(),
            })
            .value_for(entry)
    }

    /// Whether the effective value differs from its codebase default.
    #[must_use]
    pub fn is_modified(&self, entry: &EditableSetting) -> bool {
        if matches!(entry.kind, crate::theme::ValueKind::KeyChord) {
            let defaults = DEFAULTS.get_or_init(|| Self {
                settings: crate::theme::Settings::default(),
                theme: crate::theme::default_theme(),
            });
            return keybind_chords_for_key(entry.key, &self.settings.keymap)
                != keybind_chords_for_key(entry.key, &defaults.settings.keymap);
        }
        self.value_for(entry) != self.default_for(entry)
    }

    /// Additional display-only chords after the first editable binding.
    #[must_use]
    pub fn extra_chords(&self, entry: &EditableSetting) -> Vec<String> {
        keybind_chords_for_key(entry.key, &self.settings.keymap)
            .iter()
            .skip(1)
            .map(chord_to_canonical_string)
            .collect()
    }
}

/// Resolve one effective value; renderers should share a `ValueSnapshot` instead.
#[must_use]
pub fn current_value_string(entry: &EditableSetting) -> String {
    ValueSnapshot::capture().value_for(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults share the effective-value mapping and ignore user state.
    #[test]
    fn snapshot_defaults_modified_and_extra_chords() {
        let mut snapshot = ValueSnapshot {
            settings: crate::theme::Settings::default(),
            theme: crate::theme::default_theme(),
        };
        for entry in crate::theme::EDITABLE_SETTINGS
            .iter()
            .chain(crate::theme::EDITABLE_KEYBINDS)
            .chain(crate::theme::EDITABLE_THEME)
        {
            assert!(!snapshot.is_modified(entry), "{}", entry.key);
        }
        let count = crate::theme::find_setting("mirror_count").expect("count");
        let default = snapshot.default_for(count);
        snapshot.settings.mirror_count += 1;
        assert!(snapshot.is_modified(count));
        assert_eq!(snapshot.default_for(count), default);
        let key = crate::theme::find_setting("keybind_recent_remove").expect("binding");
        assert!(!snapshot.extra_chords(key).is_empty());
        assert_eq!(
            snapshot.value_for(crate::theme::find_setting("semantic_error").expect("color")),
            color_from_theme("semantic_error", &crate::theme::default_theme())
        );
    }
}
