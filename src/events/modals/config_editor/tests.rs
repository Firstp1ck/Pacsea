use super::popup::{bump_int, handle_popup_bool};
use super::*;
use crate::state::config_editor::validate::*;
use crate::state::{ConfigEditorView, EditPopupKind};
use crate::theme::find_setting;

fn setting(key: &str) -> &'static crate::theme::EditableSetting {
    find_setting(key).expect("schema entry")
}

#[test]
fn pi_scan_setting_shortcut_opens_guided_setup() {
    let mut app = AppState {
        app_mode: crate::state::types::AppMode::ConfigEditor,
        config_editor_state: Box::new(ConfigEditorState {
            view: ConfigEditorView::KeyList,
            query: "pi_scan_enabled".to_string(),
            key_cursor: 0,
            ..ConfigEditorState::default()
        }),
        ..AppState::default()
    };

    assert!(handle_config_editor_mode_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
        &mut app,
    ));
    assert_eq!(app.app_mode, crate::state::types::AppMode::PiScan);
    assert!(app.pi_scan.wizard.is_some());
}

#[test]
fn validate_int_range_rejects_out_of_bounds() {
    let s = setting("mirror_count");
    let popup = EditPopupState {
        kind: EditPopupKind::Int { min: 1, max: 5 },
        buffer: "3".to_string(),
        caret: 1,
        ..EditPopupState::from_current(s, "3")
    };
    assert!(validate_value(popup.setting, "3").is_ok());
    assert!(validate_value(popup.setting, "10").is_ok());
    assert!(validate_value(popup.setting, "1000").is_err());
    assert!(validate_value(popup.setting, "abc").is_err());
}

#[test]
fn validate_pi_scan_token_cap_accepts_full_u64_range_only() {
    let s = setting("pi_scan_background_token_cap_24h");
    let popup = EditPopupState::from_current(s, "0");
    assert!(validate_value(popup.setting, "0").is_ok());
    assert!(validate_value(popup.setting, &u64::MAX.to_string()).is_ok());
    assert!(validate_value(popup.setting, "abc").is_err());
    assert!(validate_value(popup.setting, "18446744073709551616").is_err());
}

#[test]
fn validate_pi_scan_cost_cap_accepts_exact_micro_usd_native_bounds_only() {
    let s = setting("pi_scan_background_cost_cap_24h");
    let popup = EditPopupState::from_current(s, "0");
    assert!(validate_value(popup.setting, "0").is_ok());
    assert!(validate_value(popup.setting, "0.00").is_ok());
    assert!(validate_value(popup.setting, "18446744073709.551615").is_ok());
    assert!(validate_value(popup.setting, "18446744073709.551616").is_err());
    assert!(validate_value(popup.setting, "malformed").is_err());
}

#[test]
fn validate_optional_unsigned_or_all_accepts_all_and_numbers() {
    let s = setting("news_max_age_days");
    let popup = EditPopupState::from_current(s, "all");
    assert!(validate_value(popup.setting, "all").is_ok());
    assert!(validate_value(popup.setting, "30").is_ok());
    assert!(validate_value(popup.setting, "nope").is_err());
}

#[test]
fn validate_main_pane_order_rejects_invalid_permutation() {
    let s = setting("main_pane_order");
    let popup = EditPopupState::from_current(s, "results, search, package_info");
    assert!(validate_value(popup.setting, "results, search, package_info").is_ok());
    assert!(validate_value(popup.setting, "results, results, search").is_err());
}

#[test]
fn popup_bool_toggle_flips_buffer() {
    let s = setting("show_install_pane");
    let mut popup = EditPopupState::from_current(s, "false");
    let ke = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    handle_popup_bool(ke, &mut popup);
    assert_eq!(popup.canonical_value(), "true");
}

#[test]
fn popup_enum_arrow_cycles() {
    let s = setting("sort_mode");
    let mut popup = EditPopupState::from_current(s, "best_matches");
    let ke = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    handle_popup_enum_test(ke, &mut popup);
    assert_ne!(popup.canonical_value(), "best_matches");
}

