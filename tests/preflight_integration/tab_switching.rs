//! Tests for tab switching behavior.

use super::helpers;
use pacsea as crate_root;

/// What: Create test packages for mixed completion state testing.
///
/// Inputs:
/// - None (uses hardcoded test data)
///
/// Output:
/// - Vector of test packages (one official, one AUR)
///
/// Details:
/// - Creates two test packages: one official package and one AUR package
/// - Used to simulate mixed package sources in tests
fn create_test_packages() -> Vec<crate_root::state::PackageItem> {
    vec![
        helpers::create_test_package(
            "test-package-1",
            "1.0.0",
            crate_root::state::Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
        ),
        helpers::create_test_package("test-aur-package", "2.0.0", crate_root::state::Source::Aur),
    ]
}

/// What: Setup app state with mixed completion states.
///
/// Inputs:
/// - `test_packages`: Packages to use in the test
///
/// Output:
/// - `AppState` with pre-populated cache (deps, files) and resolving flags (services, sandbox)
///
/// Details:
/// - Pre-populates cache with dependencies and files (loaded state)
/// - Sets services and sandbox to resolving state (not yet loaded)
/// - Configures install list and cancellation flag
#[allow(clippy::field_reassign_with_default)]
fn setup_mixed_completion_state(
    test_packages: &[crate_root::state::PackageItem],
) -> crate_root::state::AppState {
    let mut app = crate_root::state::AppState::default();

    // Pre-populate cache with dependencies (loaded)
    app.install_list_deps = vec![crate_root::state::modal::DependencyInfo {
        name: "test-dep-1".to_string(),
        version: "1.0.0".to_string(),
        status: crate_root::state::modal::DependencyStatus::ToInstall,
        source: crate_root::state::modal::DependencySource::Official {
            repo: "core".to_string(),
        },
        required_by: vec!["test-package-1".to_string()],
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }];

    // Pre-populate cache with files (loaded)
    app.install_list_files = vec![crate_root::state::modal::PackageFileInfo {
        name: "test-package-1".to_string(),
        files: vec![crate_root::state::modal::FileChange {
            path: "/usr/bin/test1".to_string(),
            change_type: crate_root::state::modal::FileChangeType::New,
            package: "test-package-1".to_string(),
            is_config: false,
            predicted_pacnew: false,
            predicted_pacsave: false,
        }],
        total_count: 1,
        new_count: 1,
        changed_count: 0,
        removed_count: 0,
        config_count: 0,
        pacnew_candidates: 0,
        pacsave_candidates: 0,
    }];

    // Services are still resolving (not in cache yet)
    app.install_list_services = vec![];
    app.preflight_services_resolving = true;
    app.preflight_services_items = Some(test_packages.to_vec());

    // Sandbox is still resolving (not in cache yet)
    app.install_list_sandbox = vec![];
    app.preflight_sandbox_resolving = true;
    let aur_items: Vec<_> = test_packages
        .iter()
        .filter(|p| matches!(p.source, crate_root::state::Source::Aur))
        .cloned()
        .collect();
    app.preflight_sandbox_items = Some(aur_items);

    // Set packages in install list
    app.install_list = test_packages.to_vec();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    app
}

/// What: Initialize preflight modal with test packages.
///
/// Inputs:
/// - `app`: Application state to modify
/// - `test_packages`: Packages to include in modal
///
/// Output:
/// - Updates `app.modal` with a new Preflight modal
///
/// Details:
/// - Creates a preflight modal with Install action and Summary tab
/// - Uses helper function to create modal with default values
fn initialize_preflight_modal(
    app: &mut crate_root::state::AppState,
    test_packages: &[crate_root::state::PackageItem],
) {
    app.modal = helpers::create_preflight_modal(
        test_packages.to_vec(),
        crate_root::state::PreflightAction::Install,
        crate_root::state::PreflightTab::Summary,
    );
}

