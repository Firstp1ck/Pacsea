//! //! Tests for cache synchronization.

use pacsea as crate_root;
use super::helpers::*;


#[test]
/// What: Verify that preflight modal syncs updated cache data when background resolution completes.
///
/// Inputs:
/// - Packages in install_list
/// - Preflight modal opened with some data missing
/// - Background resolution completes and updates cache while modal is open
/// - User switches to affected tab
///
/// Output:
/// - Updated data appears when switching to the tab
/// - Old data is replaced with new data
/// - Modal state is correctly updated
///
/// Details:
/// - Tests that cache updates during modal open are handled correctly
/// - Verifies data synchronization when background work completes
/// - Ensures modal reflects latest cached data
fn preflight_syncs_cache_updates_during_modal_open() {
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

    // Initially, only dependencies are cached
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

    // Files are not cached yet (still resolving)
    app.install_list_files = vec![];
    app.preflight_files_resolving = true;
    app.preflight_files_items = Some(test_packages.clone());

    // Services are not cached yet (still resolving)
    app.install_list_services = vec![];
    app.preflight_services_resolving = true;
    app.preflight_services_items = Some(test_packages.clone());

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
    };

    // Test 1: Switch to Deps tab - should load initial cached data
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
        assert_eq!(dependency_info.len(), 1, "Should have 1 initial dependency");
        assert_eq!(
            dependency_info[0].name, "test-dep-1",
            "Initial dependency should be test-dep-1"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Simulate background resolution completing and updating cache
    // Add new dependency to cache
    app.install_list_deps
        .push(crate_root::state::modal::DependencyInfo {
            name: "test-dep-2".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package-1".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        });

    // Files resolution completes - update cache
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
    app.preflight_files_resolving = false;
    app.preflight_files_items = None;

    // Services resolution completes - update cache
    app.install_list_services = vec![crate_root::state::modal::ServiceImpact {
        unit_name: "test-service.service".to_string(),
        providers: vec!["test-package-1".to_string()],
        is_active: true,
        needs_restart: true,
        recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
    }];
    app.preflight_services_resolving = false;
    app.preflight_services_items = None;

    // Test 2: Switch back to Deps tab - should sync updated cache (now has 2 deps)
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

        // Re-sync to get updated cache
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
            "Should have 2 dependencies after cache update"
        );
        assert!(
            dependency_info.iter().any(|d| d.name == "test-dep-1"),
            "Should still have test-dep-1"
        );
        assert!(
            dependency_info.iter().any(|d| d.name == "test-dep-2"),
            "Should have new test-dep-2"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Files tab - should load newly cached files
    if let crate_root::state::Modal::Preflight {
        items,
        tab,
        file_info,
        file_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Files;

        // Simulate sync_files logic - should now find cached files
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
        assert!(
            !file_info.is_empty(),
            "Files should be loaded from updated cache"
        );
        assert_eq!(file_info.len(), 1, "Should have 1 file entry");
        assert_eq!(
            file_info[0].name, "test-package-1",
            "File package name should match"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Services tab - should load newly cached services
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

        // Simulate sync_services logic - should now find cached services
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
        assert!(
            !service_info.is_empty(),
            "Services should be loaded from updated cache"
        );
        assert!(*services_loaded, "Services should be marked as loaded");
        assert_eq!(service_info.len(), 1, "Should have 1 service");
        assert_eq!(
            service_info[0].unit_name, "test-service.service",
            "Service unit name should match"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Verify resolving flags are cleared
    assert!(
        !app.preflight_files_resolving,
        "Files resolving flag should be cleared"
    );
    assert!(
        app.preflight_files_items.is_none(),
        "Files items should be cleared"
    );
    assert!(
        !app.preflight_services_resolving,
        "Services resolving flag should be cleared"
    );
    assert!(
        app.preflight_services_items.is_none(),
        "Services items should be cleared"
    );

    // Final verification: All updated data is present
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        services_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            dependency_info.len(),
            2,
            "Should have 2 dependencies after cache update"
        );
        assert!(!file_info.is_empty(), "Files should be loaded");
        assert!(!service_info.is_empty(), "Services should be loaded");
        assert!(*services_loaded, "Services should be marked as loaded");
    } else {
        panic!("Expected Preflight modal");
    }
}
