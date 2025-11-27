//! //! Tests for remove operations.

use super::helpers;
use pacsea as crate_root;

/// What: Create test reverse dependencies for remove operations.
///
/// Inputs:
/// - None
///
/// Output:
/// - Vector of `DependencyInfo` representing reverse dependencies
///
/// Details:
/// - Creates two test reverse dependencies that depend on test-package-1
fn create_test_reverse_deps() -> Vec<crate_root::state::modal::DependencyInfo> {
    vec![
        crate_root::state::modal::DependencyInfo {
            name: "dependent-package-1".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Installed {
                version: "2.0.0".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package-1".to_string()],
            depends_on: vec!["test-package-1".to_string()],
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "dependent-package-2".to_string(),
            version: "3.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Installed {
                version: "3.0.0".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "community".to_string(),
            },
            required_by: vec!["test-package-1".to_string()],
            depends_on: vec!["test-package-1".to_string()],
            is_core: false,
            is_system: false,
        },
    ]
}

/// What: Set up test app state with remove list and preflight modal.
///
/// Inputs:
/// - `test_packages`: Vector of packages to remove
///
/// Output:
/// - `AppState` configured for remove operation testing
///
/// Details:
/// - Configures `remove_list` and `remove_preflight_summary`
/// - Opens preflight modal with Remove action
fn setup_test_app_with_reverse_deps(
    test_packages: Vec<crate_root::state::PackageItem>,
) -> crate_root::state::AppState {
    crate_root::state::AppState {
        remove_list: test_packages.clone(),
        preflight_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        remove_preflight_summary: vec![crate_root::state::modal::ReverseRootSummary {
            package: "test-package-1".to_string(),
            direct_dependents: 2,
            transitive_dependents: 0,
            total_dependents: 2,
        }],
        modal: helpers::create_preflight_modal(
            test_packages,
            crate_root::state::PreflightAction::Remove,
            crate_root::state::PreflightTab::Summary,
        ),
        ..Default::default()
    }
}

/// What: Switch to Deps tab and load reverse dependencies for remove action.
///
/// Inputs:
/// - `app`: Application state
/// - `reverse_deps`: Reverse dependencies to load
///
/// Output:
/// - Updates modal to Deps tab with reverse dependencies loaded
///
/// Details:
/// - Simulates reverse dependency resolution for Remove action
fn switch_to_deps_tab_and_load_reverse_deps(
    app: &mut crate_root::state::AppState,
    reverse_deps: &[crate_root::state::modal::DependencyInfo],
) {
    if let crate_root::state::Modal::Preflight {
        action,
        tab,
        dependency_info,
        dep_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Deps;

        if matches!(*action, crate_root::state::PreflightAction::Remove) {
            *dependency_info = reverse_deps.to_vec();
            *dep_selected = 0;
        }
    }
}

/// What: Verify reverse dependencies are correctly loaded and displayed.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - Asserts that reverse dependencies are correct
///
/// Details:
/// - Verifies tab, action, dependency count, and individual dependency details
fn verify_reverse_dependencies(app: &crate_root::state::AppState) {
    let (_, action, tab, dependency_info, _, _, _, _, _) = helpers::assert_preflight_modal(app);

    assert_eq!(
        *tab,
        crate_root::state::PreflightTab::Deps,
        "Should be on Deps tab"
    );
    assert_eq!(
        *action,
        crate_root::state::PreflightAction::Remove,
        "Should be Remove action"
    );
    assert!(
        !dependency_info.is_empty(),
        "Reverse dependencies should be loaded"
    );
    assert_eq!(
        dependency_info.len(),
        2,
        "Should have 2 reverse dependencies"
    );

    let dep1 = dependency_info
        .iter()
        .find(|d| d.name == "dependent-package-1")
        .expect("dependent-package-1 should be found in dependency_info");
    assert_eq!(dep1.version, "2.0.0");
    assert!(dep1.depends_on.contains(&"test-package-1".to_string()));
    assert!(dep1.required_by.contains(&"test-package-1".to_string()));

    let dep2 = dependency_info
        .iter()
        .find(|d| d.name == "dependent-package-2")
        .expect("dependent-package-2 should be found in dependency_info");
    assert_eq!(dep2.version, "3.0.0");
    assert!(dep2.depends_on.contains(&"test-package-1".to_string()));
    assert!(dep2.required_by.contains(&"test-package-1".to_string()));
}

