//! Render tests for the config editor (TestBackend, deterministic values).

use std::path::Path;

use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

use super::{FrameCtx, keybind_footer_top, render_frame};
use crate::state::config_editor::{
    Suggestion, SuggestionContext, SuggestionTag, ValueSnapshot, suggestions_for,
};
use crate::state::{AppState, ConfigEditorState, ConfigEditorTab, EditPopupKind, EditPopupState};
use crate::theme::{EditableSetting, Settings, default_theme, find_setting};

/// Application state with the shipped English translations loaded.
fn english_app() -> AppState {
    let mut app = AppState::default();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
    let translations = crate::i18n::load_locale_file("en-US", &dir).expect("English locale");
    app.translations.clone_from(&translations);
    app.translations_fallback = translations;
    app.show_keybinds_footer = true;
    app
}

/// Editor state without persisted history or bookmarks from the developer's machine.
fn clean_state(app: &AppState) -> ConfigEditorState {
    let mut state = ConfigEditorState {
        recent_queries: Vec::new(),
        bookmarked_keys: Vec::new(),
        ..ConfigEditorState::default()
    };
    state.build_search_text(app);
    state
}

/// Snapshot of codebase defaults so user configuration never leaks into assertions.
fn default_values() -> ValueSnapshot {
    ValueSnapshot::from_parts(Settings::default(), default_theme())
}

/// Schema row by key.
fn entry(key: &str) -> &'static EditableSetting {
    find_setting(key).expect("schema key")
}

/// Render one editor frame and return the buffer plus the cursor position.
fn draw(
    app: &AppState,
    state: &mut ConfigEditorState,
    values: &ValueSnapshot,
    width: u16,
    height: u16,
) -> (Buffer, ratatui::layout::Position) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    let th = default_theme();
    terminal
        .draw(|frame| {
            let ctx = FrameCtx {
                app,
                th: &th,
                values,
            };
            render_frame(frame, &ctx, frame.area(), state);
        })
        .expect("editor render");
    let cursor = terminal.get_cursor_position().expect("cursor position");
    (terminal.backend().buffer().clone(), cursor)
}

