use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// no longer writing skeleton here
use super::parsing::{parse_key_chord, strip_inline_comment};
use super::paths::{resolve_keybinds_config_path, resolve_settings_config_path};
// Repo-local config is disabled; always use HOME/XDG.
use super::types::Settings;

/// What: Load user settings and keybinds from config files under HOME/XDG.
///
/// Inputs:
/// - None (reads `settings.conf` and `keybinds.conf` if present)
///
/// Output:
/// - A `Settings` value; falls back to `Settings::default()` when missing or invalid.
pub fn settings() -> Settings {
    let mut out = Settings::default();
    // Load settings from settings.conf (or legacy pacsea.conf)
    let settings_path = resolve_settings_config_path().or_else(|| {
        env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")))
            .map(|base| base.join("pacsea").join("settings.conf"))
    });
    if let Some(p) = settings_path.as_ref()
        && let Ok(content) = fs::read_to_string(p)
    {
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
                        out.layout_left_pct = v;
                    }
                }
                "layout_center_pct" => {
                    if let Ok(v) = val.parse::<u16>() {
                        out.layout_center_pct = v;
                    }
                }
                "layout_right_pct" => {
                    if let Ok(v) = val.parse::<u16>() {
                        out.layout_right_pct = v;
                    }
                }
                "app_dry_run_default" => {
                    let lv = val.to_ascii_lowercase();
                    out.app_dry_run_default =
                        lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "sort_mode" | "results_sort" => {
                    if let Some(sm) = crate::state::SortMode::from_config_key(val) {
                        out.sort_mode = sm;
                    }
                }
                "clipboard_suffix" | "copy_suffix" => {
                    out.clipboard_suffix = val.to_string();
                }
                "show_recent_pane" | "recent_visible" => {
                    let lv = val.to_ascii_lowercase();
                    out.show_recent_pane = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "show_install_pane" | "install_visible" | "show_install_list" => {
                    let lv = val.to_ascii_lowercase();
                    out.show_install_pane = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "show_keybinds_footer" | "keybinds_visible" => {
                    let lv = val.to_ascii_lowercase();
                    out.show_keybinds_footer =
                        lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "selected_countries" | "countries" | "country" => {
                    // Accept comma-separated list; trimming occurs in normalization
                    out.selected_countries = val.to_string();
                }
                "mirror_count" | "mirrors" => {
                    if let Ok(v) = val.parse::<u16>() {
                        out.mirror_count = v;
                    }
                }
                "virustotal_api_key" | "vt_api_key" | "virustotal" => {
                    // VirusTotal API key; stored as-is and trimmed later
                    out.virustotal_api_key = val.to_string();
                }
                "scan_do_clamav" => {
                    let lv = val.to_ascii_lowercase();
                    out.scan_do_clamav = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "scan_do_trivy" => {
                    let lv = val.to_ascii_lowercase();
                    out.scan_do_trivy = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "scan_do_semgrep" => {
                    let lv = val.to_ascii_lowercase();
                    out.scan_do_semgrep = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "scan_do_shellcheck" => {
                    let lv = val.to_ascii_lowercase();
                    out.scan_do_shellcheck = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                "scan_do_virustotal" => {
                    let lv = val.to_ascii_lowercase();
                    out.scan_do_virustotal = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
                }
                // Note: we intentionally ignore keybind_* in settings.conf now; keybinds load below
                _ => {}
            }
        }
    }

    // Normalize mirror settings parsed from settings.conf
    if out.mirror_count == 0 {
        out.mirror_count = 20;
    }
    if out.mirror_count > 200 {
        out.mirror_count = 200;
    }
    if !out.selected_countries.is_empty() {
        out.selected_countries = out
            .selected_countries
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }
    // Normalize VirusTotal API key (trim whitespace)
    out.virustotal_api_key = out.virustotal_api_key.trim().to_string();

    // Load keybinds from keybinds.conf if available; otherwise fall back to legacy keys in settings file
    let keybinds_path = resolve_keybinds_config_path();
    if let Some(kp) = keybinds_path.as_ref() {
        if let Ok(content) = fs::read_to_string(kp) {
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
                    // Global
                    "keybind_help" | "keybind_help_overlay" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.help_overlay = vec![ch];
                        }
                    }
                    // New: dropdown toggles
                    "keybind_toggle_config" | "keybind_config_menu" | "keybind_config_lists" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.config_menu_toggle = vec![ch];
                        }
                    }
                    "keybind_toggle_options" | "keybind_options_menu" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.options_menu_toggle = vec![ch];
                        }
                    }
                    "keybind_toggle_panels" | "keybind_panels_menu" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.panels_menu_toggle = vec![ch];
                        }
                    }
                    "keybind_reload_theme" | "keybind_reload" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.reload_theme = vec![ch];
                        }
                    }
                    "keybind_exit" | "keybind_quit" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.exit = vec![ch];
                        }
                    }
                    "keybind_show_pkgbuild" | "keybind_pkgbuild" | "keybind_toggle_pkgbuild" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.show_pkgbuild = vec![ch];
                        }
                    }
                    "keybind_change_sort" | "keybind_sort" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.change_sort = vec![ch];
                        }
                    }
                    "keybind_pane_next" | "keybind_next_pane" | "keybind_switch_pane" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.pane_next = vec![ch];
                        }
                    }
                    "keybind_pane_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.pane_left = vec![ch];
                        }
                    }
                    "keybind_pane_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.pane_right = vec![ch];
                        }
                    }

                    // Search pane
                    "keybind_search_move_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_move_up = vec![ch];
                        }
                    }
                    "keybind_search_move_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_move_down = vec![ch];
                        }
                    }
                    "keybind_search_page_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_page_up = vec![ch];
                        }
                    }
                    "keybind_search_page_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_page_down = vec![ch];
                        }
                    }
                    "keybind_search_add" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_add = vec![ch];
                        }
                    }
                    "keybind_search_install" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_install = vec![ch];
                        }
                    }
                    "keybind_search_focus_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_focus_left = vec![ch];
                        }
                    }
                    "keybind_search_focus_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_focus_right = vec![ch];
                        }
                    }
                    "keybind_search_backspace" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_backspace = vec![ch];
                        }
                    }
                    "keybind_search_normal_toggle" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_toggle = vec![ch];
                        }
                    }
                    "keybind_search_normal_insert" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_insert = vec![ch];
                        }
                    }
                    "keybind_search_normal_select_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_select_left = vec![ch];
                        }
                    }
                    "keybind_search_normal_select_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_select_right = vec![ch];
                        }
                    }
                    "keybind_search_normal_delete" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_delete = vec![ch];
                        }
                    }
                    "keybind_search_normal_open_status"
                    | "keybind_normal_open_status"
                    | "keybind_open_status" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_open_status = vec![ch];
                        }
                    }
                    "keybind_search_normal_import" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_import = vec![ch];
                        }
                    }
                    "keybind_search_normal_export" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_export = vec![ch];
                        }
                    }

                    // Recent pane
                    "keybind_recent_move_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_move_up = vec![ch];
                        }
                    }
                    "keybind_recent_move_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_move_down = vec![ch];
                        }
                    }
                    "keybind_recent_find" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_find = vec![ch];
                        }
                    }
                    "keybind_recent_use" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_use = vec![ch];
                        }
                    }
                    "keybind_recent_add" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_add = vec![ch];
                        }
                    }
                    "keybind_recent_to_search" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_to_search = vec![ch];
                        }
                    }
                    "keybind_recent_focus_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_focus_right = vec![ch];
                        }
                    }
                    "keybind_recent_remove" => {
                        if let Some(ch) = parse_key_chord(val)
                            && out
                                .keymap
                                .recent_remove
                                .iter()
                                .all(|c| c.code != ch.code || c.mods != ch.mods)
                        {
                            out.keymap.recent_remove.push(ch);
                        }
                    }
                    "keybind_recent_clear" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_clear = vec![ch];
                        }
                    }

                    // Install pane
                    "keybind_install_move_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_move_up = vec![ch];
                        }
                    }
                    "keybind_install_move_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_move_down = vec![ch];
                        }
                    }
                    "keybind_install_confirm" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_confirm = vec![ch];
                        }
                    }
                    "keybind_install_remove" => {
                        if let Some(ch) = parse_key_chord(val)
                            && out
                                .keymap
                                .install_remove
                                .iter()
                                .all(|c| c.code != ch.code || c.mods != ch.mods)
                        {
                            out.keymap.install_remove.push(ch);
                        }
                    }
                    "keybind_install_clear" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_clear = vec![ch];
                        }
                    }
                    "keybind_install_find" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_find = vec![ch];
                        }
                    }
                    "keybind_install_to_search" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_to_search = vec![ch];
                        }
                    }
                    "keybind_install_focus_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_focus_left = vec![ch];
                        }
                    }
                    _ => {}
                }
            }
            // Done; keybinds loaded from dedicated file, so we can return now after validation
        }
    } else if let Some(p) = settings_path.as_ref() {
        // Fallback: parse legacy keybind_* from settings file if keybinds.conf not present
        if let Ok(content) = fs::read_to_string(p) {
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
                    "keybind_help" | "keybind_help_overlay" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.help_overlay = vec![ch];
                        }
                    }
                    // New: dropdown toggles (legacy fallback)
                    "keybind_toggle_config" | "keybind_config_menu" | "keybind_config_lists" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.config_menu_toggle = vec![ch];
                        }
                    }
                    "keybind_toggle_options" | "keybind_options_menu" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.options_menu_toggle = vec![ch];
                        }
                    }
                    "keybind_toggle_panels" | "keybind_panels_menu" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.panels_menu_toggle = vec![ch];
                        }
                    }
                    "keybind_reload_theme" | "keybind_reload" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.reload_theme = vec![ch];
                        }
                    }
                    "keybind_exit" | "keybind_quit" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.exit = vec![ch];
                        }
                    }
                    "keybind_show_pkgbuild" | "keybind_pkgbuild" | "keybind_toggle_pkgbuild" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.show_pkgbuild = vec![ch];
                        }
                    }
                    "keybind_change_sort" | "keybind_sort" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.change_sort = vec![ch];
                        }
                    }
                    "keybind_pane_next" | "keybind_next_pane" | "keybind_switch_pane" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.pane_next = vec![ch];
                        }
                    }
                    "keybind_pane_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.pane_left = vec![ch];
                        }
                    }
                    "keybind_pane_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.pane_right = vec![ch];
                        }
                    }
                    // Search
                    "keybind_search_move_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_move_up = vec![ch];
                        }
                    }
                    "keybind_search_move_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_move_down = vec![ch];
                        }
                    }
                    "keybind_search_page_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_page_up = vec![ch];
                        }
                    }
                    "keybind_search_page_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_page_down = vec![ch];
                        }
                    }
                    "keybind_search_add" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_add = vec![ch];
                        }
                    }
                    "keybind_search_install" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_install = vec![ch];
                        }
                    }
                    "keybind_search_focus_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_focus_left = vec![ch];
                        }
                    }
                    "keybind_search_focus_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_focus_right = vec![ch];
                        }
                    }
                    "keybind_search_backspace" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_backspace = vec![ch];
                        }
                    }
                    "keybind_search_normal_toggle" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_toggle = vec![ch];
                        }
                    }
                    "keybind_search_normal_insert" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_insert = vec![ch];
                        }
                    }
                    "keybind_search_normal_select_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_select_left = vec![ch];
                        }
                    }
                    "keybind_search_normal_select_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_select_right = vec![ch];
                        }
                    }
                    "keybind_search_normal_delete" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_delete = vec![ch];
                        }
                    }
                    "keybind_search_normal_open_status"
                    | "keybind_normal_open_status"
                    | "keybind_open_status" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_open_status = vec![ch];
                        }
                    }
                    "keybind_search_normal_import" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_import = vec![ch];
                        }
                    }
                    "keybind_search_normal_export" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.search_normal_export = vec![ch];
                        }
                    }
                    // Recent
                    "keybind_recent_move_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_move_up = vec![ch];
                        }
                    }
                    "keybind_recent_move_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_move_down = vec![ch];
                        }
                    }
                    "keybind_recent_find" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_find = vec![ch];
                        }
                    }
                    "keybind_recent_use" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_use = vec![ch];
                        }
                    }
                    "keybind_recent_add" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_add = vec![ch];
                        }
                    }
                    "keybind_recent_to_search" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_to_search = vec![ch];
                        }
                    }
                    "keybind_recent_focus_right" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_focus_right = vec![ch];
                        }
                    }
                    "keybind_recent_remove" => {
                        if let Some(ch) = parse_key_chord(val)
                            && out
                                .keymap
                                .recent_remove
                                .iter()
                                .all(|c| c.code != ch.code || c.mods != ch.mods)
                        {
                            out.keymap.recent_remove.push(ch);
                        }
                    }
                    "keybind_recent_clear" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.recent_clear = vec![ch];
                        }
                    }
                    // Install
                    "keybind_install_move_up" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_move_up = vec![ch];
                        }
                    }
                    "keybind_install_move_down" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_move_down = vec![ch];
                        }
                    }
                    "keybind_install_confirm" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_confirm = vec![ch];
                        }
                    }
                    "keybind_install_remove" => {
                        if let Some(ch) = parse_key_chord(val)
                            && out
                                .keymap
                                .install_remove
                                .iter()
                                .all(|c| c.code != ch.code || c.mods != ch.mods)
                        {
                            out.keymap.install_remove.push(ch);
                        }
                    }
                    "keybind_install_clear" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_clear = vec![ch];
                        }
                    }
                    "keybind_install_find" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_find = vec![ch];
                        }
                    }
                    "keybind_install_to_search" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_to_search = vec![ch];
                        }
                    }
                    "keybind_install_focus_left" => {
                        if let Some(ch) = parse_key_chord(val) {
                            out.keymap.install_focus_left = vec![ch];
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // Validate sum; if invalid, revert to defaults
    let sum = out
        .layout_left_pct
        .saturating_add(out.layout_center_pct)
        .saturating_add(out.layout_right_pct);
    if sum != 100
        || out.layout_left_pct == 0
        || out.layout_center_pct == 0
        || out.layout_right_pct == 0
    {
        out = Settings::default();
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn settings_parse_values_and_keybinds_with_defaults_on_invalid_sum() {
        let _guard = crate::theme::test_mutex().lock().unwrap();
        let orig_home = std::env::var_os("HOME");
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_settings_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cfg = base.join(".config").join("pacsea");
        let _ = std::fs::create_dir_all(&cfg);
        unsafe { std::env::set_var("HOME", base.display().to_string()) };

        // Write settings.conf with values and bad sum (should reset to defaults)
        let settings_path = cfg.join("settings.conf");
        std::fs::write(
            &settings_path,
            "layout_left_pct=10\nlayout_center_pct=10\nlayout_right_pct=10\nsort_mode=aur_popularity\nclipboard_suffix=OK\nshow_recent_pane=true\nshow_install_pane=false\nshow_keybinds_footer=true\n",
        )
        .unwrap();
        // Write keybinds.conf
        let keybinds_path = cfg.join("keybinds.conf");
        std::fs::write(&keybinds_path, "keybind_exit = Ctrl+Q\nkeybind_help = F1\n").unwrap();

        let s = super::settings();
        // Invalid layout sum -> defaults
        assert_eq!(
            s.layout_left_pct + s.layout_center_pct + s.layout_right_pct,
            100
        );
        // Keybinds parsed
        assert!(!s.keymap.exit.is_empty());
        assert!(!s.keymap.help_overlay.is_empty());

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
