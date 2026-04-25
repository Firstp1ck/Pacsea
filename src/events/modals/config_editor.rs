//! Key handling for the integrated TUI config editor (Phase 1+).
//!
//! What: Routes key events while the integrated config editor mode is active. The
//! editor has three input contexts:
//!
//! 1. List navigation (file list or key list).
//! 2. Search input (typing fuzzy-search query).
//! 3. Edit popup (toggle/text/secret/etc.).
//!
//! When a popup is active it consumes all key events first; otherwise the
//! handler routes by `state.focus` (List vs Search). `Esc` always closes
//! the popup first, then drops to the file list.
//!
//! Save flow: pressing `Ctrl+S` inside the popup builds a
//! [`PatchRequest`](crate::theme::PatchRequest), calls
//! [`patch_key`](crate::theme::patch_key), and on success triggers a
//! settings reload via [`reload_after_save`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

use crate::state::{
    AppState, ConfigEditorFocus, ConfigEditorSearchFocus, ConfigEditorState, ConfigEditorView,
    EditPopupKind, EditPopupState, Modal,
};
use crate::theme::{ConfigFile, PatchOutcome, PatchRequest, patch_key};

/// What: Handle a key event while `AppMode::ConfigEditor` is active.
///
/// Inputs:
/// - `ke`: Crossterm key event.
/// - `app`: Mutable application state holding `config_editor_state`.
///
/// Output:
/// - Returns `true` once the event has been consumed.
///
/// Details:
/// - Moves editor state out of `app` temporarily to avoid mutable borrow
///   conflicts when save handlers mutate global app fields.
pub(in crate::events) fn handle_config_editor_mode_key(ke: KeyEvent, app: &mut AppState) -> bool {
    let mut state = std::mem::take(&mut app.config_editor_state);
    if state.popup.is_some() {
        handle_popup_key(ke, app, &mut state);
    } else {
        handle_editor_key(ke, &mut state, app);
    }
    app.config_editor_state = state;
    true
}

/// What: Legacy modal-path handler retained for compatibility while the
/// modal variant still exists.
pub(super) fn handle_config_editor_modal(ke: KeyEvent, app: &mut AppState, modal: Modal) -> bool {
    let Modal::ConfigEditor { mut state } = modal else {
        app.modal = modal;
        return false;
    };
    let close = if state.popup.is_some() {
        handle_popup_key(ke, app, &mut state)
    } else {
        handle_editor_key(ke, &mut state, app)
    };
    if !close {
        app.modal = Modal::ConfigEditor { state };
    }
    true
}

/// Editor-mode key handling (no popup active). Returns `true` if the
/// modal should be closed.
fn handle_editor_key(ke: KeyEvent, state: &mut ConfigEditorState, app: &AppState) -> bool {
    // Keep config editor search-first like package mode:
    // typed input always targets the middle pane.
    state.focus = ConfigEditorFocus::Search;
    handle_search_key(ke, state, app);
    false
}