/// Text of one screen row.
fn row(buffer: &Buffer, y: u16) -> String {
    (buffer.area.x..buffer.area.right())
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

/// Text inside a hit rectangle (first row).
fn rect_text(buffer: &Buffer, rect: (u16, u16, u16, u16)) -> String {
    (rect.0..rect.0 + rect.2)
        .map(|x| buffer[(x, rect.1)].symbol())
        .collect()
}

/// Whole screen as newline-separated rows.
fn screen(buffer: &Buffer) -> String {
    (buffer.area.y..buffer.area.bottom())
        .map(|y| row(buffer, y))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `inner` lies inside `outer`.
const fn inside(inner: (u16, u16, u16, u16), outer: (u16, u16, u16, u16)) -> bool {
    inner.0 >= outer.0
        && inner.1 >= outer.1
        && inner.0 + inner.2 <= outer.0 + outer.2
        && inner.1 + inner.3 <= outer.1 + outer.3
}

/// Open a popup for `key` with deterministic suggestions.
fn open_popup(state: &mut ConfigEditorState, values: &ValueSnapshot, key: &str) {
    let setting = entry(key);
    let context = SuggestionContext {
        current: values.value_for(setting),
        default: values.default_for(setting),
        ..SuggestionContext::default()
    };
    let mut popup = EditPopupState::from_current(setting, &context.current);
    popup.set_suggestions(suggestions_for(setting, &context));
    state.popup = Some(popup);
}

/// The tab strip brackets only the active tab and exposes one rect per tab.
#[test]
fn tab_strip_marks_active_tab_and_records_rects() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    assert_eq!(state.tab_rects.len(), 4);
    let texts: Vec<_> = state
        .tab_rects
        .iter()
        .map(|(_, rect)| rect_text(&buffer, *rect))
        .collect();
    assert_eq!(texts[0].trim(), "[ settings.conf ]");
    assert_eq!(texts[1].trim(), "keybinds.conf");
    assert!(texts[3].contains("★ Starred"));

    state.set_tab(ConfigEditorTab::Theme);
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    let theme_rect = state
        .tab_rects
        .iter()
        .find(|(tab, _)| *tab == ConfigEditorTab::Theme)
        .expect("theme tab")
        .1;
    assert_eq!(rect_text(&buffer, theme_rect).trim(), "[ theme.conf ]");
    assert_eq!(
        rect_text(&buffer, state.tab_rects[0].1).trim(),
        "settings.conf"
    );
}

/// Group headings precede rows; values, modified, and restart markers render.
#[test]
fn grouped_list_shows_headings_values_and_markers() {
    let app = english_app();
    let mut state = clean_state(&app);
    let mut settings = Settings::default();
    settings.mirror_count = settings.mirror_count.saturating_add(7);
    let values = ValueSnapshot::from_parts(settings.clone(), default_theme());
    state.query = "mirror_count".to_string();
    state.clamp_key_cursor();
    let (buffer, _) = draw(&app, &mut state, &values, 160, 40);
    let text = screen(&buffer);
    assert!(text.contains(" Mirrors"), "{text}");
    let (index, rect) = state.row_rects[0];
    assert_eq!(index, 0);
    let row_text = rect_text(&buffer, rect);
    assert!(row_text.contains("Mirror count"), "{row_text}");
    assert!(row_text.contains(&settings.mirror_count.to_string()));
    assert!(
        row_text.contains('●'),
        "modified marker missing: {row_text}"
    );
    assert!(
        row_text.contains("mirror_count"),
        "wide lists show the raw key"
    );
    assert_eq!(
        rect_text(&buffer, (rect.0, rect.1 - 1, rect.2, 1)).trim(),
        "Mirrors",
        "group heading directly above its first row"
    );

    state.query = "app_dry_run_default".to_string();
    state.clamp_key_cursor();
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    let row_text = rect_text(&buffer, state.row_rects[0].1);
    assert!(row_text.contains('↻'), "restart marker missing: {row_text}");
    assert!(
        row_text.contains("off"),
        "bools render as on/off: {row_text}"
    );
    assert!(!row_text.contains('●'));
}

/// Every row rect addresses `filtered_keys()` and shows that key's label.
#[test]
fn row_rects_align_with_rendered_rows_and_page_rows() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    let (buffer, _) = draw(&app, &mut state, &values, 160, 30);
    let keys = state.filtered_keys();
    assert_ne!(state.row_rects.len(), 0);
    assert!(state.row_rects.len() < keys.len(), "list should be clipped");
    for (index, rect) in &state.row_rects {
        let label = super::format::setting_label(
            &FrameCtx {
                app: &app,
                th: &default_theme(),
                values: &values,
            },
            keys[*index],
        );
        let prefix: String = label.chars().take(10).collect();
        assert!(
            rect_text(&buffer, *rect).contains(&prefix),
            "row {index} should show {label}"
        );
    }
    assert_eq!(
        usize::from(state.list_page_rows),
        state.row_rects.len(),
        "page rows equal visible selectable rows"
    );
    let list = state.list_rect.expect("list rect");
    assert!(state.row_rects.iter().all(|(_, rect)| inside(*rect, list)));
}

/// Moving the cursor to the end keeps the selected row inside the viewport.
#[test]
fn selected_row_stays_visible_at_end_of_list() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    state.key_cursor = state.filtered_keys().len() - 1;
    let (buffer, _) = draw(&app, &mut state, &values, 120, 30);
    let (_, rect) = state
        .row_rects
        .iter()
        .find(|(index, _)| *index == state.key_cursor)
        .expect("selected row visible");
    assert!(rect_text(&buffer, *rect).starts_with("▶"));
}

/// The details pane lists label, key, values, type, apply behavior, file, and summary.
#[test]
fn details_pane_shows_all_fields() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.bookmarked_keys = vec!["mirror_count".to_string()];
    let mut settings = Settings::default();
    settings.mirror_count = settings.mirror_count.saturating_add(1);
    let values = ValueSnapshot::from_parts(settings, default_theme());
    state.query = "mirror_count".to_string();
    state.clamp_key_cursor();
    let (buffer, _) = draw(&app, &mut state, &values, 140, 40);
    let text = screen(&buffer);
    for needle in [
        " Details ",
        "★ starred",
        "Key",
        "Current",
        "Default",
        "● modified",
        "integer 1–200",
        "when saved",
        "File",
        "Number of mirrors",
    ] {
        assert!(text.contains(needle), "missing {needle}:\n{text}");
    }
}

