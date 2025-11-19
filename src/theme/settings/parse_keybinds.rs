use crate::theme::parsing::{parse_key_chord, strip_inline_comment};
use crate::theme::types::{KeyChord, Settings};

/// What: Assign a parsed key chord to a keymap field, replacing any existing bindings.
///
/// Inputs:
/// - `chord`: Optional parsed key chord.
/// - `target`: Mutable reference to the target vector in the keymap.
///
/// Output:
/// - None (modifies `target` in-place).
///
/// Details:
/// - If parsing succeeds, replaces the entire vector with a single-element vector containing the chord.
/// - If parsing fails, the target vector remains unchanged.
fn assign_keybind(chord: Option<KeyChord>, target: &mut Vec<KeyChord>) {
    if let Some(ch) = chord {
        *target = vec![ch];
    }
}

/// What: Add a parsed key chord to a keymap field, avoiding duplicates.
///
/// Inputs:
/// - `chord`: Optional parsed key chord.
/// - `target`: Mutable reference to the target vector in the keymap.
///
/// Output:
/// - None (modifies `target` in-place).
///
/// Details:
/// - If parsing succeeds and the chord is not already present (same code and modifiers), appends it to the vector.
/// - If parsing fails or the chord already exists, the target vector remains unchanged.
fn assign_keybind_with_duplicate_check(chord: Option<KeyChord>, target: &mut Vec<KeyChord>) {
    if let Some(ch) = chord
        && target
            .iter()
            .all(|c| c.code != ch.code || c.mods != ch.mods)
    {
        target.push(ch);
    }
}

