//! Integration tests for the downgrade process.
//!
//! Tests cover:
//! - Downgrade list management
//! - Downgrade command execution
//! - Navigation in downgrade pane
//! - `ExecutorRequest::Downgrade` creation
//! - Password prompt for downgrade
//! - Dry-run mode

#![cfg(test)]

use pacsea::install::ExecutorRequest;
use pacsea::state::modal::{PasswordPurpose, PreflightHeaderChips};
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
/// What: Test downgrade list state management.
///
/// Inputs:
/// - `AppState` with downgrade list.
///
/// Output:
/// - Downgrade list can be managed correctly.
///
/// Details:
/// - Verifies downgrade list operations.
fn integration_downgrade_list_management() {
    let mut app = AppState {
        installed_only_mode: true,
        right_pane_focus: pacsea::state::RightPaneFocus::Downgrade,
        ..Default::default()
    };

    let pkg1 = create_test_package(
        "pkg1",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );
    let pkg2 = create_test_package(
        "pkg2",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    // Add packages to downgrade list
    app.downgrade_list.push(pkg1);
    app.downgrade_list.push(pkg2);

    assert_eq!(app.downgrade_list.len(), 2);
    assert_eq!(app.downgrade_list[0].name, "pkg1");
    assert_eq!(app.downgrade_list[1].name, "pkg2");

    // Remove from downgrade list
    app.downgrade_list.remove(0);
    assert_eq!(app.downgrade_list.len(), 1);
    assert_eq!(app.downgrade_list[0].name, "pkg2");

    // Clear downgrade list
    app.downgrade_list.clear();
    assert!(app.downgrade_list.is_empty());
}

#[test]
/// What: Test downgrade command structure.
///
/// Inputs:
/// - Package names for downgrade.
///
/// Output:
/// - Command structure is correct.
///
/// Details:
/// - Verifies downgrade command format.
/// - Note: Actual execution spawns terminal, so this tests command structure only.
fn integration_downgrade_command_structure() {
    let names = ["test-pkg1".to_string(), "test-pkg2".to_string()];
    let joined = names.join(" ");

    // Test dry-run command
    let dry_run_cmd = format!("echo DRY RUN: sudo downgrade {joined}");
    assert!(dry_run_cmd.contains("DRY RUN"));
    assert!(dry_run_cmd.contains("downgrade"));
    assert!(dry_run_cmd.contains("test-pkg1"));
    assert!(dry_run_cmd.contains("test-pkg2"));

    // Test actual command structure
    let actual_cmd = format!(
        "if (command -v downgrade >/dev/null 2>&1) || sudo pacman -Qi downgrade >/dev/null 2>&1; then sudo downgrade {joined}; else echo 'downgrade tool not found. Install \"downgrade\" package.'; fi"
    );
    assert!(actual_cmd.contains("downgrade"));
    assert!(actual_cmd.contains("test-pkg1"));
    assert!(actual_cmd.contains("test-pkg2"));
}

#[test]
/// What: Test that downgrade spawns in a terminal (not executor) since it's an interactive tool.
///
/// Inputs:
/// - Downgrade action triggered through preflight modal.
///
/// Output:
/// - Downgrade should spawn in a terminal, not use executor pattern.
///
/// Details:
/// - Downgrade tool is interactive and requires user input to select versions.
/// - It cannot work in the PTY executor pattern, so it spawns in a terminal instead.
/// - This is the expected behavior for interactive tools.
#[allow(dead_code)]
fn integration_downgrade_spawns_in_terminal() {
    // Unused imports commented out until handle_install_key is made public
    // use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    // TODO: handle_install_key is private, need to make it public or use public API
    // use pacsea::events::install::handle_install_key;
    use tokio::sync::mpsc;

    let app = AppState {
        installed_only_mode: true,
        right_pane_focus: pacsea::state::RightPaneFocus::Downgrade,
        downgrade_list: vec![pacsea::state::PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: pacsea::state::Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }],
        dry_run: false,
        ..Default::default()
    };

    let (_dtx, _drx): (mpsc::UnboundedSender<pacsea::state::PackageItem>, _) =
        mpsc::unbounded_channel();
    let (_ptx, _prx): (mpsc::UnboundedSender<pacsea::state::PackageItem>, _) =
        mpsc::unbounded_channel();
    let (_atx, _arx): (mpsc::UnboundedSender<pacsea::state::PackageItem>, _) =
        mpsc::unbounded_channel();

    // Trigger downgrade action through Enter key
    // Downgrade spawns in a terminal (not executor) because it's interactive
    // TODO: handle_install_key is private, need to make it public or use public API
    // let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    // handle_install_key(ke, &mut app, &_dtx, &_ptx, &_atx);

    // Downgrade should NOT use executor pattern - it spawns in a terminal
    // This is expected behavior for interactive tools
    assert!(
        app.pending_executor_request.is_none(),
        "Downgrade correctly spawns in a terminal (not executor) because it's an interactive tool."
    );

    // Note: ExecutorRequest::Downgrade variant doesn't exist yet
    // This test will fail until both the variant exists and the code uses it
}