/// Pi Scan keys advertise the configured guided-setup chord.
#[test]
fn pi_scan_details_show_configured_setup_chord() {
    let mut app = english_app();
    app.keymap.config_editor_pi_scan_setup = vec![crate::theme::KeyChord {
        code: crossterm::event::KeyCode::Char('p'),
        mods: crossterm::event::KeyModifiers::ALT,
    }];
    let mut state = clean_state(&app);
    state.query = "pi_scan_binary".to_string();
    state.clamp_key_cursor();
    let (buffer, _) = draw(&app, &mut state, &default_values(), 140, 40);
    let text = screen(&buffer);
    assert!(
        text.contains("[Alt+P] opens the guided Pi Scan setup"),
        "{text}"
    );
}

/// Keybinds with several chords list the extra ones in the details pane.
#[test]
fn details_list_extra_chords_for_multi_bound_keys() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.set_tab(ConfigEditorTab::Keybinds);
    state.query = "keybind_recent_remove".to_string();
    state.clamp_key_cursor();
    let (buffer, _) = draw(&app, &mut state, &default_values(), 140, 40);
    let text = screen(&buffer);
    assert!(text.contains("Also bound"), "{text}");
    assert!(text.contains("conflict scope: Search history"), "{text}");
    assert!(text.contains("Del"), "extra chord listed: {text}");
}

/// Search line: query with a real cursor, match count, and recent hint.
#[test]
fn search_line_places_cursor_after_query_and_counts_matches() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.query = "mirror".to_string();
    state.clamp_key_cursor();
    let (buffer, cursor) = draw(&app, &mut state, &default_values(), 120, 40);
    let search_row = row(&buffer, 2);
    assert!(search_row.contains("Search: mirror"), "{search_row}");
    assert!(search_row.contains(&format!("{} of ", state.filtered_keys().len())));
    assert!(search_row.contains("Ctrl+R recent"));
    let column = u16::try_from(search_row.find("mirror").expect("query")).expect("column");
    assert_eq!(cursor.y, 2);
    assert_eq!(
        cursor.x,
        column + 6 - 2,
        "cursor after the query (box chars are 3 bytes)"
    );
}

/// No matches and an empty Starred tab explain what to do next.
#[test]
fn empty_states_explain_next_steps() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.query = "zzzz_no_such_key".to_string();
    state.clamp_key_cursor();
    let (buffer, _) = draw(&app, &mut state, &default_values(), 120, 40);
    assert!(screen(&buffer).contains("No keys match \"zzzz_no_such_key\""));
    assert_eq!(state.row_rects.len(), 0);

    state.query.clear();
    state.set_tab(ConfigEditorTab::Starred);
    let (buffer, _) = draw(&app, &mut state, &default_values(), 120, 40);
    assert!(screen(&buffer).contains("No starred keys yet"));
}

/// Narrow terminals stack the details pane under the key list.
#[test]
fn narrow_layout_stacks_details_below_list() {
    let app = english_app();
    let mut state = clean_state(&app);
    let (buffer, _) = draw(&app, &mut state, &default_values(), 80, 40);
    let list = state.list_rect.expect("list rect");
    assert!(list.2 >= 74, "list spans the full width: {list:?}");
    let details_row = (0..40)
        .find(|y| row(&buffer, *y).contains(" Details "))
        .expect("details pane title");
    assert!(details_row >= list.1 + list.3, "details below list");
    assert!(
        !rect_text(&buffer, state.row_rects[0].1).contains("background_base"),
        "raw key column hidden on narrow lists"
    );
}

/// The footer starts exactly at `keybind_footer_top` and the status row is reserved.
#[test]
fn footer_reserved_rows_match_rendered_footer() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.status = Some("Saved mirror_count".to_string());
    let (buffer, _) = draw(&app, &mut state, &default_values(), 120, 40);
    let area = Rect::new(0, 0, 120, 40);
    let top = keybind_footer_top(&app, area, &state).expect("footer visible");
    assert!(
        row(&buffer, top).contains("GLOBALS"),
        "{}",
        row(&buffer, top)
    );
    assert!(
        row(&buffer, top - 1).contains('╰'),
        "body ends above the footer"
    );
    assert!(row(&buffer, 38).contains("Saved mirror_count"));
    let reserved = crate::ui::details::config_editor_footer_reserved_rows(
        true,
        &app,
        &state,
        state.selected_key(),
        118,
    );
    assert_eq!(top + reserved, 39, "footer fills the rows above the border");
}

