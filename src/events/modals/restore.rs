//! Helper functions for restoring modal state after event handling.

use crate::state::{AppState, Modal};
use crossterm::event::{KeyCode, KeyEvent};

/// What: Restore a modal if it wasn't closed by the handler and the key event
/// doesn't match any excluded keys.
///
/// Inputs:
/// - `app`: Mutable application state to check and restore modal in
/// - `ke`: Key event to check against excluded keys
/// - `excluded_keys`: Slice of key codes that should prevent restoration
/// - `modal`: Modal variant to restore if conditions are met
///
/// Output:
/// - None (mutates `app.modal` directly)
///
/// Details:
/// - Checks if `app.modal` is `None` (indicating handler closed it)
/// - If modal is still `None` and key doesn't match excluded keys, restores the modal
/// - Used for modals like `PreflightExec` and `PostSummary` that exclude Esc/q or Esc/Enter/q
pub(super) fn restore_if_not_closed_with_excluded_keys(
    app: &mut AppState,
    ke: &KeyEvent,
    excluded_keys: &[KeyCode],
    modal: Modal,
) {
    if matches!(app.modal, Modal::None) && !excluded_keys.contains(&ke.code) {
        app.modal = modal;
    }
}

/// What: Restore a modal if it wasn't closed by the handler, considering an
/// Option<bool> result and excluding Esc/q keys.
///
/// Inputs:
/// - `app`: Mutable application state to check and restore modal in
/// - `ke`: Key event to check against Esc/q keys
/// - `should_stop`: Optional boolean indicating if event propagation should stop
/// - `modal`: Modal variant to restore if conditions are met
///
/// Output:
/// - The boolean value from `should_stop`, or `false` if `None`
/// - Returns `true` if Esc or 'q' was pressed and modal was closed (to stop propagation)
///
/// Details:
/// - Used for modals like `SystemUpdate` and `OptionalDeps` that return `Option<bool>`
/// - Restores modal if handler didn't close it and Esc/q wasn't pressed
/// - Esc/q keys close modal even if `should_stop` is `Some(false)`
/// - When Esc/q closes the modal, returns `true` to stop event propagation
pub(super) fn restore_if_not_closed_with_option_result(
    app: &mut AppState,
    ke: &KeyEvent,
    should_stop: Option<bool>,
    modal: Modal,
) -> bool {
    if matches!(app.modal, Modal::None) {
        // If Esc or 'q' was pressed and modal was closed, stop propagation
        if matches!(ke.code, KeyCode::Esc | KeyCode::Char('q')) {
            return true;
        }
        // Only restore if handler didn't intentionally close (Esc/q returns Some(false) but closes modal)
        // For navigation/toggle keys, handler returns Some(false) but doesn't close, so we restore
        if should_stop.is_none() || should_stop == Some(false) {
            app.modal = modal;
        }
    }
    should_stop.unwrap_or(false)
}

/// What: Restore a modal if it wasn't closed by the handler and Esc wasn't pressed.
///
/// Inputs:
/// - `app`: Mutable application state to check and restore modal in
/// - `ke`: Key event to check against Esc key
/// - `modal`: Modal variant to restore if conditions are met
///
/// Output:
/// - None (mutates `app.modal` directly)
///
/// Details:
/// - Checks if `app.modal` is `None` (indicating handler closed it)
/// - If modal is still `None` and key is not Esc, restores the modal
/// - Used for modals like `ScanConfig` and `VirusTotalSetup` that only exclude Esc
pub(super) fn restore_if_not_closed_with_esc(app: &mut AppState, ke: &KeyEvent, modal: Modal) {
    if matches!(app.modal, Modal::None) && !matches!(ke.code, KeyCode::Esc) {
        app.modal = modal;
    }
}

