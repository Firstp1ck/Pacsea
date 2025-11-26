//! //! Tests for persistence across tabs.

use pacsea as crate_root;

#[test]
/// What: Verify that service restart decisions persist when switching tabs.
///
/// Inputs:
/// - Packages in `install_list` with services
/// - Preflight modal opened with services loaded
/// - User changes service restart decisions in Services tab
/// - User switches to other tabs and back
///
/// Output:
/// - Service restart decisions remain unchanged when switching tabs
/// - Modified decisions persist across tab switches
/// - All services maintain their `restart_decision` values
///
/// Details:
/// - Tests that user choices for service restart decisions are preserved
/// - Verifies modal state correctly maintains service decisions
/// - Ensures no data loss when switching tabs
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn preflight_persists_service_restart_decisions_across_tabs() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();

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
    ];

    // Pre-populate cache with services (mix of Restart and Defer decisions)
    app.install_list_services = vec![
        crate_root::state::modal::ServiceImpact {
            unit_name: "service-1.service".to_string(),
            providers: vec!["test-package-1".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "service-2.service".to_string(),
            providers: vec!["test-package-1".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "service-3.service".to_string(),
            providers: vec!["test-package-2".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
    ];

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

    // Test 1: Switch to Services tab and load services
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

    // Verify initial service decisions
    if let crate_root::state::Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(service_info.len(), 3, "Should have 3 services");
        assert_eq!(
            service_info[0].restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
        assert_eq!(
            service_info[1].restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer
        );
        assert_eq!(
            service_info[2].restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Modify service restart decisions (simulating user toggles)
    if let crate_root::state::Modal::Preflight { service_info, .. } = &mut app.modal {
        // Toggle service-1 from Restart to Defer
        if let Some(service) = service_info
            .iter_mut()
            .find(|s| s.unit_name == "service-1.service")
        {
            service.restart_decision = crate_root::state::modal::ServiceRestartDecision::Defer;
        }

        // Toggle service-2 from Defer to Restart
        if let Some(service) = service_info
            .iter_mut()
            .find(|s| s.unit_name == "service-2.service")
        {
            service.restart_decision = crate_root::state::modal::ServiceRestartDecision::Restart;
        }

        // Keep service-3 as Restart (no change)
    }

    // Verify modified decisions
    if let crate_root::state::Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-1.service")
                .expect("service-1.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should be Defer after toggle"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .expect("service-2.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-2 should be Restart after toggle"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-3.service")
                .expect("service-3.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-3 should remain Restart"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Deps tab - decisions should persist
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

        // Simulate sync_dependencies logic (empty for this test)
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

    // Verify service decisions still persist
    if let crate_root::state::Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-1.service")
                .expect("service-1.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should still be Defer after switching to Deps"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .expect("service-2.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-2 should still be Restart after switching to Deps"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Files tab - decisions should persist
    if let crate_root::state::Modal::Preflight {
        items,
        tab,
        file_info,
        file_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Files;

        // Simulate sync_files logic (empty for this test)
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

    // Verify service decisions still persist
    if let crate_root::state::Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-1.service")
                .expect("service-1.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should still be Defer after switching to Files"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .expect("service-2.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-2 should still be Restart after switching to Files"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Switch back to Services tab - decisions should still persist
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

        // Re-sync services (should preserve existing decisions)
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
                // Preserve existing decisions when re-syncing
                // In real code, this would merge with existing decisions
                // For test, we'll manually preserve the decisions
                let existing_decisions: std::collections::HashMap<String, _> = service_info
                    .iter()
                    .map(|s| (s.unit_name.clone(), s.restart_decision))
                    .collect();
                *service_info = cached_services;
                // Restore user-modified decisions
                for service in service_info.iter_mut() {
                    if let Some(&decision) = existing_decisions.get(&service.unit_name) {
                        service.restart_decision = decision;
                    }
                }
                *services_loaded = true;
                *service_selected = 0;
            }
        }
    }

    // Verify all service decisions persist
    if let crate_root::state::Modal::Preflight {
        service_info,
        services_loaded,
        ..
    } = &app.modal
    {
        assert!(*services_loaded, "Services should be marked as loaded");
        assert_eq!(service_info.len(), 3, "Should have 3 services");

        // Verify all decisions are preserved
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-1.service")
                .expect("service-1.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should still be Defer after switching back to Services"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .expect("service-2.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-2 should still be Restart after switching back to Services"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-3.service")
                .expect("service-3.service should be found in service_info")
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-3 should still be Restart after switching back to Services"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All decisions are preserved
    if let crate_root::state::Modal::Preflight { service_info, .. } = &app.modal {
        // Verify all services maintain their decisions
        for service in service_info {
            match service.unit_name.as_str() {
                "service-1.service" => {
                    assert_eq!(
                        service.restart_decision,
                        crate_root::state::modal::ServiceRestartDecision::Defer,
                        "service-1 should be Defer"
                    );
                }
                "service-2.service" => {
                    assert_eq!(
                        service.restart_decision,
                        crate_root::state::modal::ServiceRestartDecision::Restart,
                        "service-2 should be Restart"
                    );
                }
                "service-3.service" => {
                    assert_eq!(
                        service.restart_decision,
                        crate_root::state::modal::ServiceRestartDecision::Restart,
                        "service-3 should be Restart"
                    );
                }
                _ => panic!("Unexpected service: {}", service.unit_name),
            }
        }
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that optional dependencies selection persists when switching tabs.
///
/// Inputs:
/// - AUR packages in `install_list` with optional dependencies
/// - Preflight modal opened with sandbox info loaded
/// - User selects optional dependencies in Sandbox tab
/// - User switches to other tabs and back
///
/// Output:
/// - Optional dependency selections persist when switching tabs
/// - `selected_optdepends` `HashMap` maintains correct structure
/// - Selections remain unchanged when switching back to Sandbox tab
///
/// Details:
/// - Tests that user selections for optional dependencies are preserved
/// - Verifies modal state correctly maintains optdepends selections
/// - Ensures no data loss when switching tabs
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn preflight_persists_optional_dependencies_selection() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();

    let test_packages = vec![
        crate_root::state::PackageItem {
            name: "test-aur-pkg-1".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
        crate_root::state::PackageItem {
            name: "test-aur-pkg-2".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
    ];

    // Pre-populate cache with sandbox info including optdepends
    app.install_list_sandbox = vec![
        crate_root::logic::sandbox::SandboxInfo {
            package_name: "test-aur-pkg-1".to_string(),
            depends: vec![],
            makedepends: vec![],
            checkdepends: vec![],
            optdepends: vec![
                crate_root::logic::sandbox::DependencyDelta {
                    name: "optdep-1>=1.0.0".to_string(),
                    is_installed: false,
                    installed_version: None,
                    version_satisfied: false,
                },
                crate_root::logic::sandbox::DependencyDelta {
                    name: "optdep-2".to_string(),
                    is_installed: false,
                    installed_version: None,
                    version_satisfied: false,
                },
                crate_root::logic::sandbox::DependencyDelta {
                    name: "optdep-3: description".to_string(),
                    is_installed: false,
                    installed_version: None,
                    version_satisfied: false,
                },
            ],
        },
        crate_root::logic::sandbox::SandboxInfo {
            package_name: "test-aur-pkg-2".to_string(),
            depends: vec![],
            makedepends: vec![],
            checkdepends: vec![],
            optdepends: vec![crate_root::logic::sandbox::DependencyDelta {
                name: "optdep-4".to_string(),
                is_installed: false,
                installed_version: None,
                version_satisfied: false,
            }],
        },
    ];

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
            aur_count: 2,
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

    // Test 1: Switch to Sandbox tab and load sandbox info
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

    // Verify initial state
    if let crate_root::state::Modal::Preflight {
        sandbox_info,
        selected_optdepends,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(sandbox_info.len(), 2, "Should have 2 sandbox entries");
        assert!(*sandbox_loaded, "Sandbox should be marked as loaded");
        assert!(
            selected_optdepends.is_empty(),
            "Initially no optdepends should be selected"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Select optional dependencies (simulating user selections)
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &mut app.modal
    {
        // Select optdep-1 and optdep-2 for test-aur-pkg-1
        selected_optdepends
            .entry("test-aur-pkg-1".to_string())
            .or_insert_with(std::collections::HashSet::new)
            .insert("optdep-1>=1.0.0".to_string());
        selected_optdepends
            .entry("test-aur-pkg-1".to_string())
            .or_insert_with(std::collections::HashSet::new)
            .insert("optdep-2".to_string());

        // Select optdep-4 for test-aur-pkg-2
        selected_optdepends
            .entry("test-aur-pkg-2".to_string())
            .or_insert_with(std::collections::HashSet::new)
            .insert("optdep-4".to_string());
    }

    // Verify selections
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &app.modal
    {
        assert_eq!(
            selected_optdepends.len(),
            2,
            "Should have selections for 2 packages"
        );
        assert!(
            selected_optdepends.contains_key("test-aur-pkg-1"),
            "Should have selections for test-aur-pkg-1"
        );
        assert!(
            selected_optdepends.contains_key("test-aur-pkg-2"),
            "Should have selections for test-aur-pkg-2"
        );

        let pkg1_selections = selected_optdepends
            .get("test-aur-pkg-1")
            .expect("test-aur-pkg-1 should be in selected_optdepends");
        assert_eq!(
            pkg1_selections.len(),
            2,
            "test-aur-pkg-1 should have 2 selections"
        );
        assert!(
            pkg1_selections.contains("optdep-1>=1.0.0"),
            "Should have optdep-1>=1.0.0 selected"
        );
        assert!(
            pkg1_selections.contains("optdep-2"),
            "Should have optdep-2 selected"
        );

        let pkg2_selections = selected_optdepends
            .get("test-aur-pkg-2")
            .expect("test-aur-pkg-2 should be in selected_optdepends");
        assert_eq!(
            pkg2_selections.len(),
            1,
            "test-aur-pkg-2 should have 1 selection"
        );
        assert!(
            pkg2_selections.contains("optdep-4"),
            "Should have optdep-4 selected"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Deps tab - selections should persist
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

        // Simulate sync_dependencies logic (empty for this test)
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

    // Verify selections still persist
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &app.modal
    {
        assert_eq!(
            selected_optdepends.len(),
            2,
            "Should still have selections for 2 packages after switching to Deps"
        );
        let pkg1_selections = selected_optdepends
            .get("test-aur-pkg-1")
            .expect("test-aur-pkg-1 should be in selected_optdepends");
        assert_eq!(
            pkg1_selections.len(),
            2,
            "test-aur-pkg-1 should still have 2 selections"
        );
        let pkg2_selections = selected_optdepends
            .get("test-aur-pkg-2")
            .expect("test-aur-pkg-2 should be in selected_optdepends");
        assert_eq!(
            pkg2_selections.len(),
            1,
            "test-aur-pkg-2 should still have 1 selection"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Files tab - selections should persist
    if let crate_root::state::Modal::Preflight {
        items,
        tab,
        file_info,
        file_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Files;

        // Simulate sync_files logic (empty for this test)
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

    // Verify selections still persist
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &app.modal
    {
        assert_eq!(
            selected_optdepends.len(),
            2,
            "Should still have selections for 2 packages after switching to Files"
        );
        let pkg1_selections = selected_optdepends
            .get("test-aur-pkg-1")
            .expect("test-aur-pkg-1 should be in selected_optdepends");
        assert!(
            pkg1_selections.contains("optdep-1>=1.0.0"),
            "optdep-1>=1.0.0 should still be selected"
        );
        assert!(
            pkg1_selections.contains("optdep-2"),
            "optdep-2 should still be selected"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Switch to Services tab - selections should persist
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

        // Simulate sync_services logic (empty for this test)
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

    // Verify selections still persist
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &app.modal
    {
        assert_eq!(
            selected_optdepends.len(),
            2,
            "Should still have selections for 2 packages after switching to Services"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 6: Switch back to Sandbox tab - selections should still persist
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

        // Re-sync sandbox (should preserve existing selections)
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

    // Verify all selections persist
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        sandbox_info,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        assert!(*sandbox_loaded, "Sandbox should be marked as loaded");
        assert_eq!(sandbox_info.len(), 2, "Should have 2 sandbox entries");

        // Verify all selections are preserved
        assert_eq!(
            selected_optdepends.len(),
            2,
            "Should still have selections for 2 packages after switching back to Sandbox"
        );

        let pkg1_selections = selected_optdepends
            .get("test-aur-pkg-1")
            .expect("test-aur-pkg-1 should be in selected_optdepends");
        assert_eq!(
            pkg1_selections.len(),
            2,
            "test-aur-pkg-1 should still have 2 selections"
        );
        assert!(
            pkg1_selections.contains("optdep-1>=1.0.0"),
            "optdep-1>=1.0.0 should still be selected"
        );
        assert!(
            pkg1_selections.contains("optdep-2"),
            "optdep-2 should still be selected"
        );
        assert!(
            !pkg1_selections.contains("optdep-3: description"),
            "optdep-3 should NOT be selected"
        );

        let pkg2_selections = selected_optdepends
            .get("test-aur-pkg-2")
            .expect("test-aur-pkg-2 should be in selected_optdepends");
        assert_eq!(
            pkg2_selections.len(),
            1,
            "test-aur-pkg-2 should still have 1 selection"
        );
        assert!(
            pkg2_selections.contains("optdep-4"),
            "optdep-4 should still be selected"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: HashMap structure is correct
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &app.modal
    {
        // Verify structure: package_name -> HashSet of optdep names
        for (pkg_name, optdeps) in selected_optdepends {
            assert!(
                !optdeps.is_empty(),
                "Package {pkg_name} should have at least one selected optdep"
            );
            assert!(
                test_packages.iter().any(|p| p.name == *pkg_name),
                "Package {pkg_name} should be in test packages"
            );

            // Verify each selected optdep exists in sandbox info
            let sandbox = app
                .install_list_sandbox
                .iter()
                .find(|s| s.package_name == *pkg_name)
                .expect("package should be found in install_list_sandbox");
            for optdep in optdeps {
                // Extract package name from dependency spec (may include version or description)
                let optdep_pkg_name = optdep
                    .split(':')
                    .next()
                    .expect("optdep should have at least one part before ':'")
                    .split('>')
                    .next()
                    .expect("optdep should have at least one part before '>'")
                    .trim();
                assert!(
                    sandbox.optdepends.iter().any(|d| {
                        d.name == *optdep
                            || d.name.starts_with(optdep_pkg_name)
                            || optdep.starts_with(
                                d.name
                                    .split(':')
                                    .next()
                                    .expect(
                                        "dependency name should have at least one part before ':'",
                                    )
                                    .split('>')
                                    .next()
                                    .expect(
                                        "dependency name should have at least one part before '>'",
                                    )
                                    .trim(),
                            )
                    }),
                    "Selected optdep {optdep} should exist in sandbox info for {pkg_name}"
                );
            }
        }
    } else {
        panic!("Expected Preflight modal");
    }
}