/// Popup: title, meta, dropdown with tags, validation, and "Enter saves" line.
#[test]
fn popup_renders_dropdown_tags_validation_and_saves_line() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    open_popup(&mut state, &values, "mirror_count");
    let (buffer, cursor) = draw(&app, &mut state, &values, 120, 40);
    let text = screen(&buffer);
    let default = values.default_for(entry("mirror_count"));
    for needle in [
        "Edit · Mirror count",
        "mirror_count · integer 1–200 · on save",
        "[Current]",
        "[Default]",
        "[Min]",
        "✓ Valid",
        &format!("Enter saves: {default}"),
        "+/- adjust",
    ] {
        assert!(text.contains(needle), "missing {needle}:\n{text}");
    }
    let popup_rect = state.popup_rect.expect("popup rect");
    let popup = state.popup.as_ref().expect("popup");
    assert_ne!(state.popup_choice_rects.len(), 0);
    for (row_index, rect) in state.popup_choice_rects.iter().enumerate() {
        assert!(inside(*rect, popup_rect));
        let suggestion = &popup.suggestions[popup.visible[popup.scroll + row_index]];
        assert!(
            rect_text(&buffer, *rect).contains(&suggestion.value),
            "row {row_index} should show {}",
            suggestion.value
        );
    }
    assert!(
        inside((cursor.x, cursor.y, 1, 1), popup_rect),
        "cursor in input"
    );

    if let Some(popup) = state.popup.as_mut() {
        popup.buffer = "999".to_string();
        popup.caret = 3;
        popup.refresh_validation();
    }
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    assert!(screen(&buffer).contains("✗"), "invalid value is flagged");
}

/// Keybind popups show a Record button inside the popup and a recording state.
#[test]
fn keybind_popup_record_button_and_recording_state() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    open_popup(&mut state, &values, "keybind_help");
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    let button = state.record_button_rect.expect("record button");
    assert!(inside(button, state.popup_rect.expect("popup rect")));
    assert!(rect_text(&buffer, button).contains("● Record (Ctrl+K)"));

    if let Some(popup) = state.popup.as_mut() {
        popup.kind = EditPopupKind::KeyChord { capturing: true };
    }
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    let text = screen(&buffer);
    assert!(text.contains("Press the new key combination · Esc stops recording"));
    assert!(rect_text(&buffer, state.record_button_rect.expect("button")).contains("Recording"));
    assert!(text.contains("[Esc] Stop recording"), "{text}");
    assert!(!text.contains("[Enter/Ctrl+S]"));
}

/// Bool popups are a two-row on/off picker without an input box.
#[test]
fn bool_popup_is_small_on_off_picker() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    open_popup(&mut state, &values, "app_dry_run_default");
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    let rows: Vec<_> = state
        .popup_choice_rects
        .iter()
        .map(|rect| rect_text(&buffer, *rect))
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.contains("on")));
    assert!(
        rows.iter()
            .any(|row| row.contains("off") && row.starts_with("▶"))
    );
    assert!(state.popup_rect.expect("popup").2 <= 48);
}

/// Strict pickers show a filter placeholder, translated descriptions, and the empty label.
#[test]
fn enum_popup_shows_placeholder_descriptions_and_empty_label() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    open_popup(&mut state, &values, "locale");
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    let text = screen(&buffer);
    assert!(text.contains("type to filter"), "{text}");
    assert!(text.contains("Auto-detect"), "{text}");
    assert!(text.contains("— German (Germany)"), "{text}");
}

/// Narrow popups keep each clickable choice on its own row.
#[test]
fn narrow_enum_popup_choice_rows_match_hit_rects() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    open_popup(&mut state, &values, "auth_mode");
    let (buffer, _) = draw(&app, &mut state, &values, 30, 30);
    let popup = state.popup.as_ref().expect("popup");
    assert_eq!(state.popup_choice_rects.len(), popup.visible.len());
    for (row_index, rect) in state.popup_choice_rects.iter().enumerate() {
        let value = &popup.suggestions[popup.visible[popup.scroll + row_index]].value;
        let prefix: String = value.chars().take(8).collect();
        assert!(rect_text(&buffer, *rect).contains(&prefix), "{value}");
    }
}

