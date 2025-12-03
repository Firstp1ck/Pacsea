//! Unit tests for install and remove modal handlers.

#![cfg(test)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::state::{AppState, PackageItem, Source};

use super::{handle_confirm_install, handle_confirm_remove};

/// What: Create a test package item with specified source.
///
/// Inputs:
/// - `name`: Package name
/// - `source`: Package source (Official or AUR)
///
/// Output:
/// - PackageItem ready for testing
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
/// What: Verify ConfirmInstall modal handles Esc to close.
///
/// Inputs:
/// - ConfirmInstall modal, Esc key event.
///
/// Output:
/// - Modal is closed.
///
/// Details:
/// - Tests that Esc closes the ConfirmInstall modal.
fn confirm_install_esc_closes_modal() {
    let mut app = AppState::default();
    let items = vec![create_test_package("test-pkg", Source::Aur)];
    app.modal = crate::state::Modal::ConfirmInstall {
        items: items.clone(),
    };

    let ke = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let _ = handle_confirm_install(ke, &mut app, &items);

    match app.modal {
        crate::state::Modal::None => {}
        _ => panic!("Expected modal to be closed"),
    }
}

#[test]
/// What: Verify ConfirmRemove modal handles Esc to cancel.
///
/// Inputs:
/// - ConfirmRemove modal, Esc key event.
///
/// Output:
/// - Modal is closed (removal cancelled).
///
/// Details:
/// - Tests that Esc cancels removal and closes modal.
fn confirm_remove_esc_cancels() {
    let mut app = AppState::default();
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];
    app.modal = crate::state::Modal::ConfirmRemove {
        items: items.clone(),
    };

    let ke = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let _ = handle_confirm_remove(ke, &mut app, &items);

    match app.modal {
        crate::state::Modal::None => {}
        _ => panic!("Expected modal to be closed"),
    }
}

#[test]
/// What: Verify ConfirmRemove modal requires explicit 'y' confirmation.
///
/// Inputs:
/// - ConfirmRemove modal, Enter key event (should cancel).
///
/// Output:
/// - Modal is closed (removal cancelled, Enter defaults to No).
///
/// Details:
/// - Tests that Enter cancels removal (defaults to No).
fn confirm_remove_enter_defaults_to_no() {
    let mut app = AppState::default();
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];
    app.modal = crate::state::Modal::ConfirmRemove {
        items: items.clone(),
    };

    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let _ = handle_confirm_remove(ke, &mut app, &items);

    match app.modal {
        crate::state::Modal::None => {}
        _ => panic!("Expected modal to be closed (Enter defaults to No)"),
    }
}

#[test]
/// What: Verify ConfirmRemove modal proceeds with 'y' confirmation.
///
/// Inputs:
/// - ConfirmRemove modal, 'y' key event.
///
/// Output:
/// - Removal is executed (spawns terminal - will fail in test environment).
///
/// Details:
/// - Tests that 'y' triggers removal execution.
/// - Note: This will spawn a terminal, so it's expected to fail in test environment.
fn confirm_remove_y_confirms() {
    let mut app = AppState::default();
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];
    app.modal = crate::state::Modal::ConfirmRemove {
        items: items.clone(),
    };
    app.remove_cascade_mode = crate::state::modal::CascadeMode::Basic;

    let ke = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty());
    let _ = handle_confirm_remove(ke, &mut app, &items);

    // Modal should be closed after removal is triggered
    match app.modal {
        crate::state::Modal::None => {}
        _ => panic!("Expected modal to be closed after removal"),
    }
    // Remove list should be cleared
    assert!(app.remove_list.is_empty());
}

#[test]
/// What: Verify ConfirmInstall modal handles Enter to install.
///
/// Inputs:
/// - ConfirmInstall modal, Enter key event.
///
/// Output:
/// - Install is executed (spawns terminal - will fail in test environment).
///
/// Details:
/// - Tests that Enter triggers install execution.
/// - Note: This will spawn a terminal, so it's expected to fail in test environment.
fn confirm_install_enter_triggers_install() {
    let mut app = AppState::default();
    let items = vec![create_test_package("test-pkg", Source::Aur)];
    app.modal = crate::state::Modal::ConfirmInstall {
        items: items.clone(),
    };

    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let _ = handle_confirm_install(ke, &mut app, &items);

    // Modal should be closed after install is triggered
    match app.modal {
        crate::state::Modal::None => {}
        _ => panic!("Expected modal to be closed after install"),
    }
}

#[test]
/// What: Verify ConfirmInstall modal handles 's' to open scan config.
///
/// Inputs:
/// - ConfirmInstall modal with AUR packages, 's' key event.
///
/// Output:
/// - ScanConfig modal is opened.
///
/// Details:
/// - Tests that 's' opens scan configuration for AUR packages.
fn confirm_install_s_opens_scan_config() {
    let mut app = AppState::default();
    let items = vec![create_test_package("test-pkg", Source::Aur)];
    app.modal = crate::state::Modal::ConfirmInstall {
        items: items.clone(),
    };

    let ke = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty());
    let _ = handle_confirm_install(ke, &mut app, &items);

    match app.modal {
        crate::state::Modal::ScanConfig { .. } => {}
        _ => panic!("Expected ScanConfig modal"),
    }
}

#[test]
/// What: Verify ConfirmInstall modal 's' shows alert for non-AUR packages.
///
/// Inputs:
/// - ConfirmInstall modal with only official packages, 's' key event.
///
/// Output:
/// - Alert modal is shown.
///
/// Details:
/// - Tests that 's' shows alert when no AUR packages are selected.
fn confirm_install_s_no_aur_shows_alert() {
    let mut app = AppState::default();
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];
    app.modal = crate::state::Modal::ConfirmInstall {
        items: items.clone(),
    };

    let ke = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::empty());
    let _ = handle_confirm_install(ke, &mut app, &items);

    match app.modal {
        crate::state::Modal::Alert { .. } => {}
        _ => panic!("Expected Alert modal for non-AUR packages"),
    }
}