/// Cycling forward from the final locale wraps to the first shipped choice.
#[test]
fn locale_popup_cycles_forward_from_last_to_first() {
    let mut popup = EditPopupState::from_current(setting("locale"), "hu-HU");
    handle_popup_enum_test(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut popup);
    assert!(matches!(popup.kind, EditPopupKind::Enum { index: 0, .. }));
    assert_eq!(popup.canonical_value(), "");
}

/// Enum validation rejects any value outside the schema choices.
#[test]
fn locale_popup_rejects_non_choice_values() {
    let popup = EditPopupState::from_current(setting("locale"), "en-US");
    assert!(validate_value(popup.setting, "de-DE").is_ok());
    assert!(validate_value(popup.setting, "fr-FR").is_err());
    assert!(validate_value(popup.setting, "").is_ok());
}

/// A click on a choice row selects without saving; other coordinates are ignored.
#[test]
fn config_editor_popup_click_selects_only_choice_rows() {
    let mut app = AppState::default();
    app.config_editor_state.popup = Some(EditPopupState::from_current(setting("locale"), "en-US"));
    app.config_editor_state.popup_choice_rects =
        vec![(10, 20, 25, 1), (10, 21, 25, 1), (10, 22, 25, 1)];
    assert!(handle_config_editor_popup_click(12, 22, &mut app));
    let popup = app
        .config_editor_state
        .popup
        .as_ref()
        .expect("popup remains open");
    assert_eq!(popup.canonical_value(), "de-DE");
    assert!(!handle_config_editor_popup_click(9, 22, &mut app));
    assert!(!handle_config_editor_popup_click(12, 23, &mut app));
    app.config_editor_state.popup = None;
    assert!(!handle_config_editor_popup_click(12, 22, &mut app));
}

#[test]
fn popup_int_bumps_within_range() {
    let s = setting("mirror_count");
    let mut popup = EditPopupState::from_current(s, "10");
    bump_int(&mut popup, 5, 1, 200);
    assert_eq!(popup.canonical_value(), "15");
    bump_int(&mut popup, 1_000, 1, 200);
    assert_eq!(popup.canonical_value(), "200");
}

#[test]
fn validate_color_accepts_hex_and_rgb_and_rejects_invalid() {
    let popup = EditPopupState {
        kind: EditPopupKind::Text,
        buffer: String::new(),
        caret: 0,
        ..EditPopupState::from_current(setting("clipboard_suffix"), "")
    };
    assert!(validate_color_value("#1e1e2e").is_ok());
    assert!(validate_color_value("1e1e2e").is_ok());
    assert!(validate_color_value("255, 0, 10").is_ok());
    assert!(validate_color_value("256,0,0").is_err());
    assert!(validate_color_value("#xyzxyz").is_err());
    // keep compiler aware validate_value uses per-kind branching
    assert!(validate_value(popup.setting, "anything").is_ok());
}

#[test]
fn validate_key_chord_accepts_expected_forms_and_rejects_invalid() {
    assert!(validate_key_chord("Ctrl+R").is_ok());
    assert!(validate_key_chord("Shift+Tab").is_ok());
    assert!(validate_key_chord("F5").is_ok());
    assert!(validate_key_chord("Ctrl+").is_err());
    assert!(validate_key_chord("Ctrl+NoSuchToken").is_err());
}

#[test]
fn parse_validated_key_chord_returns_canonical_chord() {
    let c = parse_validated_key_chord("Ctrl+R").expect("ok");
    assert_eq!(c.code, KeyCode::Char('r'));
    assert!(c.mods.contains(KeyModifiers::CONTROL));
    let bt = parse_validated_key_chord("Shift+Tab").expect("ok");
    assert_eq!(bt.code, KeyCode::BackTab);
    assert!(bt.mods.is_empty());
}

