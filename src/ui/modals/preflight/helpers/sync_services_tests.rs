//! Unit tests for sync_services function.

use super::sync;
use crate::state::AppState;
use crate::state::modal::{PreflightAction, ServiceImpact, ServiceRestartDecision};
use crate::state::{PackageItem, Source};

/// What: Test sync_services early return for Remove action.
///
/// Inputs:
/// - `action`: PreflightAction::Remove
/// - `service_info`: Empty vector
///
/// Output:
/// - `service_info` remains unchanged
///
/// Details:
/// - Verifies that service sync is skipped for remove actions.
#[test]
fn test_sync_services_early_return_remove() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Remove;
    let mut service_info = Vec::new();
    let mut service_selected = 0;
    let mut services_loaded = false;

    sync::sync_services(
        &app,
        &items,
        &action,
        &mut service_info,
        &mut service_selected,
        &mut services_loaded,
    );

    assert!(service_info.is_empty());
    assert!(!services_loaded);
}

/// What: Test sync_services filters services by providers.
///
/// Inputs:
/// - `app`: AppState with cached service info
/// - `items`: Packages that provide services
///
/// Output:
/// - `service_info` contains only services provided by items
///
/// Details:
/// - Verifies that service filtering works correctly.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_services_filters_by_providers() {
    let mut app = AppState::default();
    app.install_list_services = vec![
        ServiceImpact {
            unit_name: "test.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
        ServiceImpact {
            unit_name: "other.service".to_string(),
            providers: vec!["other-pkg".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: ServiceRestartDecision::Defer,
            restart_decision: ServiceRestartDecision::Defer,
        },
    ];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let mut service_info = Vec::new();
    let mut service_selected = 0;
    let mut services_loaded = false;

    sync::sync_services(
        &app,
        &items,
        &action,
        &mut service_info,
        &mut service_selected,
        &mut services_loaded,
    );

    assert_eq!(service_info.len(), 1);
    assert_eq!(service_info[0].unit_name, "test.service");
    assert!(services_loaded);
}

/// What: Test sync_services adjusts selection when out of bounds.
///
/// Inputs:
/// - `service_info`: 3 services
/// - `service_selected`: 5 (out of bounds)
///
/// Output:
/// - `service_selected` is adjusted to 2 (last valid index)
///
/// Details:
/// - Verifies that selection is clamped to valid range.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_services_adjusts_selection_out_of_bounds() {
    let mut app = AppState::default();
    app.install_list_services = vec![
        ServiceImpact {
            unit_name: "service1.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
        ServiceImpact {
            unit_name: "service2.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
        ServiceImpact {
            unit_name: "service3.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
    ];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let mut service_info = Vec::new();
    let mut service_selected = 5; // Out of bounds
    let mut services_loaded = false;

    sync::sync_services(
        &app,
        &items,
        &action,
        &mut service_info,
        &mut service_selected,
        &mut services_loaded,
    );

    assert_eq!(service_info.len(), 3);
    assert_eq!(service_selected, 2); // Clamped to last valid index
}
