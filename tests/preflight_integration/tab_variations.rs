//! //! Tests for tab switching variations.

use pacsea as crate_root;
use super::helpers::*;


#[test]
/// What: Verify that preflight modal loads data correctly regardless of tab switching order.
///
/// Inputs:
/// - Packages in install_list with all data cached
/// - User switches tabs in different orders (e.g., Summary → Sandbox → Deps → Files → Services)
///
/// Output:
/// - Each tab loads its data correctly when accessed
/// - Data persists when switching back to previously visited tabs
/// - No data corruption regardless of switching order
///
/// Details:
/// - Tests that tab switching order doesn't affect data loading
/// - Verifies data persistence across tab switches
/// - Ensures no race conditions or data loss
fn preflight_tab_switching_order_variations() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    let test_packages = vec![
        create_test_package(
            "test-package-1",
            "1.0.0",
            crate_root::state::Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
        ),
        create_test_package("test-aur-package", "2.0.0", crate_root::state::Source::Aur),
    ];

    // Pre-populate cache with all data
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

    app.install_list_services = vec![crate_root::state::modal::ServiceImpact {
        unit_name: "test-service.service".to_string(),
        providers: vec!["test-package-1".to_string()],
        is_active: true,
        needs_restart: true,
        recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
    }];

    app.install_list_sandbox = vec![crate_root::logic::sandbox::SandboxInfo {
        package_name: "test-aur-package".to_string(),
        depends: vec![],
        makedepends: vec![],
        checkdepends: vec![],
        optdepends: vec![],
    }];

    // Set packages in install list
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Open preflight modal
    app.modal = create_preflight_modal(
        test_packages.clone(),
        crate_root::state::PreflightAction::Install,
        crate_root::state::PreflightTab::Summary,
    );

    // Test different tab switching orders
    // Order 1: Summary → Sandbox → Deps → Files → Services
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Sandbox);
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Deps);
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Files);
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Services);

    // Verify all tabs have data after Order 1
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
    ) = assert_preflight_modal(&app);
    assert!(!dependency_info.is_empty(), "Deps should have data");
    assert!(!file_info.is_empty(), "Files should have data");
    assert!(!service_info.is_empty(), "Services should have data");
    assert!(*services_loaded, "Services should be loaded");
    assert!(!sandbox_info.is_empty(), "Sandbox should have data");
    assert!(*sandbox_loaded, "Sandbox should be loaded");

    // Order 2: Services → Files → Deps → Sandbox → Summary
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Files);
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Deps);
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Sandbox);

    // Verify all tabs still have data after Order 2
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
    ) = assert_preflight_modal(&app);
    assert!(!dependency_info.is_empty(), "Deps should still have data");
    assert!(!file_info.is_empty(), "Files should still have data");
    assert!(!service_info.is_empty(), "Services should still have data");
    assert!(*services_loaded, "Services should still be loaded");
    assert!(!sandbox_info.is_empty(), "Sandbox should still have data");
    assert!(*sandbox_loaded, "Sandbox should still be loaded");

    // Order 3: Sandbox → Deps (back to first tab)
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Deps);

    // Final verification: All data persists regardless of switching order
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
    ) = assert_preflight_modal(&app);
    assert_eq!(dependency_info.len(), 1, "Deps should have 1 dependency");
    assert_eq!(file_info.len(), 1, "Files should have 1 file entry");
    assert_eq!(service_info.len(), 1, "Services should have 1 service");
    assert_eq!(sandbox_info.len(), 1, "Sandbox should have 1 entry");
    assert!(*services_loaded, "Services should be loaded");
    assert!(*sandbox_loaded, "Sandbox should be loaded");

    // Verify data integrity
    assert_eq!(
        dependency_info[0].name, "test-dep-1",
        "Dependency name should match"
    );
    assert_eq!(
        file_info[0].name, "test-package-1",
        "File package name should match"
    );
    assert_eq!(
        service_info[0].unit_name, "test-service.service",
        "Service unit name should match"
    );
    assert_eq!(
        sandbox_info[0].package_name, "test-aur-package",
        "Sandbox package name should match"
    );
}
