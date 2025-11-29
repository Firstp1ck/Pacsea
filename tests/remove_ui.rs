//! UI tests for remove process modals.
//!
//! Tests cover:
//! - Remove modal rendering structure
//! - `PreflightExec` modal for remove
//! - `ConfirmRemove` modal structure
//!
//! Note: These tests verify modal state structure rather than actual rendering.

#![cfg(test)]

// CascadeMode is imported but not used in tests
// use pacsea::state::modal::CascadeMode;
use pacsea::state::{AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source};

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
/// What: Test `PreflightExec` modal structure for remove action.
///
/// Inputs:
/// - `PreflightExec` modal with remove action.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies `PreflightExec` modal can be created for remove operations.
fn ui_preflight_exec_remove_rendering() {
    let mut app = AppState::default();
    let items = vec![create_test_package(
        "old-pkg1",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    app.modal = Modal::PreflightExec {
        items,
        action: PreflightAction::Remove,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: Vec::new(),
        abortable: false,
        header_chips: pacsea::state::modal::PreflightHeaderChips {
            package_count: 1,
            download_bytes: 0,
            install_delta_bytes: -1000,
            aur_count: 0,
            risk_score: 0,
            risk_level: pacsea::state::modal::RiskLevel::Low,
        },
    };

    match app.modal {
        Modal::PreflightExec {
            items: ref exec_items,
            action,
            tab,
            verbose,
            log_lines,
            abortable,
            ..
        } => {
            assert_eq!(exec_items.len(), 1);
            assert_eq!(action, PreflightAction::Remove);
            assert_eq!(tab, PreflightTab::Summary);
            assert!(!verbose);
            assert!(log_lines.is_empty());
            assert!(!abortable);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `ConfirmRemove` modal structure.
///
/// Inputs:
/// - `ConfirmRemove` modal with packages.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies `ConfirmRemove` modal can be created.
fn ui_confirm_remove_modal_rendering() {
    let mut app = AppState::default();
    let items = vec![
        create_test_package(
            "old-pkg1",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package(
            "old-pkg2",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
    ];

    app.modal = Modal::ConfirmRemove { items };

    match app.modal {
        Modal::ConfirmRemove {
            items: ref modal_items,
        } => {
            assert_eq!(modal_items.len(), 2);
            assert_eq!(modal_items[0].name, "old-pkg1");
            assert_eq!(modal_items[1].name, "old-pkg2");
        }
        _ => panic!("Expected ConfirmRemove modal"),
    }
}
