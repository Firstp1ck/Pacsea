use std::path::Path;

use crate::theme::parsing::strip_inline_comment;
use crate::theme::types::{PackageMarker, Settings};

/// What: Parse a boolean value from config string.
///
/// Inputs:
/// - `val`: Value string from config
///
/// Output:
/// - `true` if value represents true, `false` otherwise
///
/// Details:
/// - Accepts "true", "1", "yes", "on" (case-insensitive)
fn parse_bool(val: &str) -> bool {
    let lv = val.to_ascii_lowercase();
    lv == "true" || lv == "1" || lv == "yes" || lv == "on"
}

/// What: Parse layout settings.
///
/// Inputs:
/// - `key`: Normalized config key
/// - `val`: Config value
/// - `settings`: Mutable settings to update
///
/// Output:
/// - `true` if key was handled, `false` otherwise
fn parse_layout_settings(key: &str, val: &str, settings: &mut Settings) -> bool {
    match key {
        "layout_left_pct" => {
            if let Ok(v) = val.parse::<u16>() {
                settings.layout_left_pct = v;
            }
            true
        }
        "layout_center_pct" => {
            if let Ok(v) = val.parse::<u16>() {
                settings.layout_center_pct = v;
            }
            true
        }
        "layout_right_pct" => {
            if let Ok(v) = val.parse::<u16>() {
                settings.layout_right_pct = v;
            }
            true
        }
        _ => false,
    }
}

/// What: Parse app/UI settings.
///
/// Inputs:
/// - `key`: Normalized config key
/// - `val`: Config value
/// - `settings`: Mutable settings to update
///
/// Output:
/// - `true` if key was handled, `false` otherwise
fn parse_app_settings(key: &str, val: &str, settings: &mut Settings) -> bool {
    match key {
        "app_dry_run_default" => {
            settings.app_dry_run_default = parse_bool(val);
            true
        }
        "sort_mode" | "results_sort" => {
            if let Some(sm) = crate::state::SortMode::from_config_key(val) {
                settings.sort_mode = sm;
            }
            true
        }
        "clipboard_suffix" | "copy_suffix" => {
            settings.clipboard_suffix = val.to_string();
            true
        }
        "show_search_history_pane" | "show_recent_pane" | "recent_visible" => {
            settings.show_recent_pane = parse_bool(val);
            true
        }
        "show_install_pane" | "install_visible" | "show_install_list" => {
            settings.show_install_pane = parse_bool(val);
            true
        }
        "show_keybinds_footer" | "keybinds_visible" => {
            settings.show_keybinds_footer = parse_bool(val);
            true
        }
        "package_marker" => {
            let lv = val.to_ascii_lowercase();
            settings.package_marker = match lv.as_str() {
                "full" | "full_line" | "line" | "color_line" | "color" => PackageMarker::FullLine,
                "end" | "suffix" => PackageMarker::End,
                _ => PackageMarker::Front,
            };
            true
        }
        "skip_preflight" | "preflight_skip" | "bypass_preflight" => {
            settings.skip_preflight = parse_bool(val);
            true
        }
        _ => false,
    }
}

/// What: Parse scan-related settings.
///
/// Inputs:
/// - `key`: Normalized config key
/// - `val`: Config value
/// - `settings`: Mutable settings to update
///
/// Output:
/// - `true` if key was handled, `false` otherwise
fn parse_scan_settings(key: &str, val: &str, settings: &mut Settings) -> bool {
    match key {
        "scan_do_clamav" => {
            settings.scan_do_clamav = parse_bool(val);
            true
        }
        "scan_do_trivy" => {
            settings.scan_do_trivy = parse_bool(val);
            true
        }
        "scan_do_semgrep" => {
            settings.scan_do_semgrep = parse_bool(val);
            true
        }
        "scan_do_shellcheck" => {
            settings.scan_do_shellcheck = parse_bool(val);
            true
        }
        "scan_do_virustotal" => {
            settings.scan_do_virustotal = parse_bool(val);
            true
        }
        "scan_do_custom" => {
            settings.scan_do_custom = parse_bool(val);
            true
        }
        "scan_do_sleuth" => {
            settings.scan_do_sleuth = parse_bool(val);
            true
        }
        "virustotal_api_key" | "vt_api_key" | "virustotal" => {
            // VirusTotal API key; stored as-is and trimmed later
            settings.virustotal_api_key = val.to_string();
            true
        }
        _ => false,
    }
}

