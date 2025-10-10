use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::config::SKELETON_CONFIG_CONTENT;
use super::parsing::{parse_key_chord, strip_inline_comment};
use super::paths::repo_config_path;
use super::types::Settings;

/// Load user settings from the same config file as the theme.
/// Falls back to `Settings::default()` when missing or invalid.
pub fn settings() -> Settings {
    let mut out = Settings::default();
    // Prefer repository-local config for cargo/dev runs, creating it if missing
    let path = if let Some(rcp) = repo_config_path() {
        if rcp.is_file() {
            Some(rcp)
        } else {
            if let Some(dir) = rcp.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&rcp, SKELETON_CONFIG_CONTENT);
            Some(rcp)
        }
    } else {
        None
    }
    .or_else(|| {
        env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")))
            .map(|base| base.join("pacsea").join("pacsea.conf"))
    });
    let Some(p) = path else {
        return out;
    };
    let Ok(content) = fs::read_to_string(&p) else {
        return out;
    };
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
                out.app_dry_run_default = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            "sort_mode" | "results_sort" => {
                if let Some(sm) = crate::state::SortMode::from_config_key(val) {
                    out.sort_mode = sm;
                }
            }
            "clipboard_suffix" | "copy_suffix" => {
                out.clipboard_suffix = val.to_string();
            }
            // Keybindings (single chord per action); overrides full list
            "keybind_help" | "keybind_help_overlay" => {
                if let Some(ch) = parse_key_chord(val) {
                    out.keymap.help_overlay = vec![ch];
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
            "keybind_pane_next" | "keybind_next_pane" | "keybind_switch_pane" => {
                if let Some(ch) = parse_key_chord(val) {
                    out.keymap.pane_next = vec![ch];
                }
            }
            "keybind_pane_prev" | "keybind_prev_pane" => {
                if let Some(ch) = parse_key_chord(val) {
                    out.keymap.pane_prev = vec![ch];
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
            // Search normal mode
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
