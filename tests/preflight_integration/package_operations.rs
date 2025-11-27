//! //! Tests for package operations and management.

use pacsea as crate_root;

/// Sets up first package with cached dependencies, files, and services.
///
/// What: Creates a first package and pre-populates app state with its cached data
/// including dependencies, files, and services to simulate a package that was already
/// added and resolved.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` to populate with cached data
///
/// Output:
/// - Returns the created `PackageItem` for "first-package"
/// - Updates `app.install_list_deps` with 2 dependencies (first-dep-1, first-dep-2)
/// - Updates `app.install_list_files` with file info (2 files, 1 config, 1 pacnew candidate)
/// - Updates `app.install_list_services` with service info (first-service.service)
/// - Sets `app.install_list` to contain the first package
/// - Sets `app.preflight_cancelled` to false
///
/// Details:
/// - Creates a package from "core" repo with version "1.0.0"
/// - Sets up dependencies from both "core" and "extra" repos
/// - Includes one config file that will generate a pacnew file
/// - Service is active and requires restart
fn setup_first_package_with_cache(
    app: &mut crate_root::state::AppState,
) -> crate_root::state::PackageItem {
    let first_package = crate_root::state::PackageItem {
        name: "first-package".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
    };

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

    app.install_list = vec![first_package.clone()];
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    first_package
}

/// Adds second package with conflict to app state.
///
/// What: Creates a second package and adds its cached data to app state, including
/// a dependency conflict with the first package's dependencies.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` to append cached data to
///
/// Output:
/// - Returns the created `PackageItem` for "second-package"
/// - Appends to `app.install_list_deps`:
///   - second-dep-1 (`ToInstall` status)
///   - first-dep-1 version 2.0.0 (Conflict status, conflicts with first package)
/// - Appends to `app.install_list_files` with file info (1 file, no config)
/// - Appends to `app.install_list_services` with service info (second-service.service)
///
/// Details:
/// - Creates a package from "extra" repo with version "2.0.0"
/// - Introduces a conflict: requires first-dep-1 version 2.0.0 while first package
///   requires first-dep-1 version 1.0.0
/// - Service is inactive and does not need restart
fn setup_second_package_with_conflict(
    app: &mut crate_root::state::AppState,
) -> crate_root::state::PackageItem {
    let second_package = crate_root::state::PackageItem {
        name: "second-package".to_string(),
        version: "2.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
    };

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

    second_package
}

/// Creates and opens preflight modal with both packages.
///
/// What: Initializes and opens a Preflight modal containing both packages for
/// installation, setting up the modal state with default values.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` to update
/// - `first_package`: First package to include in the modal
/// - `second_package`: Second package to include in the modal
///
/// Output:
/// - Updates `app.install_list` to contain both packages
/// - Sets `app.modal` to Preflight variant with:
///   - Both packages in items list
///   - Install action
///   - Summary tab as initial tab
///   - Empty `dependency_info`, `file_info`, `service_info` (to be populated by tabs)
///   - Default header chips with `package_count=2`
///
/// Details:
/// - Modal starts on Summary tab
/// - All tab-specific data structures are initialized as empty
/// - Header chips indicate 2 packages, 0 download bytes, low risk level
fn open_preflight_modal(
    app: &mut crate_root::state::AppState,
    first_package: crate_root::state::PackageItem,
    second_package: crate_root::state::PackageItem,
) {
    app.install_list = vec![first_package.clone(), second_package.clone()];

    app.modal = crate_root::state::Modal::Preflight {
        items: vec![first_package, second_package],
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
        cached_reverse_deps_report: None,
    };
}

/// Syncs dependencies tab and verifies both packages' dependencies and conflicts.
///
/// What: Switches to Deps tab, syncs dependencies from cache, and verifies that
/// both packages' dependencies are correctly loaded and conflicts are detected.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` containing preflight modal and cached dependencies
///
/// Output:
/// - Switches modal tab to Deps
/// - Populates `dependency_info` with filtered dependencies from `app.install_list_deps`
/// - Resets `dep_selected` to 0
/// - Asserts that dependencies are loaded correctly
/// - Asserts that conflicts are detected
///
/// Details:
/// - Filters dependencies by checking if any `required_by` package is in the modal's items
/// - Verifies first package has 2 dependencies (first-dep-1, first-dep-2)
/// - Verifies second package has 2 dependencies (second-dep-1, conflict entry for first-dep-1)
/// - Verifies exactly 1 conflict exists (first-dep-1 version conflict)
/// - Verifies first package's first-dep-1 remains `ToInstall` with version 1.0.0
fn test_deps_tab(app: &mut crate_root::state::AppState) {
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

        let conflict = conflicts[0];
        assert_eq!(conflict.name, "first-dep-1");
        assert!(conflict.required_by.contains(&"second-package".to_string()));

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
}

