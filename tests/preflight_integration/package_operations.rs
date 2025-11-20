//! //! Tests for package operations and management.

use pacsea as crate_root;
use super::helpers::*;


#[test]
/// What: Verify that adding a second package to install list preserves first package's cached data.
///
/// Inputs:
/// - First package already in install_list with cached data
/// - Second package added to install_list
/// - Preflight modal opened with both packages
///
/// Output:
/// - First package's cached data is preserved (except for conflict checking)
/// - Both packages are correctly loaded in all tabs
/// - Conflicts between packages are detected
///
/// Details:
/// - Tests edge case where install list grows after initial caching
/// - Verifies that existing cached data is not lost when new packages are added
/// - Ensures conflict detection works correctly between packages
fn preflight_preserves_first_package_when_second_package_added() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    // First package with cached data
    let first_package = crate_root::state::PackageItem {
        name: "first-package".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    };

    // Pre-populate cache with first package's data
    app.install_list_deps = vec![
        crate_root::state::modal::DependencyInfo {
            name: "first-dep-1".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["first-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "first-dep-2".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["first-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    app.install_list_files = vec![crate_root::state::modal::PackageFileInfo {
        name: "first-package".to_string(),
        files: vec![
            crate_root::state::modal::FileChange {
                path: "/usr/bin/first".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "first-package".to_string(),
                is_config: false,
                predicted_pacnew: false,
                predicted_pacsave: false,
            },
            crate_root::state::modal::FileChange {
                path: "/etc/first.conf".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "first-package".to_string(),
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
    }];

    app.install_list_services = vec![crate_root::state::modal::ServiceImpact {
        unit_name: "first-service.service".to_string(),
        providers: vec!["first-package".to_string()],
        is_active: true,
        needs_restart: true,
        recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
    }];

    // Set first package in install list
    app.install_list = vec![first_package.clone()];
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Now add second package
    let second_package = crate_root::state::PackageItem {
        name: "second-package".to_string(),
        version: "2.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    };

    // Add second package's data to cache (simulating it being resolved)
    app.install_list_deps
        .push(crate_root::state::modal::DependencyInfo {
            name: "second-dep-1".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["second-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        });

    // Add a conflict: second package conflicts with first-dep-1
    app.install_list_deps
        .push(crate_root::state::modal::DependencyInfo {
            name: "first-dep-1".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "Conflicts with first-package's dependency first-dep-1 (1.0.0)".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["second-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        });

    app.install_list_files
        .push(crate_root::state::modal::PackageFileInfo {
            name: "second-package".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/second".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "second-package".to_string(),
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
        });

    app.install_list_services
        .push(crate_root::state::modal::ServiceImpact {
            unit_name: "second-service.service".to_string(),
            providers: vec!["second-package".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
        });

    // Update install list to include both packages
    app.install_list = vec![first_package.clone(), second_package.clone()];

    // Open preflight modal with both packages
    app.modal = crate_root::state::Modal::Preflight {
        items: vec![first_package.clone(), second_package.clone()],
        action: crate_root::state::PreflightAction::Install,
        tab: crate_root::state::PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate_root::state::modal::PreflightHeaderChips {
            package_count: 2,
            download_bytes: 0,
            install_delta_bytes: 0,
            aur_count: 0,
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
        sandbox_loaded: true,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: crate_root::state::modal::CascadeMode::Basic,
    };

    // Test 1: Verify Deps tab loads both packages correctly and detects conflicts
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

        // Verify first package's dependencies are present
        let first_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"first-package".to_string()))
            .collect();
        assert!(
            !first_deps.is_empty(),
            "First package's dependencies should be present"
        );
        assert_eq!(
            first_deps.len(),
            2,
            "First package should have 2 dependencies"
        );

        // Verify second package's dependencies are present
        let second_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"second-package".to_string()))
            .collect();
        assert!(
            !second_deps.is_empty(),
            "Second package's dependencies should be present"
        );
        assert_eq!(
            second_deps.len(),
            2,
            "Second package should have 2 dependencies (one is conflict)"
        );

        // Verify conflict is detected
        let conflicts: Vec<_> = dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .collect();
        assert!(!conflicts.is_empty(), "Conflict should be detected");
        assert_eq!(conflicts.len(), 1, "Should have 1 conflict");

        // Verify conflict involves first-dep-1
        let conflict = conflicts[0];
        assert_eq!(conflict.name, "first-dep-1");
        assert!(conflict.required_by.contains(&"second-package".to_string()));

        // Verify first package's original dependencies are unchanged
        let first_dep_1 = dependency_info
            .iter()
            .find(|d| {
                d.name == "first-dep-1"
                    && d.required_by.contains(&"first-package".to_string())
                    && matches!(
                        d.status,
                        crate_root::state::modal::DependencyStatus::ToInstall
                    )
            })
            .expect("First package's first-dep-1 should still be ToInstall");
        assert_eq!(first_dep_1.version, "1.0.0");

        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Verify Files tab loads both packages correctly
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

        // Verify first package's files are preserved
        let first_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .unwrap();
        assert_eq!(
            first_files.files.len(),
            2,
            "First package should have 2 files"
        );
        assert_eq!(first_files.total_count, 2);
        assert_eq!(first_files.new_count, 2);
        assert_eq!(first_files.config_count, 1);
        assert_eq!(first_files.pacnew_candidates, 1);

        // Verify second package's files are loaded
        let second_files = file_info
            .iter()
            .find(|f| f.name == "second-package")
            .unwrap();
        assert_eq!(
            second_files.files.len(),
            1,
            "Second package should have 1 file"
        );
        assert_eq!(second_files.total_count, 1);
        assert_eq!(second_files.new_count, 1);

        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Verify Services tab loads both packages correctly
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

        // Verify first package's service is preserved
        let first_svc = service_info
            .iter()
            .find(|s| s.unit_name == "first-service.service")
            .unwrap();
        assert!(first_svc.is_active);
        assert!(first_svc.needs_restart);
        assert_eq!(
            first_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
        assert!(first_svc.providers.contains(&"first-package".to_string()));

        // Verify second package's service is loaded
        let second_svc = service_info
            .iter()
            .find(|s| s.unit_name == "second-service.service")
            .unwrap();
        assert!(!second_svc.is_active);
        assert!(!second_svc.needs_restart);
        assert_eq!(
            second_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer
        );
        assert!(second_svc.providers.contains(&"second-package".to_string()));

        assert_eq!(*service_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All data for both packages should be present
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        ..
    } = &app.modal
    {
        // Verify both packages have dependencies
        let first_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"first-package".to_string()))
            .collect();
        let second_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"second-package".to_string()))
            .collect();
        assert!(
            !first_pkg_deps.is_empty(),
            "First package should have dependencies"
        );
        assert!(
            !second_pkg_deps.is_empty(),
            "Second package should have dependencies"
        );

        // Verify both packages have files
        assert!(
            file_info.iter().any(|f| f.name == "first-package"),
            "First package should have files"
        );
        assert!(
            file_info.iter().any(|f| f.name == "second-package"),
            "Second package should have files"
        );

        // Verify both packages have services
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"first-package".to_string())),
            "First package should have services"
        );
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"second-package".to_string())),
            "Second package should have services"
        );
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that adding a second package while first package is loading preserves independence.
///
/// Inputs:
/// - First package added to install_list and starts loading
/// - Second package added while first package is still loading
/// - Preflight modal opened with both packages
///
/// Output:
/// - First package's data is not influenced by second package (except conflict detection)
/// - Second package's data is not influenced by first package
/// - Both packages load correctly in all tabs
/// - Conflicts are detected if present
///
/// Details:
/// - Tests edge case where packages are added sequentially while resolution is in progress
/// - Verifies that each package's data remains independent
/// - Ensures conflict detection works correctly between independently loaded packages
fn preflight_independent_loading_when_packages_added_sequentially() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    // First package
    let first_package = crate_root::state::PackageItem {
        name: "first-package".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    };

    // Simulate first package being added and starting to load
    // Some data is already cached, some is still resolving
    app.install_list = vec![first_package.clone()];
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // First package's dependencies (partially loaded)
    app.install_list_deps = vec![crate_root::state::modal::DependencyInfo {
        name: "first-dep-1".to_string(),
        version: "1.0.0".to_string(),
        status: crate_root::state::modal::DependencyStatus::ToInstall,
        source: crate_root::state::modal::DependencySource::Official {
            repo: "core".to_string(),
        },
        required_by: vec!["first-package".to_string()],
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }];

    // First package's files (loaded)
    app.install_list_files = vec![crate_root::state::modal::PackageFileInfo {
        name: "first-package".to_string(),
        files: vec![crate_root::state::modal::FileChange {
            path: "/usr/bin/first".to_string(),
            change_type: crate_root::state::modal::FileChangeType::New,
            package: "first-package".to_string(),
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

    // First package's services (still loading - not in cache yet)
    app.install_list_services = vec![];

    // Simulate first package's dependency resolution still in progress
    app.preflight_deps_resolving = true;
    app.preflight_deps_items = Some(vec![first_package.clone()]);

    // Now add second package while first is still loading
    let second_package = crate_root::state::PackageItem {
        name: "second-package".to_string(),
        version: "2.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    };

    // Update install list to include both packages
    app.install_list = vec![first_package.clone(), second_package.clone()];

    // Add second package's data to cache (independent of first package)
    app.install_list_deps
        .push(crate_root::state::modal::DependencyInfo {
            name: "second-dep-1".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["second-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        });

    // Add a conflict: second package requires a different version of first-dep-1
    app.install_list_deps
        .push(crate_root::state::modal::DependencyInfo {
            name: "first-dep-1".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "Conflicts with first-package's dependency first-dep-1 (1.0.0)".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["second-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        });

    // Second package's files (loaded independently)
    app.install_list_files
        .push(crate_root::state::modal::PackageFileInfo {
            name: "second-package".to_string(),
            files: vec![
                crate_root::state::modal::FileChange {
                    path: "/usr/bin/second".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "second-package".to_string(),
                    is_config: false,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                },
                crate_root::state::modal::FileChange {
                    path: "/etc/second.conf".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "second-package".to_string(),
                    is_config: true,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                },
            ],
            total_count: 2,
            new_count: 2,
            changed_count: 0,
            removed_count: 0,
            config_count: 1,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        });

    // Second package's services (loaded independently)
    app.install_list_services
        .push(crate_root::state::modal::ServiceImpact {
            unit_name: "second-service.service".to_string(),
            providers: vec!["second-package".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        });

    // Open preflight modal with both packages
    app.modal = crate_root::state::Modal::Preflight {
        items: vec![first_package.clone(), second_package.clone()],
        action: crate_root::state::PreflightAction::Install,
        tab: crate_root::state::PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate_root::state::modal::PreflightHeaderChips {
            package_count: 2,
            download_bytes: 0,
            install_delta_bytes: 0,
            aur_count: 0,
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
        sandbox_loaded: true,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: crate_root::state::modal::CascadeMode::Basic,
    };

    // Test 1: Verify Deps tab loads both packages independently and detects conflicts
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

        // Verify first package's dependencies are independent
        let first_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"first-package".to_string()))
            .collect();
        assert!(
            !first_deps.is_empty(),
            "First package's dependencies should be present"
        );
        assert_eq!(
            first_deps.len(),
            1,
            "First package should have 1 dependency (first-dep-1)"
        );

        // Verify first package's dependency is correct and independent
        let first_dep_1 = first_deps
            .iter()
            .find(|d| d.name == "first-dep-1")
            .expect("First package should have first-dep-1");
        assert_eq!(first_dep_1.version, "1.0.0");
        assert!(matches!(
            first_dep_1.status,
            crate_root::state::modal::DependencyStatus::ToInstall
        ));

        // Verify second package's dependencies are independent
        let second_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"second-package".to_string()))
            .collect();
        assert!(
            !second_deps.is_empty(),
            "Second package's dependencies should be present"
        );
        assert_eq!(
            second_deps.len(),
            2,
            "Second package should have 2 dependencies (one is conflict)"
        );

        // Verify conflict is detected
        let conflicts: Vec<_> = dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .collect();
        assert!(!conflicts.is_empty(), "Conflict should be detected");
        assert_eq!(conflicts.len(), 1, "Should have 1 conflict");

        // Verify conflict involves first-dep-1 but is required by second package
        let conflict = conflicts[0];
        assert_eq!(conflict.name, "first-dep-1");
        assert_eq!(conflict.version, "2.0.0");
        assert!(conflict.required_by.contains(&"second-package".to_string()));
        assert!(!conflict.required_by.contains(&"first-package".to_string()));

        // Verify first package's dependency is not affected by conflict
        // (first package still has its own first-dep-1 with version 1.0.0)
        assert_eq!(first_dep_1.version, "1.0.0");
        assert!(matches!(
            first_dep_1.status,
            crate_root::state::modal::DependencyStatus::ToInstall
        ));

        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Verify Files tab loads both packages independently
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

        // Verify first package's files are independent
        let first_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .unwrap();
        assert_eq!(
            first_files.files.len(),
            1,
            "First package should have 1 file"
        );
        assert_eq!(first_files.total_count, 1);
        assert_eq!(first_files.new_count, 1);
        assert_eq!(first_files.config_count, 0);

        // Verify second package's files are independent
        let second_files = file_info
            .iter()
            .find(|f| f.name == "second-package")
            .unwrap();
        assert_eq!(
            second_files.files.len(),
            2,
            "Second package should have 2 files"
        );
        assert_eq!(second_files.total_count, 2);
        assert_eq!(second_files.new_count, 2);
        assert_eq!(second_files.config_count, 1);

        // Verify files are independent - first package's file count is not affected
        assert_eq!(first_files.files.len(), 1);
        assert_eq!(second_files.files.len(), 2);

        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Verify Services tab loads both packages independently
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

        // Verify second package's service is loaded (first package's service was still loading)
        assert_eq!(
            service_info.len(),
            1,
            "Should have 1 service (second package's, first still loading)"
        );

        // Verify second package's service is independent
        let second_svc = service_info
            .iter()
            .find(|s| s.unit_name == "second-service.service")
            .unwrap();
        assert!(second_svc.is_active);
        assert!(second_svc.needs_restart);
        assert_eq!(
            second_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
        assert!(second_svc.providers.contains(&"second-package".to_string()));
        assert!(!second_svc.providers.contains(&"first-package".to_string()));

        assert_eq!(*service_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: Both packages should be independent
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        ..
    } = &app.modal
    {
        // Verify first package's data is independent
        let first_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"first-package".to_string()))
            .collect();
        let first_pkg_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .unwrap();

        // Verify second package's data is independent
        let second_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"second-package".to_string()))
            .collect();
        let second_pkg_files = file_info
            .iter()
            .find(|f| f.name == "second-package")
            .unwrap();

        // Verify independence: first package's data is not affected by second
        assert_eq!(
            first_pkg_deps.len(),
            1,
            "First package should have 1 dependency (independent)"
        );
        assert_eq!(
            first_pkg_files.files.len(),
            1,
            "First package should have 1 file (independent)"
        );

        // Verify independence: second package's data is not affected by first
        assert_eq!(
            second_pkg_deps.len(),
            2,
            "Second package should have 2 dependencies (independent, one conflict)"
        );
        assert_eq!(
            second_pkg_files.files.len(),
            2,
            "Second package should have 2 files (independent)"
        );

        // Verify conflict detection works (the only interaction between packages)
        let conflicts: Vec<_> = dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .collect();
        assert!(!conflicts.is_empty(), "Conflict should be detected");
        assert_eq!(conflicts.len(), 1, "Should have exactly 1 conflict");
    } else {
        panic!("Expected Preflight modal");
    }
}
