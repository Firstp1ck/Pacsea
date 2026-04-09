//! Unit tests for optional dependencies modal handlers.

use crate::state::{AppState, types::OptionalDepRow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;

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
        selected_pkg_names: std::collections::HashSet::new(),
    };

    let mut selected = 0;
    let mut selected_pkg_names = std::collections::HashSet::new();
    let ke = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let result = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);

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
        selected_pkg_names: std::collections::HashSet::new(),
    };

    let mut selected = 0;
    let mut selected_pkg_names = std::collections::HashSet::new();
    let ke = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let _ = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);

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
        selected_pkg_names: std::collections::HashSet::new(),
    };

    let mut selected = 0;
    let mut selected_pkg_names = std::collections::HashSet::new();
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let result = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);

    // Install flow now delegates to preflight proceed logic.
    assert_eq!(result, Some(false));
    // Modal should transition to auth/preflight execution path.
    match app.modal {
        crate::state::Modal::PreflightExec { .. } | crate::state::Modal::PasswordPrompt { .. } => {}
        _ => panic!("Expected modal to transition to PreflightExec or PasswordPrompt"),
    }
}

#[test]
/// What: Verify `OptionalDeps` modal Enter on installed package shows reinstall confirmation.
///
/// Inputs:
/// - `OptionalDeps` modal with installed row, Enter key event.
///
/// Output:
/// - `ConfirmReinstall` modal is opened.
///
/// Details:
/// - Tests that Enter on installed packages shows reinstall confirmation modal.
/// - After confirmation, the package will be reinstalled.
fn optional_deps_enter_installed_shows_reinstall() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("test-pkg", true, false)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
        selected_pkg_names: std::collections::HashSet::new(),
    };

    let mut selected = 0;
    let mut selected_pkg_names = std::collections::HashSet::new();
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let result = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);

    // Should return Some(false) when showing reinstall confirmation
    assert_eq!(result, Some(false));
    // Modal should transition to ConfirmReinstall
    match app.modal {
        crate::state::Modal::ConfirmReinstall { .. } => {}
        _ => panic!("Expected modal to transition to ConfirmReinstall"),
    }
}

/// What: Verify `OptionalDeps` Enter on sudo-timestamp-setup opens setup modal.
///
/// Inputs:
/// - `OptionalDeps` modal with sudo-timestamp-setup row.
///
/// Output:
/// - `SudoTimestampSetup` modal is opened.
///
/// Details:
/// - Ensures the pseudo-package wiring matches other setup helpers.
#[test]
fn optional_deps_enter_sudo_timestamp_setup_opens_modal() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("sudo-timestamp-setup", false, true)];
    let mut selected = 0usize;
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
        selected_pkg_names: std::collections::HashSet::new(),
    };
    let mut selected_pkg_names = std::collections::HashSet::new();
    let _ = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);
    assert!(
        matches!(app.modal, crate::state::Modal::SudoTimestampSetup { .. }),
        "expected SudoTimestampSetup modal"
    );
}

/// What: Verify `OptionalDeps` Enter on doas-persist-setup opens setup modal.
#[test]
fn optional_deps_enter_doas_persist_setup_opens_modal() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("doas-persist-setup", false, true)];
    let mut selected = 0usize;
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
        selected_pkg_names: std::collections::HashSet::new(),
    };
    let mut selected_pkg_names = std::collections::HashSet::new();
    let _ = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);
    assert!(
        matches!(app.modal, crate::state::Modal::DoasPersistSetup { .. }),
        "expected DoasPersistSetup modal"
    );
}

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
#[test]
fn optional_deps_enter_virustotal_setup() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("virustotal-setup", false, true)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
        selected_pkg_names: std::collections::HashSet::new(),
    };

    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let mut selected_pkg_names = std::collections::HashSet::new();
    let _ = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);

    match app.modal {
        crate::state::Modal::VirusTotalSetup { .. } => {}
        _ => panic!("Expected VirusTotalSetup modal"),
    }
}

#[test]
/// What: Verify `OptionalDeps` Enter on AUR SSH setup opens setup modal.
fn optional_deps_enter_aur_ssh_setup_opens_modal() {
    let mut app = AppState::default();
    let rows = vec![create_test_row("aur-ssh-setup", false, true)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
        selected_pkg_names: std::collections::HashSet::new(),
    };
    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let mut selected_pkg_names = std::collections::HashSet::new();
    let result = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);

    assert_eq!(result, Some(false));
    match app.modal {
        crate::state::Modal::SshAurSetup { step, .. } => {
            assert_eq!(step, crate::state::SshSetupStep::Intro);
        }
        _ => panic!("Expected SshAurSetup modal"),
    }
}

#[test]
/// What: Verify SSH setup overwrite confirmation can be cancelled safely.
fn ssh_setup_confirm_overwrite_cancel_closes_modal() {
    let mut app = AppState::default();
    let mut step = crate::state::SshSetupStep::ConfirmOverwrite;
    let mut status_lines = vec!["Existing host block requires confirmation.".to_string()];
    let mut existing_host_block = Some("Host aur.archlinux.org\n  User aur\n".to_string());

    let result = super::handle_ssh_setup_modal(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()),
        &mut app,
        &mut step,
        &mut status_lines,
        &mut existing_host_block,
    );

    assert_eq!(result, Some(true));
    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify startup optional-deps Esc advances to next queued startup setup step.
fn optional_deps_esc_advances_startup_queue() {
    let mut app = AppState {
        pending_startup_setup_steps: VecDeque::from([
            crate::state::modal::StartupSetupTask::VirusTotalSetup,
        ]),
        ..AppState::default()
    };
    let rows = vec![create_test_row("test-pkg", false, true)];
    app.modal = crate::state::Modal::OptionalDeps {
        rows: rows.clone(),
        selected: 0,
        selected_pkg_names: std::collections::HashSet::new(),
    };

    let mut selected = 0;
    let ke = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let mut selected_pkg_names = std::collections::HashSet::new();
    let _ = handle_optional_deps(ke, &mut app, &rows, &mut selected, &mut selected_pkg_names);

    assert!(matches!(
        app.modal,
        crate::state::Modal::VirusTotalSetup { .. }
    ));
    assert!(app.pending_startup_setup_steps.is_empty());
}

#[test]
/// What: Verify direct setup opener routes to `VirusTotal` setup modal.
fn open_setup_package_opens_virustotal_modal() {
    let mut app = AppState::default();
    super::open_setup_package(&mut app, "virustotal-setup");
    assert!(matches!(
        app.modal,
        crate::state::Modal::VirusTotalSetup { .. }
    ));
}
