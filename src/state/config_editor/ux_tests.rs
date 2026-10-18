//! Regression tests for shared configuration editor state.
use super::*;

/// Schema helper for deterministic popup tests.
fn entry(key: &str) -> &'static EditableSetting {
    find_setting(key).expect("schema")
}

/// Tabs preserve search, wrap, and show bookmarks across files in stable grouped order.
#[test]
fn tabs_starred_grouping_and_multiterm_search() {
    let mut state = ConfigEditorState {
        bookmarked_keys: vec![
            "semantic_error".into(),
            "sort_mode".into(),
            "keybind_help".into(),
        ],
        ..ConfigEditorState::default()
    };
    assert_eq!(state.active_tab(), ConfigEditorTab::Settings);
    state.cycle_tab(false);
    assert_eq!(state.active_tab(), ConfigEditorTab::Starred);
    assert_eq!(
        state
            .filtered_keys()
            .iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>(),
        ["sort_mode", "keybind_help", "semantic_error"]
    );
    state.query = "semantic".into();
    state.set_tab(ConfigEditorTab::Theme);
    assert_eq!(state.query, "semantic");
    state.cycle_tab(true);
    state.cycle_tab(true);
    assert_eq!(state.active_tab(), ConfigEditorTab::Settings);
    state.query = "MIRROR count".into();
    assert_eq!(state.filtered_keys().len(), 1);
    state.query = "recent pane".into();
    assert!(
        state
            .filtered_keys()
            .iter()
            .any(|entry| entry.key == "show_search_history_pane")
    );
    state.query = "mirrors refresh".into();
    state
        .search_text
        .insert("mirror_count", "localized mirrors refresh count".into());
    assert_eq!(
        state
            .filtered_keys()
            .iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>(),
        ["mirror_count"]
    );
    state.query.clear();
    for file in [
        ConfigFile::Settings,
        ConfigFile::Keybinds,
        ConfigFile::Theme,
    ] {
        state.selected_file = file;
        let groups: Vec<_> = state
            .filtered_keys()
            .iter()
            .map(|entry| setting_group(entry))
            .collect();
        assert!(groups.windows(2).all(|groups| groups[0] <= groups[1]));
        for group in &groups {
            let actual: Vec<_> = state
                .filtered_keys()
                .into_iter()
                .filter(|entry| setting_group(entry) == *group)
                .map(|entry| entry.key)
                .collect();
            let expected: Vec<_> = settings_for(file)
                .into_iter()
                .filter(|entry| setting_group(entry) == *group)
                .map(|entry| entry.key)
                .collect();
            assert_eq!(actual, expected);
            assert!(
                group
                    .i18n_key()
                    .starts_with("app.modals.config_editor.groups.")
            );
        }
    }
}

/// Untranslated identifiers are not searchable, and summaries are indexed once.
#[test]
fn localized_search_index_excludes_missing_translations() {
    let app = crate::state::AppState::default();
    let mut state = ConfigEditorState::default();
    state.build_search_text(&app);
    assert!(!state.search_text.is_empty());
    assert!(
        state
            .search_text
            .values()
            .all(|text| !text.contains("app.modals.config_editor.settings."))
    );
}

/// Free text saves typed input unless the user explicitly highlights or completes a candidate.
#[test]
fn popup_free_highlight_completion_and_effective_value() {
    let mut popup = EditPopupState::from_current(entry("preferred_terminal"), "kitty");
    assert!(!popup.is_strict());
    assert_eq!(popup.highlight, None);
    popup.buffer.clear();
    popup.caret = 0;
    for ch in "ala".chars() {
        popup.edit_buffer(KeyCode::Char(ch));
    }
    assert_eq!(popup.effective_value(), "ala");
    assert_eq!(popup.highlight, None);
    popup.move_highlight(1);
    assert_eq!(popup.effective_value(), "alacritty");
    assert_eq!(popup.buffer, "ala");
    popup.move_highlight(-1);
    assert_eq!(popup.highlight, None);
    popup.complete_suggestion();
    assert_eq!(popup.buffer, "alacritty");
    assert_eq!(popup.highlight, None);
    popup.edit_buffer(KeyCode::Char('x'));
    assert_eq!(popup.effective_value(), "alacrittyx");
}

/// Strict pickers synchronize their control state and never accept arbitrary typed values.
#[test]
fn strict_selection_and_filter() {
    for (key, current) in [
        ("locale", "en-US"),
        ("show_install_pane", "true"),
        ("pi_scan_thinking", "low"),
        ("main_pane_order", "results, search, package_info"),
    ] {
        let mut popup = EditPopupState::from_current(entry(key), current);
        assert!(popup.is_strict());
        assert!(popup.highlight.is_some());
        assert_eq!(popup.effective_value(), current);
        popup.select_highlight(0);
        assert_eq!(popup.effective_value(), popup.canonical_value());
    }
    let mut popup = EditPopupState::from_current(entry("pi_scan_thinking"), "low");
    popup.edit_buffer(KeyCode::Char('m'));
    popup.edit_buffer(KeyCode::Char('a'));
    popup.edit_buffer(KeyCode::Char('x'));
    assert_eq!(popup.effective_value(), "max");
    assert!(popup.highlight.is_some());
    popup.edit_buffer(KeyCode::Char('!'));
    assert!(popup.highlight.is_some());
    assert_eq!(popup.effective_value(), "max");
}

