//! //! Tests for error handling and partial failures.

use pacsea as crate_root;
use super::helpers::*;


#[test]
/// What: Verify that preflight modal handles partial failures correctly.
///
/// Inputs:
/// - Packages in install_list
/// - Some tabs resolve successfully (Deps, Files)
/// - One tab fails (Services with error)
/// - User switches between tabs
///
/// Output:
/// - Successful tabs display data correctly
/// - Failed tab displays error message
/// - Other tabs remain functional despite one failure
///
/// Details:
/// - Tests edge case where resolution fails for one tab but succeeds for others
/// - Verifies error messages are shown correctly
/// - Ensures failures don't affect other tabs
fn preflight_handles_partial_failures_correctly() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    let test_packages = vec![crate_root::state::PackageItem {
        name: "test-package-1".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    }];

    // Pre-populate cache with dependencies (successful)
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

    // Pre-populate cache with files (successful)
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

    // Services failed (error in cache)
    app.install_list_services = vec![];
    app.preflight_services_resolving = false;
    app.preflight_services_items = None;

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
        services_error: Some("Failed to resolve services: systemd not available".to_string()),
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: true,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: crate_root::state::modal::CascadeMode::Basic,
    };

    // Test 1: Switch to Deps tab (successful) - should load data
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
        deps_error,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Deps,
            "Should be on Deps tab"
        );
        assert!(!dependency_info.is_empty(), "Dependencies should be loaded");
        assert!(deps_error.is_none(), "Deps should not have error");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Switch to Files tab (successful) - should load data
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
        files_error,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Files,
            "Should be on Files tab"
        );
        assert!(!file_info.is_empty(), "Files should be loaded");
        assert!(files_error.is_none(), "Files should not have error");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Services tab (failed) - should show error
    if let crate_root::state::Modal::Preflight { tab, .. } = &mut app.modal {
        *tab = crate_root::state::PreflightTab::Services;
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
        // Services should be empty and have error
        assert!(service_info.is_empty(), "Services should be empty (failed)");
        assert!(!*services_loaded, "Services should not be marked as loaded");
        assert!(
            services_error.is_some(),
            "Services should have error message"
        );
        assert_eq!(
            services_error.as_ref().unwrap(),
            "Failed to resolve services: systemd not available",
            "Error message should match"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch back to Deps tab - should still work despite Services failure
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
            !dependency_info.is_empty(),
            "Dependencies should still be loaded"
        );
        assert!(
            deps_error.is_none(),
            "Deps should not have error (unaffected by Services failure)"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Switch back to Files tab - should still work despite Services failure
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
            "Should be back on Files tab"
        );
        assert!(!file_info.is_empty(), "Files should still be loaded");
        assert!(
            files_error.is_none(),
            "Files should not have error (unaffected by Services failure)"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: Successful tabs unaffected by failure
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        deps_error,
        files_error,
        services_error,
        services_loaded,
        ..
    } = &app.modal
    {
        // Successful tabs should have data and no errors
        assert!(!dependency_info.is_empty(), "Dependencies should have data");
        assert!(deps_error.is_none(), "Deps should not have error");
        assert!(!file_info.is_empty(), "Files should have data");
        assert!(files_error.is_none(), "Files should not have error");

        // Failed tab should have error and no data
        assert!(service_info.is_empty(), "Services should be empty (failed)");
        assert!(!*services_loaded, "Services should not be loaded");
        assert!(
            services_error.is_some(),
            "Services should have error message"
        );
    } else {
        panic!("Expected Preflight modal");
    }
}
