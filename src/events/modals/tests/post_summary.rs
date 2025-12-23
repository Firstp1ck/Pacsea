//! Tests for `PostSummary` modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes `PostSummary` modal and doesn't restore it.
///
/// Inputs:
/// - `PostSummary` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn post_summary_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PostSummary {
        success: true,
        changed_files: 0,
        pacnew_count: 0,
        pacsave_count: 0,
        services_pending: vec![],
        snapshot_label: None,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes `PostSummary` modal and doesn't restore it.
///
/// Inputs:
/// - `PostSummary` modal
/// - Enter key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests that Enter also works to close the modal
fn post_summary_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PostSummary {
        success: true,
        changed_files: 0,
        pacnew_count: 0,
        pacsave_count: 0,
        services_pending: vec![],
        snapshot_label: None,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}