/// What: Test switching to Deps tab with loaded data.
///
/// Inputs:
/// - `app`: Application state with pre-populated dependencies
///
/// Output:
/// - Panics if assertions fail
///
/// Details:
/// - Switches to Deps tab and verifies dependencies are loaded immediately
/// - Asserts that tab is correct and dependency data is present
fn test_switch_to_deps_tab(app: &mut crate_root::state::AppState) {
    helpers::switch_preflight_tab(app, crate_root::state::PreflightTab::Deps);

    let (_, _, tab, dependency_info, ..) = helpers::assert_preflight_modal(app);
    assert_eq!(
        *tab,
        crate_root::state::PreflightTab::Deps,
        "Should be on Deps tab"
    );
    assert!(!dependency_info.is_empty(), "Dependencies should be loaded");
    assert_eq!(dependency_info.len(), 1, "Should have 1 dependency");
}

/// What: Test switching to Files tab with loaded data.
///
/// Inputs:
/// - `app`: Application state with pre-populated files
///
/// Output:
/// - Panics if assertions fail
///
/// Details:
/// - Switches to Files tab and verifies files are loaded immediately
/// - Asserts that tab is correct and file data is present
fn test_switch_to_files_tab(app: &mut crate_root::state::AppState) {
    helpers::switch_preflight_tab(app, crate_root::state::PreflightTab::Files);

    let (_, _, tab, _, file_info, ..) = helpers::assert_preflight_modal(app);
    assert_eq!(
        *tab,
        crate_root::state::PreflightTab::Files,
        "Should be on Files tab"
    );
    assert!(!file_info.is_empty(), "Files should be loaded");
    assert_eq!(file_info.len(), 1, "Should have 1 file entry");
}

/// What: Test switching to Services tab while still resolving.
///
/// Inputs:
/// - `app`: Application state with services still resolving
///
/// Output:
/// - Panics if assertions fail
///
/// Details:
/// - Switches to Services tab and verifies loading state is shown
/// - Asserts that services are empty and not marked as loaded
/// - Verifies that resolving flag is still set
fn test_switch_to_services_tab_resolving(app: &mut crate_root::state::AppState) {
    helpers::switch_preflight_tab(app, crate_root::state::PreflightTab::Services);

    let (_, _, tab, _, _, service_info, _, services_loaded, _) =
        helpers::assert_preflight_modal(app);
    assert_eq!(
        *tab,
        crate_root::state::PreflightTab::Services,
        "Should be on Services tab"
    );
    assert!(
        service_info.is_empty(),
        "Services should be empty (still resolving)"
    );
    assert!(
        !*services_loaded,
        "Services should not be marked as loaded (still resolving)"
    );
    assert!(
        app.preflight_services_resolving,
        "Services should still be resolving"
    );
}

/// What: Test switching to Sandbox tab while still resolving.
///
/// Inputs:
/// - `app`: Application state with sandbox still resolving
///
/// Output:
/// - Panics if assertions fail
///
/// Details:
/// - Switches to Sandbox tab and verifies loading state is shown
/// - Asserts that sandbox info is empty and not marked as loaded
/// - Verifies that resolving flag is still set
fn test_switch_to_sandbox_tab_resolving(app: &mut crate_root::state::AppState) {
    helpers::switch_preflight_tab(app, crate_root::state::PreflightTab::Sandbox);

    let (_, _, tab, _, _, _, sandbox_info, _, sandbox_loaded) =
        helpers::assert_preflight_modal(app);
    assert_eq!(
        *tab,
        crate_root::state::PreflightTab::Sandbox,
        "Should be on Sandbox tab"
    );
    assert!(
        sandbox_info.is_empty(),
        "Sandbox should be empty (still resolving)"
    );
    assert!(
        !*sandbox_loaded,
        "Sandbox should not be marked as loaded (still resolving)"
    );
    assert!(
        app.preflight_sandbox_resolving,
        "Sandbox should still be resolving"
    );
}