/// What: Build editor state with an open popup for a keybind schema row.
///
/// Inputs:
/// - `key`: Canonical keybind schema key.
/// - `current`: Current chord string used to prefill the popup buffer.
///
/// Output:
/// - `ConfigEditorState` whose popup is a non-capturing `KeyChord` editor.
fn keychord_popup_state(key: &str, current: &str) -> ConfigEditorState {
    let s = find_setting(key).expect("keybind schema entry");
    ConfigEditorState {
        popup: Some(EditPopupState::from_current(s, current)),
        ..ConfigEditorState::default()
    }
}

#[test]
fn keychord_popup_ctrl_k_starts_capture_and_records_next_chord() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    let mut state = keychord_popup_state("keybind_reload_config", "Ctrl+r");
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        &mut app,
        &mut state,
    );
    assert!(matches!(
        state.popup.as_ref().map(|p| &p.kind),
        Some(EditPopupKind::KeyChord { capturing: true })
    ));

    handle_popup_key(
        KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    let popup = state.popup.as_ref().expect("popup stays open");
    assert_eq!(popup.buffer, "F9");
    assert!(matches!(
        popup.kind,
        EditPopupKind::KeyChord { capturing: false }
    ));
}

#[test]
fn keychord_capture_records_ctrl_s_instead_of_saving() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    let mut state = keychord_popup_state("keybind_reload_config", "Ctrl+r");
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        &mut app,
        &mut state,
    );
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        &mut app,
        &mut state,
    );
    let popup = state.popup.as_ref().expect("capture must not trigger save");
    assert_eq!(popup.buffer, "Ctrl+s");
    assert!(state.status.is_none(), "no save status while capturing");
}

#[test]
fn keychord_capture_esc_cancels_and_keeps_buffer() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    let mut state = keychord_popup_state("keybind_reload_config", "Ctrl+r");
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        &mut app,
        &mut state,
    );
    handle_popup_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    let popup = state.popup.as_ref().expect("Esc only cancels capture");
    assert_eq!(popup.buffer, "Ctrl+r");
    assert!(matches!(
        popup.kind,
        EditPopupKind::KeyChord { capturing: false }
    ));
}

#[test]
fn keychord_capture_ignores_unsupported_keys() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    let mut state = keychord_popup_state("keybind_reload_config", "Ctrl+r");
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        &mut app,
        &mut state,
    );
    handle_popup_key(
        KeyEvent::new(KeyCode::Null, KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    let popup = state.popup.as_ref().expect("popup stays open");
    assert_eq!(popup.buffer, "Ctrl+r", "unsupported key must not record");
    assert!(matches!(
        popup.kind,
        EditPopupKind::KeyChord { capturing: true }
    ));
}

#[test]
fn popup_ctrl_z_resets_buffer_to_opening_value() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    let mut state = keychord_popup_state("keybind_reload_config", "Ctrl+r");
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    assert_eq!(
        state.popup.as_ref().map(|p| p.buffer.as_str()),
        Some("Ctrl+rx")
    );
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
        &mut app,
        &mut state,
    );
    let popup = state.popup.as_ref().expect("popup stays open after reset");
    assert_eq!(popup.buffer, "Ctrl+r");
    assert_eq!(popup.original, "Ctrl+r");
}

#[test]
fn keybind_conflict_detected_within_same_scope() {
    // `keybind_help` defaults include `?`; trying to assign `?` to another
    // global action must be flagged as a conflict.
    let s = crate::theme::find_setting("keybind_reload_config").expect("schema entry");
    let err = validate_keybind_conflict(s, "?").expect_err("conflict must be detected");
    assert!(
        err.contains("keybind_help"),
        "error should name conflicting action, got: {err}"
    );
}

#[test]
fn keybind_conflict_allows_self_rebind() {
    let s = crate::theme::find_setting("keybind_reload_config").expect("schema entry");
    // Reassigning the same chord to the action that already has it is fine
    // because the conflict check excludes the entry being edited.
    assert!(validate_keybind_conflict(s, "Ctrl+R").is_ok());
}

