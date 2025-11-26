//! Unit tests for `sync_sandbox` function.

use super::sync;
use crate::state::AppState;
use crate::state::modal::{PreflightAction, PreflightTab};
use crate::state::{PackageItem, Source};

/// What: Test `sync_sandbox` early return for Remove action.
///
/// Inputs:
/// - `action`: `PreflightAction::Remove`
/// - `sandbox_info`: Empty vector
///
/// Output:
/// - `sandbox_info` remains unchanged (unless no AUR packages)
///
/// Details:
/// - Verifies that sandbox sync handles remove actions correctly.
#[test]
fn test_sync_sandbox_early_return_remove() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }];
    let action = PreflightAction::Remove;
    let tab = PreflightTab::Sandbox;
    let mut sandbox_info = Vec::new();
    let mut sandbox_loaded = false;

    sync::sync_sandbox(
        &app,
        &items,
        action,
        tab,
        &mut sandbox_info,
        &mut sandbox_loaded,
    );

    // For remove with no AUR packages, should mark as loaded
    assert!(sandbox_loaded);
}

/// What: Test `sync_sandbox` early return when not on Sandbox tab.
///
/// Inputs:
/// - `tab`: `PreflightTab::Summary`
/// - `sandbox_info`: Empty vector
///
/// Output:
/// - `sandbox_info` remains unchanged
///
/// Details:
/// - Verifies that sandbox sync is skipped when not on Sandbox tab.
#[test]
fn test_sync_sandbox_early_return_wrong_tab() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }];
    let action = PreflightAction::Install;
    let tab = PreflightTab::Summary;
    let mut sandbox_info = Vec::new();
    let mut sandbox_loaded = false;

    sync::sync_sandbox(
        &app,
        &items,
        action,
        tab,
        &mut sandbox_info,
        &mut sandbox_loaded,
    );

    assert!(sandbox_info.is_empty());
}