/// Comma-list filtering matches the final token and preserves completed selections.
#[test]
fn comma_list_completes_last_token() {
    let mut popup = EditPopupState::from_current(entry("selected_countries"), "Germany");
    for ch in ", Fr".chars() {
        popup.edit_buffer(KeyCode::Char(ch));
    }
    assert!(popup.comma_list());
    popup.move_highlight(1);
    assert_eq!(popup.effective_value(), "Germany, France");
    popup.complete_suggestion();
    assert_eq!(popup.buffer, "Germany, France");
    assert_eq!(popup.caret, popup.buffer.len());
}

/// Mid-buffer edits, deletion, and navigation remain on Unicode character boundaries.
#[test]
fn caret_edits_utf8_boundaries() {
    let mut popup = EditPopupState::from_current(entry("clipboard_suffix"), "aé中z");
    popup.edit_buffer(KeyCode::Left);
    assert_eq!(popup.caret_column(), 3);
    popup.edit_buffer(KeyCode::Backspace);
    assert_eq!(popup.buffer, "aéz");
    popup.edit_buffer(KeyCode::Left);
    popup.edit_buffer(KeyCode::Delete);
    assert_eq!(popup.buffer, "az");
    popup.edit_buffer(KeyCode::Char('ß'));
    assert_eq!(popup.buffer, "aßz");
    popup.edit_buffer(KeyCode::Home);
    popup.edit_buffer(KeyCode::Backspace);
    assert_eq!(popup.caret_column(), 0);
    popup.edit_buffer(KeyCode::Right);
    assert_eq!(popup.caret_column(), 1);
    popup.edit_buffer(KeyCode::End);
    assert_eq!(popup.caret_column(), 3);
}

/// Arrowing through a filtered strict picker keeps the typed filter text intact.
#[test]
fn strict_highlight_keeps_filter_text() {
    let mut popup = EditPopupState::from_current(entry("sort_mode"), "best_matches");
    popup.set_suggestions(
        [
            "best_matches",
            "alphabetical",
            "official_first",
            "aur_popularity",
        ]
        .into_iter()
        .map(|value| Suggestion {
            value: value.to_string(),
            ..Suggestion::default()
        })
        .collect(),
    );
    popup.edit_buffer(KeyCode::Char('a'));
    popup.move_highlight(1);
    assert_eq!(popup.buffer, "a");
    popup.edit_buffer(KeyCode::Char('l'));
    assert_eq!(popup.buffer, "al");
    assert_eq!(popup.effective_value(), "alphabetical");
}

/// Temporary state replacements must have no persistence locations or loaded lists.
#[test]
fn regression_default_state_is_in_memory_only() {
    let state = ConfigEditorState::default();
    assert!(state.recent_queries_path.as_os_str().is_empty());
    assert!(state.bookmarked_keys_path.as_os_str().is_empty());
    assert_eq!(state.recent_queries.len(), 0);
    assert_eq!(state.bookmarked_keys.len(), 0);
}

/// Proxy validation matches the runtime's HTTPS and userinfo restrictions.
#[test]
fn regression_proxy_validation() {
    for value in ["", "https://proxy.invalid"] {
        assert!(validate_value(entry("pi_scan_https_proxy"), value).is_ok());
    }
    for value in [
        "http://proxy.invalid",
        "https://user:password@proxy.invalid",
    ] {
        assert!(
            validate_value(entry("pi_scan_https_proxy"), value).is_err(),
            "unsafe proxy accepted"
        );
    }
}

/// Comma-list completion cannot repeat a token from the completed prefix.
#[test]
fn regression_comma_list_excludes_completed_tokens() {
    let mut popup = EditPopupState::from_current(entry("selected_countries"), "Germany, Ger");
    popup.filter_active = true;
    popup.refresh_visible();
    assert!(
        !popup
            .visible
            .iter()
            .any(|&i| popup.suggestions[i].value == "Germany")
    );
    popup.buffer = "France, Ger".into();
    popup.refresh_visible();
    assert!(
        popup
            .visible
            .iter()
            .any(|&i| popup.suggestions[i].value == "Germany")
    );
}

/// An appended binding changes the full action even when its first chord is unchanged.
#[test]
fn regression_extra_chord_marks_modified() {
    let mut settings = crate::theme::Settings::default();
    settings.keymap.recent_remove.push(KeyChord {
        code: KeyCode::F(9),
        mods: KeyModifiers::NONE,
    });
    let snapshot = ValueSnapshot::from_parts(settings, crate::theme::default_theme());
    assert!(snapshot.is_modified(entry("keybind_recent_remove")));
}