#[test]
fn keybind_conflict_ignores_cross_scope_overlap() {
    // `Left` is bound globally to `pane_left` and in the search scope to
    // `search_focus_left`. Editing `search_focus_left` to `Left` must not
    // be flagged as a conflict because scopes differ.
    let s = crate::theme::find_setting("keybind_search_focus_left").expect("schema entry");
    assert!(validate_keybind_conflict(s, "Left").is_ok());
}

#[test]
fn validate_preferred_terminal_rejects_spaces_and_controls() {
    assert!(validate_preferred_terminal_value("").is_ok());
    assert!(validate_preferred_terminal_value("kitty").is_ok());
    assert!(validate_preferred_terminal_value("kitty -e").is_err());
    assert!(validate_preferred_terminal_value("kitty\n").is_ok());
    assert!(validate_preferred_terminal_value("kitty\tterm").is_err());
}

#[test]
fn validate_selected_countries_rejects_empty_or_invalid_entries() {
    assert!(validate_selected_countries_value("Worldwide").is_ok());
    assert!(validate_selected_countries_value("Germany, France").is_ok());
    assert!(validate_selected_countries_value("").is_err());
    assert!(validate_selected_countries_value("Germany, ").is_err());
    assert!(validate_selected_countries_value("Ger{many").is_err());
}

/// Cycle the strict picker for legacy control tests.
fn handle_popup_enum_test(ke: KeyEvent, popup: &mut EditPopupState) {
    let mut state = ConfigEditorState {
        popup: Some(popup.clone()),
        ..ConfigEditorState::default()
    };
    handle_popup_key(ke, &mut AppState::default(), &mut state);
    *popup = state.popup.expect("popup");
}

/// Recent selection, query clearing, and leaving the editor follow the Esc ladder.
#[test]
fn list_keys_tabs_recents_and_escape_ladder() {
    let mut app = AppState {
        app_mode: crate::state::types::AppMode::ConfigEditor,
        ..AppState::default()
    };
    app.config_editor_state.recent_queries = vec!["mirror".into(), "news".into()];
    app.config_editor_state.bookmarked_keys.clear();
    for (code, mods) in [
        (KeyCode::Char('b'), KeyModifiers::NONE),
        (KeyCode::Char('r'), KeyModifiers::CONTROL),
        (KeyCode::Down, KeyModifiers::NONE),
        (KeyCode::Enter, KeyModifiers::NONE),
    ] {
        handle_config_editor_mode_key(KeyEvent::new(code, mods), &mut app);
    }
    assert_eq!(app.config_editor_state.query, "news");
    assert!(app.config_editor_state.recent_dropdown.is_none());
    handle_config_editor_mode_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        &mut app,
    );
    handle_config_editor_mode_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut app);
    assert_eq!(app.config_editor_state.query, "news");
    handle_config_editor_mode_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut app);
    assert_eq!(app.config_editor_state.query.len(), 0);
    assert_eq!(app.app_mode, crate::state::types::AppMode::ConfigEditor);
    handle_config_editor_mode_key(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        &mut app,
    );
    assert!(
        app.config_editor_state.is_bookmarked(
            app.config_editor_state
                .selected_key()
                .expect("selected")
                .key
        )
    );
    handle_config_editor_mode_key(
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE),
        &mut app,
    );
    assert_eq!(app.config_editor_state.view, ConfigEditorView::Starred);
    handle_config_editor_mode_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut app);
    assert!(app.config_editor_state.popup.is_some());
    handle_config_editor_mode_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut app);
    handle_config_editor_mode_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut app);
    assert_eq!(app.app_mode, crate::state::types::AppMode::Package);
}