/// Secrets stay masked in the input and in the "Enter saves" line.
#[test]
fn secret_popup_masks_value_until_revealed() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    let mut popup = EditPopupState::from_current(entry("virustotal_api_key"), "hunter22");
    popup.refresh_validation();
    state.popup = Some(popup);
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    let text = screen(&buffer);
    assert!(!text.contains("hunter22"), "{text}");
    assert!(text.contains("••••••••"));
    if let Some(popup) = state.popup.as_mut() {
        popup.kind = EditPopupKind::Secret { revealed: true };
    }
    let (buffer, _) = draw(&app, &mut state, &values, 120, 40);
    assert!(screen(&buffer).contains("hunter22"));
}

/// The renderer clamps `scroll` so the highlight stays in the real dropdown window.
#[test]
fn popup_scroll_follows_highlight_and_shows_more_indicators() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    let mut popup = EditPopupState::from_current(entry("preferred_terminal"), "");
    popup.suggestions = (0..30)
        .map(|n| Suggestion {
            value: format!("term{n:02}"),
            tags: if n == 25 {
                vec![SuggestionTag::Installed]
            } else {
                Vec::new()
            },
            ..Suggestion::default()
        })
        .collect();
    popup.visible = (0..30).collect();
    popup.highlight = Some(25);
    popup.scroll = 0;
    state.popup = Some(popup);
    let (buffer, _) = draw(&app, &mut state, &values, 100, 30);
    let popup = state.popup.as_ref().expect("popup");
    let rows = state.popup_choice_rects.len();
    assert!(rows > 0 && rows < 30);
    assert!(
        popup.scroll <= 25 && 25 < popup.scroll + rows,
        "highlight visible"
    );
    let highlighted = state.popup_choice_rects[25 - popup.scroll];
    let text = rect_text(&buffer, highlighted);
    assert!(
        text.contains("term25") && text.contains("[Installed]"),
        "{text}"
    );
    let screen_text = screen(&buffer);
    assert!(screen_text.contains("more"), "{screen_text}");
}

/// The recent-search dropdown lists queries with rects, or explains the empty state.
#[test]
fn recent_dropdown_lists_queries_and_empty_state() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.recent_queries = vec!["mirror".to_string(), "theme".to_string()];
    state.recent_dropdown = Some(1);
    let (buffer, _) = draw(&app, &mut state, &default_values(), 120, 40);
    assert_eq!(state.recent_rects.len(), 2);
    assert!(rect_text(&buffer, state.recent_rects[0].1).contains("mirror"));
    assert!(rect_text(&buffer, state.recent_rects[1].1).starts_with("▶ theme"));
    assert!(screen(&buffer).contains("Recent searches"));

    state.recent_queries.clear();
    state.recent_dropdown = Some(0);
    let (buffer, _) = draw(&app, &mut state, &default_values(), 120, 40);
    assert!(screen(&buffer).contains("No recent searches yet"));
    assert_eq!(state.recent_rects.len(), 0);
}

/// Stale rects from a previous frame are cleared when their widgets disappear.
#[test]
fn hit_rects_are_cleared_each_frame() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    open_popup(&mut state, &values, "keybind_help");
    draw(&app, &mut state, &values, 120, 40);
    assert!(state.record_button_rect.is_some());
    state.popup = None;
    draw(&app, &mut state, &values, 120, 40);
    assert!(state.record_button_rect.is_none());
    assert!(state.popup_rect.is_none());
    assert_eq!(state.popup_choice_rects.len(), 0);
    assert_ne!(state.row_rects.len(), 0);
}

