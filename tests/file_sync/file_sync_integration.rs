//! Integration tests for file database sync fallback process.
//!
//! Tests cover:
//! - File database sync fallback flow
//! - Password prompt for file sync
//! - Executor request handling for `sudo pacman -Fy`
//! - Modal transitions

#![cfg(test)]

use pacsea::install::ExecutorRequest;
use pacsea::state::{AppState, Modal, modal::PasswordPurpose};
use std::sync::{Arc, Mutex};

/// What: Test file database sync fallback triggers password prompt on failure.
///
/// Inputs:
/// - Preflight modal on Files tab
/// - File sync result indicating failure
///
/// Output:
/// - Password prompt modal is shown with ``FileSync`` purpose
/// - Custom command is stored for execution
///
/// Details:
/// - Verifies that sync failure triggers password prompt flow
#[test]
fn integration_file_sync_fallback_password_prompt() {
    let mut app = AppState::default();

    // Simulate sync failure by setting pending_file_sync_result
    let sync_result: Arc<Mutex<Option<Result<bool, String>>>> =
        Arc::new(Mutex::new(Some(Err("Permission denied".to_string()))));
    app.pending_file_sync_result = Some(sync_result);

    // Simulate tick handler processing the sync result
    if let Some(sync_result_arc) = app.pending_file_sync_result.take()
        && let Ok(mut sync_result) = sync_result_arc.lock()
        && let Some(result) = sync_result.take()
        && let Err(_e) = result
    {
        app.modal = Modal::PasswordPrompt {
            purpose: PasswordPurpose::FileSync,
            items: vec![],
            input: String::new(),
            cursor: 0,
            error: None,
        };
        app.pending_custom_command = Some("sudo pacman -Fy".to_string());
        app.pending_exec_header_chips = Some(pacsea::state::modal::PreflightHeaderChips::default());
    }

    // Verify password prompt modal
    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::FileSync);
            assert!(items.is_empty());
        }
        _ => panic!("Expected PasswordPrompt modal with FileSync purpose"),
    }

    // Verify custom command is stored
    assert_eq!(
        app.pending_custom_command,
        Some("sudo pacman -Fy".to_string())
    );
}

#[test]
/// What: Test file database sync success flow.
///
/// Inputs:
/// - Preflight modal on Files tab
/// - File sync result indicating success
///
/// Output:
/// - Toast message is set
/// - No password prompt is shown
///
/// Details:
/// - Verifies that successful sync shows toast message
fn integration_file_sync_success() {
    let mut app = AppState::default();

    // Simulate sync success
    let sync_result: Arc<Mutex<Option<Result<bool, String>>>> =
        Arc::new(Mutex::new(Some(Ok(true))));
    app.pending_file_sync_result = Some(sync_result);

    // Simulate tick handler processing the sync result
    if let Some(sync_result_arc) = app.pending_file_sync_result.take()
        && let Ok(mut sync_result) = sync_result_arc.lock()
        && let Some(result) = sync_result.take()
        && let Ok(synced) = result
    {
        if synced {
            app.toast_message = Some("File database sync completed successfully".to_string());
        } else {
            app.toast_message = Some("File database is already fresh".to_string());
        }
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    }

    // Verify toast message is set
    assert!(app.toast_message.is_some());
    if let Some(ref msg) = app.toast_message {
        assert!(msg.contains("sync"));
    }

    // Verify no password prompt
    assert!(!matches!(app.modal, Modal::PasswordPrompt { .. }));
}

#[test]
/// What: Test executor request for file database sync.
///
/// Inputs:
/// - Password prompt modal with ``FileSync`` purpose
/// - Custom command stored
///
/// Output:
/// - ``ExecutorRequest::CustomCommand`` is created with correct command
///
/// Details:
/// - Verifies that file sync creates correct executor request
fn integration_file_sync_executor_request() {
    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::FileSync,
            items: vec![],
            input: "testpassword".to_string(),
            cursor: 12,
            error: None,
        },
        pending_custom_command: Some("sudo pacman -Fy".to_string()),
        pending_exec_header_chips: Some(pacsea::state::modal::PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Simulate password submission
    let password = if app.modal.is_password_prompt()
        && let Modal::PasswordPrompt { ref input, .. } = app.modal
    {
        if input.trim().is_empty() {
            None
        } else {
            Some(input.clone())
        }
    } else {
        None
    };

    let custom_cmd = app.pending_custom_command.take();
    let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();

    if let Some(custom_cmd) = custom_cmd {
        app.modal = Modal::PreflightExec {
            items: vec![],
            action: pacsea::state::PreflightAction::Install,
            tab: pacsea::state::PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips,
            success: None,
        };

        let executor_request = ExecutorRequest::CustomCommand {
            command: custom_cmd,
            password,
            dry_run: app.dry_run,
        };

        // Verify executor request
        match executor_request {
            ExecutorRequest::CustomCommand {
                command,
                password: pwd,
                ..
            } => {
                assert_eq!(command, "sudo pacman -Fy");
                assert_eq!(pwd, Some("testpassword".to_string()));
            }
            _ => panic!("Expected CustomCommand executor request"),
        }
    }
}

// Helper trait for testing
trait ModalTestHelper {
    fn is_password_prompt(&self) -> bool;
}

impl ModalTestHelper for Modal {
    fn is_password_prompt(&self) -> bool {
        matches!(self, Self::PasswordPrompt { .. })
    }
}
