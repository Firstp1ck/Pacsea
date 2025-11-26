//! //! Tests for edge cases.

use pacsea as crate_root;

#[test]
/// What: Verify that preflight modal handles empty results gracefully across all tabs.
///
/// Inputs:
/// - Packages in `install_list`
/// - All resolution stages return empty results (no deps, files, services, sandbox)
/// - User switches between all tabs
///
/// Output:
/// - All tabs display appropriate empty state messages
/// - No panics or errors occur
/// - UI remains functional
///
/// Details:
/// - Tests edge case where packages have no dependencies, files, services, or sandbox data
/// - Verifies graceful handling of empty results
/// - Ensures UI doesn't break with empty data
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn preflight_handles_empty_results_gracefully() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState::default();

    let test_packages = vec![crate_root::state::PackageItem {
        name: "test-package-empty".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }];

    // All caches are empty (no dependencies, files, services, sandbox)
    app.install_list_deps = vec![];
    app.install_list_files = vec![];
    app.install_list_services = vec![];
    app.install_list_sandbox = vec![];

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

    // Test 1: Switch to Deps tab - should handle empty results
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
            // Even if empty, we should handle it gracefully
            *dependency_info = filtered;
            *dep_selected = 0;
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        dependency_info,
        deps_error,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Deps,
            "Should be on Deps tab"
        );
        assert!(dependency_info.is_empty(), "Dependencies should be empty");
        assert!(
            deps_error.is_none(),
            "Should not have error for empty results"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Switch to Files tab - should handle empty results
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
        // Even if empty, we should handle it gracefully
        *file_info = cached_files;
        *file_selected = 0;
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        file_info,
        files_error,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Files,
            "Should be on Files tab"
        );
        assert!(file_info.is_empty(), "Files should be empty");
        assert!(
            files_error.is_none(),
            "Should not have error for empty results"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Services tab - should handle empty results
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
            // Even if empty, we should handle it gracefully
            if cached_services.is_empty() {
                // Mark as loaded even if empty
            } else {
                *service_info = cached_services;
            }
            *services_loaded = true;
            *service_selected = 0;
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        service_info,
        services_loaded,
        services_error,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Services,
            "Should be on Services tab"
        );
        assert!(service_info.is_empty(), "Services should be empty");
        assert!(*services_loaded, "Services should be marked as loaded");
        assert!(
            services_error.is_none(),
            "Should not have error for empty results"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Sandbox tab - should handle empty results
    // Note: Sandbox only applies to AUR packages, so empty is expected for official packages
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
            // Even if empty, we should handle it gracefully
            if cached_sandbox.is_empty() {
                // Mark as loaded even if empty
            } else {
                *sandbox_info = cached_sandbox;
            }
            *sandbox_loaded = true;
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        sandbox_info,
        sandbox_loaded,
        sandbox_error,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Sandbox,
            "Should be on Sandbox tab"
        );
        assert!(sandbox_info.is_empty(), "Sandbox should be empty");
        assert!(*sandbox_loaded, "Sandbox should be marked as loaded");
        assert!(
            sandbox_error.is_none(),
            "Should not have error for empty results"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Switch back to Deps tab - should still handle empty gracefully
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

        // Re-sync to ensure empty state persists
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
            *dependency_info = filtered;
            *dep_selected = 0;
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        dependency_info,
        deps_error,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Deps,
            "Should be back on Deps tab"
        );
        assert!(
            dependency_info.is_empty(),
            "Dependencies should still be empty"
        );
        assert!(
            deps_error.is_none(),
            "Should not have error for empty results"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All tabs handle empty results gracefully
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        deps_error,
        files_error,
        services_error,
        sandbox_error,
        services_loaded,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        // All tabs should be empty but without errors
        assert!(dependency_info.is_empty(), "Deps should be empty");
        assert!(deps_error.is_none(), "Deps should not have error");
        assert!(file_info.is_empty(), "Files should be empty");
        assert!(files_error.is_none(), "Files should not have error");
        assert!(service_info.is_empty(), "Services should be empty");
        assert!(*services_loaded, "Services should be marked as loaded");
        assert!(services_error.is_none(), "Services should not have error");
        assert!(sandbox_info.is_empty(), "Sandbox should be empty");
        assert!(*sandbox_loaded, "Sandbox should be marked as loaded");
        assert!(sandbox_error.is_none(), "Sandbox should not have error");
    } else {
        panic!("Expected Preflight modal");
    }
}