/// Translation keys the renderer and footer resolve under `app.modals.config_editor.`.
const RENDERER_KEYS: &[&str] = &[
    "title",
    "tabs.settings",
    "tabs.keybinds",
    "tabs.theme",
    "tabs.starred",
    "search.label",
    "search.placeholder",
    "search.count",
    "search.recent_hint",
    "recent.title",
    "recent.empty",
    "list.title",
    "list.no_matches",
    "list.starred_empty",
    "details.title",
    "details.no_key",
    "details.key",
    "details.current",
    "details.default",
    "details.modified",
    "details.type",
    "details.applies",
    "details.file",
    "details.extra_chords",
    "details.extra_chords_note",
    "details.pi_scan_setup",
    "details.pi_scan_setup_hint",
    "details.starred",
    "types.bool",
    "types.enum",
    "types.int",
    "types.text",
    "types.list",
    "types.path",
    "types.secret",
    "types.days",
    "types.color",
    "types.pane_order",
    "types.chord",
    "applies.auto_reload",
    "applies.on_save",
    "applies.restart",
    "values.on",
    "values.off",
    "values.empty",
    "values.unbound",
    "values.secret_unset",
    "tags.current",
    "tags.default",
    "tags.auto_detect",
    "tags.unlimited",
    "tags.installed",
    "tags.detected",
    "tags.minimum",
    "tags.maximum",
    "duration.seconds",
    "duration.minutes",
    "duration.hours",
    "duration.day",
    "duration.days",
    "popup.type_chord",
    "popup.applies_auto_reload",
    "popup.applies_on_save",
    "popup.applies_restart",
    "popup.title",
    "popup.placeholder_filter",
    "popup.placeholder_text",
    "popup.record",
    "popup.record_alt",
    "popup.recording",
    "popup.recording_prompt",
    "popup.valid",
    "popup.invalid",
    "popup.saves",
    "popup.more_above",
    "popup.more_below",
    "popup.no_matches",
    "popup.hint_bool",
    "popup.hint_enum",
    "popup.hint_int",
    "popup.hint_text",
    "popup.hint_list",
    "popup.hint_secret",
    "popup.hint_chord",
    "popup.hint_recording",
    "popup_empty_choice",
    "footer.heading_menus",
    "footer.heading_editor",
    "footer.heading_popup",
    "footer.heading_recording",
    "footer.heading_recent",
    "footer.menu_config",
    "footer.menu_options",
    "footer.menu_panels",
    "footer.move_key",
    "footer.edit_key",
    "footer.switch_tab",
    "footer.star",
    "footer.recent",
    "footer.export",
    "footer.esc_leave",
    "footer.esc_clear",
    "footer.pi_scan_setup",
    "footer.recent_choose",
    "footer.recent_apply",
    "footer.recent_close",
    "footer.popup_save",
    "footer.popup_cancel",
    "footer.popup_complete",
    "footer.popup_next_choice",
    "footer.popup_pick",
    "footer.popup_reset",
    "footer.popup_reveal",
    "footer.popup_record",
    "footer.popup_adjust",
    "footer.popup_toggle",
    "footer.popup_capture_cancel",
];

/// Enum settings whose choices carry translated descriptions.
const DESCRIBED_ENUMS: &[&str] = &[
    "sort_mode",
    "privilege_mode",
    "auth_mode",
    "installed_packages_mode",
    "search_startup_mode",
    "package_marker",
    "locale",
    "pi_scan_thinking",
    "main_pane_order",
];

/// Every key used by the renderer, plus generated row/group/choice keys, exists in all locales.
#[test]
fn renderer_locale_keys_exist_in_all_locales() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
    let mut required: Vec<String> = RENDERER_KEYS
        .iter()
        .map(|key| format!("{}{key}", super::format::KEY_PREFIX))
        .collect();
    required.push("app.modals.help.config_editor_lines".to_string());
    for file in [
        crate::theme::ConfigFile::Settings,
        crate::theme::ConfigFile::Keybinds,
        crate::theme::ConfigFile::Theme,
    ] {
        for setting in crate::theme::settings_for(file) {
            required.push(setting.label_i18n_key());
            if file != crate::theme::ConfigFile::Keybinds {
                required.push(setting.summary_i18n_key());
            }
        }
    }
    for file in [
        crate::theme::ConfigFile::Settings,
        crate::theme::ConfigFile::Keybinds,
        crate::theme::ConfigFile::Theme,
    ] {
        for setting in crate::theme::settings_for(file) {
            required.push(
                crate::state::config_editor::setting_group(setting)
                    .i18n_key()
                    .to_string(),
            );
        }
    }
    for key in DESCRIBED_ENUMS {
        let context = SuggestionContext::default();
        for suggestion in suggestions_for(entry(key), &context) {
            required.push(suggestion.label_key.expect("choice description key"));
        }
    }
    for locale in ["en-US", "de-DE", "hu-HU"] {
        let translations = crate::i18n::load_locale_file(locale, &dir)
            .unwrap_or_else(|error| panic!("{locale} locale failed: {error}"));
        let missing: Vec<_> = required
            .iter()
            .filter(|key| !translations.contains_key(key.as_str()))
            .collect();
        assert!(missing.is_empty(), "{locale} is missing {missing:?}");
    }
}