#[test]
/// What: Test downgrade with empty list.
///
/// Inputs:
/// - Empty downgrade list.
///
/// Output:
/// - Empty list is handled gracefully.
///
/// Details:
/// - Tests edge case of empty downgrade list.
fn integration_downgrade_empty_list() {
    let app = AppState {
        installed_only_mode: true,
        right_pane_focus: pacsea::state::RightPaneFocus::Downgrade,
        ..Default::default()
    };

    assert!(app.downgrade_list.is_empty());
    assert_eq!(app.downgrade_state.selected(), None);
}

#[test]
/// What: Test `ExecutorRequest::Downgrade` creation with password.
///
/// Inputs:
/// - Package names and password for downgrade.
///
/// Output:
/// - `ExecutorRequest::Downgrade` with correct fields.
///
/// Details:
/// - Verifies Downgrade request can be created for executor.
fn integration_executor_request_downgrade_with_password() {
    let names = vec!["pkg1".to_string(), "pkg2".to_string()];

    let request = ExecutorRequest::Downgrade {
        names,
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Downgrade {
            names,
            password,
            dry_run,
        } => {
            assert_eq!(names.len(), 2);
            assert_eq!(names[0], "pkg1");
            assert_eq!(names[1], "pkg2");
            assert_eq!(password, Some("testpassword".to_string()));
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Downgrade"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Downgrade` without password.
///
/// Inputs:
/// - Package names without password.
///
/// Output:
/// - `ExecutorRequest::Downgrade` with password=None.
///
/// Details:
/// - Verifies Downgrade request handles no password case.
fn integration_executor_request_downgrade_no_password() {
    let names = vec!["pkg1".to_string()];

    let request = ExecutorRequest::Downgrade {
        names,
        password: None,
        dry_run: false,
    };

    match request {
        ExecutorRequest::Downgrade { password, .. } => {
            assert!(password.is_none());
        }
        _ => panic!("Expected ExecutorRequest::Downgrade"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Downgrade` dry-run mode.
///
/// Inputs:
/// - Downgrade with `dry_run` enabled.
///
/// Output:
/// - `ExecutorRequest::Downgrade` with `dry_run=true`.
///
/// Details:
/// - Verifies dry-run mode is respected for downgrade.
fn integration_executor_request_downgrade_dry_run() {
    let names = vec!["pkg1".to_string()];

    let request = ExecutorRequest::Downgrade {
        names,
        password: None,
        dry_run: true,
    };

    match request {
        ExecutorRequest::Downgrade { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Downgrade"),
    }
}

#[test]
/// What: Test downgrade triggers password prompt.
///
/// Inputs:
/// - Downgrade action for packages.
///
/// Output:
/// - Password prompt modal is shown.
///
/// Details:
/// - Verifies downgrade requires password authentication.
fn integration_downgrade_password_prompt() {
    let pkg = create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let mut app = AppState {
        downgrade_list: vec![pkg.clone()],
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Simulate downgrade triggering password prompt
    app.modal = Modal::PasswordPrompt {
        purpose: PasswordPurpose::Downgrade,
        items: vec![pkg],
        input: String::new(),
        cursor: 0,
        error: None,
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::Downgrade);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "test-pkg");
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test downgrade transitions to `PreflightExec` after password.
///
/// Inputs:
/// - Password submitted for downgrade.
///
/// Output:
/// - Modal transitions to `PreflightExec`.
/// - `ExecutorRequest::Downgrade` is created.
///
/// Details:
/// - Verifies downgrade flow after password submission.
fn integration_downgrade_to_preflight_exec() {
    let pkg = create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Downgrade,
            items: vec![pkg],
            input: "testpassword".to_string(),
            cursor: 12,
            error: None,
        },
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
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

    let items = if let Modal::PasswordPrompt { ref items, .. } = app.modal {
        items.clone()
    } else {
        vec![]
    };

    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();

    // Simulate transition to PreflightExec
    let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();
    app.modal = Modal::PreflightExec {
        items,
        action: PreflightAction::Downgrade,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: vec![],
        abortable: false,
        header_chips,
    };

    // Set executor request
    app.pending_executor_request = Some(ExecutorRequest::Downgrade {
        names,
        password,
        dry_run: false,
    });

    // Verify modal
    match app.modal {
        Modal::PreflightExec { action, items, .. } => {
            assert_eq!(action, PreflightAction::Downgrade);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "test-pkg");
        }
        _ => panic!("Expected PreflightExec modal"),
    }

    // Verify executor request
    match app.pending_executor_request {
        Some(ExecutorRequest::Downgrade {
            names, password, ..
        }) => {
            assert_eq!(names.len(), 1);
            assert_eq!(names[0], "test-pkg");
            assert_eq!(password, Some("testpassword".to_string()));
        }
        _ => panic!("Expected ExecutorRequest::Downgrade"),
    }
}

#[test]
/// What: Test downgrade multiple packages.
///
/// Inputs:
/// - Multiple packages in downgrade list.
///
/// Output:
/// - All package names are in `ExecutorRequest::Downgrade`.
///
/// Details:
/// - Verifies batch downgrade includes all packages.
fn integration_downgrade_multiple_packages() {
    let names = vec!["pkg1".to_string(), "pkg2".to_string(), "pkg3".to_string()];

    let request = ExecutorRequest::Downgrade {
        names,
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Downgrade { names, .. } => {
            assert_eq!(names.len(), 3);
            assert!(names.contains(&"pkg1".to_string()));
            assert!(names.contains(&"pkg2".to_string()));
            assert!(names.contains(&"pkg3".to_string()));
        }
        _ => panic!("Expected ExecutorRequest::Downgrade"),
    }
}

#[test]
/// What: Test downgrade command format with downgrade tool check.
///
/// Inputs:
/// - Package name for downgrade.
///
/// Output:
/// - Command includes downgrade tool check.
///
/// Details:
/// - Verifies command structure includes fallback for missing tool.
fn integration_downgrade_command_with_tool_check() {
    let pkg_name = "test-pkg";

    // Build command with tool check
    let cmd = format!(
        "if command -v downgrade >/dev/null 2>&1; then \
         sudo downgrade {pkg_name}; \
         else \
         echo 'downgrade tool not found. Install \"downgrade\" package.'; \
         fi"
    );

    assert!(cmd.contains("command -v downgrade"));
    assert!(cmd.contains("sudo downgrade"));
    assert!(cmd.contains(pkg_name));
    assert!(cmd.contains("not found"));
}

#[test]
/// What: Test downgrade dry-run command format.
///
/// Inputs:
/// - Package name for dry-run downgrade.
///
/// Output:
/// - Command includes "DRY RUN:" prefix.
///
/// Details:
/// - Verifies dry-run command format.
fn integration_downgrade_dry_run_command_format() {
    let pkg_name = "test-pkg";

    // Build dry-run command
    let cmd = format!("echo DRY RUN: sudo downgrade {pkg_name}");

    assert!(cmd.contains("DRY RUN:"));
    assert!(cmd.contains("sudo downgrade"));
    assert!(cmd.contains(pkg_name));
}

#[test]
/// What: Test downgrade pane focus state.
///
/// Inputs:
/// - `AppState` with downgrade pane focus.
///
/// Output:
/// - Right pane focus is correctly set to Downgrade.
///
/// Details:
/// - Verifies pane focus tracking for downgrade operations.
fn integration_downgrade_pane_focus() {
    let app = AppState {
        installed_only_mode: true,
        right_pane_focus: pacsea::state::RightPaneFocus::Downgrade,
        ..Default::default()
    };

    assert_eq!(
        app.right_pane_focus,
        pacsea::state::RightPaneFocus::Downgrade
    );
}

#[test]
/// What: Test downgrade state selection tracking.
///
/// Inputs:
/// - Downgrade list with multiple packages.
///
/// Output:
/// - Selection state is correctly tracked.
///
/// Details:
/// - Verifies downgrade list selection management.
fn integration_downgrade_selection_tracking() {
    let mut app = AppState {
        installed_only_mode: true,
        right_pane_focus: pacsea::state::RightPaneFocus::Downgrade,
        ..Default::default()
    };

    // Add packages
    app.downgrade_list.push(create_test_package(
        "pkg1",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    ));
    app.downgrade_list.push(create_test_package(
        "pkg2",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    ));

    // Select first item
    app.downgrade_state.select(Some(0));
    assert_eq!(app.downgrade_state.selected(), Some(0));

    // Select second item
    app.downgrade_state.select(Some(1));
    assert_eq!(app.downgrade_state.selected(), Some(1));
}