/// What: Parse mirror and country settings.
///
/// Inputs:
/// - `key`: Normalized config key
/// - `val`: Config value
/// - `settings`: Mutable settings to update
///
/// Output:
/// - `true` if key was handled, `false` otherwise
fn parse_mirror_settings(key: &str, val: &str, settings: &mut Settings) -> bool {
    match key {
        "selected_countries" | "countries" | "country" => {
            // Accept comma-separated list; trimming occurs in normalization
            settings.selected_countries = val.to_string();
            true
        }
        "mirror_count" | "mirrors" => {
            if let Ok(v) = val.parse::<u16>() {
                settings.mirror_count = v;
            }
            true
        }
        _ => false,
    }
}

/// What: Parse news-related settings.
///
/// Inputs:
/// - `key`: Normalized config key
/// - `val`: Config value
/// - `settings`: Mutable settings to update
///
/// Output:
/// - `true` if key was handled, `false` otherwise
fn parse_news_settings(key: &str, val: &str, settings: &mut Settings) -> bool {
    match key {
        "news_read_symbol" | "news_read_mark" => {
            settings.news_read_symbol = val.to_string();
            true
        }
        "news_unread_symbol" | "news_unread_mark" => {
            settings.news_unread_symbol = val.to_string();
            true
        }
        "news_filter_show_arch_news" | "news_filter_arch" => {
            settings.news_filter_show_arch_news = parse_bool(val);
            true
        }
        "news_filter_show_advisories" | "news_filter_advisories" => {
            settings.news_filter_show_advisories = parse_bool(val);
            true
        }
        "news_filter_show_pkg_updates" | "news_filter_pkg_updates" | "news_filter_updates" => {
            settings.news_filter_show_pkg_updates = parse_bool(val);
            true
        }
        "news_filter_show_aur_updates"
        | "news_filter_aur_updates"
        | "news_filter_aur_upd"
        | "news_filter_aur_upd_updates" => {
            settings.news_filter_show_aur_updates = parse_bool(val);
            true
        }
        "news_filter_show_aur_comments" | "news_filter_aur_comments" | "news_filter_comments" => {
            settings.news_filter_show_aur_comments = parse_bool(val);
            true
        }
        "news_filter_installed_only" | "news_filter_installed" | "news_installed_only" => {
            settings.news_filter_installed_only = parse_bool(val);
            true
        }
        "news_max_age_days" | "news_age_days" | "news_age" => {
            let lv = val.trim().to_ascii_lowercase();
            settings.news_max_age_days = match lv.as_str() {
                "" | "all" | "none" | "unlimited" => None,
                _ => val.parse::<u32>().ok(),
            };
            true
        }
        "startup_news_configured" => {
            settings.startup_news_configured = parse_bool(val);
            true
        }
        "startup_news_show_arch_news" => {
            settings.startup_news_show_arch_news = parse_bool(val);
            true
        }
        "startup_news_show_advisories" => {
            settings.startup_news_show_advisories = parse_bool(val);
            true
        }
        "startup_news_show_aur_updates" => {
            settings.startup_news_show_aur_updates = parse_bool(val);
            true
        }
        "startup_news_show_aur_comments" => {
            settings.startup_news_show_aur_comments = parse_bool(val);
            true
        }
        "startup_news_show_pkg_updates" => {
            settings.startup_news_show_pkg_updates = parse_bool(val);
            true
        }
        "startup_news_max_age_days" => {
            let lv = val.trim().to_ascii_lowercase();
            settings.startup_news_max_age_days = match lv.as_str() {
                "" | "all" | "none" | "unlimited" => None,
                _ => val.parse::<u32>().ok(),
            };
            true
        }
        "news_cache_ttl_days" => {
            if let Ok(days) = val.parse::<u32>() {
                settings.news_cache_ttl_days = days.max(1); // Minimum 1 day
            }
            true
        }
        _ => false,
    }
}

