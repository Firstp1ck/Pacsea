//! Hit testing against the renderer's configuration editor rectangles.
use super::{AppState, ConfigEditorState, apply_recent, move_list, open_popup_for_selection};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

/// Whether a coordinate is inside a half-open screen rectangle.
const fn contains(rect: (u16, u16, u16, u16), x: u16, y: u16) -> bool {
    x >= rect.0
        && y >= rect.1
        && x < rect.0.saturating_add(rect.2)
        && y < rect.1.saturating_add(rect.3)
}

/// What: Consume list, tab, recent, record, and popup mouse actions.
///
/// Inputs: Mouse event and application state with frame hit rectangles.
///
/// Output: True for an editor interaction; this never requests application exit.
///
/// Details: Clicking an already-selected row opens its popup; suggestion clicks only highlight.
/// Returns false for modal/menu overlays and pointer motion before taking editor state.
pub(in crate::events) fn handle_config_editor_mouse(event: MouseEvent, app: &mut AppState) -> bool {
    // Top-row dropdowns are drawn over the editor; the menu router must see their clicks.
    if !matches!(app.modal, crate::state::Modal::None)
        || app.config_menu_open
        || app.options_menu_open
        || app.panels_menu_open
        || matches!(event.kind, MouseEventKind::Moved | MouseEventKind::Drag(_))
    {
        return false;
    }
    let mut state = std::mem::take(&mut app.config_editor_state);
    let handled = if state.popup.is_some() {
        popup_mouse(event, &mut state)
    } else {
        list_mouse(event, &mut state, app)
    };
    app.config_editor_state = state;
    handled
}

