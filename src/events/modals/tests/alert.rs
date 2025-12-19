//! Tests for Alert modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes Alert modal.
///
/// Inputs:
/// - Alert modal with message
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes Alert modal correctly
fn alert_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Alert {
        message: "Test alert message".to_string(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes Alert modal.
///
/// Inputs:
/// - Alert modal with message
/// - Enter key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Enter also closes Alert modal
fn alert_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Alert {
        message: "Test alert message".to_string(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}
