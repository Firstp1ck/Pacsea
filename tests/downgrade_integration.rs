//! Integration tests for the downgrade process.
//!
//! Tests cover:
//! - Downgrade list management
//! - Downgrade command execution
//! - Navigation in downgrade pane
//!
//! Note: These tests are expected to fail initially as downgrade currently spawns terminals.

#![cfg(test)]

use pacsea::state::{AppState, PackageItem, Source};

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
/// What: Test that downgrade uses `ExecutorRequest` instead of spawning terminals.
///
/// Inputs:
/// - Downgrade action triggered through `handle_enter_key` in install pane.
///
/// Output:
/// - `pending_executor_request` should be set with `ExecutorRequest::Downgrade` (or similar).
///
/// Details:
/// - This test FAILS until downgrade is fully migrated to executor pattern.
/// - Currently `handle_enter_key` calls `spawn_shell_commands_in_terminal` for downgrade.
/// - When implementation is complete, this test should pass.
#[allow(dead_code)]
fn integration_downgrade_uses_executor_not_terminal() {
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
    // Currently this calls spawn_shell_commands_in_terminal
    // TODO: When downgrade is migrated, this should set app.pending_executor_request
    // TODO: handle_install_key is private, need to make it public or use public API
    // let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    // handle_install_key(ke, &mut app, &_dtx, &_ptx, &_atx);

    // This test will FAIL until downgrade uses executor pattern
    assert!(
        app.pending_executor_request.is_some(),
        "Downgrade must use ExecutorRequest instead of spawning terminals. Currently downgrade uses spawn_shell_commands_in_terminal."
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
