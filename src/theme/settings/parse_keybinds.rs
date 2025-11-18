use crate::theme::parsing::{parse_key_chord, strip_inline_comment};
use crate::theme::types::Settings;

/// What: Parse keybind entries from configuration file content.
///
/// Inputs:
/// - `content`: Content of the configuration file (keybinds.conf or settings.conf) as a string.
/// - `settings`: Mutable reference to `Settings` to populate keybinds.
///
/// Output:
/// - None (modifies `settings.keymap` in-place).
///
/// Details:
/// - Parses all keybind_* entries from the content.
/// - Handles both dedicated keybinds.conf format and legacy settings.conf format.
/// - For some keybinds (recent_remove, install_remove), allows multiple bindings by checking for duplicates.
pub fn parse_keybinds(content: &str, settings: &mut Settings) {
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
                    settings.keymap.help_overlay = vec![ch];
                }
            }
            // New: dropdown toggles
            "keybind_toggle_config" | "keybind_config_menu" | "keybind_config_lists" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.config_menu_toggle = vec![ch];
                }
            }
            "keybind_toggle_options" | "keybind_options_menu" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.options_menu_toggle = vec![ch];
                }
            }
            "keybind_toggle_panels" | "keybind_panels_menu" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.panels_menu_toggle = vec![ch];
                }
            }
            "keybind_reload_theme" | "keybind_reload" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.reload_theme = vec![ch];
                }
            }
            "keybind_exit" | "keybind_quit" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.exit = vec![ch];
                }
            }
            "keybind_show_pkgbuild" | "keybind_pkgbuild" | "keybind_toggle_pkgbuild" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.show_pkgbuild = vec![ch];
                }
            }
            "keybind_change_sort" | "keybind_sort" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.change_sort = vec![ch];
                }
            }
            "keybind_pane_next" | "keybind_next_pane" | "keybind_switch_pane" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.pane_next = vec![ch];
                }
            }
            "keybind_pane_left" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.pane_left = vec![ch];
                }
            }
            "keybind_pane_right" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.pane_right = vec![ch];
                }
            }

            // Search pane
            "keybind_search_move_up" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_move_up = vec![ch];
                }
            }
            "keybind_search_move_down" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_move_down = vec![ch];
                }
            }
            "keybind_search_page_up" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_page_up = vec![ch];
                }
            }
            "keybind_search_page_down" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_page_down = vec![ch];
                }
            }
            "keybind_search_add" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_add = vec![ch];
                }
            }
            "keybind_search_install" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_install = vec![ch];
                }
            }
            "keybind_search_focus_left" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_focus_left = vec![ch];
                }
            }
            "keybind_search_focus_right" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_focus_right = vec![ch];
                }
            }
            "keybind_search_backspace" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_backspace = vec![ch];
                }
            }
            "keybind_search_normal_toggle" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_toggle = vec![ch];
                }
            }
            "keybind_search_normal_insert" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_insert = vec![ch];
                }
            }
            "keybind_search_normal_select_left" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_select_left = vec![ch];
                }
            }
            "keybind_search_normal_select_right" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_select_right = vec![ch];
                }
            }
            "keybind_search_normal_delete" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_delete = vec![ch];
                }
            }
            "keybind_search_normal_clear" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_clear = vec![ch];
                }
            }
            "keybind_search_normal_open_status"
            | "keybind_normal_open_status"
            | "keybind_open_status" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_open_status = vec![ch];
                }
            }
            "keybind_search_normal_import" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_import = vec![ch];
                }
            }
            "keybind_search_normal_export" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.search_normal_export = vec![ch];
                }
            }

            // Recent pane
            "keybind_recent_move_up" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_move_up = vec![ch];
                }
            }
            "keybind_recent_move_down" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_move_down = vec![ch];
                }
            }
            "keybind_recent_find" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_find = vec![ch];
                }
            }
            "keybind_recent_use" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_use = vec![ch];
                }
            }
            "keybind_recent_add" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_add = vec![ch];
                }
            }
            "keybind_recent_to_search" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_to_search = vec![ch];
                }
            }
            "keybind_recent_focus_right" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_focus_right = vec![ch];
                }
            }
            "keybind_recent_remove" => {
                if let Some(ch) = parse_key_chord(val)
                    && settings
                        .keymap
                        .recent_remove
                        .iter()
                        .all(|c| c.code != ch.code || c.mods != ch.mods)
                {
                    settings.keymap.recent_remove.push(ch);
                }
            }
            "keybind_recent_clear" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.recent_clear = vec![ch];
                }
            }

            // Install pane
            "keybind_install_move_up" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.install_move_up = vec![ch];
                }
            }
            "keybind_install_move_down" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.install_move_down = vec![ch];
                }
            }
            "keybind_install_confirm" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.install_confirm = vec![ch];
                }
            }
            "keybind_install_remove" => {
                if let Some(ch) = parse_key_chord(val)
                    && settings
                        .keymap
                        .install_remove
                        .iter()
                        .all(|c| c.code != ch.code || c.mods != ch.mods)
                {
                    settings.keymap.install_remove.push(ch);
                }
            }
            "keybind_install_clear" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.install_clear = vec![ch];
                }
            }
            "keybind_install_find" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.install_find = vec![ch];
                }
            }
            "keybind_install_to_search" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.install_to_search = vec![ch];
                }
            }
            "keybind_install_focus_left" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.install_focus_left = vec![ch];
                }
            }
            "keybind_news_mark_all_read" => {
                if let Some(ch) = parse_key_chord(val) {
                    settings.keymap.news_mark_all_read = vec![ch];
                }
            }
            _ => {}
        }
    }
}