/// Key handling for the middle-pane search input.
fn handle_search_key(ke: KeyEvent, state: &mut ConfigEditorState, app: &AppState) {
    normalize_search_focus(state, app);
    match (ke.code, ke.modifiers) {
        (KeyCode::Esc, _) => {
            if !matches!(state.search_focus, ConfigEditorSearchFocus::Input) {
                state.search_focus = ConfigEditorSearchFocus::Input;
                return;
            }
            // Keep middle input focused. Esc can step the top pane back from
            // key list to file list, but never drops input focus.
            if matches!(state.view, ConfigEditorView::KeyList) && state.query.trim().is_empty() {
                back_to_file_list(state);
            }
        }
        (KeyCode::Tab | KeyCode::Right, _) => cycle_search_focus(state, app, true),
        (KeyCode::BackTab | KeyCode::Left, _) => cycle_search_focus(state, app, false),
        (KeyCode::Up, _) => {
            if matches!(state.search_focus, ConfigEditorSearchFocus::Input) {
                move_list(state, -1);
            } else {
                move_search_list_cursor(state, -1);
            }
        }
        (KeyCode::Down, _) => {
            if matches!(state.search_focus, ConfigEditorSearchFocus::Input) {
                move_list(state, 1);
            } else {
                move_search_list_cursor(state, 1);
            }
        }
        (KeyCode::Enter, _) => match state.search_focus {
            ConfigEditorSearchFocus::Input => {
                if state.query.trim().is_empty() {
                    // Match package-mode ergonomics: Enter from the center
                    // input still acts on the current top-list selection.
                    activate_list_row(state);
                } else {
                    let current_query = state.query.clone();
                    state.push_recent_query(&current_query);
                    state.view = ConfigEditorView::KeyList;
                    state.key_cursor = 0;
                }
                state.search_focus = ConfigEditorSearchFocus::Input;
            }
            ConfigEditorSearchFocus::Recent => {
                if let Some(q) = state.selected_recent_query().map(str::to_string) {
                    state.query.clone_from(&q);
                    state.query_caret = state.query.len();
                    state.push_recent_query(&q);
                    state.view = ConfigEditorView::KeyList;
                    state.clamp_key_cursor();
                    state.search_focus = ConfigEditorSearchFocus::Input;
                }
            }
            ConfigEditorSearchFocus::Bookmarks => {
                if let Some(key) = state.selected_bookmarked_key().map(str::to_string) {
                    open_bookmarked_key_popup(state, &key);
                    state.search_focus = ConfigEditorSearchFocus::Input;
                }
            }
        },
        (KeyCode::Backspace, _) => {
            if matches!(state.search_focus, ConfigEditorSearchFocus::Input) {
                state.query.pop();
                state.query_caret = state.query.len();
                state.view = ConfigEditorView::KeyList;
                state.clamp_key_cursor();
                mark_query_input_changed(state);
            }
        }
        (KeyCode::Char('b'), KeyModifiers::NONE)
            if matches!(state.view, ConfigEditorView::KeyList)
                && matches!(state.search_focus, ConfigEditorSearchFocus::Input) =>
        {
            toggle_selected_bookmark(state);
        }
        (KeyCode::Char('b'), KeyModifiers::NONE)
            if matches!(state.search_focus, ConfigEditorSearchFocus::Bookmarks) =>
        {
            if let Some(key) = state.selected_bookmarked_key().map(str::to_string) {
                let removed = !state.toggle_bookmark_key(&key);
                if removed {
                    state.status = Some(format!("Removed bookmark {key}"));
                }
            }
        }
        (KeyCode::Char(c), m)
            if matches!(state.search_focus, ConfigEditorSearchFocus::Input)
                && !m.contains(KeyModifiers::CONTROL)
                && !m.contains(KeyModifiers::ALT) =>
        {
            state.query.push(c);
            state.query_caret = state.query.len();
            state.view = ConfigEditorView::KeyList;
            state.clamp_key_cursor();
            mark_query_input_changed(state);
        }
        _ => {}
    }
}

/// Record that user typed/edited the config-editor query so debounce can persist recents.
fn mark_query_input_changed(state: &mut ConfigEditorState) {
    state.query_last_input_change = Instant::now();
    state.last_saved_query_value = None;
}

/// Toggle bookmark state for the currently selected key.
fn toggle_selected_bookmark(state: &mut ConfigEditorState) {
    let Some(entry) = state.selected_key() else {
        return;
    };
    let added = state.toggle_bookmark_key(entry.key);
    state.status = Some(if added {
        format!("Bookmarked {}", entry.key)
    } else {
        format!("Removed bookmark {}", entry.key)
    });
}

/// Cycle search sub-focus between input, recents, bookmarks.
fn cycle_search_focus(state: &mut ConfigEditorState, app: &AppState, forward: bool) {
    let mut order: Vec<ConfigEditorSearchFocus> = Vec::with_capacity(3);
    if app.show_recent_pane {
        order.push(ConfigEditorSearchFocus::Recent);
    }
    order.push(ConfigEditorSearchFocus::Input);
    if app.show_install_pane {
        order.push(ConfigEditorSearchFocus::Bookmarks);
    }
    if order.is_empty() {
        state.search_focus = ConfigEditorSearchFocus::Input;
        return;
    }
    let current_idx = order
        .iter()
        .position(|f| *f == state.search_focus)
        .unwrap_or(0);
    let next_idx = if forward {
        (current_idx + 1) % order.len()
    } else if current_idx == 0 {
        order.len() - 1
    } else {
        current_idx - 1
    };
    state.search_focus = order[next_idx];
}