/// Every narrow strip keeps its active tab visible with rectangles matching drawn tabs.
#[test]
fn regression_narrow_tab_strip_keeps_active_visible() {
    let app = english_app();
    for width in [40, 60] {
        for active in ConfigEditorTab::ALL {
            let mut state = clean_state(&app);
            state.set_tab(active);
            let (buffer, _) = draw(&app, &mut state, &default_values(), width, 40);
            let (_, rect) = state
                .tab_rects
                .iter()
                .find(|(tab, _)| *tab == active)
                .unwrap_or_else(|| panic!("active {active:?} missing at width {width}"));
            let text = rect_text(&buffer, *rect);
            assert!(text.starts_with('[') && text.ends_with(']'), "{text}");
            for (_, rect) in &state.tab_rects {
                assert_ne!(rect_text(&buffer, *rect).trim().len(), 0);
                assert!(rect.0 + rect.2 <= width);
            }
        }
    }
}

/// Dropdown page size matches keyboard scrolling on tall terminals.
#[test]
fn regression_dropdown_uses_eight_row_viewport() {
    let app = english_app();
    let mut state = clean_state(&app);
    open_popup(&mut state, &default_values(), "updates_refresh_interval");
    state.popup.as_mut().expect("popup").set_suggestions(
        (0..9)
            .map(|n| Suggestion {
                value: n.to_string(),
                ..Suggestion::default()
            })
            .collect(),
    );
    draw(&app, &mut state, &default_values(), 120, 50);
    assert_eq!(state.popup_choice_rects.len(), 8);
}

/// Empty terminal selection displays the automatic-discovery meaning in list and details.
#[test]
fn regression_empty_terminal_displays_auto_detect() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.query = "preferred_terminal".into();
    let (buffer, _) = draw(&app, &mut state, &default_values(), 130, 40);
    let text = screen(&buffer);
    assert!(text.matches("Auto-detect").count() >= 3, "{text}");
    assert!(!text.contains("(empty)"));
}

/// Wrapped current values retain the value-column indent on continuation lines.
#[test]
fn regression_details_values_have_hanging_indent() {
    let app = english_app();
    let state = clean_state(&app);
    let settings = Settings {
        preferred_terminal: "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu"
            .into(),
        ..Settings::default()
    };
    let values = ValueSnapshot::from_parts(settings, default_theme());
    let th = default_theme();
    let mut terminal = Terminal::new(TestBackend::new(46, 24)).expect("terminal");
    terminal
        .draw(|frame| {
            let ctx = FrameCtx {
                app: &app,
                th: &th,
                values: &values,
            };
            super::details::render_details(
                frame,
                &ctx,
                Some(frame.area()),
                &state,
                Some(entry("preferred_terminal")),
            );
        })
        .expect("details");
    let buffer = terminal.backend().buffer();
    let y = (0..24)
        .find(|&y| row(buffer, y).contains("Current"))
        .expect("current row");
    let x = (0..46)
        .find(|&x| buffer[(x, y)].symbol() == "a")
        .expect("first value");
    let continuation = row(buffer, y + 1);
    assert!(
        (1..x).all(|x| buffer[(x, y + 1)].symbol() == " "),
        "{continuation}"
    );
    assert!(
        buffer[(x, y + 1)]
            .symbol()
            .chars()
            .next()
            .is_some_and(char::is_alphabetic),
        "{continuation}"
    );
}

/// Apply wording reads naturally in details and keybind metadata fits a 76-column popup.
#[test]
fn regression_apply_wording_and_compact_popup_header() {
    let app = english_app();
    let mut state = clean_state(&app);
    state.query = "keybind_help".into();
    state.set_tab(ConfigEditorTab::Keybinds);
    let values = default_values();
    let (buffer, _) = draw(&app, &mut state, &values, 130, 40);
    assert!(!screen(&buffer).contains("applies on save"));
    assert!(screen(&buffer).contains("when saved"));
}

/// Keybind popup metadata stays on one line while details retain full scope information.
#[test]
fn regression_compact_keybind_popup_header() {
    let app = english_app();
    let mut state = clean_state(&app);
    let values = default_values();
    open_popup(&mut state, &values, "keybind_help");
    let (buffer, _) = draw(&app, &mut state, &values, 80, 40);
    let text = screen(&buffer);
    assert!(
        (0..40).any(|y| {
            let line = row(&buffer, y);
            line.contains("keybind_help ·") && line.contains("on save")
        }),
        "{text}"
    );
    assert_eq!(state.popup_rect.expect("popup").2, 76);
}
