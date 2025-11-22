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

/// What: Apply a parsed key chord to global keymap fields.
///
/// Inputs:
/// - `key`: Normalized keybind name (lowercase, underscores).
/// - `chord`: Optional parsed key chord.
/// - `settings`: Mutable reference to `Settings` to populate keybinds.
///
/// Output:
/// - `true` if the key was handled, `false` otherwise.
///
/// Details:
/// - Handles global keybinds (help, menus, panes, etc.).
/// - Returns `true` if the key matched a global keybind pattern.
fn apply_global_keybind(key: &str, chord: Option<KeyChord>, settings: &mut Settings) -> bool {
    match key {
        "keybind_help" | "keybind_help_overlay" => {
            assign_keybind(chord, &mut settings.keymap.help_overlay);
            true
        }
        "keybind_toggle_config" | "keybind_config_menu" | "keybind_config_lists" => {
            assign_keybind(chord, &mut settings.keymap.config_menu_toggle);
            true
        }
        "keybind_toggle_options" | "keybind_options_menu" => {
            assign_keybind(chord, &mut settings.keymap.options_menu_toggle);
            true
        }
        "keybind_toggle_panels" | "keybind_panels_menu" => {
            assign_keybind(chord, &mut settings.keymap.panels_menu_toggle);
            true
        }
        "keybind_reload_theme" | "keybind_reload" => {
            assign_keybind(chord, &mut settings.keymap.reload_theme);
            true
        }
        "keybind_exit" | "keybind_quit" => {
            assign_keybind(chord, &mut settings.keymap.exit);
            true
        }
        "keybind_show_pkgbuild" | "keybind_pkgbuild" | "keybind_toggle_pkgbuild" => {
            assign_keybind(chord, &mut settings.keymap.show_pkgbuild);
            true
        }
        "keybind_change_sort" | "keybind_sort" => {
            assign_keybind(chord, &mut settings.keymap.change_sort);
            true
        }
        "keybind_pane_next" | "keybind_next_pane" | "keybind_switch_pane" => {
            assign_keybind(chord, &mut settings.keymap.pane_next);
            true
        }
        "keybind_pane_left" => {
            assign_keybind(chord, &mut settings.keymap.pane_left);
            true
        }
        "keybind_pane_right" => {
            assign_keybind(chord, &mut settings.keymap.pane_right);
            true
        }
        _ => false,
    }
}

/// What: Apply a parsed key chord to search pane keymap fields.
///
/// Inputs:
/// - `key`: Normalized keybind name (lowercase, underscores).
/// - `chord`: Optional parsed key chord.
/// - `settings`: Mutable reference to `Settings` to populate keybinds.
///
/// Output:
/// - `true` if the key was handled, `false` otherwise.
///
/// Details:
/// - Handles search pane keybinds (navigation, actions, normal mode, etc.).
/// - Returns `true` if the key matched a search pane keybind pattern.
fn apply_search_keybind(key: &str, chord: Option<KeyChord>, settings: &mut Settings) -> bool {
    match key {
        "keybind_search_move_up" => {
            assign_keybind(chord, &mut settings.keymap.search_move_up);
            true
        }
        "keybind_search_move_down" => {
            assign_keybind(chord, &mut settings.keymap.search_move_down);
            true
        }
        "keybind_search_page_up" => {
            assign_keybind(chord, &mut settings.keymap.search_page_up);
            true
        }
        "keybind_search_page_down" => {
            assign_keybind(chord, &mut settings.keymap.search_page_down);
            true
        }
        "keybind_search_add" => {
            assign_keybind(chord, &mut settings.keymap.search_add);
            true
        }
        "keybind_search_install" => {
            assign_keybind(chord, &mut settings.keymap.search_install);
            true
        }
        "keybind_search_focus_left" => {
            assign_keybind(chord, &mut settings.keymap.search_focus_left);
            true
        }
        "keybind_search_focus_right" => {
            assign_keybind(chord, &mut settings.keymap.search_focus_right);
            true
        }
        "keybind_search_backspace" => {
            assign_keybind(chord, &mut settings.keymap.search_backspace);
            true
        }
        "keybind_search_normal_toggle" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_toggle);
            true
        }
        "keybind_search_normal_insert" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_insert);
            true
        }
        "keybind_search_normal_select_left" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_select_left);
            true
        }
        "keybind_search_normal_select_right" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_select_right);
            true
        }
        "keybind_search_normal_delete" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_delete);
            true
        }
        "keybind_search_normal_clear" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_clear);
            true
        }
        "keybind_search_normal_open_status"
        | "keybind_normal_open_status"
        | "keybind_open_status" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_open_status);
            true
        }
        "keybind_search_normal_import" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_import);
            true
        }
        "keybind_search_normal_export" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_export);
            true
        }
        "keybind_search_normal_updates" => {
            assign_keybind(chord, &mut settings.keymap.search_normal_updates);
            true
        }
        _ => false,
    }
}