/// What: Parse search-related settings.
///
/// Inputs:
/// - `key`: Normalized config key
/// - `val`: Config value
/// - `settings`: Mutable settings to update
///
/// Output:
/// - `true` if key was handled, `false` otherwise
fn parse_search_settings(key: &str, val: &str, settings: &mut Settings) -> bool {
    match key {
        "search_startup_mode" | "startup_mode" | "search_mode" => {
            let lv = val.to_ascii_lowercase();
            // Accept both boolean and mode name formats
            settings.search_startup_mode = lv == "true"
                || lv == "1"
                || lv == "yes"
                || lv == "on"
                || lv == "normal_mode"
                || lv == "normal";
            true
        }
        "fuzzy_search" | "fuzzy_search_enabled" | "fuzzy_mode" => {
            settings.fuzzy_search = parse_bool(val);
            true
        }
        _ => false,
    }
}

/// What: Parse miscellaneous settings.
///
/// Inputs:
/// - `key`: Normalized config key
/// - `val`: Config value
/// - `settings`: Mutable settings to update
///
/// Output:
/// - `true` if key was handled, `false` otherwise
fn parse_misc_settings(key: &str, val: &str, settings: &mut Settings) -> bool {
    match key {
        "preferred_terminal" | "terminal_preferred" | "terminal" => {
            settings.preferred_terminal = val.to_string();
            true
        }
        "app_start_mode" | "start_mode" | "start_in_news" => {
            let lv = val.trim().to_ascii_lowercase();
            settings.start_in_news = matches!(lv.as_str(), "news" | "true" | "1" | "on" | "yes");
            true
        }
        "locale" | "language" => {
            settings.locale = val.trim().to_string();
            true
        }
        "updates_refresh_interval" | "updates_interval" | "refresh_interval" => {
            if let Ok(v) = val.parse::<u64>() {
                // Ensure minimum value of 1 second to prevent invalid intervals
                settings.updates_refresh_interval = v.max(1);
            }
            true
        }
        "get_announcement" | "get_announcements" => {
            settings.get_announcement = parse_bool(val.trim());
            true
        }
        "installed_packages_mode" | "installed_mode" | "installed_filter" => {
            if let Some(mode) = crate::state::InstalledPackagesMode::from_config_key(val) {
                settings.installed_packages_mode = mode;
            }
            true
        }
        "use_passwordless_sudo" | "passwordless_sudo" | "allow_passwordless_sudo" => {
            settings.use_passwordless_sudo = parse_bool(val);
            true
        }
        "use_terminal_theme" | "terminal_theme" => {
            settings.use_terminal_theme = parse_bool(val);
            true
        }
        _ => false,
    }
}

/// What: Parse non-keybind settings from settings.conf content.
///
/// Inputs:
/// - `content`: Content of the settings.conf file as a string.
/// - `_settings_path`: Path to the settings.conf file (for appending defaults).
/// - `settings`: Mutable reference to `Settings` to populate.
///
/// Output:
/// - None (modifies `settings` in-place).
///
/// Details:
/// - Parses layout percentages, app settings, scan settings, and other configuration.
/// - Missing settings are handled by `ensure_settings_keys_present` with proper comments.
/// - Intentionally ignores keybind_* entries (handled separately).
pub fn parse_settings(content: &str, _settings_path: &Path, settings: &mut Settings) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.contains('=') {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let raw_key = parts.next().unwrap_or("");
        let key = raw_key.trim().to_lowercase().replace(['.', '-', ' '], "_");
        let val_raw = parts.next().unwrap_or("").trim();
        let val = strip_inline_comment(val_raw);

        // Try each category parser in order
        // Note: we intentionally ignore keybind_* in settings.conf now; keybinds load below
        let _ = parse_layout_settings(&key, val, settings)
            || parse_app_settings(&key, val, settings)
            || parse_scan_settings(&key, val, settings)
            || parse_mirror_settings(&key, val, settings)
            || parse_news_settings(&key, val, settings)
            || parse_search_settings(&key, val, settings)
            || parse_misc_settings(&key, val, settings);
    }
}
