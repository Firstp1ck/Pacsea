//! UI tests for file database sync modals.
//!
//! Tests cover:
//! - ``PasswordPrompt`` modal structure for ``FileSync`` purpose
//! - Modal state transitions
//! - ``PreflightExec`` modal for file sync execution

#![cfg(test)]

use pacsea::state::{AppState, Modal, PreflightTab, modal::PasswordPurpose};

#[test]
/// What: Test ``PasswordPrompt`` modal structure for ``FileSync`` purpose.
///
/// Inputs:
/// - ``PasswordPrompt`` modal with ``FileSync`` purpose.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies ``PasswordPrompt`` modal can be created with ``FileSync`` purpose.
fn ui_file_sync_password_prompt_structure() {
    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::FileSync,
            items: vec![],
            input: String::new(),
            cursor: 0,
            error: None,
        },
        pending_custom_command: Some("sudo pacman -Fy".to_string()),
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt {
            purpose,
            items,
            input,
            cursor,
            error,
        } => {
            assert_eq!(purpose, PasswordPurpose::FileSync);
            assert!(items.is_empty());
            assert!(input.is_empty());
            assert_eq!(cursor, 0);
            assert!(error.is_none());
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }

    assert_eq!(
        app.pending_custom_command,
        Some("sudo pacman -Fy".to_string())
    );
}

#[test]
/// What: Test ``PreflightExec`` modal structure for file sync execution.
///
/// Inputs:
/// - ``PreflightExec`` modal after password submission for file sync.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies ``PreflightExec`` modal can be created for file sync execution.
fn ui_file_sync_preflight_exec_structure() {
    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![],
            action: pacsea::state::PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: pacsea::state::modal::PreflightHeaderChips::default(),
            success: None,
        },
        pending_executor_request: Some(pacsea::install::ExecutorRequest::CustomCommand {
            command: "sudo pacman -Fy".to_string(),
            password: Some("testpassword".to_string()),
            dry_run: false,
        }),
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec {
            items,
            action,
            tab,
            verbose,
            log_lines,
            abortable,
            ..
        } => {
            assert!(items.is_empty());
            assert_eq!(action, pacsea::state::PreflightAction::Install);
            assert_eq!(tab, PreflightTab::Summary);
            assert!(!verbose);
            assert!(log_lines.is_empty());
            assert!(!abortable);
        }
        _ => panic!("Expected PreflightExec modal"),
    }

    match app.pending_executor_request {
        Some(pacsea::install::ExecutorRequest::CustomCommand {
            command,
            password,
            dry_run,
        }) => {
            assert_eq!(command, "sudo pacman -Fy");
            assert_eq!(password, Some("testpassword".to_string()));
            assert!(!dry_run);
        }
        _ => panic!("Expected CustomCommand executor request"),
    }
}

#[test]
/// What: Test modal transition from ``PasswordPrompt`` to ``PreflightExec`` for file sync.
///
/// Inputs:
/// - ``PasswordPrompt`` modal with ``FileSync`` purpose and password entered.
///
/// Output:
/// - Modal transitions to ``PreflightExec``.
/// - Executor request is created.
///
/// Details:
/// - Verifies modal state transition flow for file sync.
fn ui_file_sync_modal_transition() {
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
    let password = if let Modal::PasswordPrompt { ref input, .. } = app.modal {
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
            success: None,
            items: vec![],
            action: pacsea::state::PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips,
        };

        app.pending_executor_request = Some(pacsea::install::ExecutorRequest::CustomCommand {
            command: custom_cmd,
            password,
            dry_run: app.dry_run,
        });
    }

    // Verify transition to PreflightExec
    assert!(matches!(app.modal, Modal::PreflightExec { .. }));
    assert!(app.pending_executor_request.is_some());
}
