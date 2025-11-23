//! //! Tests for AUR and official package mixing.

use pacsea as crate_root;

#[test]
/// What: Verify that preflight modal handles mix of AUR and official packages correctly.
///
/// Inputs:
/// - Mix of AUR and official packages in `install_list`
/// - Different loading characteristics for each type
/// - Preflight modal opened with both types
///
/// Output:
/// - Sandbox tab only shows AUR packages
/// - Other tabs (Deps, Files, Services) show all packages
/// - AUR-specific features (sandbox) work correctly
/// - Official packages are excluded from sandbox
///
/// Details:
/// - Tests that filtering works correctly for AUR vs official packages
/// - Verifies sandbox tab only displays AUR packages
/// - Ensures other tabs display all packages regardless of source
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn preflight_handles_aur_and_official_package_mix() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();

    let test_packages = vec![
        crate_root::state::PackageItem {
            name: "test-official-package".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
        crate_root::state::PackageItem {
            name: "test-aur-package".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        },
    ];

    // Pre-populate cache with dependencies for both packages
    app.install_list_deps = vec![
        crate_root::state::modal::DependencyInfo {
            name: "official-dep-1".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-official-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "aur-dep-1".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Aur,
            required_by: vec!["test-aur-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    // Pre-populate cache with files for both packages
    app.install_list_files = vec![
        crate_root::state::modal::PackageFileInfo {
            name: "test-official-package".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/official".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "test-official-package".to_string(),
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
        },
        crate_root::state::modal::PackageFileInfo {
            name: "test-aur-package".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/aur".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "test-aur-package".to_string(),
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
        },
    ];

    // Pre-populate cache with services for both packages
    app.install_list_services = vec![
        crate_root::state::modal::ServiceImpact {
            unit_name: "official-service.service".to_string(),
            providers: vec!["test-official-package".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "aur-service.service".to_string(),
            providers: vec!["test-aur-package".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
        },
    ];

    // Pre-populate cache with sandbox info (only for AUR package)
    app.install_list_sandbox = vec![crate_root::logic::sandbox::SandboxInfo {
        package_name: "test-aur-package".to_string(),
        depends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "aur-dep-1".to_string(),
            is_installed: false,
            installed_version: None,
            version_satisfied: false,
        }],
        makedepends: vec![],
        checkdepends: vec![],
        optdepends: vec![],
    }];

    // Set packages in install list
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Open preflight modal
    app.modal = crate_root::state::Modal::Preflight {
        items: test_packages.clone(),
        action: crate_root::state::PreflightAction::Install,
        tab: crate_root::state::PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate_root::state::modal::PreflightHeaderChips {
            package_count: test_packages.len(),
            download_bytes: 0,
            install_delta_bytes: 0,
            aur_count: 1,
            risk_score: 0,
            risk_level: crate_root::state::modal::RiskLevel::Low,
        },
        dependency_info: Vec::new(),
        dep_selected: 0,
        dep_tree_expanded: std::collections::HashSet::new(),
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: std::collections::HashSet::new(),
        files_error: None,
        service_info: Vec::new(),
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: crate_root::state::modal::CascadeMode::Basic,
    };

    // Test 1: Deps tab - should show dependencies for both packages
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        tab,
        dependency_info,
        dep_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Deps;

        // Simulate sync_dependencies logic
        if matches!(*action, crate_root::state::PreflightAction::Install) {
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let filtered: Vec<_> = app
                .install_list_deps
                .iter()
                .filter(|dep| {
                    dep.required_by
                        .iter()
                        .any(|req_by| item_names.contains(req_by))
                })
                .cloned()
                .collect();
            if !filtered.is_empty() {
                *dependency_info = filtered;
                *dep_selected = 0;
            }
        }
    }

    if let crate_root::state::Modal::Preflight {
        dependency_info, ..
    } = &app.modal
    {
        assert_eq!(
            dependency_info.len(),
            2,
            "Should have 2 dependencies (one for each package)"
        );
        assert!(
            dependency_info
                .iter()
                .any(|d| d.required_by.contains(&"test-official-package".to_string())),
            "Should have dependency for official package"
        );
        assert!(
            dependency_info
                .iter()
                .any(|d| d.required_by.contains(&"test-aur-package".to_string())),
            "Should have dependency for AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Files tab - should show files for both packages
    if let crate_root::state::Modal::Preflight {
        items,
        tab,
        file_info,
        file_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Files;

        // Simulate sync_files logic
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_files: Vec<_> = app
            .install_list_files
            .iter()
            .filter(|file_info| item_names.contains(&file_info.name))
            .cloned()
            .collect();
        if !cached_files.is_empty() {
            *file_info = cached_files;
            *file_selected = 0;
        }
    }

    if let crate_root::state::Modal::Preflight { file_info, .. } = &app.modal {
        assert_eq!(
            file_info.len(),
            2,
            "Should have 2 file entries (one for each package)"
        );
        assert!(
            file_info.iter().any(|f| f.name == "test-official-package"),
            "Should have files for official package"
        );
        assert!(
            file_info.iter().any(|f| f.name == "test-aur-package"),
            "Should have files for AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Services tab - should show services for both packages
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        tab,
        service_info,
        service_selected,
        services_loaded,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Services;

        // Simulate sync_services logic
        if matches!(*action, crate_root::state::PreflightAction::Install) {
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let cached_services: Vec<_> = app
                .install_list_services
                .iter()
                .filter(|s| s.providers.iter().any(|p| item_names.contains(p)))
                .cloned()
                .collect();
            if !cached_services.is_empty() {
                *service_info = cached_services;
                *services_loaded = true;
                *service_selected = 0;
            }
        }
    }

    if let crate_root::state::Modal::Preflight {
        service_info,
        services_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            service_info.len(),
            2,
            "Should have 2 services (one for each package)"
        );
        assert!(*services_loaded, "Services should be marked as loaded");
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"test-official-package".to_string())),
            "Should have service for official package"
        );
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"test-aur-package".to_string())),
            "Should have service for AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Sandbox tab - should ONLY show AUR package (official excluded)
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        tab,
        sandbox_info,
        sandbox_loaded,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Sandbox;

        // Simulate sync_sandbox logic - should filter to only AUR packages
        if matches!(*action, crate_root::state::PreflightAction::Install) {
            // Filter items to only AUR packages
            let aur_items: Vec<_> = items
                .iter()
                .filter(|p| matches!(p.source, crate_root::state::Source::Aur))
                .map(|p| p.name.clone())
                .collect();
            let cached_sandbox: Vec<_> = app
                .install_list_sandbox
                .iter()
                .filter(|s| aur_items.contains(&s.package_name))
                .cloned()
                .collect();
            if !cached_sandbox.is_empty() {
                *sandbox_info = cached_sandbox;
                *sandbox_loaded = true;
            }
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        sandbox_info,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Sandbox,
            "Should be on Sandbox tab"
        );
        assert!(*sandbox_loaded, "Sandbox should be marked as loaded");
        assert_eq!(
            sandbox_info.len(),
            1,
            "Should have 1 sandbox entry (only AUR package)"
        );
        assert_eq!(
            sandbox_info[0].package_name, "test-aur-package",
            "Sandbox should only contain AUR package"
        );
        // Verify official package is NOT in sandbox
        assert!(
            !sandbox_info
                .iter()
                .any(|s| s.package_name == "test-official-package"),
            "Official package should NOT be in sandbox"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All tabs show correct data
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        ..
    } = &app.modal
    {
        // Deps, Files, Services should show both packages
        assert_eq!(dependency_info.len(), 2, "Deps should show both packages");
        assert_eq!(file_info.len(), 2, "Files should show both packages");
        assert_eq!(service_info.len(), 2, "Services should show both packages");

        // Sandbox should only show AUR package
        assert_eq!(
            sandbox_info.len(),
            1,
            "Sandbox should only show AUR package"
        );
        assert_eq!(
            sandbox_info[0].package_name, "test-aur-package",
            "Sandbox should contain AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }
}