/// Handle popup suggestion selection, wheel movement, and the Record action.
fn popup_mouse(event: MouseEvent, state: &mut ConfigEditorState) -> bool {
    let Some(popup) = state.popup.as_mut() else {
        return false;
    };
    if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
        if state
            .record_button_rect
            .is_some_and(|rect| contains(rect, event.column, event.row))
        {
            if let crate::state::EditPopupKind::KeyChord { capturing } = &mut popup.kind {
                *capturing = true;
            }
            return true;
        }
        if let Some(row) = state
            .popup_choice_rects
            .iter()
            .position(|&rect| contains(rect, event.column, event.row))
        {
            return popup.select_highlight(popup.scroll + row);
        }
    }
    if state
        .popup_rect
        .is_some_and(|rect| contains(rect, event.column, event.row))
    {
        match event.kind {
            MouseEventKind::ScrollUp => {
                popup.move_highlight(-1);
                return true;
            }
            MouseEventKind::ScrollDown => {
                popup.move_highlight(1);
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Handle tabs, recent queries, selectable rows, and scrolling over the key list.
fn list_mouse(event: MouseEvent, state: &mut ConfigEditorState, app: &AppState) -> bool {
    if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
        if state.recent_dropdown.is_some() {
            if let Some(&(index, _)) = state
                .recent_rects
                .iter()
                .find(|(_, rect)| contains(*rect, event.column, event.row))
            {
                apply_recent(state, index);
            } else {
                state.recent_dropdown = None;
            }
            return true;
        }
        if let Some(&(tab, _)) = state
            .tab_rects
            .iter()
            .find(|(_, rect)| contains(*rect, event.column, event.row))
        {
            state.set_tab(tab);
            return true;
        }
        if let Some(&(index, _)) = state
            .row_rects
            .iter()
            .find(|(_, rect)| contains(*rect, event.column, event.row))
        {
            if index >= state.filtered_keys().len() {
                return false;
            }
            if state.key_cursor == index {
                open_popup_for_selection(state, app);
            } else {
                state.key_cursor = index;
            }
            return true;
        }
    }
    if state
        .list_rect
        .is_some_and(|rect| contains(rect, event.column, event.row))
    {
        match event.kind {
            MouseEventKind::ScrollUp => {
                move_list(state, -1);
                return true;
            }
            MouseEventKind::ScrollDown => {
                move_list(state, 1);
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Compatibility click entrypoint used by existing enum-popup tests.
#[cfg(test)]
pub(in crate::events) fn handle_config_editor_popup_click(
    x: u16,
    y: u16,
    app: &mut AppState,
) -> bool {
    handle_config_editor_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        app,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ConfigEditorTab, EditPopupKind, EditPopupState};

    /// Mouse event at a known test rectangle.
    fn event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    /// Tabs, rows, recent queries, and wheel input consume their exact hit regions.
    #[test]
    fn list_mouse_rectangles() {
        let mut app = AppState::default();
        app.config_editor_state.tab_rects = vec![(ConfigEditorTab::Theme, (1, 1, 5, 1))];
        assert!(handle_config_editor_mouse(
            event(MouseEventKind::Down(MouseButton::Left), 1, 1),
            &mut app
        ));
        assert_eq!(app.config_editor_state.active_tab(), ConfigEditorTab::Theme);
        app.config_editor_state.row_rects = vec![(1, (1, 3, 20, 1))];
        assert!(handle_config_editor_mouse(
            event(MouseEventKind::Down(MouseButton::Left), 2, 3),
            &mut app
        ));
        assert_eq!(app.config_editor_state.key_cursor, 1);
        assert!(app.config_editor_state.popup.is_none());
        app.config_editor_state.list_rect = Some((1, 3, 20, 8));
        assert!(handle_config_editor_mouse(
            event(MouseEventKind::ScrollDown, 2, 4),
            &mut app
        ));
        assert_eq!(app.config_editor_state.key_cursor, 2);
        handle_config_editor_mouse(event(MouseEventKind::ScrollUp, 2, 4), &mut app);
        assert!(handle_config_editor_mouse(
            event(MouseEventKind::Down(MouseButton::Left), 2, 3),
            &mut app
        ));
        assert!(app.config_editor_state.popup.is_some());
        app.config_editor_state.popup = None;
        app.config_editor_state.recent_dropdown = Some(0);
        app.config_editor_state.recent_queries = vec!["semantic".into()];
        app.config_editor_state.recent_rects = vec![(0, (2, 2, 20, 1))];
        handle_config_editor_mouse(
            event(MouseEventKind::Down(MouseButton::Left), 3, 2),
            &mut app,
        );
        assert_eq!(app.config_editor_state.query, "semantic");
        assert!(app.config_editor_state.recent_dropdown.is_none());
        assert!(!handle_config_editor_mouse(
            event(MouseEventKind::ScrollDown, 40, 40),
            &mut app
        ));
    }

    /// An open top-row dropdown owns clicks even where it overlaps editor rows.
    #[test]
    fn open_dropdown_menu_owns_clicks_over_rows() {
        let mut app = AppState {
            config_menu_open: true,
            ..AppState::default()
        };
        app.config_editor_state.row_rects = vec![(1, (1, 3, 20, 1))];
        assert!(!handle_config_editor_mouse(
            event(MouseEventKind::Down(MouseButton::Left), 2, 3),
            &mut app
        ));
        assert_eq!(app.config_editor_state.key_cursor, 0);
    }

    /// Popup clicks use visible indices and scroll offsets; Record starts capture without saving.
    #[test]
    fn popup_mouse_rectangles() {
        let mut app = AppState::default();
        let entry = crate::theme::find_setting("pi_scan_thinking").expect("schema");
        let mut popup = EditPopupState::from_current(entry, "off");
        popup.scroll = 2;
        app.config_editor_state.popup = Some(popup);
        app.config_editor_state.popup_choice_rects = vec![(2, 3, 10, 1)];
        handle_config_editor_mouse(
            event(MouseEventKind::Down(MouseButton::Left), 3, 3),
            &mut app,
        );
        assert_eq!(
            app.config_editor_state
                .popup
                .as_ref()
                .expect("popup")
                .effective_value(),
            "low"
        );
        app.config_editor_state.popup_rect = Some((1, 1, 20, 12));
        handle_config_editor_mouse(event(MouseEventKind::ScrollDown, 3, 4), &mut app);
        assert_eq!(
            app.config_editor_state
                .popup
                .as_ref()
                .expect("popup")
                .effective_value(),
            "medium"
        );
        app.config_editor_state.popup = Some(EditPopupState::from_current(
            crate::theme::find_setting("keybind_reload_config").expect("schema"),
            "Ctrl+r",
        ));
        app.config_editor_state.record_button_rect = Some((4, 10, 10, 1));
        handle_config_editor_mouse(
            event(MouseEventKind::Down(MouseButton::Left), 4, 10),
            &mut app,
        );
        assert!(matches!(
            app.config_editor_state.popup.as_ref().expect("popup").kind,
            EditPopupKind::KeyChord { capturing: true }
        ));
    }
}

/// Regression coverage for overlay ownership and inert pointer motion.
#[cfg(test)]
mod regression_tests {
    use super::*;
    use crate::state::{ConfigEditorTab, Modal};

    /// A click at a deterministic editor coordinate.
    fn click(x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    /// Motion and drag events never select rows or start capture.
    #[test]
    fn config_editor_motion_preserves_state() {
        let mut app = AppState::default();
        app.config_editor_state.row_rects = vec![(1, (1, 3, 20, 1))];
        for kind in [
            MouseEventKind::Moved,
            MouseEventKind::Drag(MouseButton::Left),
        ] {
            let mut event = click(2, 3);
            event.kind = kind;
            assert!(!handle_config_editor_mouse(event, &mut app));
            assert_eq!(app.config_editor_state.key_cursor, 0);
            assert_eq!(app.config_editor_state.row_rects.len(), 1);
        }
    }

    /// Alerts own input even over valid editor tabs and list rows.
    #[test]
    fn regression_alert_blocks_editor_mouse() {
        let mut app = AppState {
            modal: Modal::Alert {
                message: "Alert".into(),
            },
            ..AppState::default()
        };
        app.config_editor_state.tab_rects = vec![(ConfigEditorTab::Theme, (1, 1, 10, 1))];
        app.config_editor_state.row_rects = vec![(0, (1, 3, 20, 1))];
        assert!(!handle_config_editor_mouse(click(2, 1), &mut app));
        assert_eq!(
            app.config_editor_state.active_tab(),
            ConfigEditorTab::Settings
        );
        assert!(!handle_config_editor_mouse(click(2, 3), &mut app));
        assert!(app.config_editor_state.popup.is_none());
    }

    /// Dismissing recents consumes the click instead of selecting the covered row or tab.
    #[test]
    fn regression_recent_miss_consumes_click() {
        for (x, y) in [(2, 3), (2, 1), (40, 40)] {
            let mut app = AppState::default();
            app.config_editor_state.recent_dropdown = Some(0);
            app.config_editor_state.recent_rects = vec![(0, (1, 2, 20, 1))];
            app.config_editor_state.row_rects = vec![(1, (1, 3, 20, 1))];
            app.config_editor_state.tab_rects = vec![(ConfigEditorTab::Theme, (1, 1, 10, 1))];
            assert!(handle_config_editor_mouse(click(x, y), &mut app));
            assert!(app.config_editor_state.recent_dropdown.is_none());
            assert_eq!(app.config_editor_state.key_cursor, 0);
            assert_eq!(
                app.config_editor_state.active_tab(),
                ConfigEditorTab::Settings
            );
            assert!(app.config_editor_state.popup.is_none());
        }
    }
}
