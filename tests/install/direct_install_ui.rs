//! UI tests for direct install/remove modals.
//!
//! Tests cover:
//! - ``PasswordPrompt`` modal structure for direct install/remove
//! - ``PreflightExec`` modal structure for direct operations
//! - Modal state transitions

#![cfg(test)]

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
/// What: Test ``PasswordPrompt`` modal structure for direct install.
///
/// Inputs:
/// - ``PasswordPrompt`` modal with Install purpose and packages.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies ``PasswordPrompt`` modal can be created for direct install.
fn ui_direct_install_password_prompt_structure() {
    let items = vec![create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Install,
            items,
            input: String::new(),
            cursor: 0,
            error: None,
        },
        pending_exec_header_chips: Some(pacsea::state::modal::PreflightHeaderChips::default()),
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
            assert_eq!(modal_items[0].name, "ripgrep");
            assert!(input.is_empty());
            assert_eq!(cursor, 0);
            assert!(error.is_none());
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test ``PreflightExec`` modal structure for direct install (AUR package).
///
/// Inputs:
/// - ``PreflightExec`` modal for direct install of AUR package.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies ``PreflightExec`` modal can be created for direct install.
fn ui_direct_install_preflight_exec_structure() {
    let items = vec![create_test_package("yay-bin", Source::Aur)];

    let app = AppState {
        modal: Modal::PreflightExec {
            items: items.clone(),
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: pacsea::state::modal::PreflightHeaderChips::default(),
        },
        pending_executor_request: Some(pacsea::install::ExecutorRequest::Install {
            items,
            password: None,
            dry_run: false,
        }),
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec {
            items: modal_items,
            action,
            tab,
            verbose,
            log_lines,
            abortable,
            ..
        } => {
            assert_eq!(modal_items.len(), 1);
            assert_eq!(modal_items[0].name, "yay-bin");
            assert_eq!(action, PreflightAction::Install);
            assert_eq!(tab, PreflightTab::Summary);
            assert!(!verbose);
            assert!(log_lines.is_empty());
            assert!(!abortable);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test ``PasswordPrompt`` modal structure for direct remove.
///
/// Inputs:
/// - ``PasswordPrompt`` modal with Remove purpose and packages.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies ``PasswordPrompt`` modal can be created for direct remove.
fn ui_direct_remove_password_prompt_structure() {
    let items = vec![
        create_test_package(
            "test-package-1",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package(
            "test-package-2",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
    ];

    let app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Remove,
            items,
            input: String::new(),
            cursor: 0,
            error: None,
        },
        remove_cascade_mode: CascadeMode::Basic,
        pending_exec_header_chips: Some(pacsea::state::modal::PreflightHeaderChips::default()),
        ..Default::default()
    };

    match app.modal {
        Modal::PasswordPrompt {
            purpose,
            items: modal_items,
            ..
        } => {
            assert_eq!(purpose, PasswordPurpose::Remove);
            assert_eq!(modal_items.len(), 2);
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }

    assert_eq!(app.remove_cascade_mode, CascadeMode::Basic);
}

#[test]
/// What: Test modal transition from ``PasswordPrompt`` to ``PreflightExec`` for direct install.
///
/// Inputs:
/// - ``PasswordPrompt`` modal with Install purpose and password entered.
///
/// Output:
/// - Modal transitions to ``PreflightExec``.
/// - Executor request is created.
///
/// Details:
/// - Verifies modal state transition flow for direct install.
fn ui_direct_install_modal_transition() {
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
    };

    app.pending_executor_request = Some(pacsea::install::ExecutorRequest::Install {
        items,
        password,
        dry_run: app.dry_run,
    });

    // Verify transition to PreflightExec
    assert!(matches!(app.modal, Modal::PreflightExec { .. }));
    assert!(app.pending_executor_request.is_some());
}

#[test]
/// What: Test modal transition from ``PasswordPrompt`` to ``PreflightExec`` for direct remove.
///
/// Inputs:
/// - ``PasswordPrompt`` modal with Remove purpose and password entered.
///
/// Output:
/// - Modal transitions to ``PreflightExec``.
/// - Executor request is created with cascade mode.
///
/// Details:
/// - Verifies modal state transition flow for direct remove.
fn ui_direct_remove_modal_transition() {
    let items = vec![create_test_package(
        "test-package",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Remove,
            items: items.clone(),
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
    };

    app.pending_executor_request = Some(pacsea::install::ExecutorRequest::Remove {
        names,
        password,
        cascade,
        dry_run: app.dry_run,
    });

    // Verify transition to PreflightExec
    assert!(matches!(app.modal, Modal::PreflightExec { .. }));
    match app.pending_executor_request {
        Some(pacsea::install::ExecutorRequest::Remove { cascade, .. }) => {
            assert_eq!(cascade, CascadeMode::Cascade);
        }
        _ => panic!("Expected Remove executor request"),
    }
}
