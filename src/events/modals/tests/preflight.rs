//! Tests for `PreflightExec` modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{
    PackageItem, PreflightAction, PreflightTab,
    modal::{PreflightHeaderChips, RepoOverlapApplyPending},
};

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

#[test]
/// What: Verify Enter does not open post-summary while repo-apply overlap is pending and the
/// executor has not yet reported completion.
///
/// Inputs:
/// - `PreflightExec` with `success: None`, empty items, `pending_repo_apply_overlap_check` set
/// - Enter key
///
/// Output:
/// - Modal stays `PreflightExec`, `pending_post_summary_items` stays unset, toast is shown
///
/// Details:
/// - Prevents skipping the foreign-vs-repo overlap step when Enter is pressed too early
fn preflight_exec_enter_deferred_when_repo_overlap_pending_and_exec_running() {
    let mut app = new_app();
    app.pending_repo_apply_overlap_check = Some(RepoOverlapApplyPending {
        repo_section: "chaotic-aur".to_string(),
        pre_apply_foreign_snapshot: None,
    });
    app.modal = crate::state::Modal::PreflightExec {
        verbose: false,
        log_lines: vec![":: Synchronizing package databases…".to_string()],
        abortable: true,
        items: vec![],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        success: None,
        header_chips: PreflightHeaderChips::default(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(app.pending_post_summary_items.is_none());
    assert!(app.toast_message.is_some());
    assert!(
        matches!(
            app.modal,
            crate::state::Modal::PreflightExec { success: None, .. }
        ),
        "expected PreflightExec with success None, got {:?}",
        std::mem::discriminant(&app.modal)
    );
}

#[test]
/// What: Verify Enter still opens post-summary when overlap is not pending and success is unknown.
///
/// Inputs:
/// - `PreflightExec` with `success: None`, empty items, no pending overlap
/// - Enter key
///
/// Output:
/// - Modal transitions to `Loading` (post-summary pipeline)
///
/// Details:
/// - Ensures the deferral guard does not block unrelated empty-item preflight flows
fn preflight_exec_enter_loading_when_no_repo_overlap_pending() {
    let mut app = new_app();
    assert!(app.pending_repo_apply_overlap_check.is_none());
    app.modal = crate::state::Modal::PreflightExec {
        verbose: false,
        log_lines: vec![],
        abortable: false,
        items: vec![],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        success: None,
        header_chips: PreflightHeaderChips::default(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::Loading { .. }));
    assert!(app.pending_post_summary_items.is_some());
}
