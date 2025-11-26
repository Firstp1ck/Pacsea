//! //! Tests for tab switching behavior.

use pacsea as crate_root;

#[test]
/// What: Verify that preflight modal handles mixed completion states correctly when switching tabs.
///
/// Inputs:
/// - Packages in `install_list`
/// - Some tabs have data loaded (Deps, Files)
/// - Some tabs are still resolving (Services, Sandbox)
/// - User switches between tabs
///
/// Output:
/// - Tabs with loaded data display correctly
/// - Tabs still resolving show appropriate loading state
/// - No data corruption or mixing between tabs
///
/// Details:
/// - Tests edge case where resolution completes at different times
/// - Verifies that partial data doesn't cause issues when switching tabs
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn preflight_handles_mixed_completion_states_when_switching_tabs() {
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
            name: "test-aur-package".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
    ];

    // Pre-populate cache with dependencies (loaded)
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

    // Pre-populate cache with files (loaded)
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

    // Services are still resolving (not in cache yet)
    app.install_list_services = vec![];
    app.preflight_services_resolving = true;
    app.preflight_services_items = Some(test_packages.clone());

    // Sandbox is still resolving (not in cache yet)
    app.install_list_sandbox = vec![];
    app.preflight_sandbox_resolving = true;
    let aur_items: Vec<_> = test_packages
        .iter()
        .filter(|p| matches!(p.source, crate_root::state::Source::Aur))
        .cloned()
        .collect();
    app.preflight_sandbox_items = Some(aur_items);

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
        cached_reverse_deps_report: None,
    };

    // Test 1: Switch to Deps tab (has data) - should load immediately
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
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Deps,
            "Should be on Deps tab"
        );
        assert!(!dependency_info.is_empty(), "Dependencies should be loaded");
        assert_eq!(dependency_info.len(), 1, "Should have 1 dependency");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Switch to Files tab (has data) - should load immediately
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

    if let crate_root::state::Modal::Preflight { tab, file_info, .. } = &app.modal {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Files,
            "Should be on Files tab"
        );
        assert!(!file_info.is_empty(), "Files should be loaded");
        assert_eq!(file_info.len(), 1, "Should have 1 file entry");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Services tab (still resolving) - should show loading state
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

        // Simulate sync_services logic (should not load since still resolving)
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
        services_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Services,
            "Should be on Services tab"
        );
        // Services should be empty and not loaded since still resolving
        assert!(
            service_info.is_empty(),
            "Services should be empty (still resolving)"
        );
        assert!(
            !*services_loaded,
            "Services should not be marked as loaded (still resolving)"
        );
        // Verify resolving flag is still set
        assert!(
            app.preflight_services_resolving,
            "Services should still be resolving"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Sandbox tab (still resolving) - should show loading state
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

        // Simulate sync_sandbox logic (should not load since still resolving)
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
        // Sandbox should be empty and not loaded since still resolving
        assert!(
            sandbox_info.is_empty(),
            "Sandbox should be empty (still resolving)"
        );
        assert!(
            !*sandbox_loaded,
            "Sandbox should not be marked as loaded (still resolving)"
        );
        // Verify resolving flag is still set
        assert!(
            app.preflight_sandbox_resolving,
            "Sandbox should still be resolving"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Switch back to Deps tab - data should still be there
    if let crate_root::state::Modal::Preflight {
        dependency_info, ..
    } = &app.modal
    {
        // Just verify data is still there (we're already on Deps from previous sync)
        assert!(
            !dependency_info.is_empty(),
            "Dependencies should still be loaded when switching back"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 6: Switch back to Files tab - data should still be there
    if let crate_root::state::Modal::Preflight {
        items,
        tab,
        file_info,
        file_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Files;

        // Re-sync to ensure data persists
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

    if let crate_root::state::Modal::Preflight { tab, file_info, .. } = &app.modal {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Files,
            "Should be back on Files tab"
        );
        assert!(
            !file_info.is_empty(),
            "Files should still be loaded when switching back"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: Mixed state is maintained correctly
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        services_loaded,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        // Tabs with data should have data
        assert!(!dependency_info.is_empty(), "Dependencies should have data");
        assert!(!file_info.is_empty(), "Files should have data");

        // Tabs still resolving should be empty
        assert!(
            service_info.is_empty(),
            "Services should be empty (still resolving)"
        );
        assert!(!*services_loaded, "Services should not be loaded");
        assert!(
            sandbox_info.is_empty(),
            "Sandbox should be empty (still resolving)"
        );
        assert!(!*sandbox_loaded, "Sandbox should not be loaded");
    } else {
        panic!("Expected Preflight modal");
    }
}