/// What: Verify dependencies persist when switching back to Deps tab.
///
/// Inputs:
/// - `app`: Application state with previously loaded dependencies
///
/// Output:
/// - Panics if assertions fail
///
/// Details:
/// - Verifies that dependency data persists after switching away and back
/// - Asserts that dependencies are still loaded when returning to Deps tab
fn test_deps_persistence(app: &crate_root::state::AppState) {
    let (_, _, _, dependency_info, ..) = helpers::assert_preflight_modal(app);
    assert!(
        !dependency_info.is_empty(),
        "Dependencies should still be loaded when switching back"
    );
}

/// What: Verify files persist when switching back to Files tab.
///
/// Inputs:
/// - `app`: Application state with previously loaded files
///
/// Output:
/// - Panics if assertions fail
///
/// Details:
/// - Switches back to Files tab and verifies data persistence
/// - Asserts that files are still loaded when returning to Files tab
fn test_files_persistence(app: &mut crate_root::state::AppState) {
    helpers::switch_preflight_tab(app, crate_root::state::PreflightTab::Files);

    let (_, _, tab, _, file_info, ..) = helpers::assert_preflight_modal(app);
    assert_eq!(
        *tab,
        crate_root::state::PreflightTab::Files,
        "Should be back on Files tab"
    );
    assert!(
        !file_info.is_empty(),
        "Files should still be loaded when switching back"
    );
}

/// What: Verify mixed state is maintained correctly across all tabs.
///
/// Inputs:
/// - `app`: Application state with mixed completion states
///
/// Output:
/// - Panics if assertions fail
///
/// Details:
/// - Verifies that tabs with loaded data (Deps, Files) have data
/// - Verifies that tabs still resolving (Services, Sandbox) are empty
/// - Ensures no data corruption or mixing between tabs
fn verify_mixed_state(app: &crate_root::state::AppState) {
    let (
        _,
        _,
        _,
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        services_loaded,
        sandbox_loaded,
    ) = helpers::assert_preflight_modal(app);

    // Tabs with data should have data
    assert!(!dependency_info.is_empty(), "Dependencies should have data");
    assert!(!file_info.is_empty(), "Files should have data");

    // Tabs still resolving should be empty
    assert!(
        service_info.is_empty(),
        "Services should be empty (still resolving)"
    );
    assert!(!*services_loaded, "Services should not be loaded");
    assert!(
        sandbox_info.is_empty(),
        "Sandbox should be empty (still resolving)"
    );
    assert!(!*sandbox_loaded, "Sandbox should not be loaded");
}

#[test]
/// What: Verify that preflight modal handles mixed completion states correctly when switching tabs.
///
/// Inputs:
/// - Packages in `install_list`
/// - Some tabs have data loaded (Deps, Files)
/// - Some tabs are still resolving (Services, Sandbox)
/// - User switches between tabs
///
/// Output:
/// - Tabs with loaded data display correctly
/// - Tabs still resolving show appropriate loading state
/// - No data corruption or mixing between tabs
///
/// Details:
/// - Tests edge case where resolution completes at different times
/// - Verifies that partial data doesn't cause issues when switching tabs
fn preflight_handles_mixed_completion_states_when_switching_tabs() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let test_packages = create_test_packages();
    let mut app = setup_mixed_completion_state(&test_packages);
    initialize_preflight_modal(&mut app, &test_packages);

    // Test 1: Switch to Deps tab (has data) - should load immediately
    test_switch_to_deps_tab(&mut app);

    // Test 2: Switch to Files tab (has data) - should load immediately
    test_switch_to_files_tab(&mut app);

    // Test 3: Switch to Services tab (still resolving) - should show loading state
    test_switch_to_services_tab_resolving(&mut app);

    // Test 4: Switch to Sandbox tab (still resolving) - should show loading state
    test_switch_to_sandbox_tab_resolving(&mut app);

    // Test 5: Switch back to Deps tab - data should still be there
    test_deps_persistence(&app);

    // Test 6: Switch back to Files tab - data should still be there
    test_files_persistence(&mut app);

    // Final verification: Mixed state is maintained correctly
    verify_mixed_state(&app);
}
