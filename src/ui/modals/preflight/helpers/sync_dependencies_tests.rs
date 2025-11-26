//! Unit tests for `sync_dependencies` function.

use super::sync;
use crate::state::AppState;
use crate::state::modal::{
    DependencyInfo, DependencySource, DependencyStatus, PreflightAction, PreflightTab,
};
use crate::state::{PackageItem, Source};

/// What: Test `sync_dependencies` early return for Remove action.
///
/// Inputs:
/// - `action`: `PreflightAction::Remove`
/// - `dependency_info`: Empty vector
///
/// Output:
/// - `dependency_info` remains unchanged
///
/// Details:
/// - Verifies that dependency sync is skipped for remove actions.
#[test]
fn test_sync_dependencies_early_return_remove() {
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
    let action = PreflightAction::Remove;
    let tab = PreflightTab::Deps;
    let mut dependency_info = Vec::new();
    let mut dep_selected = 0;

    sync::sync_dependencies(
        &app,
        &items,
        action,
        tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert!(dependency_info.is_empty());
}

/// What: Test `sync_dependencies` early return when not on Deps tab.
///
/// Inputs:
/// - `tab`: `PreflightTab::Summary`
/// - `dependency_info`: Empty vector
///
/// Output:
/// - `dependency_info` remains unchanged
///
/// Details:
/// - Verifies that dependency sync is skipped when not on Deps tab.
#[test]
fn test_sync_dependencies_early_return_wrong_tab() {
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
    let mut dependency_info = Vec::new();
    let mut dep_selected = 0;

    sync::sync_dependencies(
        &app,
        &items,
        action,
        tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert!(dependency_info.is_empty());
}

/// What: Test `sync_dependencies` filters dependencies by `required_by`.
///
/// Inputs:
/// - `app`: `AppState` with cached dependencies
/// - `items`: Packages that require dependencies
///
/// Output:
/// - `dependency_info` contains only dependencies required by items
///
/// Details:
/// - Verifies that dependency filtering works correctly.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_dependencies_filters_by_required_by() {
    let mut app = AppState::default();
    app.install_list_deps = vec![
        DependencyInfo {
            name: "dep1".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        DependencyInfo {
            name: "dep2".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["other-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

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
    let tab = PreflightTab::Deps;
    let mut dependency_info = Vec::new();
    let mut dep_selected = 0;

    sync::sync_dependencies(
        &app,
        &items,
        action,
        tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert_eq!(dependency_info.len(), 1);
    assert_eq!(dependency_info[0].name, "dep1");
    assert_eq!(dep_selected, 0);
}

/// What: Test `sync_dependencies` resets selection on first load.
///
/// Inputs:
/// - `dependency_info`: Empty (first load)
/// - `dep_selected`: 5
///
/// Output:
/// - `dep_selected` is reset to 0
///
/// Details:
/// - Verifies that selection is reset on first load.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_dependencies_resets_selection_on_first_load() {
    let mut app = AppState::default();
    app.install_list_deps = vec![DependencyInfo {
        name: "dep1".to_string(),
        version: "1.0".to_string(),
        status: DependencyStatus::ToInstall,
        source: DependencySource::Official {
            repo: "core".to_string(),
        },
        required_by: vec!["test-pkg".to_string()],
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }];

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
    let tab = PreflightTab::Deps;
    let mut dependency_info = Vec::new();
    let mut dep_selected = 5;

    sync::sync_dependencies(
        &app,
        &items,
        action,
        tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert_eq!(dep_selected, 0);
}

/// What: Test `sync_dependencies` does not reset selection on subsequent loads.
///
/// Inputs:
/// - `dependency_info`: Already populated
/// - `dep_selected`: 2
///
/// Output:
/// - `dep_selected` remains 2
///
/// Details:
/// - Verifies that selection is preserved on subsequent syncs.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_dependencies_preserves_selection_on_subsequent_load() {
    let mut app = AppState::default();
    app.install_list_deps = vec![
        DependencyInfo {
            name: "dep1".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        DependencyInfo {
            name: "dep2".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

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
    let tab = PreflightTab::Deps;
    let mut dependency_info = vec![DependencyInfo {
        name: "dep1".to_string(),
        version: "1.0".to_string(),
        status: DependencyStatus::ToInstall,
        source: DependencySource::Official {
            repo: "core".to_string(),
        },
        required_by: vec!["test-pkg".to_string()],
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }];
    let mut dep_selected = 2;

    sync::sync_dependencies(
        &app,
        &items,
        action,
        tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert_eq!(dep_selected, 2);
}
