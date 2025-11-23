use std::path::Path;

use crate::theme::parsing::strip_inline_comment;
use crate::theme::types::{PackageMarker, Settings};

/// What: Parse non-keybind settings from settings.conf content.
///
/// Inputs:
/// - `content`: Content of the settings.conf file as a string.
/// - `settings_path`: Path to the settings.conf file (for appending defaults).
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
        match key.as_str() {
            "layout_left_pct" => {
                if let Ok(v) = val.parse::<u16>() {
                    settings.layout_left_pct = v;
                }
            }
            "layout_center_pct" => {
                if let Ok(v) = val.parse::<u16>() {
                    settings.layout_center_pct = v;
                }
            }
            "layout_right_pct" => {
                if let Ok(v) = val.parse::<u16>() {
                    settings.layout_right_pct = v;
                }
            }
            "app_dry_run_default" => {
                let lv = val.to_ascii_lowercase();
                settings.app_dry_run_default =
                    lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "sort_mode" | "results_sort" => {
                if let Some(sm) = crate::state::SortMode::from_config_key(val) {
                    settings.sort_mode = sm;
                }
            }
            "clipboard_suffix" | "copy_suffix" => {
                settings.clipboard_suffix = val.to_string();
            }
            "show_recent_pane" | "recent_visible" => {
                let lv = val.to_ascii_lowercase();
                settings.show_recent_pane = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "show_install_pane" | "install_visible" | "show_install_list" => {
                let lv = val.to_ascii_lowercase();
                settings.show_install_pane = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "show_keybinds_footer" | "keybinds_visible" => {
                let lv = val.to_ascii_lowercase();
                settings.show_keybinds_footer =
                    lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "selected_countries" | "countries" | "country" => {
                // Accept comma-separated list; trimming occurs in normalization
                settings.selected_countries = val.to_string();
            }
            "mirror_count" | "mirrors" => {
                if let Ok(v) = val.parse::<u16>() {
                    settings.mirror_count = v;
                }
            }
            "virustotal_api_key" | "vt_api_key" | "virustotal" => {
                // VirusTotal API key; stored as-is and trimmed later
                settings.virustotal_api_key = val.to_string();
            }
            "scan_do_clamav" => {
                let lv = val.to_ascii_lowercase();
                settings.scan_do_clamav = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "scan_do_trivy" => {
                let lv = val.to_ascii_lowercase();
                settings.scan_do_trivy = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "scan_do_semgrep" => {
                let lv = val.to_ascii_lowercase();
                settings.scan_do_semgrep = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "scan_do_shellcheck" => {
                let lv = val.to_ascii_lowercase();
                settings.scan_do_shellcheck =
                    lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "scan_do_virustotal" => {
                let lv = val.to_ascii_lowercase();
                settings.scan_do_virustotal =
                    lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "scan_do_custom" => {
                let lv = val.to_ascii_lowercase();
                settings.scan_do_custom = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "scan_do_sleuth" => {
                let lv = val.to_ascii_lowercase();
                settings.scan_do_sleuth = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "news_read_symbol" | "news_read_mark" => {
                settings.news_read_symbol = val.to_string();
            }
            "news_unread_symbol" | "news_unread_mark" => {
                settings.news_unread_symbol = val.to_string();
            }
            "preferred_terminal" | "terminal_preferred" | "terminal" => {
                settings.preferred_terminal = val.to_string();
            }
            "package_marker" => {
                let lv = val.to_ascii_lowercase();
                settings.package_marker = match lv.as_str() {
                    "full" | "full_line" | "line" | "color_line" | "color" => {
                        PackageMarker::FullLine
                    }
                    "end" | "suffix" => PackageMarker::End,
                    _ => PackageMarker::Front,
                };
            }
            "skip_preflight" | "preflight_skip" | "bypass_preflight" => {
                let lv = val.to_ascii_lowercase();
                settings.skip_preflight = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "locale" | "language" => {
                settings.locale = val.trim().to_string();
            }
            // Note: we intentionally ignore keybind_* in settings.conf now; keybinds load below
            _ => {}
        }
    }
}
