//! //! Tests for persistence across tabs.

use pacsea as crate_root;

/// Helper: Create test packages for service restart decision tests.
///
/// Output:
/// - Vector of test `PackageItem` instances
fn create_test_packages() -> Vec<crate_root::state::PackageItem> {
    vec![
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
    ]
}

/// Helper: Create test services for service restart decision tests.
///
/// Output:
/// - Vector of test `ServiceImpact` instances
fn create_test_services() -> Vec<crate_root::state::modal::ServiceImpact> {
    vec![
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
    ]
}

/// Helper: Create a preflight modal with default settings.
///
/// Inputs:
/// - `test_packages`: Vector of packages to include in modal
///
/// Output:
/// - Preflight modal instance
fn create_preflight_modal(
    test_packages: &[crate_root::state::PackageItem],
) -> crate_root::state::Modal {
    crate_root::state::Modal::Preflight {
        items: test_packages.to_vec(),
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
    }
}

/// Helper: Switch to Services tab and sync services from cache.
///
/// Inputs:
/// - `app`: Application state with `install_list_services` populated
///
/// Details:
/// - Switches modal to Services tab
/// - Syncs services from `install_list_services` cache
fn switch_to_services_tab_and_sync(app: &mut crate_root::state::AppState) {
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
}

/// Helper: Re-sync services while preserving existing decisions.
///
/// Inputs:
/// - `app`: Application state with modified service decisions
///
/// Details:
/// - Re-syncs services from cache but preserves user-modified decisions
fn resync_services_preserving_decisions(app: &mut crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        service_info,
        service_selected,
        services_loaded,
        ..
    } = &mut app.modal
        && matches!(*action, crate_root::state::PreflightAction::Install)
        && !items.is_empty()
    {
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_services: Vec<_> = app
            .install_list_services
            .iter()
            .filter(|s| s.providers.iter().any(|p| item_names.contains(p)))
            .cloned()
            .collect();
        if !cached_services.is_empty() {
            let existing_decisions: std::collections::HashMap<String, _> = service_info
                .iter()
                .map(|s| (s.unit_name.clone(), s.restart_decision))
                .collect();
            *service_info = cached_services;
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

/// Helper: Switch to Deps tab and sync dependencies.
///
/// Inputs:
/// - `app`: Application state
fn switch_to_deps_tab(app: &mut crate_root::state::AppState) {
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
}

/// Helper: Switch to Files tab and sync files.
///
/// Inputs:
/// - `app`: Application state
fn switch_to_files_tab(app: &mut crate_root::state::AppState) {
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
}

/// Helper: Verify service restart decisions match expected values.
///
/// Inputs:
/// - `app`: Application state
/// - `expected_decisions`: `HashMap` mapping service unit names to expected decisions
/// - `context`: Context string for error messages
fn verify_service_decisions(
    app: &crate_root::state::AppState,
    expected_decisions: &std::collections::HashMap<
        String,
        crate_root::state::modal::ServiceRestartDecision,
    >,
    context: &str,
) {
    if let crate_root::state::Modal::Preflight { service_info, .. } = &app.modal {
        for (unit_name, expected_decision) in expected_decisions {
            let service = service_info
                .iter()
                .find(|s| s.unit_name == *unit_name)
                .unwrap_or_else(|| panic!("{unit_name} should be found in service_info"));
            assert_eq!(
                service.restart_decision, *expected_decision,
                "{unit_name} should be {expected_decision:?} {context}"
            );
        }
    } else {
        panic!("Expected Preflight modal");
    }
}

/// Helper: Modify service restart decisions (simulating user toggles).
///
/// Inputs:
/// - `app`: Application state
/// - `modifications`: Vector of (`unit_name`, `new_decision`) tuples
fn modify_service_decisions(
    app: &mut crate_root::state::AppState,
    modifications: &[(&str, crate_root::state::modal::ServiceRestartDecision)],
) {
    if let crate_root::state::Modal::Preflight { service_info, .. } = &mut app.modal {
        for (unit_name, new_decision) in modifications {
            if let Some(service) = service_info.iter_mut().find(|s| s.unit_name == *unit_name) {
                service.restart_decision = *new_decision;
            }
        }
    }
}

/// Helper: Verify initial service decisions after loading.
///
/// Inputs:
/// - `app`: Application state
fn verify_initial_service_decisions(app: &crate_root::state::AppState) {
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
}

/// Helper: Verify all service decisions after final sync.
///
/// Inputs:
/// - `app`: Application state
fn verify_final_service_decisions(app: &crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight {
        service_info,
        services_loaded,
        ..
    } = &app.modal
    {
        assert!(*services_loaded, "Services should be marked as loaded");
        assert_eq!(service_info.len(), 3, "Should have 3 services");

        let mut expected = std::collections::HashMap::new();
        expected.insert(
            "service-1.service".to_string(),
            crate_root::state::modal::ServiceRestartDecision::Defer,
        );
        expected.insert(
            "service-2.service".to_string(),
            crate_root::state::modal::ServiceRestartDecision::Restart,
        );
        expected.insert(
            "service-3.service".to_string(),
            crate_root::state::modal::ServiceRestartDecision::Restart,
        );

        verify_service_decisions(app, &expected, "after switching back to Services");
    } else {
        panic!("Expected Preflight modal");
    }
}

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
fn preflight_persists_service_restart_decisions_across_tabs() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();
    let test_packages = create_test_packages();
    let test_services = create_test_services();

    // Pre-populate cache with services
    app.install_list_services = test_services;
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    app.modal = create_preflight_modal(&test_packages);

    // Test 1: Switch to Services tab and load services
    switch_to_services_tab_and_sync(&mut app);
    verify_initial_service_decisions(&app);

    // Test 2: Modify service restart decisions (simulating user toggles)
    modify_service_decisions(
        &mut app,
        &[
            (
                "service-1.service",
                crate_root::state::modal::ServiceRestartDecision::Defer,
            ),
            (
                "service-2.service",
                crate_root::state::modal::ServiceRestartDecision::Restart,
            ),
        ],
    );

    // Verify modified decisions
    let mut expected_after_modify = std::collections::HashMap::new();
    expected_after_modify.insert(
        "service-1.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Defer,
    );
    expected_after_modify.insert(
        "service-2.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Restart,
    );
    expected_after_modify.insert(
        "service-3.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Restart,
    );
    verify_service_decisions(&app, &expected_after_modify, "after toggle");

    // Test 3: Switch to Deps tab - decisions should persist
    switch_to_deps_tab(&mut app);
    let mut expected_after_deps = std::collections::HashMap::new();
    expected_after_deps.insert(
        "service-1.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Defer,
    );
    expected_after_deps.insert(
        "service-2.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Restart,
    );
    verify_service_decisions(&app, &expected_after_deps, "after switching to Deps");

    // Test 4: Switch to Files tab - decisions should persist
    switch_to_files_tab(&mut app);
    verify_service_decisions(&app, &expected_after_deps, "after switching to Files");

    // Test 5: Switch back to Services tab - decisions should still persist
    if let crate_root::state::Modal::Preflight { tab, .. } = &mut app.modal {
        *tab = crate_root::state::PreflightTab::Services;
    }
    resync_services_preserving_decisions(&mut app);
    verify_final_service_decisions(&app);

    // Final verification: All decisions are preserved
    let mut final_expected = std::collections::HashMap::new();
    final_expected.insert(
        "service-1.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Defer,
    );
    final_expected.insert(
        "service-2.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Restart,
    );
    final_expected.insert(
        "service-3.service".to_string(),
        crate_root::state::modal::ServiceRestartDecision::Restart,
    );
    verify_service_decisions(&app, &final_expected, "");
}

/// Helper function to create test AUR packages for optdepends tests.
///
/// What: Creates a vector of test AUR packages.
///
/// Output:
/// - Vector of two test AUR packages
fn create_test_aur_packages() -> Vec<crate_root::state::PackageItem> {
    vec![
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
    ]
}

/// Helper function to create sandbox info with optdepends.
///
/// What: Creates sandbox info for test packages with optional dependencies.
///
/// Output:
/// - Vector of `SandboxInfo` with optdepends
fn create_sandbox_info() -> Vec<crate_root::logic::sandbox::SandboxInfo> {
    vec![
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
    ]
}

/// Helper function to setup preflight modal for optdepends tests.
///
/// What: Creates and configures a preflight modal with test packages.
///
/// Inputs:
/// - `test_packages`: Vector of test packages
///
/// Output:
/// - Configured Preflight modal
fn setup_preflight_modal(
    test_packages: &[crate_root::state::PackageItem],
) -> crate_root::state::Modal {
    crate_root::state::Modal::Preflight {
        items: test_packages.to_vec(),
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
    }
}

/// Helper function to switch to Sandbox tab and sync sandbox info.
///
/// What: Switches modal to Sandbox tab and loads sandbox info from cache.
///
/// Inputs:
/// - app: Mutable reference to `AppState`
fn switch_to_sandbox_tab(app: &mut crate_root::state::AppState) {
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
}

/// Helper function to select optional dependencies.
///
/// What: Simulates user selecting optdepends for test packages.
///
/// Inputs:
/// - app: Mutable reference to `AppState`
fn select_optional_dependencies(app: &mut crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &mut app.modal
    {
        selected_optdepends
            .entry("test-aur-pkg-1".to_string())
            .or_insert_with(std::collections::HashSet::new)
            .insert("optdep-1>=1.0.0".to_string());
        selected_optdepends
            .entry("test-aur-pkg-1".to_string())
            .or_insert_with(std::collections::HashSet::new)
            .insert("optdep-2".to_string());
        selected_optdepends
            .entry("test-aur-pkg-2".to_string())
            .or_insert_with(std::collections::HashSet::new)
            .insert("optdep-4".to_string());
    }
}

/// Helper function to verify initial selections.
///
/// What: Verifies that optdepends selections are correct after initial selection.
///
/// Inputs:
/// - app: Reference to `AppState`
fn verify_initial_selections(app: &crate_root::state::AppState) {
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
}

/// Helper function to switch to a tab and verify selections persist.
///
/// What: Switches to a different tab and verifies optdepends selections persist.
///
/// Inputs:
/// - app: Mutable reference to `AppState`
/// - tab: The tab to switch to
fn switch_tab_and_verify_persistence(
    app: &mut crate_root::state::AppState,
    tab: crate_root::state::PreflightTab,
) {
    match tab {
        crate_root::state::PreflightTab::Deps => {
            if let crate_root::state::Modal::Preflight {
                items,
                action,
                tab: current_tab,
                dependency_info,
                dep_selected,
                ..
            } = &mut app.modal
            {
                *current_tab = crate_root::state::PreflightTab::Deps;

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
        }
        crate_root::state::PreflightTab::Files => {
            if let crate_root::state::Modal::Preflight {
                items,
                tab: current_tab,
                file_info,
                file_selected,
                ..
            } = &mut app.modal
            {
                *current_tab = crate_root::state::PreflightTab::Files;

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
        }
        crate_root::state::PreflightTab::Services => {
            if let crate_root::state::Modal::Preflight {
                items,
                action,
                tab: current_tab,
                service_info,
                service_selected,
                services_loaded,
                ..
            } = &mut app.modal
            {
                *current_tab = crate_root::state::PreflightTab::Services;

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
        }
        _ => {}
    }

    // Verify selections persist
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &app.modal
    {
        assert_eq!(
            selected_optdepends.len(),
            2,
            "Should still have selections for 2 packages after switching tabs"
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
}

/// Helper function to verify final selections after switching back to Sandbox.
///
/// What: Verifies all optdepends selections are preserved after tab switches.
///
/// Inputs:
/// - app: Reference to `AppState`
fn verify_final_selections(app: &crate_root::state::AppState) {
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        sandbox_info,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        assert!(*sandbox_loaded, "Sandbox should be marked as loaded");
        assert_eq!(sandbox_info.len(), 2, "Should have 2 sandbox entries");

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
}

/// Helper function to verify `HashMap` structure is correct.
///
/// What: Verifies that `selected_optdepends` `HashMap` has correct structure and values.
///
/// Inputs:
/// - app: Reference to `AppState`
/// - `test_packages`: Reference to test packages
fn verify_hashmap_structure(
    app: &crate_root::state::AppState,
    test_packages: &[crate_root::state::PackageItem],
) {
    if let crate_root::state::Modal::Preflight {
        selected_optdepends,
        ..
    } = &app.modal
    {
        for (pkg_name, optdeps) in selected_optdepends {
            assert!(
                !optdeps.is_empty(),
                "Package {pkg_name} should have at least one selected optdep"
            );
            assert!(
                test_packages.iter().any(|p| p.name == *pkg_name),
                "Package {pkg_name} should be in test packages"
            );

            let sandbox = app
                .install_list_sandbox
                .iter()
                .find(|s| s.package_name == *pkg_name)
                .expect("package should be found in install_list_sandbox");
            for optdep in optdeps {
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
fn preflight_persists_optional_dependencies_selection() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();
    let test_packages = create_test_aur_packages();
    app.install_list_sandbox = create_sandbox_info();
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    app.modal = setup_preflight_modal(&test_packages);

    // Test 1: Switch to Sandbox tab and load sandbox info
    switch_to_sandbox_tab(&mut app);

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

    // Test 2: Select optional dependencies
    select_optional_dependencies(&mut app);
    verify_initial_selections(&app);

    // Test 3-5: Switch to different tabs and verify persistence
    switch_tab_and_verify_persistence(&mut app, crate_root::state::PreflightTab::Deps);
    switch_tab_and_verify_persistence(&mut app, crate_root::state::PreflightTab::Files);
    switch_tab_and_verify_persistence(&mut app, crate_root::state::PreflightTab::Services);

    // Test 6: Switch back to Sandbox tab
    switch_to_sandbox_tab(&mut app);
    verify_final_selections(&app);

    // Final verification: HashMap structure
    verify_hashmap_structure(&app, &test_packages);
}
