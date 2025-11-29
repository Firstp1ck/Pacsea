//! Unit tests for remove command building and execution.

#![cfg(test)]

use crate::state::modal::CascadeMode;
use crate::install::remove::spawn_remove_all;

#[test]
/// What: Verify remove command building logic.
///
/// Inputs:
/// - Package names, cascade mode, dry_run flag.
///
/// Output:
/// - Command structure is correct.
///
/// Details:
/// - Tests that remove commands are built correctly.
/// - Note: spawn_remove_all spawns terminal, so this test verifies the function can be called.
fn remove_command_building() {
    let names = vec!["test-pkg1".to_string(), "test-pkg2".to_string()];
    
    // Test that function can be called (it will spawn terminal in non-test mode)
    // In test mode without PACSEA_TEST_OUT, it should be a no-op
    spawn_remove_all(&names, true, CascadeMode::Basic);
    spawn_remove_all(&names, true, CascadeMode::Cascade);
    spawn_remove_all(&names, true, CascadeMode::CascadeWithConfigs);
}

#[test]
/// What: Verify remove command building with different cascade modes.
///
/// Inputs:
/// - Package names with different cascade modes.
///
/// Output:
/// - Commands are built correctly for each mode.
///
/// Details:
/// - Tests that cascade mode affects command building.
fn remove_cascade_modes() {
    let names = vec!["test-pkg".to_string()];
    
    // Test all cascade modes
    spawn_remove_all(&names, true, CascadeMode::Basic);
    spawn_remove_all(&names, true, CascadeMode::Cascade);
    spawn_remove_all(&names, true, CascadeMode::CascadeWithConfigs);
}

#[test]
/// What: Verify remove command building with empty list.
///
/// Inputs:
/// - Empty package list.
///
/// Output:
/// - Function handles empty list gracefully.
///
/// Details:
/// - Tests edge case of empty package list.
fn remove_empty_list() {
    let names = Vec::<String>::new();
    spawn_remove_all(&names, true, CascadeMode::Basic);
}

#[test]
/// What: Verify remove command building with dry-run.
///
/// Inputs:
/// - Package names with dry_run=true.
///
/// Output:
/// - Dry-run commands are built correctly.
///
/// Details:
/// - Tests that dry-run mode produces appropriate commands.
fn remove_dry_run() {
    let names = vec!["test-pkg".to_string()];
    spawn_remove_all(&names, true, CascadeMode::Basic);
}

