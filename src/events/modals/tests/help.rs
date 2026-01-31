//! Tests for Help modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes Help modal.
///
/// Inputs:
/// - Help modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes Help modal correctly
fn help_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Help;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes Help modal.
///
/// Inputs:
/// - Help modal
/// - Enter key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Enter also closes Help modal
fn help_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Help;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify numpad Enter (carriage return) closes Help modal like main Enter.
///
/// Inputs:
/// - Help modal
/// - KeyCode::Char('\r')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break Help; same outcome as main Enter
fn help_numpad_enter_carriage_return_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Help;
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify numpad Enter (newline) closes Help modal like main Enter.
///
/// Inputs:
/// - Help modal
/// - KeyCode::Char('\n')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break Help; same outcome as main Enter
fn help_numpad_enter_newline_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Help;
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}