/// Ensure search-subpane focus always points to a currently visible panel.
const fn normalize_search_focus(state: &mut ConfigEditorState, app: &AppState) {
    match state.search_focus {
        ConfigEditorSearchFocus::Recent if !app.show_recent_pane => {
            state.search_focus = ConfigEditorSearchFocus::Input;
        }
        ConfigEditorSearchFocus::Bookmarks if !app.show_install_pane => {
            state.search_focus = ConfigEditorSearchFocus::Input;
        }
        _ => {}
    }
}

/// Move cursor inside recent/bookmark lists while search pane is focused.
fn move_search_list_cursor(state: &mut ConfigEditorState, delta: i32) {
    match state.search_focus {
        ConfigEditorSearchFocus::Input => {}
        ConfigEditorSearchFocus::Recent => {
            if state.recent_queries.is_empty() {
                state.recent_cursor = 0;
                return;
            }
            let cur = i32::try_from(state.recent_cursor).unwrap_or(0);
            let max = i32::try_from(state.recent_queries.len().saturating_sub(1)).unwrap_or(0);
            let next = (cur + delta).clamp(0, max);
            state.recent_cursor = usize::try_from(next).unwrap_or(0);
        }
        ConfigEditorSearchFocus::Bookmarks => {
            if state.bookmarked_keys.is_empty() {
                state.bookmark_cursor = 0;
                return;
            }
            let cur = i32::try_from(state.bookmark_cursor).unwrap_or(0);
            let max = i32::try_from(state.bookmarked_keys.len().saturating_sub(1)).unwrap_or(0);
            let next = (cur + delta).clamp(0, max);
            state.bookmark_cursor = usize::try_from(next).unwrap_or(0);
        }
    }
}

/// What to do when the user presses Esc in the list view: drop one
/// "level" (`KeyList` → `FileList`) but keep the editor open.
fn back_to_file_list(state: &mut ConfigEditorState) {
    match state.view {
        ConfigEditorView::KeyList => {
            state.view = ConfigEditorView::FileList;
            state.query.clear();
            state.query_caret = 0;
            state.key_cursor = 0;
        }
        ConfigEditorView::FileList => {}
    }
}

/// Select a specific key in the currently filtered key list.
fn select_key_by_name(state: &mut ConfigEditorState, key: &str) {
    let keys = state.filtered_keys();
    if let Some(idx) = keys.iter().position(|entry| entry.key == key) {
        state.key_cursor = idx;
    } else {
        state.key_cursor = 0;
    }
    state.clamp_key_cursor();
}

/// Jump to a bookmarked key and open its edit popup directly.
fn open_bookmarked_key_popup(state: &mut ConfigEditorState, key: &str) {
    state.query = key.to_string();
    state.query_caret = state.query.len();
    state.view = ConfigEditorView::KeyList;
    select_key_by_name(state, key);
    open_popup_for_selection(state);
}

/// Move the active list cursor by `delta` rows (negative = up).
fn move_list(state: &mut ConfigEditorState, delta: i32) {
    match state.view {
        ConfigEditorView::FileList => {
            let cur = i32::try_from(state.file_cursor).unwrap_or(0);
            let next = (cur + delta).clamp(0, 3);
            state.file_cursor = usize::try_from(next).unwrap_or(0);
        }
        ConfigEditorView::KeyList => {
            let len = state.filtered_keys().len();
            if len == 0 {
                state.key_cursor = 0;
                return;
            }
            let cur = i32::try_from(state.key_cursor).unwrap_or(0);
            let max = i32::try_from(len.saturating_sub(1)).unwrap_or(0);
            let next = (cur + delta).clamp(0, max);
            state.key_cursor = usize::try_from(next).unwrap_or(0);
        }
    }
}

/// Activate the highlighted row (Enter on file list opens that file's
/// keys; Enter on key list opens the edit popup).
fn activate_list_row(state: &mut ConfigEditorState) {
    match state.view {
        ConfigEditorView::FileList => activate_file_row(state),
        ConfigEditorView::KeyList => open_popup_for_selection(state),
    }
}

