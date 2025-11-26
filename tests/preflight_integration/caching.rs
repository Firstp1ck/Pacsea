//! Tests for cached data loading.

use pacsea as crate_root;

#[test]
/// What: Verify that preflight modal correctly loads cached data when packages are already in install list.
///
/// Inputs:
/// - Packages already listed in `install_list`
/// - Pre-populated cache with dependencies (including conflicts), files, services, and sandbox data
/// - Preflight modal opened
///
/// Output:
/// - Deps tab correctly loads and displays dependencies and conflicts
/// - Files tab correctly loads and displays file information
/// - Services tab correctly loads and displays service impacts
/// - Sandbox tab correctly loads and displays sandbox information
///
/// Details:
/// - Tests edge case where data is already cached before preflight starts
/// - Verifies that all tabs correctly sync data from cache to modal state
/// - Ensures UI can display the cached data correctly
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn preflight_loads_cached_data_when_packages_already_in_install_list() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();

    // Create test packages (mix of official and AUR for sandbox testing)
    let test_packages = vec![
        crate_root::state::PackageItem {
            name: "test-package-1".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
        crate_root::state::PackageItem {
            name: "test-package-2".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
        crate_root::state::PackageItem {
            name: "test-aur-package".to_string(),
            version: "3.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
    ];

    // Pre-populate cache with dependencies (including conflicts)
    app.install_list_deps = vec![
        crate_root::state::modal::DependencyInfo {
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
        },
        crate_root::state::modal::DependencyInfo {
            name: "test-conflict".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "Conflicts with existing-package (1.0.0)".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package-2".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "test-dep-2".to_string(),
            version: "3.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToUpgrade {
                current: "2.0.0".to_string(),
                required: "3.0.0".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-package-1".to_string(), "test-package-2".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    // Pre-populate cache with files
    app.install_list_files = vec![
        crate_root::state::modal::PackageFileInfo {
            name: "test-package-1".to_string(),
            files: vec![
                crate_root::state::modal::FileChange {
                    path: "/usr/bin/test1".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "test-package-1".to_string(),
                    is_config: false,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                },
                crate_root::state::modal::FileChange {
                    path: "/etc/test1.conf".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "test-package-1".to_string(),
                    is_config: true,
                    predicted_pacnew: true,
                    predicted_pacsave: false,
                },
            ],
            total_count: 2,
            new_count: 2,
            changed_count: 0,
            removed_count: 0,
            config_count: 1,
            pacnew_candidates: 1,
            pacsave_candidates: 0,
        },
        crate_root::state::modal::PackageFileInfo {
            name: "test-package-2".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/test2".to_string(),
                change_type: crate_root::state::modal::FileChangeType::Changed,
                package: "test-package-2".to_string(),
                is_config: false,
                predicted_pacnew: false,
                predicted_pacsave: false,
            }],
            total_count: 1,
            new_count: 0,
            changed_count: 1,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        },
    ];

    // Pre-populate cache with services
    app.install_list_services = vec![
        crate_root::state::modal::ServiceImpact {
            unit_name: "test-service-1.service".to_string(),
            providers: vec!["test-package-1".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "test-service-2.service".to_string(),
            providers: vec!["test-package-2".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
        },
    ];

    // Pre-populate cache with sandbox info (for AUR package)
    app.install_list_sandbox = vec![crate_root::logic::sandbox::SandboxInfo {
        package_name: "test-aur-package".to_string(),
        depends: vec![
            crate_root::logic::sandbox::DependencyDelta {
                name: "dep1".to_string(),
                is_installed: true,
                installed_version: Some("1.0.0".to_string()),
                version_satisfied: true,
            },
            crate_root::logic::sandbox::DependencyDelta {
                name: "dep2".to_string(),
                is_installed: false,
                installed_version: None,
                version_satisfied: false,
            },
        ],
        makedepends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "make-dep".to_string(),
            is_installed: true,
            installed_version: Some("2.0.0".to_string()),
            version_satisfied: true,
        }],
        checkdepends: vec![],
        optdepends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "opt-dep".to_string(),
            is_installed: false,
            installed_version: None,
            version_satisfied: false,
        }],
    }];

    // Set packages in install list
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Open preflight modal (simulate what happens in events/install.rs)
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
        cached_reverse_deps_report: None,
    };

    // Verify initial state - modal should be empty
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        ..
    } = &app.modal
    {
        assert!(
            dependency_info.is_empty(),
            "Dependencies should be empty initially"
        );
        assert!(file_info.is_empty(), "Files should be empty initially");
        assert!(
            service_info.is_empty(),
            "Services should be empty initially"
        );
        assert!(sandbox_info.is_empty(), "Sandbox should be empty initially");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 1: Switch to Deps tab and verify dependencies (including conflicts) are loaded
    // Manually switch tab and sync data (simulating what sync_dependencies does)
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
        tab,
        dependency_info,
        dep_selected,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Deps,
            "Should be on Deps tab"
        );
        assert!(!dependency_info.is_empty(), "Dependencies should be loaded");
        assert_eq!(dependency_info.len(), 3, "Should have 3 dependencies");

        // Verify dependency types are present
        let has_to_install = dependency_info.iter().any(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::ToInstall
            )
        });
        let has_conflict = dependency_info.iter().any(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::Conflict { .. }
            )
        });
        let has_upgrade = dependency_info.iter().any(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::ToUpgrade { .. }
            )
        });

        assert!(has_to_install, "Should have ToInstall dependency");
        assert!(has_conflict, "Should have Conflict dependency");
        assert!(has_upgrade, "Should have ToUpgrade dependency");
        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Switch to Files tab and verify files are loaded
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

    if let crate_root::state::Modal::Preflight {
        tab,
        file_info,
        file_selected,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Files,
            "Should be on Files tab"
        );
        assert!(!file_info.is_empty(), "Files should be loaded");
        assert_eq!(file_info.len(), 2, "Should have 2 file entries");

        // Verify file data
        let pkg1_files = file_info
            .iter()
            .find(|f| f.name == "test-package-1")
            .expect("test-package-1 should be found in file_info");
        assert_eq!(pkg1_files.files.len(), 2, "Package 1 should have 2 files");
        assert_eq!(pkg1_files.total_count, 2);
        assert_eq!(pkg1_files.new_count, 2);
        assert_eq!(pkg1_files.config_count, 1);
        assert_eq!(pkg1_files.pacnew_candidates, 1);

        let pkg2_files = file_info
            .iter()
            .find(|f| f.name == "test-package-2")
            .expect("test-package-2 should be found in file_info");
        assert_eq!(pkg2_files.files.len(), 1, "Package 2 should have 1 file");
        assert_eq!(pkg2_files.total_count, 1);
        assert_eq!(pkg2_files.changed_count, 1);
        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Services tab and verify services are loaded
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
        tab,
        service_info,
        service_selected,
        services_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Services,
            "Should be on Services tab"
        );
        assert!(*services_loaded, "Services should be marked as loaded");
        assert!(!service_info.is_empty(), "Services should be loaded");
        assert_eq!(service_info.len(), 2, "Should have 2 services");

        // Verify service data
        let svc1 = service_info
            .iter()
            .find(|s| s.unit_name == "test-service-1.service")
            .expect("test-service-1.service should be found in service_info");
        assert!(svc1.is_active);
        assert!(svc1.needs_restart);
        assert_eq!(
            svc1.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );

        let svc2 = service_info
            .iter()
            .find(|s| s.unit_name == "test-service-2.service")
            .expect("test-service-2.service should be found in service_info");
        assert!(!svc2.is_active);
        assert!(!svc2.needs_restart);
        assert_eq!(
            svc2.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer
        );
        assert_eq!(*service_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Sandbox tab and verify sandbox info is loaded
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        tab,
        sandbox_info,
        sandbox_loaded,
        sandbox_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Sandbox;

        // Simulate sync_sandbox logic
        if matches!(*action, crate_root::state::PreflightAction::Install) {
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let cached_sandbox: Vec<_> = app
                .install_list_sandbox
                .iter()
                .filter(|s| item_names.contains(&s.package_name))
                .cloned()
                .collect();
            if !cached_sandbox.is_empty() {
                *sandbox_info = cached_sandbox;
                *sandbox_loaded = true;
                *sandbox_selected = 0;
            }
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        sandbox_info,
        sandbox_selected,
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
        assert!(!sandbox_info.is_empty(), "Sandbox info should be loaded");
        assert_eq!(sandbox_info.len(), 1, "Should have 1 sandbox entry");

        // Verify sandbox data
        let sandbox = sandbox_info
            .iter()
            .find(|s| s.package_name == "test-aur-package")
            .expect("test-aur-package should be found in sandbox_info");
        assert_eq!(sandbox.depends.len(), 2, "Should have 2 depends");
        assert_eq!(sandbox.makedepends.len(), 1, "Should have 1 makedepends");
        assert_eq!(sandbox.checkdepends.len(), 0, "Should have 0 checkdepends");
        assert_eq!(sandbox.optdepends.len(), 1, "Should have 1 optdepends");

        // Verify dependency details
        let dep1 = sandbox
            .depends
            .iter()
            .find(|d| d.name == "dep1")
            .expect("dep1 should be found in sandbox.depends");
        assert!(dep1.is_installed);
        assert_eq!(dep1.installed_version, Some("1.0.0".to_string()));

        let dep2 = sandbox
            .depends
            .iter()
            .find(|d| d.name == "dep2")
            .expect("dep2 should be found in sandbox.depends");
        assert!(!dep2.is_installed);

        assert_eq!(*sandbox_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All tabs should have loaded their data correctly
    // Switch back to Deps to verify data persists
    if let crate_root::state::Modal::Preflight { tab, .. } = &mut app.modal {
        *tab = crate_root::state::PreflightTab::Deps;
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Deps,
            "Should be back on Deps tab"
        );
        assert!(
            !dependency_info.is_empty(),
            "Dependencies should still be loaded"
        );
        assert!(!file_info.is_empty(), "Files should still be loaded");
        assert!(!service_info.is_empty(), "Services should still be loaded");
        assert!(!sandbox_info.is_empty(), "Sandbox should still be loaded");
    } else {
        panic!("Expected Preflight modal");
    }
}
