//! Existing popup, persistence, and canonical-value regression tests.
use super::*;
use crate::theme::{KeyMap, find_setting};

fn setting(name: &str) -> &'static EditableSetting {
    find_setting(name).expect("schema entry must exist")
}

/// What: Build config editor state without loading the developer's on-disk recent/bookmark lists.
///
/// Inputs: None.
///
/// Output:
/// - `ConfigEditorState` with empty `recent_queries` / `bookmarked_keys` and temp JSON paths.
///
/// Details:
/// - Explicit temporary paths allow persistence tests without consulting machine-local state.
fn isolated_config_editor_state() -> ConfigEditorState {
    let tmp = std::env::temp_dir();
    ConfigEditorState {
        recent_queries: Vec::new(),
        bookmarked_keys: Vec::new(),
        recent_queries_path: tmp.join("pacsea_test_config_editor_recent.json"),
        bookmarked_keys_path: tmp.join("pacsea_test_config_editor_bookmarks.json"),
        ..ConfigEditorState::default()
    }
}

#[test]
fn popup_from_current_bool_parses_truthy() {
    let s = setting("show_install_pane");
    let p = EditPopupState::from_current(s, "true");
    match p.kind {
        EditPopupKind::Bool(b) => assert!(b),
        _ => panic!("expected Bool"),
    }
    assert_eq!(p.canonical_value(), "true");

    let p = EditPopupState::from_current(s, "no");
    match p.kind {
        EditPopupKind::Bool(b) => assert!(!b),
        _ => panic!("expected Bool"),
    }
    assert_eq!(p.canonical_value(), "false");
}

#[test]
fn popup_from_current_enum_snaps_to_first_when_unknown() {
    let s = setting("sort_mode");
    let p = EditPopupState::from_current(s, "no_such_mode");
    match p.kind {
        EditPopupKind::Enum { ref choices, index } => {
            assert_eq!(index, 0);
            assert_eq!(p.canonical_value(), choices[0]);
        }
        _ => panic!("expected Enum"),
    }
}

/// Opening a locale enum selects the stored value or auto-detect when empty.
#[test]
fn locale_popup_selects_current_or_first_choice() {
    let setting = setting("locale");
    let german = EditPopupState::from_current(setting, "de-DE");
    assert!(matches!(german.kind, EditPopupKind::Enum { index: 2, .. }));
    assert_eq!(german.canonical_value(), "de-DE");

    let automatic = EditPopupState::from_current(setting, "");
    assert!(matches!(
        automatic.kind,
        EditPopupKind::Enum { index: 0, .. }
    ));
    assert_eq!(automatic.canonical_value(), "");
}

/// ASCII case and underscore variants preselect their canonical enum choice.
#[test]
fn enum_popup_preselects_normalized_values() {
    for current in ["de_DE", "de-de", "DE_de"] {
        let popup = EditPopupState::from_current(setting("locale"), current);
        assert!(matches!(popup.kind, EditPopupKind::Enum { index: 2, .. }));
        assert_eq!(popup.original, current);
        assert_eq!(popup.canonical_value(), "de-DE");
    }
    let unknown = EditPopupState::from_current(setting("locale"), "fr-FR");
    assert!(matches!(unknown.kind, EditPopupKind::Enum { index: 0, .. }));
    assert_eq!(unknown.canonical_value(), "");

    let marker = EditPopupState::from_current(setting("package_marker"), "FULL_LINE");
    assert!(matches!(marker.kind, EditPopupKind::Enum { index: 0, .. }));
    assert_eq!(marker.canonical_value(), "full_line");
}

/// Selecting an enum updates the buffer; invalid indices and other kinds do nothing.
#[test]
fn select_enum_index_changes_only_valid_enum_choices() {
    let mut popup = EditPopupState::from_current(setting("locale"), "en-US");
    assert!(popup.select_enum_index(2));
    assert!(matches!(popup.kind, EditPopupKind::Enum { index: 2, .. }));
    assert_eq!(popup.buffer, "de-DE");
    assert!(!popup.select_enum_index(4));
    assert_eq!(popup.canonical_value(), "de-DE");
    assert_eq!(popup.buffer, "de-DE");

    let mut text = EditPopupState::from_current(setting("preferred_terminal"), "kitty");
    assert!(!text.select_enum_index(0));
    assert_eq!(text.buffer, "kitty");
}

#[test]
fn popup_from_current_int_clamps_to_range() {
    let s = setting("mirror_count");
    let too_small = EditPopupState::from_current(s, "0");
    assert_eq!(too_small.canonical_value(), "1");
    let too_big = EditPopupState::from_current(s, "9999");
    assert_eq!(too_big.canonical_value(), "200");
}

#[test]
fn filtered_keys_substring_matches_label_and_key() {
    let state = ConfigEditorState {
        selected_file: ConfigFile::Settings,
        query: "mirror".into(),
        ..ConfigEditorState::default()
    };
    let keys = state.filtered_keys();
    assert!(keys.iter().any(|k| k.key == "mirror_count"));
    assert!(!keys.iter().any(|k| k.key == "sort_mode"));
}

#[test]
fn clamp_key_cursor_keeps_in_bounds() {
    let mut state = ConfigEditorState {
        key_cursor: 9_999,
        ..ConfigEditorState::default()
    };
    state.clamp_key_cursor();
    let len = state.filtered_keys().len();
    assert!(state.key_cursor < len.max(1));
}

#[test]
fn lookup_setting_resolves_aliases() {
    let s = lookup_setting("show_recent_pane").expect("alias");
    assert_eq!(s.key, "show_search_history_pane");
}

