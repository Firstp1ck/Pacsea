//! Tests for `PreflightExec` modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{PackageItem, PreflightAction, PreflightTab, modal::PreflightHeaderChips};

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes `PreflightExec` modal and doesn't restore it.
///
/// Inputs:
/// - `PreflightExec` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn preflight_exec_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PreflightExec {
        verbose: false,
        log_lines: vec![],
        abortable: true,
        items: vec![],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        success: None,
        header_chips: PreflightHeaderChips::default(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify 'q' key closes `PreflightExec` modal and doesn't restore it.
///
/// Inputs:
/// - `PreflightExec` modal
/// - 'q' key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests that 'q' also works to close the modal
fn preflight_exec_q_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PreflightExec {
        verbose: false,
        log_lines: vec![],
        abortable: true,
        items: vec![],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        success: None,
        header_chips: PreflightHeaderChips::default(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('q'), KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}