/// Open the key list for the currently highlighted file row, or report a
/// "coming soon" hint for non-Settings files.
fn activate_file_row(state: &mut ConfigEditorState) {
    let file = match state.file_cursor {
        0 => ConfigFile::Settings,
        1 => ConfigFile::Keybinds,
        2 => ConfigFile::Theme,
        3 => ConfigFile::Repos,
        _ => return,
    };
    if !matches!(file, ConfigFile::Settings) {
        state.status = Some(format!(
            "{} editing lands in a later phase. Edit the file directly for now.",
            file_label_for_status(file)
        ));
        return;
    }
    state.selected_file = file;
    state.view = ConfigEditorView::KeyList;
    state.key_cursor = 0;
    state.clamp_key_cursor();
}

/// Open the edit popup for the currently highlighted key row.
fn open_popup_for_selection(state: &mut ConfigEditorState) {
    if let Some(entry) = state.selected_key() {
        let current = crate::state::config_editor::current_value_string(entry);
        state.popup = Some(EditPopupState::from_current(entry, &current));
        state.status = None;
    }
}

/// Short human label used in status messages for non-Settings files.
const fn file_label_for_status(file: ConfigFile) -> &'static str {
    match file {
        ConfigFile::Settings => "settings.conf",
        ConfigFile::Keybinds => "keybinds.conf",
        ConfigFile::Theme => "theme.conf",
        ConfigFile::Repos => "repos.conf",
    }
}

/// Key handling while the edit popup is active. Returns `true` if the
/// editor should close (currently always `false`).
fn handle_popup_key(ke: KeyEvent, app: &mut AppState, state: &mut ConfigEditorState) -> bool {
    if state.popup.is_none() {
        return false;
    }
    match (ke.code, ke.modifiers) {
        (KeyCode::Esc, _) => {
            state.popup = None;
        }
        (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
            try_save_popup(app, state);
        }
        (KeyCode::Char('r'), m) if m.contains(KeyModifiers::CONTROL) => {
            if let Some(popup) = state.popup.as_mut()
                && let EditPopupKind::Secret { revealed } = &mut popup.kind
            {
                *revealed = !*revealed;
            }
        }
        _ => {
            if let Some(popup) = state.popup.as_mut() {
                handle_popup_value_key(ke, popup);
            }
        }
    }
    false
}

/// Type-specific popup key handling for value mutation.
fn handle_popup_value_key(ke: KeyEvent, popup: &mut EditPopupState) {
    match popup.kind {
        EditPopupKind::Bool(_) => handle_popup_bool(ke, popup),
        EditPopupKind::Enum { .. } => handle_popup_enum(ke, popup),
        EditPopupKind::Int { min, max } => handle_popup_int(ke, popup, min, max),
        EditPopupKind::Text | EditPopupKind::Secret { .. } => handle_popup_text(ke, popup),
    }
}

/// Bool popup: Space / arrows toggle.
fn handle_popup_bool(ke: KeyEvent, popup: &mut EditPopupState) {
    let EditPopupKind::Bool(ref mut b) = popup.kind else {
        return;
    };
    match ke.code {
        KeyCode::Char(' ')
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Up
        | KeyCode::Down
        | KeyCode::Tab => {
            *b = !*b;
            popup.buffer = crate::state::config_editor::bool_to_canonical(*b).to_string();
        }
        _ => {}
    }
}

/// Enum popup: arrows cycle through choices.
fn handle_popup_enum(ke: KeyEvent, popup: &mut EditPopupState) {
    let EditPopupKind::Enum {
        ref choices,
        ref mut index,
    } = popup.kind
    else {
        return;
    };
    if choices.is_empty() {
        return;
    }
    let len = choices.len();
    match ke.code {
        KeyCode::Up | KeyCode::Left => {
            *index = (*index + len - 1) % len;
        }
        KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
            *index = (*index + 1) % len;
        }
        _ => return,
    }
    popup.buffer = choices.get(*index).cloned().unwrap_or_default();
}