/// What: Restore a modal if it wasn't closed by the handler and the boolean
/// result indicates the event wasn't fully handled.
///
/// Inputs:
/// - `app`: Mutable application state to check and restore modal in
/// - `result`: Boolean indicating if event was fully handled (true = don't restore)
/// - `modal`: Modal variant to restore if conditions are met
///
/// Output:
/// - The boolean result value
///
/// Details:
/// - Used for modals like News that return a boolean indicating if propagation should stop
/// - If result is `false` (event not fully handled) and modal is `None`, restores the modal
pub(super) fn restore_if_not_closed_with_bool_result(
    app: &mut AppState,
    result: bool,
    modal: Modal,
) -> bool {
    // Restore modal if handler didn't change it and Esc wasn't pressed (result != true)
    // Esc returns true to stop propagation, so we shouldn't restore in that case
    if !result && matches!(app.modal, Modal::None) {
        app.modal = modal;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn create_key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn create_app_state_with_modal(modal: Modal) -> AppState {
        AppState {
            modal,
            ..Default::default()
        }
    }

    #[test]
    fn test_restore_if_not_closed_with_excluded_keys_restores_when_not_excluded() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Char('a'));
        let modal = Modal::Help;
        let excluded = [KeyCode::Esc, KeyCode::Char('q')];

        restore_if_not_closed_with_excluded_keys(&mut app, &ke, &excluded, modal);

        assert!(matches!(app.modal, Modal::Help));
    }

    #[test]
    fn test_restore_if_not_closed_with_excluded_keys_doesnt_restore_when_excluded() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Esc);
        let modal = Modal::Help;
        let excluded = [KeyCode::Esc, KeyCode::Char('q')];

        restore_if_not_closed_with_excluded_keys(&mut app, &ke, &excluded, modal);

        assert!(matches!(app.modal, Modal::None));
    }

    #[test]
    fn test_restore_if_not_closed_with_excluded_keys_doesnt_restore_when_modal_not_none() {
        let mut app = create_app_state_with_modal(Modal::Help);
        let ke = create_key_event(KeyCode::Char('a'));
        let modal = Modal::News {
            items: vec![],
            selected: 0,
        };
        let excluded = [KeyCode::Esc];

        restore_if_not_closed_with_excluded_keys(&mut app, &ke, &excluded, modal);

        assert!(matches!(app.modal, Modal::Help));
    }

    #[test]
    fn test_restore_if_not_closed_with_option_result_restores_when_none() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Char('a'));
        let modal = Modal::Help;

        let result = restore_if_not_closed_with_option_result(&mut app, &ke, None, modal);

        assert!(matches!(app.modal, Modal::Help));
        assert!(!result);
    }

    #[test]
    fn test_restore_if_not_closed_with_option_result_restores_when_false_and_not_esc() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Char('a'));
        let modal = Modal::Help;

        let result = restore_if_not_closed_with_option_result(&mut app, &ke, Some(false), modal);

        assert!(matches!(app.modal, Modal::Help));
        assert!(!result);
    }

    #[test]
    fn test_restore_if_not_closed_with_option_result_doesnt_restore_when_esc() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Esc);
        let modal = Modal::Help;

        let result = restore_if_not_closed_with_option_result(&mut app, &ke, Some(false), modal);

        assert!(matches!(app.modal, Modal::None));
        assert!(result); // Esc returns true to stop propagation
    }

    #[test]
    fn test_restore_if_not_closed_with_option_result_doesnt_restore_when_q() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Char('q'));
        let modal = Modal::Help;

        let result = restore_if_not_closed_with_option_result(&mut app, &ke, Some(false), modal);

        assert!(matches!(app.modal, Modal::None));
        assert!(result); // 'q' returns true to stop propagation
    }

    #[test]
    fn test_restore_if_not_closed_with_option_result_returns_true_when_some_true() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Char('a'));
        let modal = Modal::Help;

        let result = restore_if_not_closed_with_option_result(&mut app, &ke, Some(true), modal);

        assert!(matches!(app.modal, Modal::None));
        assert!(result);
    }

    #[test]
    fn test_restore_if_not_closed_with_esc_restores_when_not_esc() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Char('a'));
        let modal = Modal::Help;

        restore_if_not_closed_with_esc(&mut app, &ke, modal);

        assert!(matches!(app.modal, Modal::Help));
    }

    #[test]
    fn test_restore_if_not_closed_with_esc_doesnt_restore_when_esc() {
        let mut app = create_app_state_with_modal(Modal::None);
        let ke = create_key_event(KeyCode::Esc);
        let modal = Modal::Help;

        restore_if_not_closed_with_esc(&mut app, &ke, modal);

        assert!(matches!(app.modal, Modal::None));
    }

    #[test]
    fn test_restore_if_not_closed_with_esc_doesnt_restore_when_modal_not_none() {
        let mut app = create_app_state_with_modal(Modal::Help);
        let ke = create_key_event(KeyCode::Char('a'));
        let modal = Modal::News {
            items: vec![],
            selected: 0,
        };

        restore_if_not_closed_with_esc(&mut app, &ke, modal);

        assert!(matches!(app.modal, Modal::Help));
    }

    #[test]
    fn test_restore_if_not_closed_with_bool_result_restores_when_false() {
        let mut app = create_app_state_with_modal(Modal::None);
        let modal = Modal::Help;

        let result = restore_if_not_closed_with_bool_result(&mut app, false, modal);

        assert!(matches!(app.modal, Modal::Help));
        assert!(!result);
    }

    #[test]
    fn test_restore_if_not_closed_with_bool_result_doesnt_restore_when_true() {
        let mut app = create_app_state_with_modal(Modal::None);
        let modal = Modal::Help;

        let result = restore_if_not_closed_with_bool_result(&mut app, true, modal);

        assert!(matches!(app.modal, Modal::None));
        assert!(result);
    }

    #[test]
    fn test_restore_if_not_closed_with_bool_result_doesnt_restore_when_modal_not_none() {
        let mut app = create_app_state_with_modal(Modal::Help);
        let modal = Modal::News {
            items: vec![],
            selected: 0,
        };

        let result = restore_if_not_closed_with_bool_result(&mut app, false, modal);

        assert!(matches!(app.modal, Modal::Help));
        assert!(!result);
    }
}
