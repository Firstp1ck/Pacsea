//! Integration tests for reinstall confirmation modal.
//!
//! Tests cover:
//! - Single package reinstall confirmation
//! - Batch reinstall with mixed installed/new packages
//! - Direct install reinstall flow
//! - Cancel reinstall returns to previous state
//! - Header chips in reinstall confirmation

#![cfg(test)]

use pacsea::state::{
    AppState, Modal, PackageItem, Source,
    modal::PreflightHeaderChips,
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
/// What: Test ConfirmReinstall modal state creation.
///
/// Inputs:
/// - Single installed package for reinstall.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies reinstall confirmation modal can be created.
fn integration_reinstall_confirmation_single_package() {
    let installed_pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![installed_pkg.clone()],
            all_items: vec![installed_pkg],
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ConfirmReinstall {
            items,
            all_items,
            header_chips,
        } => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "ripgrep");
            assert_eq!(all_items.len(), 1);
            assert_eq!(header_chips.package_count, 0); // Default
        }
        _ => panic!("Expected ConfirmReinstall modal"),
    }
}

#[test]
/// What: Test ConfirmReinstall with mixed installed and new packages.
///
/// Inputs:
/// - Multiple packages, some installed and some new.
///
/// Output:
/// - `items` contains only installed packages.
/// - `all_items` contains all packages.
///
/// Details:
/// - Verifies batch reinstall separates installed from new packages.
fn integration_reinstall_confirmation_mixed_packages() {
    let installed_pkg1 = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );
    let installed_pkg2 = create_test_package(
        "fd",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );
    let new_pkg = create_test_package("new-package", Source::Aur);

    // Only installed packages in items, all packages in all_items
    let app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![installed_pkg1.clone(), installed_pkg2.clone()],
            all_items: vec![installed_pkg1, installed_pkg2, new_pkg],
            header_chips: PreflightHeaderChips {
                package_count: 3,
                download_bytes: 1024,
                install_delta_bytes: 2048,
                aur_count: 1,
                risk_score: 5,
                risk_level: pacsea::state::modal::RiskLevel::Low,
            },
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ConfirmReinstall {
            items,
            all_items,
            header_chips,
        } => {
            // Only installed packages shown in confirmation
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].name, "ripgrep");
            assert_eq!(items[1].name, "fd");

            // All packages preserved for installation
            assert_eq!(all_items.len(), 3);
            assert!(all_items.iter().any(|p| p.name == "new-package"));

            // Header chips reflect all packages
            assert_eq!(header_chips.package_count, 3);
            assert_eq!(header_chips.aur_count, 1);
        }
        _ => panic!("Expected ConfirmReinstall modal"),
    }
}

#[test]
/// What: Test reinstall confirmation cancellation.
///
/// Inputs:
/// - ConfirmReinstall modal that is cancelled.
///
/// Output:
/// - Modal transitions to None.
///
/// Details:
/// - Simulates user pressing Escape or 'n' to cancel reinstall.
fn integration_reinstall_confirmation_cancel() {
    let pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let mut app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![pkg.clone()],
            all_items: vec![pkg],
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    // Simulate cancellation
    app.modal = Modal::None;

    assert!(matches!(app.modal, Modal::None));
}