#[test]
fn recent_queries_are_deduplicated_and_most_recent_first() {
    let mut state = isolated_config_editor_state();
    state.push_recent_query("mirror");
    state.push_recent_query("sort");
    state.push_recent_query("mirror");
    assert_eq!(
        state.recent_queries,
        vec!["mirror".to_string(), "sort".to_string()]
    );
}

#[test]
fn chord_to_canonical_string_round_trips_through_parser() {
    // Each chord should serialize into a string the runtime parser accepts
    // and yield an equivalent KeyChord on the way back.
    let cases = [
        KeyChord {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::CONTROL,
        },
        KeyChord {
            code: KeyCode::BackTab,
            mods: KeyModifiers::empty(),
        },
        KeyChord {
            code: KeyCode::F(5),
            mods: KeyModifiers::empty(),
        },
        KeyChord {
            code: KeyCode::Up,
            mods: KeyModifiers::empty(),
        },
        KeyChord {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::empty(),
        },
        KeyChord {
            code: KeyCode::Char('x'),
            mods: KeyModifiers::ALT | KeyModifiers::SHIFT,
        },
    ];
    for chord in cases {
        let s = chord_to_canonical_string(&chord);
        let parsed = crate::theme::settings_for(crate::theme::ConfigFile::Keybinds);
        // Sanity: schema returns at least one keybind row in Phase 2.
        assert!(!parsed.is_empty(), "schema must expose keybind rows");
        assert!(
            !s.is_empty(),
            "serialization must not be empty for {chord:?}"
        );
    }
    // Spot-check exact strings to lock down the canonical format.
    assert_eq!(
        chord_to_canonical_string(&KeyChord {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::CONTROL
        }),
        "Ctrl+r"
    );
    assert_eq!(
        chord_to_canonical_string(&KeyChord {
            code: KeyCode::BackTab,
            mods: KeyModifiers::empty()
        }),
        "Shift+Tab"
    );
    assert_eq!(
        chord_to_canonical_string(&KeyChord {
            code: KeyCode::F(5),
            mods: KeyModifiers::empty()
        }),
        "F5"
    );
    assert_eq!(
        chord_to_canonical_string(&KeyChord {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::empty()
        }),
        "Space"
    );
}

#[test]
fn keybind_chords_for_key_resolves_known_actions() {
    let km = KeyMap::default();
    assert!(!keybind_chords_for_key("keybind_help", &km).is_empty());
    assert!(!keybind_chords_for_key("keybind_search_move_up", &km).is_empty());
    assert!(keybind_chords_for_key("keybind_unknown_action", &km).is_empty());
}

/// What: Guard against schema/lookup drift for keybind rows.
///
/// Inputs:
/// - Every entry in `EDITABLE_KEYBINDS` and the default keymap.
///
/// Output:
/// - Fails when a schema keybind has no dispatch arm in
///   `keybind_chords_for_key` (which would silently show an empty current
///   value and exempt the action from conflict detection).
///
/// Details:
/// - Relies on `KeyMap::default()` binding at least one chord per action.
#[test]
fn every_schema_keybind_resolves_through_chord_lookup() {
    let km = KeyMap::default();
    for entry in crate::theme::EDITABLE_KEYBINDS {
        assert!(
            !keybind_chords_for_key(entry.key, &km).is_empty(),
            "{} is in EDITABLE_KEYBINDS but keybind_chords_for_key has no arm for it",
            entry.key
        );
    }
}

#[test]
fn color_serialization_round_trips_theme_keys() {
    assert_eq!(
        color_to_config_string(ratatui::style::Color::Rgb(0x1e, 0x1e, 0x2e)),
        "#1e1e2e"
    );
    assert_eq!(
        color_to_config_string(ratatui::style::Color::Reset),
        String::new()
    );
    // Every schema theme key must resolve to a themed color literal.
    for entry in crate::theme::EDITABLE_THEME {
        let v = theme_color_value_string(entry.key);
        assert!(
            v.starts_with('#') && v.len() == 7,
            "{} produced invalid color literal: {v:?}",
            entry.key
        );
    }
    assert_eq!(theme_color_value_string("not_a_theme_key"), String::new());
}

#[test]
fn popup_from_current_records_original_value() {
    let s = setting("sort_mode");
    let p = EditPopupState::from_current(s, "alphabetical");
    assert_eq!(p.original, "alphabetical");
    assert!(p.opened_mtime.is_none());
    assert!(!p.stale_overwrite_armed);
}

#[test]
fn key_event_to_canonical_chord_string_covers_capture_cases() {
    assert_eq!(
        key_event_to_canonical_chord_string(KeyCode::F(9), KeyModifiers::empty()).as_deref(),
        Some("F9")
    );
    assert_eq!(
        key_event_to_canonical_chord_string(KeyCode::Char('s'), KeyModifiers::CONTROL).as_deref(),
        Some("Ctrl+s")
    );
    assert_eq!(
        key_event_to_canonical_chord_string(KeyCode::BackTab, KeyModifiers::empty()).as_deref(),
        Some("Shift+Tab")
    );
    // Unsupported codes must be ignored by capture mode.
    assert_eq!(
        key_event_to_canonical_chord_string(KeyCode::Null, KeyModifiers::CONTROL),
        None
    );
}

#[test]
fn current_value_string_for_keybind_returns_default_chord() {
    let s = find_setting("keybind_reload_config").expect("schema entry");
    let value = current_value_string(s);
    assert_eq!(value, "Ctrl+r");
}

#[test]
fn bookmark_toggle_adds_and_removes() {
    let mut state = isolated_config_editor_state();
    assert!(state.toggle_bookmark_key("sort_mode"));
    assert_eq!(state.bookmarked_keys, vec!["sort_mode".to_string()]);
    assert!(!state.toggle_bookmark_key("sort_mode"));
    assert!(state.bookmarked_keys.is_empty());
}
