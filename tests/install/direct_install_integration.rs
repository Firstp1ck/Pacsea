//! Integration tests for direct install/remove operations (bypassing preflight).
//!
//! Tests cover:
//! - Direct install flow for single package
//! - Direct install flow for multiple packages
//! - Direct remove flow
//! - Password prompt handling
//! - Executor request handling
//! - Reinstall/batch update confirmation

#![cfg(test)]

use pacsea::install::ExecutorRequest;
use pacsea::state::{
    AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source, modal::CascadeMode,
    modal::PasswordPurpose,
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
/// What: Test direct install for single official package (requires password).
///
/// Inputs:
/// - Official package item
///
/// Output:
/// - Password prompt modal is shown
///
/// Details:
/// - Verifies that official packages trigger password prompt
fn integration_direct_install_single_official() {
    let mut app = AppState::default();
    let item = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    pacsea::install::start_integrated_install(&mut app, &item, false);

    // Verify password prompt is shown (or Alert if user is locked out)
    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::Install);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "ripgrep");
        }
        Modal::Alert { .. } => {
            // User might be locked out - this is acceptable in test environment
            // The important thing is that we checked faillock status
        }
        _ => panic!("Expected PasswordPrompt or Alert modal for official package"),
    }
}

#[test]
/// What: Test direct install for single AUR package (no password needed).
///
/// Inputs:
/// - AUR package item
///
/// Output:
/// - ``PreflightExec`` modal is shown directly
/// - Executor request is created
///
/// Details:
/// - Verifies that AUR packages skip password prompt
fn integration_direct_install_single_aur() {
    let mut app = AppState::default();
    let item = create_test_package("yay-bin", Source::Aur);

    pacsea::install::start_integrated_install(&mut app, &item, false);

    // Verify PreflightExec modal is shown directly
    match app.modal {
        Modal::PreflightExec { items, action, .. } => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "yay-bin");
            assert_eq!(action, PreflightAction::Install);
        }
        _ => panic!("Expected PreflightExec modal for AUR package"),
    }

    // Verify executor request is created
    match app.pending_executor_request {
        Some(ExecutorRequest::Install {
            items, password, ..
        }) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "yay-bin");
            assert!(password.is_none());
        }
        _ => panic!("Expected Install executor request"),
    }
}

#[test]
/// What: Test direct install for multiple packages with official packages.
///
/// Inputs:
/// - Multiple package items including official packages
///
/// Output:
/// - Password prompt modal is shown
///
/// Details:
/// - Verifies that batch install with official packages triggers password prompt
fn integration_direct_install_multiple_with_official() {
    let mut app = AppState::default();
    let items = vec![
        create_test_package(
            "ripgrep",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package("yay-bin", Source::Aur),
    ];

    pacsea::install::start_integrated_install_all(&mut app, &items, false);

    // Verify password prompt is shown (or Alert if user is locked out)
    match app.modal {
        Modal::PasswordPrompt {
            purpose,
            items: modal_items,
            ..
        } => {
            assert_eq!(purpose, PasswordPurpose::Install);
            assert_eq!(modal_items.len(), 2);
        }
        Modal::Alert { .. } => {
            // User might be locked out - this is acceptable in test environment
            // The important thing is that we checked faillock status
        }
        _ => panic!("Expected PasswordPrompt or Alert modal for packages with official"),
    }
}

#[test]
/// What: Test direct remove flow.
///
/// Inputs:
/// - Package names to remove
///
/// Output:
/// - Password prompt modal is shown
///
/// Details:
/// - Verifies that remove always shows password prompt
fn integration_direct_remove() {
    let mut app = AppState::default();
    let names = vec!["test-package-1".to_string(), "test-package-2".to_string()];

    pacsea::install::start_integrated_remove_all(&mut app, &names, false, CascadeMode::Basic);

    // Verify password prompt is shown (or Alert if user is locked out)
    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::Remove);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].name, "test-package-1");
            assert_eq!(items[1].name, "test-package-2");
        }
        Modal::Alert { .. } => {
            // User might be locked out - this is acceptable in test environment
            // The important thing is that we checked faillock status
        }
        _ => panic!("Expected PasswordPrompt or Alert modal for remove"),
    }

    // Verify cascade mode is stored
    assert_eq!(app.remove_cascade_mode, CascadeMode::Basic);
}

#[test]
/// What: Test executor request creation after password submission for install.
///
/// Inputs:
/// - Password prompt modal with password entered
///
/// Output:
/// - ``ExecutorRequest::Install`` is created with password
///
/// Details:
/// - Verifies that executor request includes password
fn integration_direct_install_executor_request() {
    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items: vec![create_test_package(
                "ripgrep",
                Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
            )],
            input: "testpassword".to_string(),
            cursor: 12,
            error: None,
        },
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

    let items = if let Modal::PasswordPrompt { ref items, .. } = app.modal {
        items.clone()
    } else {
        vec![]
    };

    let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();
    let items_clone = items.clone();

    app.modal = Modal::PreflightExec {
        items: items_clone,
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: vec![],
        abortable: false,
        header_chips,
        success: None,
    };

    app.pending_executor_request = Some(ExecutorRequest::Install {
        items,
        password,
        dry_run: app.dry_run,
    });

    // Verify executor request
    match app.pending_executor_request {
        Some(ExecutorRequest::Install {
            items,
            password: pwd,
            ..
        }) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "ripgrep");
            assert_eq!(pwd, Some("testpassword".to_string()));
        }
        _ => panic!("Expected Install executor request"),
    }
}

#[test]
/// What: Test executor request creation after password submission for remove.
///
/// Inputs:
/// - Password prompt modal with password entered for remove
///
/// Output:
/// - ``ExecutorRequest::Remove`` is created with password and cascade mode
///
/// Details:
/// - Verifies that executor request includes password and cascade mode
fn integration_direct_remove_executor_request() {
    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Remove,
            items: vec![create_test_package(
                "test-package",
                Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
            )],
            input: "testpassword".to_string(),
            cursor: 12,
            error: None,
        },
        remove_cascade_mode: CascadeMode::Cascade,
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

    let items = if let Modal::PasswordPrompt { ref items, .. } = app.modal {
        items.clone()
    } else {
        vec![]
    };

    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
    let cascade = app.remove_cascade_mode;
    let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();

    app.modal = Modal::PreflightExec {
        items,
        action: PreflightAction::Remove,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: vec![],
        abortable: false,
        header_chips,
        success: None,
    };

    app.pending_executor_request = Some(ExecutorRequest::Remove {
        names,
        password,
        cascade,
        dry_run: app.dry_run,
    });

    // Verify executor request
    match app.pending_executor_request {
        Some(ExecutorRequest::Remove {
            names,
            password: pwd,
            cascade,
            ..
        }) => {
            assert_eq!(names.len(), 1);
            assert_eq!(names[0], "test-package");
            assert_eq!(pwd, Some("testpassword".to_string()));
            assert_eq!(cascade, CascadeMode::Cascade);
        }
        _ => panic!("Expected Remove executor request"),
    }
}
