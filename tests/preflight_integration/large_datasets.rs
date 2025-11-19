//! //! Tests for large datasets handling.

use pacsea as crate_root;
use super::helpers::*;


#[test]
/// What: Verify that preflight modal handles large datasets correctly.
///
/// Inputs:
/// - 10+ packages in install_list (mix of official and AUR)
/// - Each package has 3-5 dependencies
/// - Each package has 2-3 files
/// - Each package has 1-2 services
/// - AUR packages have sandbox info
/// - User switches between all tabs
///
/// Output:
/// - All tabs load and display correctly with large datasets
/// - Navigation works correctly (selection indices, tree expansion)
/// - Data integrity is maintained (correct counts, no corruption)
///
/// Details:
/// - Tests performance and correctness with large datasets
/// - Verifies that many packages don't cause data corruption
/// - Ensures navigation remains functional with many items
fn preflight_handles_large_datasets_correctly() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    // Create 12 test packages (mix of official and AUR)
    let mut test_packages = Vec::new();
    for i in 1..=8 {
        test_packages.push(crate_root::state::PackageItem {
            name: format!("test-official-pkg-{}", i),
            version: format!("{}.0.0", i),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: if i % 2 == 0 { "extra" } else { "core" }.to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        });
    }
    for i in 1..=4 {
        test_packages.push(crate_root::state::PackageItem {
            name: format!("test-aur-pkg-{}", i),
            version: format!("{}.0.0", i),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        });
    }

    // Pre-populate cache with dependencies (3-5 per package)
    let mut expected_dep_count = 0;
    for pkg in &test_packages {
        let dep_count = if pkg.name.contains("official") { 4 } else { 3 };
        for j in 1..=dep_count {
            app.install_list_deps
                .push(crate_root::state::modal::DependencyInfo {
                    name: format!("{}-dep-{}", pkg.name, j),
                    version: "1.0.0".to_string(),
                    status: crate_root::state::modal::DependencyStatus::ToInstall,
                    source: if pkg.name.contains("aur") {
                        crate_root::state::modal::DependencySource::Aur
                    } else {
                        crate_root::state::modal::DependencySource::Official {
                            repo: "core".to_string(),
                        }
                    },
                    required_by: vec![pkg.name.clone()],
                    depends_on: Vec::new(),
                    is_core: false,
                    is_system: false,
                });
            expected_dep_count += 1;
        }
    }

    // Pre-populate cache with files (2-3 per package)
    let mut expected_file_count = 0;
    for pkg in &test_packages {
        let file_count = if pkg.name.contains("official") { 3 } else { 2 };
        let mut files = Vec::new();
        for j in 1..=file_count {
            files.push(crate_root::state::modal::FileChange {
                path: format!("/usr/bin/{}-file-{}", pkg.name, j),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: pkg.name.clone(),
                is_config: j == file_count, // Last file is config
                predicted_pacnew: false,
                predicted_pacsave: false,
            });
        }
        app.install_list_files
            .push(crate_root::state::modal::PackageFileInfo {
                name: pkg.name.clone(),
                files: files.clone(),
                total_count: file_count,
                new_count: file_count,
                changed_count: 0,
                removed_count: 0,
                config_count: 1,
                pacnew_candidates: 0,
                pacsave_candidates: 0,
            });
        expected_file_count += file_count;
    }

    // Pre-populate cache with services (1-2 per package)
    let mut expected_service_count = 0;
    for pkg in &test_packages {
        let service_count = if pkg.name.contains("official") { 2 } else { 1 };
        for j in 1..=service_count {
            app.install_list_services
                .push(crate_root::state::modal::ServiceImpact {
                    unit_name: format!("{}-service-{}.service", pkg.name, j),
                    providers: vec![pkg.name.clone()],
                    is_active: j == 1,
                    needs_restart: j == 1,
                    recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
                    restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
                });
            expected_service_count += 1;
        }
    }

    // Pre-populate cache with sandbox info (only for AUR packages)
    let mut expected_sandbox_count = 0;
    for pkg in &test_packages {
        if matches!(pkg.source, crate_root::state::Source::Aur) {
            app.install_list_sandbox
                .push(crate_root::logic::sandbox::SandboxInfo {
                    package_name: pkg.name.clone(),
                    depends: vec![crate_root::logic::sandbox::DependencyDelta {
                        name: format!("{}-sandbox-dep", pkg.name),
                        is_installed: false,
                        installed_version: None,
                        version_satisfied: false,
                    }],
                    makedepends: vec![],
                    checkdepends: vec![],
                    optdepends: vec![],
                });
            expected_sandbox_count += 1;
        }
    }

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

    // Test 1: Deps tab - should load all dependencies
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Deps);
    let (_, _, _, dependency_info, _, _, _, _, _) = assert_preflight_modal(&app);
    assert_eq!(
        dependency_info.len(),
        expected_dep_count,
        "Should have all dependencies loaded"
    );
    // Verify all packages have their dependencies
    for pkg in &test_packages {
        let pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&pkg.name))
            .collect();
        let expected = if pkg.name.contains("official") { 4 } else { 3 };
        assert_eq!(
            pkg_deps.len(),
            expected,
            "Package {} should have {} dependencies",
            pkg.name,
            expected
        );
    }

    // Test 2: Files tab - should load all files
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Files);
    let (_, _, _, _, file_info, _, _, _, _) = assert_preflight_modal(&app);
    assert_eq!(
        file_info.len(),
        test_packages.len(),
        "Should have file info for all packages"
    );
    // Verify total file count
    let total_files: usize = file_info.iter().map(|f| f.files.len()).sum();
    assert_eq!(
        total_files, expected_file_count,
        "Should have all files loaded"
    );

    // Test 3: Services tab - should load all services
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Services);
    let (_, _, _, _, _, service_info, _, services_loaded, _) = assert_preflight_modal(&app);
    assert_eq!(
        service_info.len(),
        expected_service_count,
        "Should have all services loaded"
    );
    assert!(*services_loaded, "Services should be marked as loaded");

    // Test 4: Sandbox tab - should load sandbox info for AUR packages only
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Sandbox);
    let (_, _, _, _, _, _, sandbox_info, _, sandbox_loaded) = assert_preflight_modal(&app);
    assert_eq!(
        sandbox_info.len(),
        expected_sandbox_count,
        "Should have sandbox info for all AUR packages"
    );
    assert!(*sandbox_loaded, "Sandbox should be marked as loaded");

    // Test 5: Verify navigation works (selection indices)
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        dep_selected,
        file_selected,
        service_selected,
        ..
    } = &mut app.modal
    {
        if !dependency_info.is_empty() {
            *dep_selected = dependency_info.len().saturating_sub(1);
        }
        if !file_info.is_empty() {
            *file_selected = file_info.len().saturating_sub(1);
        }
        if !service_info.is_empty() {
            *service_selected = service_info.len().saturating_sub(1);
        }
    }
    let (_, _, _, dependency_info, file_info, service_info, _, _, _) = assert_preflight_modal(&app);
    if !dependency_info.is_empty()
        && let crate_root::state::Modal::Preflight { dep_selected, .. } = &app.modal
    {
        assert!(
            *dep_selected < dependency_info.len(),
            "Dependency selection should be within bounds"
        );
    }
    if !file_info.is_empty()
        && let crate_root::state::Modal::Preflight { file_selected, .. } = &app.modal
    {
        assert!(
            *file_selected < file_info.len(),
            "File selection should be within bounds"
        );
    }
    if !service_info.is_empty()
        && let crate_root::state::Modal::Preflight {
            service_selected, ..
        } = &app.modal
    {
        assert!(
            *service_selected < service_info.len(),
            "Service selection should be within bounds"
        );
    }

    // Final verification: All data is correct and no corruption
    let (_, _, _, dependency_info, file_info, service_info, sandbox_info, _, _) =
        assert_preflight_modal(&app);
    assert_eq!(
        dependency_info.len(),
        expected_dep_count,
        "Dependency count should match expected"
    );
    assert_eq!(
        file_info.len(),
        test_packages.len(),
        "File info count should match package count"
    );
    assert_eq!(
        service_info.len(),
        expected_service_count,
        "Service count should match expected"
    );
    assert_eq!(
        sandbox_info.len(),
        expected_sandbox_count,
        "Sandbox count should match expected"
    );

    // Verify data integrity - all packages should have their data
    for pkg in &test_packages {
        let pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&pkg.name))
            .collect();
        assert!(
            !pkg_deps.is_empty(),
            "Package {} should have dependencies",
            pkg.name
        );
        assert!(
            file_info.iter().any(|f| f.name == pkg.name),
            "Package {} should have file info",
            pkg.name
        );
        let pkg_services: Vec<_> = service_info
            .iter()
            .filter(|s| s.providers.contains(&pkg.name))
            .collect();
        assert!(
            !pkg_services.is_empty(),
            "Package {} should have services",
            pkg.name
        );
        if matches!(pkg.source, crate_root::state::Source::Aur) {
            assert!(
                sandbox_info.iter().any(|s| s.package_name == pkg.name),
                "AUR package {} should have sandbox info",
                pkg.name
            );
        }
    }
}
