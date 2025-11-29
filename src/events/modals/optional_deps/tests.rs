//! Unit tests for optional dependencies modal handlers.

use crate::state::{AppState, types::OptionalDepRow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::events::modals::optional_deps::handle_optional_deps;

/// What: Create a test optional dependency row.
///
/// Inputs:
/// - `package`: Package name
/// - `installed`: Whether package is installed
/// - `selectable`: Whether row is selectable
///
/// Output:
/// - `OptionalDepRow` ready for testing
///
/// Details:
/// - Helper to create test optional dependency rows
fn create_test_row(package: &str, installed: bool, selectable: bool) -> OptionalDepRow {
    OptionalDepRow {
        label: format!("Test: {package}"),
        package: package.into(),
        installed,
        selectable,
        note: None,
    }
}

#[test]
/// What: Verify `OptionalDeps` modal handles Esc to close.
///
/// Inputs:
/// - `OptionalDeps` modal, Esc key event.
///
/// Output:
/// - Modal is closed.
///
/// Details:
/// - Tests that Esc closes the `OptionalDeps` modal.
fn optional_deps_esc_closes_modal() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("test-pkg", false, true)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
    };

    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let result = handle_optional_deps(ke, &mut app, &rows, &mut selected);

    match app.modal {
        crate::state::Modal::None => {}
        _ => panic!("Expected modal to be closed"),
    }
    assert_eq!(result, Some(false));
}

#[test]
/// What: Verify `OptionalDeps` modal handles navigation.
///
/// Inputs:
/// - `OptionalDeps` modal, Down key event.
///
/// Output:
/// - Selection moves down.
///
/// Details:
/// - Tests that navigation keys work in `OptionalDeps` modal.
fn optional_deps_navigation() {
    let mut app = AppState::default();
    let rows = vec![
        create_test_row("pkg1", false, true),
        create_test_row("pkg2", false, true),
    ];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
    };

    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let _ = handle_optional_deps(ke, &mut app, &rows, &mut selected);

    assert_eq!(selected, 1, "Selection should move down");
}

#[test]
/// What: Verify `OptionalDeps` modal handles Enter to install.
///
/// Inputs:
/// - `OptionalDeps` modal with selectable row, Enter key event.
///
/// Output:
/// - Installation is executed (spawns terminal - will fail in test environment).
///
/// Details:
/// - Tests that Enter triggers optional dependency installation.
/// - Note: This will spawn a terminal, so it's expected to fail in test environment.
fn optional_deps_enter_installs() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("test-pkg", false, true)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
    };

    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let result = handle_optional_deps(ke, &mut app, &rows, &mut selected);

    // Should return Some(true) when installation is triggered
    assert_eq!(result, Some(true));
    // Modal should transition to PreflightExec after installation is triggered
    match app.modal {
        crate::state::Modal::PreflightExec { .. } => {
            // Expected - installation now uses executor pattern and transitions to PreflightExec
        }
        _ => panic!("Expected modal to transition to PreflightExec after installation"),
    }
    // Verify that pending_executor_request is set
    assert!(
        app.pending_executor_request.is_some(),
        "Optional deps installation should set pending_executor_request"
    );
}

#[test]
/// What: Verify `OptionalDeps` modal Enter on installed package does nothing.
///
/// Inputs:
/// - `OptionalDeps` modal with installed (non-selectable) row, Enter key event.
///
/// Output:
/// - No action taken, modal closes.
///
/// Details:
/// - Tests that Enter on installed packages doesn't trigger installation.
/// - The modal may close or remain open depending on implementation.
fn optional_deps_enter_installed_does_nothing() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("test-pkg", true, false)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
    };

    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let result = handle_optional_deps(ke, &mut app, &rows, &mut selected);

    // Should return Some(false) when no action is taken
    assert_eq!(result, Some(false));
    // Modal may close or remain OptionalDeps - both are acceptable
    match app.modal {
        crate::state::Modal::OptionalDeps { .. } | crate::state::Modal::None => {}
        _ => panic!("Expected modal to remain OptionalDeps or close"),
    }
}

#[test]
/// What: Verify `OptionalDeps` modal Enter on virustotal-setup opens setup modal.
///
/// Inputs:
/// - `OptionalDeps` modal with virustotal-setup row, Enter key event.
///
/// Output:
/// - `VirusTotalSetup` modal is opened.
///
/// Details:
/// - Tests that Enter on virustotal-setup opens the setup modal.
fn optional_deps_enter_virustotal_setup() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("virustotal-setup", false, true)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
    };

    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let _ = handle_optional_deps(ke, &mut app, &rows, &mut selected);

    match app.modal {
        crate::state::Modal::VirusTotalSetup { .. } => {}
        _ => panic!("Expected VirusTotalSetup modal"),
    }
}
