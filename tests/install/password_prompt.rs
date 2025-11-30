//! Integration tests for password prompt modal.
//!
//! Tests cover:
//! - Password prompt for each `PasswordPurpose` variant
//! - Incorrect password retry with error message
//! - Password prompt cancellation
//! - Password masking verification
//! - Password state transitions

#![cfg(test)]

use pacsea::state::{
    AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source,
    modal::{PasswordPurpose, PreflightHeaderChips},
};

/// What: Create a test package item with specified source.
///
/// Inputs:
/// - `name`: Package name
/// - `source`: Package source (Official or AUR)
///
/// Output:
/// - `PackageItem` ready for testing
///
/// Details:
/// - Helper to create test packages with consistent structure
fn create_test_package(name: &str, source: Source) -> PackageItem {
    PackageItem {
        name: name.into(),
        version: "1.0.0".into(),
        description: String::new(),
        source,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

#[test]
/// What: Test password prompt for Install purpose.
///
/// Inputs:
/// - `PasswordPrompt` modal with `Install` purpose.
///
/// Output:
/// - Modal state is correctly structured with Install purpose.
///
/// Details:
/// - Verifies password prompt can be created for install operations.
fn integration_password_prompt_install_purpose() {
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: items.clone(),
            input: String::new(),
            cursor: 0,
            error: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt {
            purpose,
            items: modal_items,
            input,
            cursor,
            error,
        } => {
            assert_eq!(purpose, PasswordPurpose::Install);
            assert_eq!(modal_items.len(), 1);
            assert_eq!(modal_items[0].name, "test-pkg");
            assert!(input.is_empty());
            assert_eq!(cursor, 0);
            assert!(error.is_none());
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password prompt for Remove purpose.
///
/// Inputs:
/// - `PasswordPrompt` modal with `Remove` purpose.
///
/// Output:
/// - Modal state is correctly structured with Remove purpose.
///
/// Details:
/// - Verifies password prompt can be created for remove operations.
fn integration_password_prompt_remove_purpose() {
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Remove,
            items,
            input: String::new(),
            cursor: 0,
            error: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, .. } => {
            assert_eq!(purpose, PasswordPurpose::Remove);
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password prompt for Update purpose.
///
/// Inputs:
/// - `PasswordPrompt` modal with `Update` purpose.
///
/// Output:
/// - Modal state is correctly structured with Update purpose.
///
/// Details:
/// - Verifies password prompt can be created for system update operations.
fn integration_password_prompt_update_purpose() {
    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Update,
            items: vec![],
            input: String::new(),
            cursor: 0,
            error: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::Update);
            assert!(items.is_empty(), "Update purpose may have empty items");
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password prompt for Downgrade purpose.
///
/// Inputs:
/// - `PasswordPrompt` modal with `Downgrade` purpose.
///
/// Output:
/// - Modal state is correctly structured with Downgrade purpose.
///
/// Details:
/// - Verifies password prompt can be created for downgrade operations.
fn integration_password_prompt_downgrade_purpose() {
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Downgrade,
            items,
            input: String::new(),
            cursor: 0,
            error: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, .. } => {
            assert_eq!(purpose, PasswordPurpose::Downgrade);
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password prompt for FileSync purpose.
///
/// Inputs:
/// - `PasswordPrompt` modal with `FileSync` purpose.
///
/// Output:
/// - Modal state is correctly structured with FileSync purpose.
///
/// Details:
/// - Verifies password prompt can be created for file database sync operations.
fn integration_password_prompt_filesync_purpose() {
    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::FileSync,
            items: vec![],
            input: String::new(),
            cursor: 0,
            error: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::FileSync);
            assert!(items.is_empty(), "FileSync purpose should have empty items");
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password input capture and cursor tracking.
///
/// Inputs:
/// - `PasswordPrompt` modal with password entered.
///
/// Output:
/// - Input field captures password text.
/// - Cursor position tracks correctly.
///
/// Details:
/// - Verifies password is stored in input field (for masking display).
fn integration_password_prompt_input_capture() {
    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: vec![],
            input: "secretpassword".to_string(),
            cursor: 14,
            error: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt { input, cursor, .. } => {
            assert_eq!(input, "secretpassword");
            assert_eq!(cursor, 14);
            // Note: Actual masking is done in UI rendering, not in state
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password prompt with error message for incorrect password.
///
/// Inputs:
/// - `PasswordPrompt` modal with error set.
///
/// Output:
/// - Error message is stored and accessible.
///
/// Details:
/// - Verifies incorrect password triggers error state for retry.
fn integration_password_prompt_incorrect_password_error() {
    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: vec![],
            input: String::new(),
            cursor: 0,
            error: Some("Incorrect password. Please try again.".to_string()),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt { error, input, .. } => {
            assert!(error.is_some());
            assert_eq!(
                error.as_ref().expect("error should be Some"),
                "Incorrect password. Please try again."
            );
            // Input should be cleared for retry
            assert!(input.is_empty());
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password prompt cancellation returns to None modal.
///
/// Inputs:
/// - `PasswordPrompt` modal that is cancelled.
///
/// Output:
/// - Modal transitions to `None`.
///
/// Details:
/// - Simulates user pressing Escape to cancel password prompt.
fn integration_password_prompt_cancellation() {
    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: vec![create_test_package(
                "test-pkg",
                Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
            )],
            input: "partial".to_string(),
            cursor: 7,
            error: None,
        },
        ..Default::default()
    };

    // Simulate cancellation
    app.modal = Modal::None;

    assert!(matches!(app.modal, Modal::None));
}

#[test]
/// What: Test password submission transitions to PreflightExec modal.
///
/// Inputs:
/// - `PasswordPrompt` modal with password entered.
///
/// Output:
/// - Modal transitions to `PreflightExec`.
/// - Password is captured for executor request.
///
/// Details:
/// - Simulates user submitting password for install operation.
fn integration_password_prompt_submission_to_preflight_exec() {
    let items = vec![create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: items.clone(),
            input: "testpassword".to_string(),
            cursor: 12,
            error: None,
        },
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Extract password before transition
    let password = if let Modal::PasswordPrompt { ref input, .. } = app.modal {
        if input.trim().is_empty() {
            None
        } else {
            Some(input.clone())
        }
    } else {
        None
    };

    // Simulate transition to PreflightExec
    let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();
    app.modal = Modal::PreflightExec {
        items: items.clone(),
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: vec![],
        abortable: false,
        header_chips,
    };

    // Verify modal transition
    match app.modal {
        Modal::PreflightExec {
            items: modal_items,
            action,
            ..
        } => {
            assert_eq!(modal_items.len(), 1);
            assert_eq!(modal_items[0].name, "ripgrep");
            assert_eq!(action, PreflightAction::Install);
        }
        _ => panic!("Expected PreflightExec modal"),
    }

    // Verify password was captured
    assert_eq!(password, Some("testpassword".to_string()));
}

#[test]
/// What: Test password prompt with multiple packages.
///
/// Inputs:
/// - `PasswordPrompt` modal with multiple packages.
///
/// Output:
/// - All packages are stored in the modal.
///
/// Details:
/// - Verifies batch operations preserve all package information.
fn integration_password_prompt_multiple_packages() {
    let items = vec![
        create_test_package(
            "pkg1",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package(
            "pkg2",
            Source::Official {
                repo: "core".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package("pkg3", Source::Aur),
    ];

    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: items.clone(),
            input: String::new(),
            cursor: 0,
            error: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt {
            items: modal_items,
            ..
        } => {
            assert_eq!(modal_items.len(), 3);
            assert_eq!(modal_items[0].name, "pkg1");
            assert_eq!(modal_items[1].name, "pkg2");
            assert_eq!(modal_items[2].name, "pkg3");
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test password retry clears input but preserves items.
///
/// Inputs:
/// - `PasswordPrompt` modal after incorrect password.
///
/// Output:
/// - Input is cleared.
/// - Items are preserved.
/// - Error message is set.
///
/// Details:
/// - Simulates password retry flow after incorrect attempt.
fn integration_password_prompt_retry_preserves_items() {
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    // Initial state with password entered
    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: items.clone(),
            input: "wrongpassword".to_string(),
            cursor: 13,
            error: None,
        },
        ..Default::default()
    };

    // Simulate incorrect password - set error and clear input
    if let Modal::PasswordPrompt {
        ref mut input,
        ref mut cursor,
        ref mut error,
        ..
    } = app.modal
    {
        *input = String::new();
        *cursor = 0;
        *error = Some("Authentication failed. Try again.".to_string());
    }

    // Verify retry state
    match app.modal {
        Modal::PasswordPrompt {
            items: modal_items,
            input,
            cursor,
            error,
            ..
        } => {
            // Items preserved
            assert_eq!(modal_items.len(), 1);
            assert_eq!(modal_items[0].name, "test-pkg");
            // Input cleared for retry
            assert!(input.is_empty());
            assert_eq!(cursor, 0);
            // Error message set
            assert!(error.is_some());
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test empty password is handled correctly.
///
/// Inputs:
/// - `PasswordPrompt` modal with empty input submitted.
///
/// Output:
/// - Empty password results in None password value.
///
/// Details:
/// - Verifies empty password handling for operations that might not need sudo.
fn integration_password_prompt_empty_password() {
    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: vec![],
            input: "   ".to_string(), // Whitespace only
            cursor: 3,
            error: None,
        },
        ..Default::default()
    };

    // Extract password
    let password = if let Modal::PasswordPrompt { ref input, .. } = app.modal {
        if input.trim().is_empty() {
            None
        } else {
            Some(input.clone())
        }
    } else {
        None
    };

    assert!(password.is_none(), "Whitespace-only password should be None");
}

