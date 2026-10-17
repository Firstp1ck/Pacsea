//! Configuration editor input routing, tab navigation, and popup lifecycle.
use crate::state::{AppState, ConfigEditorState, EditPopupState};
use crate::theme::{ConfigFile, EditableSetting, KeyChord};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// Mouse interaction with renderer-provided rectangles.
mod mouse;
/// Popup input and recording.
mod popup;
/// Existing patch, validation, and reload flow.
mod save;
pub(in crate::events) use mouse::handle_config_editor_mouse;
#[cfg(test)]
use mouse::handle_config_editor_popup_click;
use popup::handle_popup_key;

/// What: Consume configuration editor input before package-mode handlers.
///
/// Inputs: Key event and application state.
///
/// Output: Updates editor or leaves to package/Pi Scan mode; always consumes the event.
///
/// Details: State is moved temporarily to permit app updates during saves.
pub(in crate::events) fn handle_config_editor_mode_key(ke: KeyEvent, app: &mut AppState) -> bool {
    if app.config_editor_state.search_text.is_empty() {
        let mut state = std::mem::take(&mut app.config_editor_state);
        state.build_search_text(app);
        app.config_editor_state = state;
    }
    let pressed = KeyChord {
        code: ke.code,
        mods: ke.modifiers,
    };
    if app.keymap.config_editor_pi_scan_setup.contains(&pressed)
        && app.config_editor_state.popup.is_none()
        && app
            .config_editor_state
            .selected_key()
            .is_some_and(|entry| entry.key.starts_with("pi_scan_"))
    {
        let first_run = !app.pi_scan.setup_complete();
        app.pi_scan.origin = crate::state::types::AppMode::ConfigEditor;
        app.pi_scan.begin_setup_wizard(first_run);
        app.app_mode = crate::state::types::AppMode::PiScan;
        return true;
    }
    let mut state = std::mem::take(&mut app.config_editor_state);
    if state.popup.is_some() {
        handle_popup_key(ke, app, &mut state);
    } else {
        handle_editor_key(ke, &mut state, app);
    }
    app.config_editor_state = state;
    true
}