/// Page movement uses renderer row counts and Home/End reach the filtered boundaries.
#[test]
fn list_page_and_boundary_keys() {
    let mut app = AppState::default();
    let mut state = ConfigEditorState {
        list_page_rows: 7,
        ..ConfigEditorState::default()
    };
    handle_editor_key(
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        &mut state,
        &mut app,
    );
    assert_eq!(state.key_cursor, 7);
    handle_editor_key(
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        &mut state,
        &mut app,
    );
    assert_eq!(state.key_cursor, 0);
    handle_editor_key(
        KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        &mut state,
        &mut app,
    );
    assert_eq!(state.key_cursor, state.filtered_keys().len() - 1);
    handle_editor_key(
        KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
        &mut state,
        &mut app,
    );
    assert_eq!(state.key_cursor, 0);
}

/// Picker Enter opens before saving, free-input navigation highlights, and integer nudges clamp.
#[test]
fn popup_keyboard_contract_and_validation() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    let mut state = ConfigEditorState {
        query: "show_install_pane".into(),
        ..ConfigEditorState::default()
    };
    handle_editor_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut state,
        &mut app,
    );
    assert!(state.popup.is_some());
    handle_popup_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    assert!(state.popup.is_none());
    state.popup = Some(EditPopupState::from_current(setting("mirror_count"), "200"));
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    assert_eq!(state.popup.as_ref().expect("popup").buffer, "200");
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    assert_eq!(state.popup.as_ref().expect("popup").buffer, "199");
    handle_popup_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        &mut app,
        &mut state,
    );
    assert_eq!(state.popup.as_ref().expect("popup").buffer, "199");
    state.popup = Some(EditPopupState::from_current(
        setting("keybind_reload_config"),
        "Ctrl+r",
    ));
    let popup = state.popup.as_mut().expect("popup");
    popup.buffer = "?".into();
    popup.caret = 1;
    popup.refresh_visible();
    assert!(
        popup
            .validation
            .as_ref()
            .expect_err("conflict")
            .contains("keybind_help")
    );
    popup.buffer = "F9".into();
    popup.caret = 2;
    popup.refresh_visible();
    assert!(popup.validation.is_ok());
    state.popup = Some(EditPopupState::from_current(
        setting("virustotal_api_key"),
        "secret",
    ));
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        &mut app,
        &mut state,
    );
    assert!(matches!(
        state.popup.as_ref().expect("popup").kind,
        EditPopupKind::Secret { revealed: true }
    ));
}

/// Both signs nudge the displayed preset rather than the untouched opening buffer.
#[test]
fn regression_integer_nudge_uses_highlight() {
    for (sign, preset, expected) in [('+', "1", "2"), ('-', "5", "4")] {
        let mut state = ConfigEditorState {
            popup: Some(EditPopupState::from_current(setting("mirror_count"), "20")),
            ..ConfigEditorState::default()
        };
        let popup = state.popup.as_mut().expect("popup");
        let index = popup
            .visible
            .iter()
            .position(|&i| popup.suggestions[i].value == preset)
            .expect("preset");
        popup.select_highlight(index);
        handle_popup_key(
            KeyEvent::new(KeyCode::Char(sign), KeyModifiers::NONE),
            &mut AppState::default(),
            &mut state,
        );
        let popup = state.popup.as_ref().expect("popup");
        assert_eq!(popup.effective_value(), expected);
        assert!(popup.highlight.is_none());
    }
}

/// Bool pickers have no text box, so typing cannot apply a hidden filter.
#[test]
fn regression_bool_printable_input_is_ignored() {
    let mut state = ConfigEditorState {
        popup: Some(EditPopupState::from_current(
            setting("show_install_pane"),
            "false",
        )),
        ..ConfigEditorState::default()
    };
    handle_popup_key(
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        &mut AppState::default(),
        &mut state,
    );
    let popup = state.popup.as_ref().expect("popup");
    assert_eq!(popup.visible.len(), 2);
    assert_eq!(popup.buffer, "false");
    assert_eq!(popup.effective_value(), "false");
    assert!(!popup.filter_active);
}
