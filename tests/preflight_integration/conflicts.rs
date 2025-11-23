//! //! Tests for conflict resolution.

use super::helpers::*;
use pacsea as crate_root;

#[test]
/// What: Verify that all tabs (Deps, Files, Services, Sandbox) load and display correctly when conflicts are present.
///
/// Inputs:
/// - Packages in `install_list` with dependency conflicts
/// - All tabs have cached data (deps, files, services, sandbox)
/// - Conflicts are detected in dependencies
///
/// Output:
/// - Deps tab correctly shows conflicts
/// - Files tab loads and displays correctly despite conflicts
/// - Services tab loads and displays correctly despite conflicts
/// - Sandbox tab loads and displays correctly despite conflicts
/// - All tab data is correct and not affected by conflicts
///
/// Details:
/// - Tests that conflicts in dependencies don't affect other tabs
/// - Verifies cache loading works correctly for all tabs when conflicts exist
/// - Ensures data integrity across all tabs when conflicts are present
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn preflight_all_tabs_load_correctly_when_conflicts_present() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();

    let test_packages = vec![
        crate_root::state::PackageItem {
            name: "package-1".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
        crate_root::state::PackageItem {
            name: "package-2".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
        crate_root::state::PackageItem {
            name: "aur-package".to_string(),
            version: "3.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        },
    ];

    // Pre-populate cache with dependencies including conflicts
    app.install_list_deps = vec![
        // Package 1 dependencies
        crate_root::state::modal::DependencyInfo {
            name: "common-dep".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["package-1".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "pkg1-dep".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["package-1".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        // Package 2 dependencies - includes conflict with common-dep
        crate_root::state::modal::DependencyInfo {
            name: "common-dep".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "Conflicts with package-1's dependency common-dep (1.0.0)".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["package-2".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "pkg2-dep".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["package-2".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        // AUR package dependency
        crate_root::state::modal::DependencyInfo {
            name: "aur-dep".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Aur,
            required_by: vec!["aur-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    // Pre-populate cache with files for all packages
    app.install_list_files = vec![
        crate_root::state::modal::PackageFileInfo {
            name: "package-1".to_string(),
            files: vec![
                crate_root::state::modal::FileChange {
                    path: "/usr/bin/pkg1".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "package-1".to_string(),
                    is_config: false,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                },
                crate_root::state::modal::FileChange {
                    path: "/etc/pkg1.conf".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "package-1".to_string(),
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
            name: "package-2".to_string(),
            files: vec![
                crate_root::state::modal::FileChange {
                    path: "/usr/bin/pkg2".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "package-2".to_string(),
                    is_config: false,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                },
                crate_root::state::modal::FileChange {
                    path: "/etc/pkg2.conf".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::Changed,
                    package: "package-2".to_string(),
                    is_config: true,
                    predicted_pacnew: false,
                    predicted_pacsave: true,
                },
            ],
            total_count: 2,
            new_count: 1,
            changed_count: 1,
            removed_count: 0,
            config_count: 1,
            pacnew_candidates: 0,
            pacsave_candidates: 1,
        },
        crate_root::state::modal::PackageFileInfo {
            name: "aur-package".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/aur".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "aur-package".to_string(),
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

    // Pre-populate cache with services for all packages
    app.install_list_services = vec![
        crate_root::state::modal::ServiceImpact {
            unit_name: "pkg1.service".to_string(),
            providers: vec!["package-1".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "pkg2.service".to_string(),
            providers: vec!["package-2".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "aur.service".to_string(),
            providers: vec!["aur-package".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
    ];

    // Pre-populate cache with sandbox info for AUR package
    app.install_list_sandbox = vec![crate_root::logic::sandbox::SandboxInfo {
        package_name: "aur-package".to_string(),
        depends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "aur-dep".to_string(),
            is_installed: false,
            installed_version: None,
            version_satisfied: false,
        }],
        makedepends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "make-dep".to_string(),
            is_installed: true,
            installed_version: Some("1.0.0".to_string()),
            version_satisfied: true,
        }],
        checkdepends: vec![],
        optdepends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "optdep".to_string(),
            is_installed: false,
            installed_version: None,
            version_satisfied: false,
        }],
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

    // Test 1: Switch to Deps tab - verify conflicts are detected and shown
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
        assert_eq!(dependency_info.len(), 5, "Should have 5 dependencies");

        // Verify conflicts are detected
        let conflicts: Vec<_> = dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .collect();
        assert!(!conflicts.is_empty(), "Conflicts should be detected");
        assert_eq!(conflicts.len(), 1, "Should have 1 conflict");
        assert_eq!(conflicts[0].name, "common-dep");
        assert!(conflicts[0].required_by.contains(&"package-2".to_string()));

        // Verify non-conflicting dependencies are present
        assert_eq!(
            dependency_info
                .iter()
                .filter(|d| {
                    matches!(
                        d.status,
                        crate_root::state::modal::DependencyStatus::ToInstall
                    )
                })
                .count(),
            4,
            "Should have 4 ToInstall dependencies"
        );

        // Verify package-1's dependencies
        let pkg1_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"package-1".to_string()))
            .collect();
        assert_eq!(pkg1_deps.len(), 2, "Package-1 should have 2 dependencies");
        assert!(pkg1_deps.iter().any(|d| d.name == "common-dep"));
        assert!(pkg1_deps.iter().any(|d| d.name == "pkg1-dep"));

        // Verify package-2's dependencies (including conflict)
        let pkg2_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"package-2".to_string()))
            .collect();
        assert_eq!(pkg2_deps.len(), 2, "Package-2 should have 2 dependencies");
        assert!(pkg2_deps.iter().any(|d| d.name == "common-dep"));
        assert!(pkg2_deps.iter().any(|d| d.name == "pkg2-dep"));

        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Switch to Files tab - verify files load correctly despite conflicts
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
        assert_eq!(file_info.len(), 3, "Should have 3 file entries");

        // Verify package-1 files are correct
        let pkg1_files = file_info
            .iter()
            .find(|f| f.name == "package-1")
            .expect("package-1 should be found in file_info");
        assert_eq!(pkg1_files.files.len(), 2, "Package-1 should have 2 files");
        assert_eq!(pkg1_files.total_count, 2);
        assert_eq!(pkg1_files.new_count, 2);
        assert_eq!(pkg1_files.changed_count, 0);
        assert_eq!(pkg1_files.config_count, 1);
        assert_eq!(pkg1_files.pacnew_candidates, 1);
        assert_eq!(pkg1_files.pacsave_candidates, 0);

        // Verify package-2 files are correct
        let pkg2_files = file_info
            .iter()
            .find(|f| f.name == "package-2")
            .expect("package-2 should be found in file_info");
        assert_eq!(pkg2_files.files.len(), 2, "Package-2 should have 2 files");
        assert_eq!(pkg2_files.total_count, 2);
        assert_eq!(pkg2_files.new_count, 1);
        assert_eq!(pkg2_files.changed_count, 1);
        assert_eq!(pkg2_files.config_count, 1);
        assert_eq!(pkg2_files.pacnew_candidates, 0);
        assert_eq!(pkg2_files.pacsave_candidates, 1);

        // Verify AUR package files are correct
        let aur_files = file_info
            .iter()
            .find(|f| f.name == "aur-package")
            .expect("aur-package should be found in file_info");
        assert_eq!(aur_files.files.len(), 1, "AUR package should have 1 file");
        assert_eq!(aur_files.total_count, 1);
        assert_eq!(aur_files.new_count, 1);

        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Services tab - verify services load correctly despite conflicts
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
        assert_eq!(service_info.len(), 3, "Should have 3 services");

        // Verify package-1 service
        let pkg1_svc = service_info
            .iter()
            .find(|s| s.unit_name == "pkg1.service")
            .expect("pkg1.service should be found in service_info");
        assert!(pkg1_svc.is_active);
        assert!(pkg1_svc.needs_restart);
        assert_eq!(
            pkg1_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
        assert!(pkg1_svc.providers.contains(&"package-1".to_string()));

        // Verify package-2 service
        let pkg2_svc = service_info
            .iter()
            .find(|s| s.unit_name == "pkg2.service")
            .expect("pkg2.service should be found in service_info");
        assert!(!pkg2_svc.is_active);
        assert!(!pkg2_svc.needs_restart);
        assert_eq!(
            pkg2_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer
        );
        assert!(pkg2_svc.providers.contains(&"package-2".to_string()));

        // Verify AUR package service
        let aur_svc = service_info
            .iter()
            .find(|s| s.unit_name == "aur.service")
            .expect("aur.service should be found in service_info");
        assert!(aur_svc.is_active);
        assert!(aur_svc.needs_restart);
        assert_eq!(
            aur_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
        assert!(aur_svc.providers.contains(&"aur-package".to_string()));

        assert_eq!(*service_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Sandbox tab - verify sandbox loads correctly despite conflicts
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
        assert!(!sandbox_info.is_empty(), "Sandbox info should be loaded");
        assert_eq!(sandbox_info.len(), 1, "Should have 1 sandbox entry");

        // Verify AUR package sandbox info
        let sandbox = sandbox_info
            .iter()
            .find(|s| s.package_name == "aur-package")
            .expect("aur-package should be found in sandbox_info");
        assert_eq!(sandbox.depends.len(), 1, "Should have 1 depends");
        assert_eq!(sandbox.makedepends.len(), 1, "Should have 1 makedepends");
        assert_eq!(sandbox.checkdepends.len(), 0, "Should have 0 checkdepends");
        assert_eq!(sandbox.optdepends.len(), 1, "Should have 1 optdepends");

        // Verify dependency details
        let dep = sandbox
            .depends
            .iter()
            .find(|d| d.name == "aur-dep")
            .expect("aur-dep should be found in sandbox.depends");
        assert!(!dep.is_installed);
        assert_eq!(dep.installed_version, None);

        let makedep = sandbox
            .makedepends
            .iter()
            .find(|d| d.name == "make-dep")
            .expect("make-dep should be found in sandbox.makedepends");
        assert!(makedep.is_installed);
        assert_eq!(makedep.installed_version, Some("1.0.0".to_string()));
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Switch back to Deps tab - verify conflicts still present
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

        // Re-sync to ensure data persists
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
        // Verify conflicts are still present
        let conflicts: Vec<_> = dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .collect();
        assert!(!conflicts.is_empty(), "Conflicts should still be present");
        assert_eq!(conflicts.len(), 1, "Should still have 1 conflict");
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All tabs have correct data despite conflicts
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        ..
    } = &app.modal
    {
        // Verify Deps tab has conflicts and other dependencies
        assert_eq!(
            dependency_info
                .iter()
                .filter(|d| {
                    matches!(
                        d.status,
                        crate_root::state::modal::DependencyStatus::Conflict { .. }
                    )
                })
                .count(),
            1,
            "Should have 1 conflict"
        );
        assert_eq!(dependency_info.len(), 5, "Should have 5 total dependencies");

        // Verify Files tab has all packages' files
        assert_eq!(file_info.len(), 3, "Should have 3 file entries");
        assert!(file_info.iter().any(|f| f.name == "package-1"));
        assert!(file_info.iter().any(|f| f.name == "package-2"));
        assert!(file_info.iter().any(|f| f.name == "aur-package"));

        // Verify Services tab has all packages' services
        assert_eq!(service_info.len(), 3, "Should have 3 services");
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"package-1".to_string()))
        );
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"package-2".to_string()))
        );
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"aur-package".to_string()))
        );

        // Verify Sandbox tab has AUR package info
        assert_eq!(sandbox_info.len(), 1, "Should have 1 sandbox entry");
        assert!(sandbox_info.iter().any(|s| s.package_name == "aur-package"));
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that conflicts are not overwritten when new packages are added to install list sequentially.
///
/// Inputs:
/// - pacsea-bin added first with conflicts (pacsea, pacsea-git)
/// - jujutsu-git added second with conflicts (jujutsu)
/// - Both packages may have overlapping dependencies
///
/// Output:
/// - pacsea-bin's conflicts remain present after jujutsu-git is added
/// - jujutsu-git's conflicts are also detected
/// - No conflicts are overwritten by dependency merging
///
/// Details:
/// - Tests the fix for conflict status preservation during dependency merging
/// - Verifies that conflicts take precedence over dependency statuses
/// - Ensures timing of package addition doesn't affect conflict detection
#[allow(clippy::too_many_lines)]
fn preflight_conflicts_not_overwritten_when_packages_added_sequentially() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();

    // Step 1: Add pacsea-bin first
    let pacsea_bin = create_test_package("pacsea-bin", "0.5.1", crate_root::state::Source::Aur);

    // Pre-populate cache with pacsea-bin's conflicts
    // pacsea-bin conflicts with pacsea and pacsea-git
    app.install_list_deps = vec![
        // pacsea-bin's conflict with pacsea
        crate_root::state::modal::DependencyInfo {
            name: "pacsea".to_string(),
            version: String::new(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "conflicts with installed package pacsea".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["pacsea-bin".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        // pacsea-bin's conflict with pacsea-git
        crate_root::state::modal::DependencyInfo {
            name: "pacsea-git".to_string(),
            version: String::new(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "conflicts with installed package pacsea-git".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Aur,
            required_by: vec!["pacsea-bin".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        // pacsea-bin's regular dependency (to test that conflicts aren't overwritten by deps)
        crate_root::state::modal::DependencyInfo {
            name: "common-dep".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["pacsea-bin".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    app.install_list = vec![pacsea_bin.clone()];
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    app.modal = create_preflight_modal(
        vec![pacsea_bin.clone()],
        crate_root::state::PreflightAction::Install,
        crate_root::state::PreflightTab::Deps,
    );

    // Verify pacsea-bin's conflicts are detected
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Deps);
    let (_, _, _, dependency_info, _, _, _, _, _) = assert_preflight_modal(&app);

    let conflicts_after_first: Vec<_> = dependency_info
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::Conflict { .. }
            )
        })
        .collect();
    assert_eq!(
        conflicts_after_first.len(),
        2,
        "Should have 2 conflicts after adding pacsea-bin"
    );
    assert!(
        conflicts_after_first
            .iter()
            .any(|c| c.name == "pacsea" && c.required_by.contains(&"pacsea-bin".to_string())),
        "pacsea-bin should conflict with pacsea"
    );
    assert!(
        conflicts_after_first
            .iter()
            .any(|c| c.name == "pacsea-git" && c.required_by.contains(&"pacsea-bin".to_string())),
        "pacsea-bin should conflict with pacsea-git"
    );

    // Step 2: Add jujutsu-git (which might have dependencies that could overwrite conflicts)
    let jujutsu_git = create_test_package("jujutsu-git", "0.1.0", crate_root::state::Source::Aur);

    // Add jujutsu-git's conflicts and dependencies to cache
    // jujutsu-git conflicts with jujutsu
    // jujutsu-git might also depend on common-dep (to test conflict preservation)
    app.install_list_deps.extend(vec![
        // jujutsu-git's conflict with jujutsu
        crate_root::state::modal::DependencyInfo {
            name: "jujutsu".to_string(),
            version: String::new(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "conflicts with installed package jujutsu".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "community".to_string(),
            },
            required_by: vec!["jujutsu-git".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        // jujutsu-git also depends on common-dep (same as pacsea-bin)
        // This tests that pacsea-bin's conflict entries aren't overwritten
        crate_root::state::modal::DependencyInfo {
            name: "common-dep".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["jujutsu-git".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        // jujutsu-git's unique dependency
        crate_root::state::modal::DependencyInfo {
            name: "jujutsu-dep".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["jujutsu-git".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ]);

    // Update install list to include both packages
    app.install_list = vec![pacsea_bin.clone(), jujutsu_git.clone()];

    // Update modal to include both packages
    app.modal = create_preflight_modal(
        vec![pacsea_bin, jujutsu_git],
        crate_root::state::PreflightAction::Install,
        crate_root::state::PreflightTab::Deps,
    );

    // Step 3: Verify conflicts are still present after adding jujutsu-git
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Deps);
    let (items, _, _, dependency_info, _, _, _, _, _) = assert_preflight_modal(&app);

    assert_eq!(items.len(), 2, "Should have 2 packages in install list");

    // Verify pacsea-bin's conflicts are still present (not overwritten)
    let pacsea_conflicts: Vec<_> = dependency_info
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::Conflict { .. }
            ) && d.required_by.contains(&"pacsea-bin".to_string())
        })
        .collect();
    assert_eq!(
        pacsea_conflicts.len(),
        2,
        "pacsea-bin should still have 2 conflicts after adding jujutsu-git"
    );
    assert!(
        pacsea_conflicts.iter().any(|c| c.name == "pacsea"),
        "pacsea-bin should still conflict with pacsea"
    );
    assert!(
        pacsea_conflicts.iter().any(|c| c.name == "pacsea-git"),
        "pacsea-bin should still conflict with pacsea-git"
    );

    // Verify jujutsu-git's conflicts are also detected
    let jujutsu_conflicts: Vec<_> = dependency_info
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::Conflict { .. }
            ) && d.required_by.contains(&"jujutsu-git".to_string())
        })
        .collect();
    assert_eq!(
        jujutsu_conflicts.len(),
        1,
        "jujutsu-git should have 1 conflict"
    );
    assert!(
        jujutsu_conflicts.iter().any(|c| c.name == "jujutsu"),
        "jujutsu-git should conflict with jujutsu"
    );

    // Verify total conflicts count
    assert_eq!(
        dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .count(),
        3,
        "Should have 3 total conflicts (2 from pacsea-bin, 1 from jujutsu-git)"
    );

    // Verify that common-dep is not a conflict (it's a regular dependency)
    let common_dep = dependency_info
        .iter()
        .find(|d| d.name == "common-dep")
        .expect("common-dep should be present");
    assert!(
        matches!(
            common_dep.status,
            crate_root::state::modal::DependencyStatus::ToInstall
        ),
        "common-dep should be ToInstall, not Conflict"
    );
    assert!(
        common_dep.required_by.contains(&"pacsea-bin".to_string()),
        "common-dep should be required by pacsea-bin"
    );
    assert!(
        common_dep.required_by.contains(&"jujutsu-git".to_string()),
        "common-dep should be required by jujutsu-git"
    );

    // Step 4: Verify conflicts persist through multiple tab switches
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Summary);
    switch_preflight_tab(&mut app, crate_root::state::PreflightTab::Deps);

    let (_, _, _, dependency_info_after_switch, _, _, _, _, _) = assert_preflight_modal(&app);

    assert_eq!(
        dependency_info_after_switch
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .count(),
        3,
        "Should still have 3 conflicts after tab switches"
    );

    // Verify pacsea-bin's conflicts are still intact
    assert_eq!(
        dependency_info_after_switch
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                ) && d.required_by.contains(&"pacsea-bin".to_string())
            })
            .count(),
        2,
        "pacsea-bin should still have 2 conflicts after tab switches"
    );
}