/// What: Apply a parsed key chord to recent pane keymap fields.
///
/// Inputs:
/// - `key`: Normalized keybind name (lowercase, underscores).
/// - `chord`: Optional parsed key chord.
/// - `settings`: Mutable reference to `Settings` to populate keybinds.
///
/// Output:
/// - `true` if the key was handled, `false` otherwise.
///
/// Details:
/// - Handles recent pane keybinds (navigation, actions, etc.).
/// - Returns `true` if the key matched a recent pane keybind pattern.
/// - Uses duplicate checking for `recent_remove` to allow multiple bindings.
fn apply_recent_keybind(key: &str, chord: Option<KeyChord>, settings: &mut Settings) -> bool {
    match key {
        "keybind_recent_move_up" => {
            assign_keybind(chord, &mut settings.keymap.recent_move_up);
            true
        }
        "keybind_recent_move_down" => {
            assign_keybind(chord, &mut settings.keymap.recent_move_down);
            true
        }
        "keybind_recent_find" => {
            assign_keybind(chord, &mut settings.keymap.recent_find);
            true
        }
        "keybind_recent_use" => {
            assign_keybind(chord, &mut settings.keymap.recent_use);
            true
        }
        "keybind_recent_add" => {
            assign_keybind(chord, &mut settings.keymap.recent_add);
            true
        }
        "keybind_recent_to_search" => {
            assign_keybind(chord, &mut settings.keymap.recent_to_search);
            true
        }
        "keybind_recent_focus_right" => {
            assign_keybind(chord, &mut settings.keymap.recent_focus_right);
            true
        }
        "keybind_recent_remove" => {
            assign_keybind_with_duplicate_check(chord, &mut settings.keymap.recent_remove);
            true
        }
        "keybind_recent_clear" => {
            assign_keybind(chord, &mut settings.keymap.recent_clear);
            true
        }
        _ => false,
    }
}

/// What: Apply a parsed key chord to install pane keymap fields.
///
/// Inputs:
/// - `key`: Normalized keybind name (lowercase, underscores).
/// - `chord`: Optional parsed key chord.
/// - `settings`: Mutable reference to `Settings` to populate keybinds.
///
/// Output:
/// - `true` if the key was handled, `false` otherwise.
///
/// Details:
/// - Handles install pane keybinds (navigation, actions, etc.).
/// - Returns `true` if the key matched an install pane keybind pattern.
/// - Uses duplicate checking for `install_remove` to allow multiple bindings.
fn apply_install_keybind(key: &str, chord: Option<KeyChord>, settings: &mut Settings) -> bool {
    match key {
        "keybind_install_move_up" => {
            assign_keybind(chord, &mut settings.keymap.install_move_up);
            true
        }
        "keybind_install_move_down" => {
            assign_keybind(chord, &mut settings.keymap.install_move_down);
            true
        }
        "keybind_install_confirm" => {
            assign_keybind(chord, &mut settings.keymap.install_confirm);
            true
        }
        "keybind_install_remove" => {
            assign_keybind_with_duplicate_check(chord, &mut settings.keymap.install_remove);
            true
        }
        "keybind_install_clear" => {
            assign_keybind(chord, &mut settings.keymap.install_clear);
            true
        }
        "keybind_install_find" => {
            assign_keybind(chord, &mut settings.keymap.install_find);
            true
        }
        "keybind_install_to_search" => {
            assign_keybind(chord, &mut settings.keymap.install_to_search);
            true
        }
        "keybind_install_focus_left" => {
            assign_keybind(chord, &mut settings.keymap.install_focus_left);
            true
        }
        _ => false,
    }
}