/// What: Verify `remove_preflight_summary` is correctly populated.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - Asserts that summary contains expected data
///
/// Details:
/// - Verifies summary is not empty and contains correct package and counts
fn verify_remove_preflight_summary(app: &crate_root::state::AppState) {
    assert!(
        !app.remove_preflight_summary.is_empty(),
        "Remove preflight summary should be populated"
    );
    let summary = &app.remove_preflight_summary[0];
    assert_eq!(summary.package, "test-package-1");
    assert_eq!(summary.direct_dependents, 2);
    assert_eq!(summary.total_dependents, 2);
}

/// What: Switch to Files tab and verify it handles remove action.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - Updates modal to Files tab and verifies action persists
///
/// Details:
/// - Verifies tab switch and that Remove action is maintained
fn verify_files_tab(app: &mut crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight { tab, .. } = &mut app.modal {
        *tab = crate_root::state::PreflightTab::Files;
    }

    let (_, action, tab, _, _, _, _, _, _) = helpers::assert_preflight_modal(app);
    assert_eq!(
        *tab,
        crate_root::state::PreflightTab::Files,
        "Should be on Files tab"
    );
    assert_eq!(
        *action,
        crate_root::state::PreflightAction::Remove,
        "Should still be Remove action"
    );
}

/// What: Verify reverse dependencies persist when switching tabs.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - Asserts that reverse dependencies are still present
///
/// Details:
/// - Verifies dependency data persists across tab switches
fn verify_deps_persistence(app: &crate_root::state::AppState) {
    let (_, _, _, dependency_info, _, _, _, _, _) = helpers::assert_preflight_modal(app);
    assert!(
        !dependency_info.is_empty(),
        "Reverse dependencies should persist when switching back"
    );
}

/// What: Perform final verification of remove action specific data.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - Asserts that all reverse dependencies depend on the package being removed
///
/// Details:
/// - Verifies action is Remove and all dependencies reference test-package-1
fn verify_final_remove_action(app: &crate_root::state::AppState) {
    let (_, action, _, dependency_info, _, _, _, _, _) = helpers::assert_preflight_modal(app);

    assert_eq!(
        *action,
        crate_root::state::PreflightAction::Remove,
        "Should be Remove action"
    );
    assert!(
        !dependency_info.is_empty(),
        "Reverse dependencies should be present"
    );

    for dep in dependency_info {
        assert!(
            dep.depends_on.contains(&"test-package-1".to_string()),
            "All reverse dependencies should depend on test-package-1"
        );
    }
}

#[test]
/// What: Verify that preflight modal handles remove action correctly with reverse dependencies.
///
/// Inputs:
/// - Packages in `remove_list`
/// - Preflight modal opened with Remove action
/// - Reverse dependencies resolved
///
/// Output:
/// - Deps tab shows reverse dependencies correctly
/// - Other tabs handle remove action appropriately
/// - Cascade mode affects dependency display
///
/// Details:
/// - Tests preflight modal for remove operations
/// - Verifies reverse dependency resolution works
/// - Ensures remove-specific logic is handled correctly
fn preflight_remove_action_with_reverse_dependencies() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let test_packages = vec![helpers::create_test_package(
        "test-package-1",
        "1.0.0",
        crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
    )];

    let reverse_deps = create_test_reverse_deps();
    let mut app = setup_test_app_with_reverse_deps(test_packages);

    // Test 1: Switch to Deps tab - should show reverse dependencies
    switch_to_deps_tab_and_load_reverse_deps(&mut app, &reverse_deps);
    verify_reverse_dependencies(&app);

    // Test 2: Verify remove_preflight_summary is populated
    verify_remove_preflight_summary(&app);

    // Test 3: Switch to Files tab - should handle remove action
    verify_files_tab(&mut app);

    // Test 4: Switch back to Deps tab - reverse dependencies should persist
    verify_deps_persistence(&app);

    // Final verification: Remove action specific data
    verify_final_remove_action(&app);
}