/// Int popup: digits/Backspace edit, arrows ±1.
fn handle_popup_int(ke: KeyEvent, popup: &mut EditPopupState, min: i64, max: i64) {
    match ke.code {
        KeyCode::Up => bump_int(popup, 1, min, max),
        KeyCode::Down => bump_int(popup, -1, min, max),
        KeyCode::Backspace => {
            popup.buffer.pop();
            popup.caret = popup.buffer.len();
        }
        KeyCode::Char(c) if c.is_ascii_digit() || c == '-' => {
            popup.buffer.push(c);
            popup.caret = popup.buffer.len();
        }
        _ => {}
    }
}

/// Mutate the int buffer by `delta`, clamped into `[min, max]`.
fn bump_int(popup: &mut EditPopupState, delta: i64, min: i64, max: i64) {
    let cur = popup.buffer.trim().parse::<i64>().unwrap_or(min);
    let next = cur.saturating_add(delta).clamp(min, max);
    popup.buffer = next.to_string();
    popup.caret = popup.buffer.len();
}

/// Text/Secret popup: typed characters and backspace edit `buffer`.
fn handle_popup_text(ke: KeyEvent, popup: &mut EditPopupState) {
    match ke.code {
        KeyCode::Backspace => {
            popup.buffer.pop();
            popup.caret = popup.buffer.len();
        }
        KeyCode::Char(c) if !ke.modifiers.contains(KeyModifiers::CONTROL) => {
            popup.buffer.push(c);
            popup.caret = popup.buffer.len();
        }
        _ => {}
    }
}

/// Build a [`PatchRequest`], call [`patch_key`], and on success trigger
/// settings reload. Updates `state.status` with the result and closes
/// the popup on success.
fn try_save_popup(app: &mut AppState, state: &mut ConfigEditorState) {
    let Some(popup) = state.popup.as_ref() else {
        return;
    };
    let value = popup.canonical_value();
    if let Err(msg) = validate_value(&popup.kind, &value) {
        state.status = Some(format!("Invalid value: {msg}"));
        return;
    }
    let req = PatchRequest {
        file: popup.setting.file,
        key: popup.setting.key,
        aliases: popup.setting.aliases,
        value: &value,
        dry_run: app.dry_run,
    };
    let key_label = popup.setting.key;
    match patch_key(&req) {
        Ok(PatchOutcome::Written { path }) => {
            state.status = Some(format!("Saved {key_label} → {}", path.display()));
            reload_after_save(app);
            state.popup = None;
        }
        Ok(PatchOutcome::NoChange { .. }) => {
            state.status = Some(format!("{key_label} is already set to that value."));
            state.popup = None;
        }
        Ok(PatchOutcome::DryRun { path, .. }) => {
            state.status = Some(format!(
                "Dry-run: would update {key_label} → {}",
                path.display()
            ));
            state.popup = None;
        }
        Err(e) => {
            state.status = Some(format!("Save failed: {e}"));
        }
    }
}

/// What: Validate `value` against the popup `kind` before writing.
///
/// Inputs:
/// - `kind`: Popup variant (carries the schema-derived constraints).
/// - `value`: Canonical string the user wants to save.
///
/// Output:
/// - `Ok(())` when the value is acceptable; `Err(reason)` otherwise.
///
/// Details:
/// - Phase 1 only validates Int range here. Other kinds are accepted
///   as-is and rely on parsers in `theme::settings::parse_settings`.
fn validate_value(kind: &EditPopupKind, value: &str) -> Result<(), String> {
    match kind {
        EditPopupKind::Int { min, max } => {
            let parsed: i64 = value
                .trim()
                .parse()
                .map_err(|_| format!("expected integer in {min}..={max}"))?;
            if parsed < *min || parsed > *max {
                return Err(format!("must be within {min}..={max}"));
            }
            Ok(())
        }
        EditPopupKind::Bool(_)
        | EditPopupKind::Enum { .. }
        | EditPopupKind::Text
        | EditPopupKind::Secret { .. } => Ok(()),
    }
}

/// What: Reload the in-memory `Settings` snapshot after a successful save.
///
/// Inputs:
/// - `app`: Mutable application state.
///
/// Output:
/// - Mutates fields on `app` derived from `Settings`.
///
/// Details:
/// - Mirrors the relevant subset of `events::global::handle_reload_config`
///   for the settings-only Phase 1 flow. Theme/repos/locale reloads stay
///   on the existing `Reload Config` keybind path until later phases.
fn reload_after_save(app: &mut AppState) {
    let new_settings = crate::theme::settings();
    crate::app::apply_settings_to_app_state(app, &new_settings);
}