/// What: Apply a parsed key chord to news modal keymap fields.
///
/// Inputs:
/// - `key`: Normalized keybind name (lowercase, underscores).
/// - `chord`: Optional parsed key chord.
/// - `settings`: Mutable reference to `Settings` to populate keybinds.
///
/// Output:
/// - `true` if the key was handled, `false` otherwise.
///
/// Details:
/// - Handles news modal keybinds.
/// - Returns `true` if the key matched a news modal keybind pattern.
fn apply_news_keybind(key: &str, chord: Option<KeyChord>, settings: &mut Settings) -> bool {
    match key {
        "keybind_news_mark_read" => {
            if chord.is_none() {
                tracing::warn!("Failed to parse keybind_news_mark_read");
            }
            assign_keybind(chord, &mut settings.keymap.news_mark_read);
            true
        }
        "keybind_news_mark_all_read" => {
            if chord.is_none() {
                tracing::warn!("Failed to parse keybind_news_mark_all_read");
            }
            assign_keybind(chord, &mut settings.keymap.news_mark_all_read);
            true
        }
        _ => false,
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
/// - Routes the chord to the correct category-specific handler function.
/// - Handles both single-assignment and duplicate-checking assignment patterns.
fn apply_keybind(key: &str, chord: Option<KeyChord>, settings: &mut Settings) {
    if apply_global_keybind(key, chord, settings) {
        return;
    }
    if apply_search_keybind(key, chord, settings) {
        return;
    }
    if apply_recent_keybind(key, chord, settings) {
        return;
    }
    if apply_install_keybind(key, chord, settings) {
        return;
    }
    apply_news_keybind(key, chord, settings);
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
/// - For some keybinds (`recent_remove`, `install_remove`), allows multiple bindings by checking for duplicates.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn create_test_chord() -> KeyChord {
        KeyChord {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::CONTROL,
        }
    }

    #[test]
    /// What: Ensure `apply_global_keybind` correctly assigns global keybinds and handles aliases.
    ///
    /// Inputs:
    /// - Various global keybind names including aliases.
    /// - Valid and invalid key chords.
    ///
    /// Output:
    /// - Returns `true` for recognized global keybinds, `false` otherwise.
    /// - Correctly assigns chords to the appropriate keymap fields.
    ///
    /// Details:
    /// - Tests primary keybind names and their aliases.
    /// - Verifies that invalid chords don't modify the keymap.
    fn test_apply_global_keybind() {
        let mut settings = Settings::default();
        let chord = create_test_chord();

        // Test primary keybind name
        assert!(apply_global_keybind(
            "keybind_help",
            Some(chord),
            &mut settings
        ));
        assert_eq!(settings.keymap.help_overlay.len(), 1);
        assert_eq!(settings.keymap.help_overlay[0], chord);

        // Test alias
        let mut settings2 = Settings::default();
        assert!(apply_global_keybind(
            "keybind_help_overlay",
            Some(chord),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.help_overlay.len(), 1);

        // Test another keybind with multiple aliases
        let mut settings3 = Settings::default();
        assert!(apply_global_keybind(
            "keybind_toggle_config",
            Some(chord),
            &mut settings3
        ));
        assert_eq!(settings3.keymap.config_menu_toggle.len(), 1);

        let mut settings4 = Settings::default();
        assert!(apply_global_keybind(
            "keybind_config_menu",
            Some(chord),
            &mut settings4
        ));
        assert_eq!(settings4.keymap.config_menu_toggle.len(), 1);

        // Test invalid keybind name
        let mut settings5 = Settings::default();
        assert!(!apply_global_keybind(
            "keybind_invalid",
            Some(chord),
            &mut settings5
        ));

        // Test with None chord (should not modify)
        let mut settings6 = Settings::default();
        let initial_len = settings6.keymap.help_overlay.len();
        apply_global_keybind("keybind_help", None, &mut settings6);
        assert_eq!(settings6.keymap.help_overlay.len(), initial_len);
    }

    #[test]
    /// What: Ensure `apply_search_keybind` correctly assigns search pane keybinds.
    ///
    /// Inputs:
    /// - Various search pane keybind names including aliases.
    /// - Valid key chords.
    ///
    /// Output:
    /// - Returns `true` for recognized search keybinds, `false` otherwise.
    /// - Correctly assigns chords to the appropriate keymap fields.
    ///
    /// Details:
    /// - Tests search navigation, actions, and normal mode keybinds.
    /// - Verifies alias handling for `keybind_open_status`.
    fn test_apply_search_keybind() {
        let mut settings = Settings::default();
        let chord = create_test_chord();

        // Test search navigation
        assert!(apply_search_keybind(
            "keybind_search_move_up",
            Some(chord),
            &mut settings
        ));
        assert_eq!(settings.keymap.search_move_up.len(), 1);

        // Test search action
        assert!(apply_search_keybind(
            "keybind_search_add",
            Some(chord),
            &mut settings
        ));
        assert_eq!(settings.keymap.search_add.len(), 1);

        // Test alias for open_status
        let mut settings2 = Settings::default();
        assert!(apply_search_keybind(
            "keybind_search_normal_open_status",
            Some(chord),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.search_normal_open_status.len(), 1);

        let mut settings3 = Settings::default();
        assert!(apply_search_keybind(
            "keybind_normal_open_status",
            Some(chord),
            &mut settings3
        ));
        assert_eq!(settings3.keymap.search_normal_open_status.len(), 1);

        let mut settings4 = Settings::default();
        assert!(apply_search_keybind(
            "keybind_open_status",
            Some(chord),
            &mut settings4
        ));
        assert_eq!(settings4.keymap.search_normal_open_status.len(), 1);

        // Test invalid keybind
        assert!(!apply_search_keybind(
            "keybind_invalid",
            Some(chord),
            &mut settings
        ));
    }

    #[test]
    /// What: Ensure `apply_recent_keybind` correctly assigns recent pane keybinds and handles duplicate checking.
    ///
    /// Inputs:
    /// - Various recent pane keybind names.
    /// - Valid key chords, including duplicates for `recent_remove`.
    ///
    /// Output:
    /// - Returns `true` for recognized recent keybinds, `false` otherwise.
    /// - Correctly assigns chords, allowing duplicates for `recent_remove`.
    ///
    /// Details:
    /// - Tests that `recent_remove` allows multiple bindings via duplicate checking.
    /// - Verifies other keybinds replace existing bindings.
    fn test_apply_recent_keybind() {
        let mut settings = Settings::default();
        let chord1 = KeyChord {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::CONTROL,
        };
        let chord2 = KeyChord {
            code: KeyCode::Char('x'),
            mods: KeyModifiers::CONTROL,
        };

        // Test regular keybind (replaces existing)
        assert!(apply_recent_keybind(
            "keybind_recent_move_up",
            Some(chord1),
            &mut settings
        ));
        assert_eq!(settings.keymap.recent_move_up.len(), 1);
        assert!(apply_recent_keybind(
            "keybind_recent_move_up",
            Some(chord2),
            &mut settings
        ));
        assert_eq!(settings.keymap.recent_move_up.len(), 1); // Replaced, not appended

        // Test recent_remove with duplicate checking (default has 2 entries)
        let mut settings2 = Settings::default();
        let initial_len = settings2.keymap.recent_remove.len(); // Should be 2 from defaults
        assert!(apply_recent_keybind(
            "keybind_recent_remove",
            Some(chord1),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.recent_remove.len(), initial_len + 1); // Appended
        assert!(apply_recent_keybind(
            "keybind_recent_remove",
            Some(chord2),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.recent_remove.len(), initial_len + 2); // Appended again

        // Test duplicate prevention (same chord not added twice)
        assert!(apply_recent_keybind(
            "keybind_recent_remove",
            Some(chord1),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.recent_remove.len(), initial_len + 2); // Not added again

        // Test invalid keybind
        assert!(!apply_recent_keybind(
            "keybind_invalid",
            Some(chord1),
            &mut settings
        ));
    }

    #[test]
    /// What: Ensure `apply_install_keybind` correctly assigns install pane keybinds and handles duplicate checking.
    ///
    /// Inputs:
    /// - Various install pane keybind names.
    /// - Valid key chords, including duplicates for `install_remove`.
    ///
    /// Output:
    /// - Returns `true` for recognized install keybinds, `false` otherwise.
    /// - Correctly assigns chords, allowing duplicates for `install_remove`.
    ///
    /// Details:
    /// - Tests that `install_remove` allows multiple bindings via duplicate checking.
    /// - Verifies other keybinds replace existing bindings.
    fn test_apply_install_keybind() {
        let mut settings = Settings::default();
        let chord1 = KeyChord {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::CONTROL,
        };
        let chord2 = KeyChord {
            code: KeyCode::Char('x'),
            mods: KeyModifiers::CONTROL,
        };

        // Test regular keybind (replaces existing)
        assert!(apply_install_keybind(
            "keybind_install_move_up",
            Some(chord1),
            &mut settings
        ));
        assert_eq!(settings.keymap.install_move_up.len(), 1);
        assert!(apply_install_keybind(
            "keybind_install_move_up",
            Some(chord2),
            &mut settings
        ));
        assert_eq!(settings.keymap.install_move_up.len(), 1); // Replaced

        // Test install_remove with duplicate checking (default has 2 entries)
        let mut settings2 = Settings::default();
        let initial_len = settings2.keymap.install_remove.len(); // Should be 2 from defaults
        assert!(apply_install_keybind(
            "keybind_install_remove",
            Some(chord1),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.install_remove.len(), initial_len + 1); // Appended
        assert!(apply_install_keybind(
            "keybind_install_remove",
            Some(chord2),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.install_remove.len(), initial_len + 2); // Appended again

        // Test invalid keybind
        assert!(!apply_install_keybind(
            "keybind_invalid",
            Some(chord1),
            &mut settings
        ));
    }

    #[test]
    /// What: Ensure `apply_news_keybind` correctly assigns news modal keybinds.
    ///
    /// Inputs:
    /// - News modal keybind name.
    /// - Valid key chord.
    ///
    /// Output:
    /// - Returns `true` for recognized news keybind, `false` otherwise.
    /// - Correctly assigns chord to the appropriate keymap field.
    ///
    /// Details:
    /// - Tests the single news modal keybind.
    fn test_apply_news_keybind() {
        let mut settings = Settings::default();
        let chord = create_test_chord();

        // Test news_mark_read
        assert!(apply_news_keybind(
            "keybind_news_mark_read",
            Some(chord),
            &mut settings
        ));
        assert_eq!(settings.keymap.news_mark_read.len(), 1);
        assert_eq!(settings.keymap.news_mark_read[0], chord);

        // Test news_mark_all_read
        let mut settings2 = Settings::default();
        assert!(apply_news_keybind(
            "keybind_news_mark_all_read",
            Some(chord),
            &mut settings2
        ));
        assert_eq!(settings2.keymap.news_mark_all_read.len(), 1);
        assert_eq!(settings2.keymap.news_mark_all_read[0], chord);

        // Test invalid keybind
        assert!(!apply_news_keybind(
            "keybind_invalid",
            Some(chord),
            &mut settings
        ));
    }

    #[test]
    /// What: Ensure `apply_keybind` correctly routes to category-specific handlers.
    ///
    /// Inputs:
    /// - Keybind names from different categories.
    /// - Valid key chords.
    ///
    /// Output:
    /// - Correctly routes and assigns chords to appropriate keymap fields.
    ///
    /// Details:
    /// - Tests routing through the main dispatcher function.
    /// - Verifies that each category is handled correctly.
    fn test_apply_keybind_routing() {
        let mut settings = Settings::default();
        let chord = create_test_chord();

        // Test global routing
        apply_keybind("keybind_help", Some(chord), &mut settings);
        assert_eq!(settings.keymap.help_overlay.len(), 1);

        // Test search routing
        let mut settings2 = Settings::default();
        apply_keybind("keybind_search_move_up", Some(chord), &mut settings2);
        assert_eq!(settings2.keymap.search_move_up.len(), 1);

        // Test recent routing
        let mut settings3 = Settings::default();
        apply_keybind("keybind_recent_move_up", Some(chord), &mut settings3);
        assert_eq!(settings3.keymap.recent_move_up.len(), 1);

        // Test install routing
        let mut settings4 = Settings::default();
        apply_keybind("keybind_install_move_up", Some(chord), &mut settings4);
        assert_eq!(settings4.keymap.install_move_up.len(), 1);

        // Test news routing
        let mut settings5 = Settings::default();
        apply_keybind("keybind_news_mark_all_read", Some(chord), &mut settings5);
        assert_eq!(settings5.keymap.news_mark_all_read.len(), 1);

        // Test unknown keybind (should not panic)
        let mut settings6 = Settings::default();
        apply_keybind("keybind_unknown", Some(chord), &mut settings6);
    }

    #[test]
    /// What: Ensure `parse_keybinds` correctly parses configuration content and handles various formats.
    ///
    /// Inputs:
    /// - Configuration content with keybind entries, comments, and empty lines.
    /// - Various keybind name formats (with dots, dashes, spaces).
    ///
    /// Output:
    /// - Correctly parses and assigns keybinds to settings.
    /// - Ignores comments and empty lines.
    ///
    /// Details:
    /// - Tests parsing of keybind entries with different formats.
    /// - Verifies comment stripping and normalization.
    fn test_parse_keybinds() {
        let mut settings = Settings::default();
        let content = r"
# This is a comment
keybind_help = Ctrl+R
keybind.search.move.up = Ctrl+Up
keybind-search-add = Ctrl+A
keybind search install = Ctrl+I
// Another comment
keybind_recent_remove = Ctrl+D
keybind_recent_remove = Ctrl+X
invalid_line_without_equals
";

        parse_keybinds(content, &mut settings);

        // Verify parsed keybinds
        assert_eq!(settings.keymap.help_overlay.len(), 1);
        assert_eq!(settings.keymap.search_move_up.len(), 1);
        assert_eq!(settings.keymap.search_add.len(), 1);
        assert_eq!(settings.keymap.search_install.len(), 1);
        // recent_remove should have 4 entries: 2 defaults + 2 from parsing (due to duplicate checking)
        assert_eq!(settings.keymap.recent_remove.len(), 4);
    }

    #[test]
    /// What: Ensure news keybinds are parsed correctly from config file.
    ///
    /// Inputs:
    /// - Configuration content with news keybind entries.
    ///
    /// Output:
    /// - Correctly parses and assigns news keybinds to settings.
    ///
    /// Details:
    /// - Tests parsing of `news_mark_read` and `news_mark_all_read` keybinds.
    /// - Verifies that both keybinds are loaded correctly.
    fn test_parse_news_keybinds() {
        let mut settings = Settings::default();
        let content = r"
keybind_news_mark_read = r
keybind_news_mark_all_read = CTRL+R
";

        parse_keybinds(content, &mut settings);

        // Verify parsed news keybinds
        assert_eq!(settings.keymap.news_mark_read.len(), 1);
        assert_eq!(settings.keymap.news_mark_read[0].code, KeyCode::Char('r'));
        assert!(settings.keymap.news_mark_read[0].mods.is_empty());
        assert_eq!(settings.keymap.news_mark_read[0].label(), "R");

        assert_eq!(settings.keymap.news_mark_all_read.len(), 1);
        assert_eq!(
            settings.keymap.news_mark_all_read[0].code,
            KeyCode::Char('r')
        );
        assert!(
            settings.keymap.news_mark_all_read[0]
                .mods
                .contains(KeyModifiers::CONTROL)
        );
        assert_eq!(settings.keymap.news_mark_all_read[0].label(), "Ctrl+R");
    }

    #[test]
    /// What: Ensure `assign_keybind_with_duplicate_check` prevents duplicate chords.
    ///
    /// Inputs:
    /// - Same chord added multiple times.
    /// - Different chords with same code but different modifiers.
    /// - Different chords with different codes.
    ///
    /// Output:
    /// - Prevents adding exact duplicates (same code and modifiers).
    /// - Allows different chords to be added.
    ///
    /// Details:
    /// - Tests duplicate detection logic based on code and modifiers.
    fn test_assign_keybind_with_duplicate_check() {
        let mut target = Vec::new();
        let chord1 = KeyChord {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::CONTROL,
        };
        let chord2 = KeyChord {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::CONTROL, // Same as chord1
        };
        let chord3 = KeyChord {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::ALT, // Different modifier
        };
        let chord4 = KeyChord {
            code: KeyCode::Char('x'),
            mods: KeyModifiers::CONTROL, // Different code
        };

        // Add first chord
        assign_keybind_with_duplicate_check(Some(chord1), &mut target);
        assert_eq!(target.len(), 1);

        // Try to add duplicate (should not add)
        assign_keybind_with_duplicate_check(Some(chord2), &mut target);
        assert_eq!(target.len(), 1);

        // Add chord with different modifier (should add)
        assign_keybind_with_duplicate_check(Some(chord3), &mut target);
        assert_eq!(target.len(), 2);

        // Add chord with different code (should add)
        assign_keybind_with_duplicate_check(Some(chord4), &mut target);
        assert_eq!(target.len(), 3);

        // Try None (should not modify)
        let len_before = target.len();
        assign_keybind_with_duplicate_check(None, &mut target);
        assert_eq!(target.len(), len_before);
    }
}