/// What: Apply a parsed key chord to the appropriate keymap field based on the key name.
///
/// Inputs:
/// - `key`: Normalized keybind name (lowercase, underscores).
/// - `chord`: Optional parsed key chord.
/// - `settings`: Mutable reference to `Settings` to populate keybinds.
///
/// Output:
/// - None (modifies `settings.keymap` in-place).
///
/// Details:
/// - Routes the chord to the correct keymap field based on the key name.
/// - Handles both single-assignment and duplicate-checking assignment patterns.
fn apply_keybind(key: &str, chord: Option<KeyChord>, settings: &mut Settings) {
    match key {
        // Global
        "keybind_help" | "keybind_help_overlay" => {
            assign_keybind(chord, &mut settings.keymap.help_overlay);
        }
        "keybind_toggle_config" | "keybind_config_menu" | "keybind_config_lists" => {
            assign_keybind(chord, &mut settings.keymap.config_menu_toggle);
        }
        "keybind_toggle_options" | "keybind_options_menu" => {
            assign_keybind(chord, &mut settings.keymap.options_menu_toggle);
        }
        "keybind_toggle_panels" | "keybind_panels_menu" => {
            assign_keybind(chord, &mut settings.keymap.panels_menu_toggle);
        }
        "keybind_reload_theme" | "keybind_reload" => {
            assign_keybind(chord, &mut settings.keymap.reload_theme);
        }
        "keybind_exit" | "keybind_quit" => {
            assign_keybind(chord, &mut settings.keymap.exit);
        }
        "keybind_show_pkgbuild" | "keybind_pkgbuild" | "keybind_toggle_pkgbuild" => {
            assign_keybind(chord, &mut settings.keymap.show_pkgbuild);
        }
        "keybind_change_sort" | "keybind_sort" => {
            assign_keybind(chord, &mut settings.keymap.change_sort);
        }
        "keybind_pane_next" | "keybind_next_pane" | "keybind_switch_pane" => {
            assign_keybind(chord, &mut settings.keymap.pane_next);
        }
        "keybind_pane_left" => {
            assign_keybind(chord, &mut settings.keymap.pane_left);
        }
        "keybind_pane_right" => {
            assign_keybind(chord, &mut settings.keymap.pane_right);
        }
        // Search pane
        "keybind_search_move_up" => {
            assign_keybind(chord, &mut settings.keymap.search_move_up);
        }
        "keybind_search_move_down" => {
            assign_keybind(chord, &mut settings.keymap.search_move_down);
        }
        "keybind_search_page_up" => {
            assign_keybind(chord, &mut settings.keymap.search_page_up);
        }
        "keybind_search_page_down" => {
            assign_keybind(chord, &mut settings.keymap.search_page_down);
        }
        "keybind_search_add" => {
            assign_keybind(chord, &mut settings.keymap.search_add);
        }
        "keybind_search_install" => {
            assign_keybind(chord, &mut settings.keymap.search_install);
        }
        "keybind_search_focus_left" => {
            assign_keybind(chord, &mut settings.keymap.search_focus_left);
        }
        "keybind_search_focus_right" => {
            assign_keybind(chord, &mut settings.keymap.search_focus_right);
        }
        "keybind_search_backspace" => {
            assign_keybind(chord, &mut settings.keymap.search_backspace);
        }
        "keybind_search_normal_toggle" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_toggle);
        }
        "keybind_search_normal_insert" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_insert);
        }
        "keybind_search_normal_select_left" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_select_left);
        }
        "keybind_search_normal_select_right" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_select_right);
        }
        "keybind_search_normal_delete" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_delete);
        }
        "keybind_search_normal_clear" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_clear);
        }
        "keybind_search_normal_open_status"
        | "keybind_normal_open_status"
        | "keybind_open_status" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_open_status);
        }
        "keybind_search_normal_import" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_import);
        }
        "keybind_search_normal_export" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_export);
        }
        // Recent pane
        "keybind_recent_move_up" => {
            assign_keybind(chord, &mut settings.keymap.recent_move_up);
        }
        "keybind_recent_move_down" => {
            assign_keybind(chord, &mut settings.keymap.recent_move_down);
        }
        "keybind_recent_find" => {
            assign_keybind(chord, &mut settings.keymap.recent_find);
        }
        "keybind_recent_use" => {
            assign_keybind(chord, &mut settings.keymap.recent_use);
        }
        "keybind_recent_add" => {
            assign_keybind(chord, &mut settings.keymap.recent_add);
        }
        "keybind_recent_to_search" => {
            assign_keybind(chord, &mut settings.keymap.recent_to_search);
        }
        "keybind_recent_focus_right" => {
            assign_keybind(chord, &mut settings.keymap.recent_focus_right);
        }
        "keybind_recent_remove" => {
            assign_keybind_with_duplicate_check(chord, &mut settings.keymap.recent_remove);
        }
        "keybind_recent_clear" => {
            assign_keybind(chord, &mut settings.keymap.recent_clear);
        }
        // Install pane
        "keybind_install_move_up" => {
            assign_keybind(chord, &mut settings.keymap.install_move_up);
        }
        "keybind_install_move_down" => {
            assign_keybind(chord, &mut settings.keymap.install_move_down);
        }
        "keybind_install_confirm" => {
            assign_keybind(chord, &mut settings.keymap.install_confirm);
        }
        "keybind_install_remove" => {
            assign_keybind_with_duplicate_check(chord, &mut settings.keymap.install_remove);
        }
        "keybind_install_clear" => {
            assign_keybind(chord, &mut settings.keymap.install_clear);
        }
        "keybind_install_find" => {
            assign_keybind(chord, &mut settings.keymap.install_find);
        }
        "keybind_install_to_search" => {
            assign_keybind(chord, &mut settings.keymap.install_to_search);
        }
        "keybind_install_focus_left" => {
            assign_keybind(chord, &mut settings.keymap.install_focus_left);
        }
        // News modal
        "keybind_news_mark_all_read" => {
            assign_keybind(chord, &mut settings.keymap.news_mark_all_read);
        }
        _ => {}
    }
}

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
        let chord = parse_key_chord(val);
        apply_keybind(&key, chord, settings);
    }
}