/// What: Build the initial editor state when opening from the dropdown.
///
/// Inputs: None.
///
/// Output:
/// - Boxed [`ConfigEditorState`] focused on `settings.conf`.
#[must_use]
pub fn build_initial_state() -> Box<ConfigEditorState> {
    Box::new(ConfigEditorState::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::find_setting;

    fn setting(key: &str) -> &'static crate::theme::EditableSetting {
        find_setting(key).expect("schema entry")
    }

    #[test]
    fn validate_int_range_rejects_out_of_bounds() {
        let kind = EditPopupKind::Int { min: 1, max: 5 };
        assert!(validate_value(&kind, "3").is_ok());
        assert!(validate_value(&kind, "10").is_err());
        assert!(validate_value(&kind, "abc").is_err());
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
        handle_popup_enum(ke, &mut popup);
        assert_ne!(popup.canonical_value(), "best_matches");
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
    fn escape_in_list_drops_to_filelist_and_stays_open() {
        let mut state = ConfigEditorState {
            view: ConfigEditorView::KeyList,
            ..ConfigEditorState::default()
        };
        back_to_file_list(&mut state);
        assert!(matches!(state.view, ConfigEditorView::FileList));
        back_to_file_list(&mut state);
        assert!(matches!(state.view, ConfigEditorView::FileList));
    }

    #[test]
    fn search_input_appends_and_filters() {
        let mut state = ConfigEditorState {
            focus: ConfigEditorFocus::Search,
            ..ConfigEditorState::default()
        };
        for c in "mirror".chars() {
            handle_search_key(
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
                &mut state,
                &AppState::default(),
            );
        }
        assert_eq!(state.query, "mirror");
        let keys = state.filtered_keys();
        assert!(keys.iter().any(|k| k.key == "mirror_count"));
    }

    #[test]
    fn search_input_up_down_moves_file_selection() {
        let mut state = ConfigEditorState {
            focus: ConfigEditorFocus::Search,
            search_focus: ConfigEditorSearchFocus::Input,
            ..ConfigEditorState::default()
        };
        assert_eq!(state.file_cursor, 0);

        handle_search_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut state,
            &AppState::default(),
        );
        assert_eq!(state.file_cursor, 1);

        handle_search_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut state,
            &AppState::default(),
        );
        assert_eq!(state.file_cursor, 0);
    }

    #[test]
    fn search_input_enter_opens_selected_file_when_query_empty() {
        let mut state = ConfigEditorState {
            focus: ConfigEditorFocus::Search,
            search_focus: ConfigEditorSearchFocus::Input,
            file_cursor: 0,
            ..ConfigEditorState::default()
        };
        assert!(matches!(state.view, ConfigEditorView::FileList));

        handle_search_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut state,
            &AppState::default(),
        );

        assert!(matches!(state.view, ConfigEditorView::KeyList));
        assert!(matches!(state.focus, ConfigEditorFocus::Search));
    }

    #[test]
    fn esc_keeps_search_focus_in_input() {
        let mut state = ConfigEditorState {
            view: ConfigEditorView::KeyList,
            focus: ConfigEditorFocus::Search,
            search_focus: ConfigEditorSearchFocus::Input,
            query: "mirror".to_string(),
            ..ConfigEditorState::default()
        };
        handle_search_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut state,
            &AppState::default(),
        );
        assert!(matches!(state.focus, ConfigEditorFocus::Search));
        assert!(matches!(state.search_focus, ConfigEditorSearchFocus::Input));
    }

    #[test]
    fn enter_on_bookmark_opens_edit_popup() {
        let mut state = ConfigEditorState {
            focus: ConfigEditorFocus::Search,
            search_focus: ConfigEditorSearchFocus::Bookmarks,
            view: ConfigEditorView::KeyList,
            bookmarked_keys: vec!["sort_mode".to_string()],
            bookmark_cursor: 0,
            ..ConfigEditorState::default()
        };
        handle_search_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut state,
            &AppState::default(),
        );
        let popup_key = state.popup.as_ref().map(|p| p.setting.key);
        assert_eq!(popup_key, Some("sort_mode"));
    }
}