/// What: Handle list/search keys and the Esc ladder.
///
/// Inputs: Key, editor, and application state.
///
/// Output: Moves selection, edits search, opens popups, or leaves the editor.
///
/// Details: Tabs preserve search; plain `b` remains searchable text.
fn handle_editor_key(ke: KeyEvent, state: &mut ConfigEditorState, app: &mut AppState) {
    if handle_recent_key(ke, state) {
        return;
    }
    match (ke.code, ke.modifiers) {
        (KeyCode::Esc, _) => {
            if state.query.is_empty() {
                app.app_mode = crate::state::types::AppMode::Package;
            } else {
                state.query.clear();
                state.query_caret = 0;
                state.clamp_key_cursor();
                mark_query_input_changed(state);
            }
        }
        (KeyCode::Char('b'), m) if m.contains(KeyModifiers::CONTROL) => {
            toggle_selected_bookmark(state);
        }
        (KeyCode::Char('r'), m) if m.contains(KeyModifiers::CONTROL) => {
            state.recent_dropdown = Some(0);
        }
        (KeyCode::Char('e'), m) if m.contains(KeyModifiers::CONTROL) => {
            save::export_effective_config(state, app.dry_run);
        }
        (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => state.cycle_tab(false),
        (KeyCode::Tab | KeyCode::Right, _) => state.cycle_tab(true),
        (KeyCode::BackTab | KeyCode::Left, _) => state.cycle_tab(false),
        (KeyCode::Up, _) => move_list(state, -1),
        (KeyCode::Down, _) => move_list(state, 1),
        (KeyCode::PageUp, _) => move_list(
            state,
            -isize::try_from(state.list_page_rows.max(1)).unwrap_or(1),
        ),
        (KeyCode::PageDown, _) => move_list(
            state,
            isize::try_from(state.list_page_rows.max(1)).unwrap_or(1),
        ),
        (KeyCode::Home, _) => state.key_cursor = 0,
        (KeyCode::End, _) => state.key_cursor = state.filtered_keys().len().saturating_sub(1),
        (KeyCode::Enter, _) => {
            let query = state.query.clone();
            state.push_recent_query(&query);
            open_popup_for_selection(state, app);
        }
        (KeyCode::Backspace, _) => {
            state.query.pop();
            query_changed(state);
        }
        (KeyCode::Char(ch), m) if m.is_empty() || m == KeyModifiers::SHIFT => {
            state.query.push(ch);
            query_changed(state);
        }
        _ => {}
    }
}

/// Update query metadata and keep the selected key in bounds.
fn query_changed(state: &mut ConfigEditorState) {
    state.query_caret = state.query.len();
    state.clamp_key_cursor();
    mark_query_input_changed(state);
}

/// Keep persistence debounce aware of user input.
fn mark_query_input_changed(state: &mut ConfigEditorState) {
    state.query_last_input_change = std::time::Instant::now();
    state.last_saved_query_value = None;
}

/// Consume recent-dropdown keys before list navigation.
fn handle_recent_key(ke: KeyEvent, state: &mut ConfigEditorState) -> bool {
    let Some(cursor) = state.recent_dropdown else {
        return false;
    };
    match ke.code {
        KeyCode::Esc => state.recent_dropdown = None,
        KeyCode::Up => state.recent_dropdown = Some(cursor.saturating_sub(1)),
        KeyCode::Down => {
            state.recent_dropdown =
                Some((cursor + 1).min(state.recent_queries.len().saturating_sub(1)));
        }
        KeyCode::Enter => apply_recent(state, cursor),
        _ => {}
    }
    true
}

/// Apply a recent query and dismiss its dropdown.
fn apply_recent(state: &mut ConfigEditorState, index: usize) {
    if let Some(query) = state.recent_queries.get(index).cloned() {
        state.query = query;
        query_changed(state);
    }
    state.recent_dropdown = None;
}

/// Toggle the selected key's bookmark and clamp the Starred list after removal.
fn toggle_selected_bookmark(state: &mut ConfigEditorState) {
    if let Some(entry) = state.selected_key() {
        let added = state.toggle_bookmark_key(entry.key);
        state.clamp_key_cursor();
        state.status = Some(if added {
            format!("Bookmarked {}", entry.key)
        } else {
            format!("Removed bookmark {}", entry.key)
        });
    }
}

/// Move the key selection within filtered grouped rows.
fn move_list(state: &mut ConfigEditorState, delta: isize) {
    state.key_cursor = state
        .key_cursor
        .saturating_add_signed(delta)
        .min(state.filtered_keys().len().saturating_sub(1));
}

/// Capture opening-time facts and construct the selected setting's popup.
fn open_popup_for_selection(state: &mut ConfigEditorState, app: &AppState) {
    if let Some(entry) = state.selected_key() {
        let values = crate::state::config_editor::ValueSnapshot::capture();
        let context = crate::state::config_editor::SuggestionContext::capture(app, entry, &values);
        let mut popup = EditPopupState::from_current(entry, &context.current);
        popup.set_suggestions(crate::state::config_editor::suggestions_for(
            entry, &context,
        ));
        popup.opened_mtime = target_file_mtime(entry.file);
        state.popup = Some(popup);
        state.status = None;
    }
}

/// Target modification time used by the stale-file guard.
fn target_file_mtime(file: ConfigFile) -> Option<std::time::SystemTime> {
    std::fs::metadata(crate::theme::resolved_config_path(file))
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

/// Filename used in existing English status messages.
const fn file_label_for_status(file: ConfigFile) -> &'static str {
    match file {
        ConfigFile::Settings => "settings.conf",
        ConfigFile::Keybinds => "keybinds.conf",
        ConfigFile::Theme => "theme.conf",
        ConfigFile::Repos => "repos.conf",
    }
}

/// What: Build the editor on its Settings tab with localized search ready.
///
/// Inputs: Application translations.
///
/// Output: Boxed editor state.
///
/// Details: Histories retain their existing persistence format and location.
#[must_use]
pub fn build_initial_state(app: &AppState) -> Box<ConfigEditorState> {
    let mut state = Box::new(ConfigEditorState::load_persisted());
    state.build_search_text(app);
    state
}

#[cfg(test)]
mod tests;