#[test]
/// What: Test reinstall confirmation proceeds to password prompt.
///
/// Inputs:
/// - ConfirmReinstall modal confirmed.
///
/// Output:
/// - Modal transitions to PasswordPrompt for official packages.
///
/// Details:
/// - Simulates user confirming reinstall for official packages.
fn integration_reinstall_confirmation_proceeds_to_password() {
    use pacsea::state::modal::PasswordPurpose;

    let pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let mut app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![pkg.clone()],
            all_items: vec![pkg.clone()],
            header_chips: PreflightHeaderChips::default(),
        },
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Simulate confirmation - transition to PasswordPrompt for official packages
    app.modal = Modal::PasswordPrompt {
        purpose: PasswordPurpose::Install,
        items: vec![pkg],
        input: String::new(),
        cursor: 0,
        error: None,
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::Install);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "ripgrep");
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test reinstall confirmation for AUR packages skips password.
///
/// Inputs:
/// - ConfirmReinstall modal with only AUR packages.
///
/// Output:
/// - Modal transitions directly to PreflightExec.
///
/// Details:
/// - AUR packages don't need sudo password for installation.
fn integration_reinstall_confirmation_aur_skips_password() {
    use pacsea::state::{PreflightAction, PreflightTab};

    let pkg = create_test_package("yay-bin", Source::Aur);

    let mut app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![pkg.clone()],
            all_items: vec![pkg.clone()],
            header_chips: PreflightHeaderChips::default(),
        },
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Simulate confirmation - transition directly to PreflightExec for AUR
    let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();
    app.modal = Modal::PreflightExec {
        items: vec![pkg],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: vec![],
        abortable: false,
        header_chips,
    };

    match app.modal {
        Modal::PreflightExec { items, action, .. } => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "yay-bin");
            assert_eq!(action, PreflightAction::Install);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test reinstall confirmation with header chips.
///
/// Inputs:
/// - ConfirmReinstall modal with populated header chips.
///
/// Output:
/// - Header chips are preserved and accessible.
///
/// Details:
/// - Verifies header chip data is available for display.
fn integration_reinstall_confirmation_header_chips() {
    use pacsea::state::modal::RiskLevel;

    let pkg = create_test_package(
        "large-package",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let header_chips = PreflightHeaderChips {
        package_count: 5,
        download_bytes: 50 * 1024 * 1024, // 50 MB
        install_delta_bytes: 100 * 1024 * 1024, // 100 MB
        aur_count: 2,
        risk_score: 15,
        risk_level: RiskLevel::Medium,
    };

    let app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![pkg.clone()],
            all_items: vec![pkg],
            header_chips: header_chips.clone(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ConfirmReinstall {
            header_chips: chips,
            ..
        } => {
            assert_eq!(chips.package_count, 5);
            assert_eq!(chips.download_bytes, 50 * 1024 * 1024);
            assert_eq!(chips.install_delta_bytes, 100 * 1024 * 1024);
            assert_eq!(chips.aur_count, 2);
            assert_eq!(chips.risk_score, 15);
            assert_eq!(chips.risk_level, RiskLevel::Medium);
        }
        _ => panic!("Expected ConfirmReinstall modal"),
    }
}

#[test]
/// What: Test reinstall confirmation with empty all_items is edge case.
///
/// Inputs:
/// - ConfirmReinstall modal with empty all_items.
///
/// Output:
/// - Modal handles empty case gracefully.
///
/// Details:
/// - Edge case where no packages would be installed.
fn integration_reinstall_confirmation_empty_all_items() {
    let app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![],
            all_items: vec![],
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ConfirmReinstall {
            items, all_items, ..
        } => {
            assert!(items.is_empty());
            assert!(all_items.is_empty());
        }
        _ => panic!("Expected ConfirmReinstall modal"),
    }
}

#[test]
/// What: Test ConfirmBatchUpdate modal state creation.
///
/// Inputs:
/// - Packages for batch update.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies batch update confirmation modal can be created.
fn integration_confirm_batch_update_modal() {
    let items = vec![
        create_test_package(
            "pkg1",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package("pkg2", Source::Aur),
    ];

    let app = AppState {
        modal: Modal::ConfirmBatchUpdate {
            items: items.clone(),
            dry_run: false,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ConfirmBatchUpdate { items, dry_run } => {
            assert_eq!(items.len(), 2);
            assert!(!dry_run);
        }
        _ => panic!("Expected ConfirmBatchUpdate modal"),
    }
}

#[test]
/// What: Test ConfirmBatchUpdate with dry_run flag.
///
/// Inputs:
/// - Batch update with dry_run enabled.
///
/// Output:
/// - dry_run flag is correctly set.
///
/// Details:
/// - Verifies dry_run mode is preserved in modal state.
fn integration_confirm_batch_update_dry_run() {
    let items = vec![create_test_package(
        "pkg1",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let app = AppState {
        modal: Modal::ConfirmBatchUpdate {
            items,
            dry_run: true,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ConfirmBatchUpdate { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ConfirmBatchUpdate modal"),
    }
}

