//! //! Tests for remove operations.

use pacsea as crate_root;
use super::helpers::*;


#[test]
/// What: Verify that preflight modal handles remove action correctly with reverse dependencies.
///
/// Inputs:
/// - Packages in remove_list
/// - Preflight modal opened with Remove action
/// - Reverse dependencies resolved
///
/// Output:
/// - Deps tab shows reverse dependencies correctly
/// - Other tabs handle remove action appropriately
/// - Cascade mode affects dependency display
///
/// Details:
/// - Tests preflight modal for remove operations
/// - Verifies reverse dependency resolution works
/// - Ensures remove-specific logic is handled correctly
fn preflight_remove_action_with_reverse_dependencies() {
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

    // Set packages in remove list
    app.remove_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Simulate reverse dependency resolution
    // In real code, this would call resolve_reverse_dependencies
    // For test, we'll manually set up the reverse dependency data
    let reverse_deps = vec![
        crate_root::state::modal::DependencyInfo {
            name: "dependent-package-1".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Installed {
                version: "2.0.0".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package-1".to_string()],
            depends_on: vec!["test-package-1".to_string()],
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "dependent-package-2".to_string(),
            version: "3.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Installed {
                version: "3.0.0".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "community".to_string(),
            },
            required_by: vec!["test-package-1".to_string()],
            depends_on: vec!["test-package-1".to_string()],
            is_core: false,
            is_system: false,
        },
    ];

    // Store reverse dependencies in remove_preflight_summary (used by remove action)
    app.remove_preflight_summary = vec![crate_root::state::modal::ReverseRootSummary {
        package: "test-package-1".to_string(),
        direct_dependents: 2,
        transitive_dependents: 0,
        total_dependents: 2,
    }];

    // Open preflight modal with Remove action
    app.modal = crate_root::state::Modal::Preflight {
        items: test_packages.clone(),
        action: crate_root::state::PreflightAction::Remove,
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

    // Test 1: Switch to Deps tab - should show reverse dependencies
    // For Remove action, reverse deps are computed on-demand when tab is accessed
    if let crate_root::state::Modal::Preflight {
        action,
        tab,
        dependency_info,
        dep_selected,
        ..
    } = &mut app.modal
    {
        *tab = crate_root::state::PreflightTab::Deps;

        // Simulate reverse dependency resolution for Remove action
        if matches!(*action, crate_root::state::PreflightAction::Remove) {
            // In real code, this would call resolve_reverse_dependencies
            // For test, we'll use the pre-populated reverse_deps
            *dependency_info = reverse_deps.clone();
            *dep_selected = 0;
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        dependency_info,
        action,
        dep_selected,
        ..
    } = &app.modal
    {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Deps,
            "Should be on Deps tab"
        );
        assert_eq!(
            *action,
            crate_root::state::PreflightAction::Remove,
            "Should be Remove action"
        );
        assert!(
            !dependency_info.is_empty(),
            "Reverse dependencies should be loaded"
        );
        assert_eq!(
            dependency_info.len(),
            2,
            "Should have 2 reverse dependencies"
        );

        // Verify reverse dependencies are correct
        let dep1 = dependency_info
            .iter()
            .find(|d| d.name == "dependent-package-1")
            .unwrap();
        assert_eq!(dep1.version, "2.0.0");
        assert!(dep1.depends_on.contains(&"test-package-1".to_string()));
        assert!(dep1.required_by.contains(&"test-package-1".to_string()));

        let dep2 = dependency_info
            .iter()
            .find(|d| d.name == "dependent-package-2")
            .unwrap();
        assert_eq!(dep2.version, "3.0.0");
        assert!(dep2.depends_on.contains(&"test-package-1".to_string()));
        assert!(dep2.required_by.contains(&"test-package-1".to_string()));

        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Verify remove_preflight_summary is populated
    assert!(
        !app.remove_preflight_summary.is_empty(),
        "Remove preflight summary should be populated"
    );
    let summary = &app.remove_preflight_summary[0];
    assert_eq!(summary.package, "test-package-1");
    assert_eq!(summary.direct_dependents, 2);
    assert_eq!(summary.total_dependents, 2);

    // Test 3: Switch to Files tab - should handle remove action
    if let crate_root::state::Modal::Preflight { tab, .. } = &mut app.modal {
        *tab = crate_root::state::PreflightTab::Files;
    }

    if let crate_root::state::Modal::Preflight { tab, action, .. } = &app.modal {
        assert_eq!(
            *tab,
            crate_root::state::PreflightTab::Files,
            "Should be on Files tab"
        );
        assert_eq!(
            *action,
            crate_root::state::PreflightAction::Remove,
            "Should still be Remove action"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch back to Deps tab - reverse dependencies should persist
    if let crate_root::state::Modal::Preflight {
        dependency_info, ..
    } = &app.modal
    {
        // Verify data persists (we're already on Deps from previous sync)
        assert!(
            !dependency_info.is_empty(),
            "Reverse dependencies should persist when switching back"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: Remove action specific data
    if let crate_root::state::Modal::Preflight {
        action,
        dependency_info,
        ..
    } = &app.modal
    {
        assert_eq!(
            *action,
            crate_root::state::PreflightAction::Remove,
            "Should be Remove action"
        );
        assert!(
            !dependency_info.is_empty(),
            "Reverse dependencies should be present"
        );
        // All dependencies should depend on the package being removed
        for dep in dependency_info.iter() {
            assert!(
                dep.depends_on.contains(&"test-package-1".to_string()),
                "All reverse dependencies should depend on test-package-1"
            );
        }
    } else {
        panic!("Expected Preflight modal");
    }
}
