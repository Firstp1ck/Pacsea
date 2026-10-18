//! Popup navigation, UTF-8 editing, suggestion completion, and chord capture.
use super::{AppState, ConfigEditorState, EditPopupState, KeyCode, KeyEvent, KeyModifiers};
use crate::state::{EditPopupKind, config_editor::DROPDOWN_PAGE_ROWS};

/// What: Handle a popup key with capture taking precedence over every command.
///
/// Inputs: Key event, application, and editor state.
///
/// Output: Edits the popup, saves its effective value, or cancels it.
///
/// Details: Live validation is cached after each edit; rendering does no validation work.
pub(super) fn handle_popup_key(ke: KeyEvent, app: &mut AppState, state: &mut ConfigEditorState) {
    if let Some(popup) = state.popup.as_mut()
        && matches!(popup.kind, EditPopupKind::KeyChord { capturing: true })
    {
        handle_keychord_capture(ke, popup);
        return;
    }
    match (ke.code, ke.modifiers) {
        (KeyCode::Esc, _) => state.popup = None,
        (KeyCode::Enter, _) => super::save::try_save_popup(app, state),
        (KeyCode::Char('s'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            super::save::try_save_popup(app, state);
        }
        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            reset_popup_to_original(state);
        }
        _ => {
            if let Some(popup) = state.popup.as_mut() {
                handle_popup_value_key(ke, popup);
            }
        }
    }
}

/// Restore opening input and suggestion set while preserving the stale-file timestamp.
fn reset_popup_to_original(state: &mut ConfigEditorState) {
    let Some(popup) = state.popup.as_mut() else {
        return;
    };
    let mut fresh = EditPopupState::from_current(popup.setting, &popup.original);
    fresh.opened_mtime = popup.opened_mtime;
    fresh.set_suggestions(popup.suggestions.clone());
    *popup = fresh;
    state.status = Some("Reset to the value the popup opened with.".to_string());
}

/// Record a supported chord, or stop recording on Esc while keeping the popup open.
fn handle_keychord_capture(ke: KeyEvent, popup: &mut EditPopupState) {
    if ke.code == KeyCode::Esc {
        popup.kind = EditPopupKind::KeyChord { capturing: false };
        return;
    }
    if let Some(chord) =
        crate::state::config_editor::key_event_to_canonical_chord_string(ke.code, ke.modifiers)
    {
        popup.buffer = chord;
        popup.caret = popup.buffer.len();
        popup.kind = EditPopupKind::KeyChord { capturing: false };
        popup.filter_active = true;
        popup.refresh_visible();
    }
}

/// Dispatch dropdown navigation and type-specific edits.
fn handle_popup_value_key(ke: KeyEvent, popup: &mut EditPopupState) {
    if ke.modifiers.contains(KeyModifiers::CONTROL) {
        match (&mut popup.kind, ke.code) {
            (EditPopupKind::Secret { revealed }, KeyCode::Char('r')) => *revealed = !*revealed,
            (EditPopupKind::KeyChord { capturing }, KeyCode::Char('k')) => *capturing = true,
            _ => {}
        }
        return;
    }
    match ke.code {
        KeyCode::Up => popup.move_highlight(-1),
        KeyCode::Down => popup.move_highlight(1),
        KeyCode::PageUp => popup.move_highlight(-isize::try_from(DROPDOWN_PAGE_ROWS).unwrap_or(1)),
        KeyCode::PageDown => popup.move_highlight(isize::try_from(DROPDOWN_PAGE_ROWS).unwrap_or(1)),
        KeyCode::Tab if ke.modifiers.contains(KeyModifiers::SHIFT) => {
            complete_or_cycle(popup, false);
        }
        KeyCode::Tab => complete_or_cycle(popup, true),
        KeyCode::BackTab => complete_or_cycle(popup, false),
        _ => match popup.kind {
            EditPopupKind::Bool(_)
                if matches!(ke.code, KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')) =>
            {
                handle_popup_bool(ke, popup);
            }
            EditPopupKind::Int { min, max } if matches!(ke.code, KeyCode::Char('+' | '-')) => {
                bump_int(
                    popup,
                    if ke.code == KeyCode::Char('+') { 1 } else { -1 },
                    min,
                    max,
                );
            }
            EditPopupKind::Bool(_) => {}
            _ if ke.modifiers.is_empty() || ke.modifiers == KeyModifiers::SHIFT => {
                popup.edit_buffer(ke.code);
            }
            _ => {}
        },
    }
}

/// Complete a free-input token or cycle a strict choice with wraparound.
fn complete_or_cycle(popup: &mut EditPopupState, forward: bool) {
    if !popup.is_strict() {
        popup.complete_suggestion();
        return;
    }
    if popup.visible.is_empty() {
        return;
    }
    let current = popup.highlight.unwrap_or(0);
    let next = (current + if forward { 1 } else { popup.visible.len() - 1 }) % popup.visible.len();
    popup.select_highlight(next);
}

/// Toggle the boolean picker and synchronize its dropdown highlight.
pub(super) fn handle_popup_bool(_ke: KeyEvent, popup: &mut EditPopupState) {
    if let EditPopupKind::Bool(value) = &mut popup.kind {
        *value = !*value;
        popup.buffer = popup.canonical_value();
        popup.filter_active = false;
        popup.refresh_visible();
    }
}

/// Nudge an integer by one or a test-specified delta without leaving its schema range.
pub(super) fn bump_int(popup: &mut EditPopupState, delta: i64, min: i64, max: i64) {
    popup.buffer = popup
        .effective_value()
        .trim()
        .parse::<i64>()
        .unwrap_or(min)
        .saturating_add(delta)
        .clamp(min, max)
        .to_string();
    popup.caret = popup.buffer.len();
    popup.filter_active = true;
    popup.refresh_visible();
}