/// Syncs files tab and verifies both packages' files are preserved.
///
/// What: Switches to Files tab, syncs file information from cache, and verifies
/// that both packages' file data is correctly loaded and preserved.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` containing preflight modal and cached file info
///
/// Output:
/// - Switches modal tab to Files
/// - Populates `file_info` with filtered file info from `app.install_list_files`
/// - Resets `file_selected` to 0
/// - Asserts that files are loaded correctly for both packages
///
/// Details:
/// - Filters file info by checking if package name is in the modal's items
/// - Verifies first package has 2 files (1 config file with pacnew candidate)
/// - Verifies second package has 1 file (no config files)
/// - Verifies file counts, config counts, and pacnew candidates are correct
fn test_files_tab(app: &mut crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight {
        items,
        tab,
        file_info,
        file_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Files;

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

        let first_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .expect("first-package should be found in file_info");
        assert_eq!(
            first_files.files.len(),
            2,
            "First package should have 2 files"
        );
        assert_eq!(first_files.total_count, 2);
        assert_eq!(first_files.new_count, 2);
        assert_eq!(first_files.config_count, 1);
        assert_eq!(first_files.pacnew_candidates, 1);

        let second_files = file_info
            .iter()
            .find(|f| f.name == "second-package")
            .expect("second-package should be found in file_info");
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
}

/// Syncs services tab and verifies both packages' services are preserved.
///
/// What: Switches to Services tab, syncs service information from cache, and
/// verifies that both packages' service data is correctly loaded and preserved.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` containing preflight modal and cached service info
///
/// Output:
/// - Switches modal tab to Services
/// - Populates `service_info` with filtered services from `app.install_list_services`
/// - Sets `services_loaded` to true
/// - Resets `service_selected` to 0
/// - Asserts that services are loaded correctly for both packages
///
/// Details:
/// - Filters services by checking if any provider is in the modal's items
/// - Verifies first package's service (first-service.service) is active and needs restart
/// - Verifies second package's service (second-service.service) is inactive and defers restart
/// - Verifies service restart decisions match expected values
fn test_services_tab(app: &mut crate_root::state::AppState) {
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

        let first_svc = service_info
            .iter()
            .find(|s| s.unit_name == "first-service.service")
            .expect("first-service.service should be found in service_info");
        assert!(first_svc.is_active);
        assert!(first_svc.needs_restart);
        assert_eq!(
            first_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
        assert!(first_svc.providers.contains(&"first-package".to_string()));

        let second_svc = service_info
            .iter()
            .find(|s| s.unit_name == "second-service.service")
            .expect("second-service.service should be found in service_info");
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
}

/// Verifies that all data for both packages is present in the modal.
///
/// What: Performs final verification that all cached data (dependencies, files,
/// and services) for both packages is present and accessible in the preflight modal.
///
/// Inputs:
/// - `app`: Immutable reference to `AppState` containing the preflight modal
///
/// Output:
/// - Asserts that both packages have dependencies in `dependency_info`
/// - Asserts that both packages have files in `file_info`
/// - Asserts that both packages have services in `service_info`
///
/// Details:
/// - Final comprehensive check after all tabs have been tested
/// - Verifies data preservation across all three data types (deps, files, services)
/// - Ensures no data loss occurred during tab switching and syncing
fn verify_all_data_present(app: &crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        ..
    } = &app.modal
    {
        assert!(
            dependency_info
                .iter()
                .any(|d| d.required_by.contains(&"first-package".to_string())),
            "First package should have dependencies"
        );
        assert!(
            dependency_info
                .iter()
                .any(|d| d.required_by.contains(&"second-package".to_string())),
            "Second package should have dependencies"
        );

        assert!(
            file_info.iter().any(|f| f.name == "first-package"),
            "First package should have files"
        );
        assert!(
            file_info.iter().any(|f| f.name == "second-package"),
            "Second package should have files"
        );

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
/// What: Verify that adding a second package to install list preserves first package's cached data.
///
/// Inputs:
/// - First package already in `install_list` with cached data
/// - Second package added to `install_list`
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

    let mut app = crate_root::state::AppState::default();

    let first_package = setup_first_package_with_cache(&mut app);
    let second_package = setup_second_package_with_conflict(&mut app);
    open_preflight_modal(&mut app, first_package, second_package);

    test_deps_tab(&mut app);
    test_files_tab(&mut app);
    test_services_tab(&mut app);
    verify_all_data_present(&app);
}

/// Helper: Set up test data for independent loading test.
///
/// Sets up test data for independent loading scenario.
///
/// What: Creates first and second packages with their dependencies, files, and services,
/// simulating a scenario where the first package is partially loaded when the second
/// package is added.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` to populate with test data
///
/// Output:
/// - Returns tuple of (`first_package`, `second_package`)
/// - Updates `app.install_list` to contain both packages
/// - Sets `app.preflight_deps_resolving` to true (simulating ongoing resolution)
/// - Sets `app.preflight_deps_items` to first package (simulating in-progress resolution)
/// - Populates `app.install_list_deps` with:
///   - First package: 1 dependency (first-dep-1)
///   - Second package: 2 dependencies (second-dep-1, conflict entry for first-dep-1)
/// - Populates `app.install_list_files` with file info for both packages
/// - Populates `app.install_list_services` with service info for second package only
///
/// Details:
/// - First package is partially loaded: dependencies partially loaded (1 of potentially more),
///   files loaded (1 file), services not loaded yet (empty)
/// - Second package is fully loaded independently: all dependencies, files, and services
/// - Includes a conflict: second package requires first-dep-1 version 2.0.0 while
///   first package requires version 1.0.0
fn setup_independent_loading_test_data(
    app: &mut crate_root::state::AppState,
) -> (
    crate_root::state::PackageItem,
    crate_root::state::PackageItem,
) {
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
        out_of_date: None,
        orphaned: false,
    };

    // Simulate first package being added and starting to load
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
    app.preflight_deps_items = Some((
        vec![first_package.clone()],
        crate_root::state::modal::PreflightAction::Install,
    ));

    // Second package
    let second_package = crate_root::state::PackageItem {
        name: "second-package".to_string(),
        version: "2.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
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

    (first_package, second_package)
}

/// Creates preflight modal with both packages.
///
/// What: Initializes a Preflight modal instance containing both packages for
/// installation, with all tab data structures initialized as empty.
///
/// Inputs:
/// - `first_package`: First package to include in the modal
/// - `second_package`: Second package to include in the modal
///
/// Output:
/// - Returns a Preflight Modal variant with:
///   - Both packages in items list
///   - Install action
///   - Summary tab as initial tab
///   - Empty `dependency_info`, `file_info`, `service_info` (to be populated by tabs)
///   - Default header chips with `package_count=2`
///
/// Details:
/// - Modal starts on Summary tab
/// - All tab-specific data structures are initialized as empty
/// - Header chips indicate 2 packages, 0 download bytes, low risk level
fn create_preflight_modal(
    first_package: crate_root::state::PackageItem,
    second_package: crate_root::state::PackageItem,
) -> crate_root::state::Modal {
    crate_root::state::Modal::Preflight {
        items: vec![first_package, second_package],
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
        cached_reverse_deps_report: None,
    }
}

/// Tests Deps tab for independent loading scenario.
///
/// What: Switches to Deps tab, syncs dependencies from cache, and verifies that
/// both packages' dependencies are loaded independently and conflicts are detected.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` containing preflight modal and cached dependencies
///
/// Output:
/// - Switches modal tab to Deps
/// - Populates `dependency_info` with filtered dependencies from `app.install_list_deps`
/// - Resets `dep_selected` to 0
/// - Asserts that dependencies are loaded correctly and independently
/// - Asserts that conflicts are detected
///
/// Details:
/// - Filters dependencies by checking if any `required_by` package is in the modal's items
/// - Verifies first package has 1 dependency (first-dep-1 version 1.0.0, `ToInstall`)
/// - Verifies second package has 2 dependencies (second-dep-1, conflict entry for first-dep-1)
/// - Verifies exactly 1 conflict exists (first-dep-1 version conflict)
/// - Verifies first package's dependency is not affected by the conflict
fn test_deps_tab_independent_loading(app: &mut crate_root::state::AppState) {
    // Switch to Deps tab and sync
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

    // Verify Deps tab state
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

        // Verify conflict details
        let conflict = conflicts[0];
        assert_eq!(conflict.name, "first-dep-1");
        assert_eq!(conflict.version, "2.0.0");
        assert!(conflict.required_by.contains(&"second-package".to_string()));
        assert!(!conflict.required_by.contains(&"first-package".to_string()));

        // Verify first package's dependency is not affected by conflict
        assert_eq!(first_dep_1.version, "1.0.0");
        assert!(matches!(
            first_dep_1.status,
            crate_root::state::modal::DependencyStatus::ToInstall
        ));

        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }
}

/// Tests Files tab for independent loading scenario.
///
/// What: Switches to Files tab, syncs file information from cache, and verifies
/// that both packages' file data is loaded independently.
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` containing preflight modal and cached file info
///
/// Output:
/// - Switches modal tab to Files
/// - Populates `file_info` with filtered file info from `app.install_list_files`
/// - Resets `file_selected` to 0
/// - Asserts that files are loaded correctly and independently for both packages
///
/// Details:
/// - Filters file info by checking if package name is in the modal's items
/// - Verifies first package has 1 file (no config files)
/// - Verifies second package has 2 files (1 config file)
/// - Verifies file counts are independent (first package's count not affected by second)
fn test_files_tab_independent_loading(app: &mut crate_root::state::AppState) {
    // Switch to Files tab and sync
    if let crate_root::state::Modal::Preflight {
        items,
        tab,
        file_info,
        file_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Files;

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

    // Verify Files tab state
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
            .expect("first-package should be found in file_info");
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
            .expect("second-package should be found in file_info");
        assert_eq!(
            second_files.files.len(),
            2,
            "Second package should have 2 files"
        );
        assert_eq!(second_files.total_count, 2);
        assert_eq!(second_files.new_count, 2);
        assert_eq!(second_files.config_count, 1);

        // Verify files are independent
        assert_eq!(first_files.files.len(), 1);
        assert_eq!(second_files.files.len(), 2);

        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }
}

/// Tests Services tab for independent loading scenario.
///
/// What: Switches to Services tab, syncs service information from cache, and
/// verifies that services load independently (second package's service loaded,
/// first package's service still loading).
///
/// Inputs:
/// - `app`: Mutable reference to `AppState` containing preflight modal and cached service info
///
/// Output:
/// - Switches modal tab to Services
/// - Populates `service_info` with filtered services from `app.install_list_services`
/// - Sets `services_loaded` to true
/// - Resets `service_selected` to 0
/// - Asserts that only second package's service is loaded (first still loading)
///
/// Details:
/// - Filters services by checking if any provider is in the modal's items
/// - Verifies only 1 service is loaded (second-service.service)
/// - First package's service is not in cache yet (simulating still loading)
/// - Verifies second package's service is active and needs restart
/// - Verifies service providers are correct (only second-package, not first-package)
fn test_services_tab_independent_loading(app: &mut crate_root::state::AppState) {
    // Switch to Services tab and sync
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

    // Verify Services tab state
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
        assert_eq!(
            service_info.len(),
            1,
            "Should have 1 service (second package's, first still loading)"
        );

        // Verify second package's service is independent
        let second_svc = service_info
            .iter()
            .find(|s| s.unit_name == "second-service.service")
            .expect("second-service.service should be found in service_info");
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
}

/// Verifies final independence of both packages' data.
///
/// What: Performs final comprehensive verification that both packages' data
/// (dependencies and files) remains independent after all tab operations.
///
/// Inputs:
/// - `app`: Immutable reference to `AppState` containing the preflight modal
///
/// Output:
/// - Asserts that dependency counts are independent for both packages
/// - Asserts that file counts are independent for both packages
/// - Asserts that conflict detection works correctly
///
/// Details:
/// - Final comprehensive check after all tabs have been tested
/// - Verifies first package has 1 dependency and 1 file (independent of second)
/// - Verifies second package has 2 dependencies (one conflict) and 2 files (independent of first)
/// - Verifies exactly 1 conflict exists (the only interaction between packages)
fn verify_final_independence(app: &crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        ..
    } = &app.modal
    {
        // Verify first package's data is independent
        let first_pkg_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .expect("first-package should be found in file_info");

        // Verify second package's data is independent
        let second_pkg_files = file_info
            .iter()
            .find(|f| f.name == "second-package")
            .expect("second-package should be found in file_info");

        // Verify independence: first package's data is not affected by second
        assert_eq!(
            dependency_info
                .iter()
                .filter(|d| d.required_by.contains(&"first-package".to_string()))
                .count(),
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
            dependency_info
                .iter()
                .filter(|d| d.required_by.contains(&"second-package".to_string()))
                .count(),
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

#[test]
/// What: Verify that adding a second package while first package is loading preserves independence.
///
/// Inputs:
/// - First package added to `install_list` and starts loading
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

    let mut app = crate_root::state::AppState::default();
    let (first_package, second_package) = setup_independent_loading_test_data(&mut app);
    app.modal = create_preflight_modal(first_package, second_package);

    test_deps_tab_independent_loading(&mut app);
    test_files_tab_independent_loading(&mut app);
    test_services_tab_independent_loading(&mut app);
    verify_final_independence(&app);
}
