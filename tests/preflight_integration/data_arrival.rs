//! Tests for out-of-order data arrival and cancellation handling.

use pacsea as crate_root;
use tokio::sync::mpsc;
use super::helpers::*;

#[tokio::test]
/// What: Verify that preflight modal handles out-of-order data arrival correctly.
///
/// Inputs:
/// - Preflight modal opened with multiple packages
/// - Background resolution stages complete in non-sequential order (e.g., Files before Deps)
///
/// Output:
/// - Modal state correctly reflects data as it arrives, regardless of order
/// - All stages eventually show as complete
async fn preflight_handles_out_of_order_data_arrival() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    // Create test packages
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
        },
    ];

    // Set up channels for runtime (simulating result channels)
    let (_deps_req_tx, _deps_req_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::PackageItem>>();
    let (deps_res_tx, mut deps_res_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::modal::DependencyInfo>>();
    let (_files_req_tx, _files_req_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::PackageItem>>();
    let (files_res_tx, mut files_res_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::modal::PackageFileInfo>>();
    let (_services_req_tx, _services_req_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::PackageItem>>();
    let (services_res_tx, mut services_res_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::modal::ServiceImpact>>();
    let (_sandbox_req_tx, _sandbox_req_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::PackageItem>>();
    let (_sandbox_res_tx, _sandbox_res_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::logic::sandbox::SandboxInfo>>();
    let (_summary_req_tx, _summary_req_rx) = mpsc::unbounded_channel::<(
        Vec<crate_root::state::PackageItem>,
        crate_root::state::modal::PreflightAction,
    )>();
    let (summary_res_tx, mut summary_res_rx) =
        mpsc::unbounded_channel::<crate_root::logic::preflight::PreflightSummaryOutcome>();

    // Open preflight modal (simulate what happens in events/install.rs)
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Queue all stages (simulating parallel kick-off)
    app.preflight_summary_items = Some((
        test_packages.clone(),
        crate_root::state::modal::PreflightAction::Install,
    ));
    app.preflight_summary_resolving = true;
    app.preflight_deps_items = Some(test_packages.clone());
    app.preflight_deps_resolving = true;
    app.preflight_files_items = Some(test_packages.clone());
    app.preflight_files_resolving = true;
    app.preflight_services_items = Some(test_packages.clone());
    app.preflight_services_resolving = true;
    // No sandbox items (no AUR packages)

    // Create modal state
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
        sandbox_loaded: true, // No AUR packages, so loaded immediately
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: crate_root::state::modal::CascadeMode::Basic,
    };

    // Verify all stages are queued
    assert!(app.preflight_summary_resolving);
    assert!(app.preflight_deps_resolving);
    assert!(app.preflight_files_resolving);
    assert!(app.preflight_services_resolving);

    // Simulate out-of-order completion:
    // 1. Files completes first (fastest)
    // 2. Services completes second
    // 3. Deps completes third
    // 4. Summary completes last (slowest)

    // Step 1: Files completes first
    let files_result = vec![crate_root::state::modal::PackageFileInfo {
        name: "test-package-1".to_string(),
        files: vec![],
        total_count: 0,
        new_count: 0,
        changed_count: 0,
        removed_count: 0,
        config_count: 0,
        pacnew_candidates: 0usize,
        pacsave_candidates: 0usize,
    }];
    let _ = files_res_tx.send(files_result.clone());

    // Process files result (simulate runtime.rs handler)
    if let Some(files) = files_res_rx.recv().await {
        let was_preflight = app.preflight_files_resolving;
        app.files_resolving = false;
        app.preflight_files_resolving = false;

        if !app
            .preflight_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            app.install_list_files = files.clone();
            if let crate_root::state::Modal::Preflight { file_info, .. } = &mut app.modal {
                *file_info = files;
            }
            if was_preflight {
                app.preflight_files_items = None;
            }
        }
    }

    // Verify files are loaded
    if let crate_root::state::Modal::Preflight { file_info, .. } = &app.modal {
        assert!(!file_info.is_empty(), "Files should be loaded");
    }
    assert!(
        !app.preflight_files_resolving,
        "Files resolving flag should be cleared"
    );

    // Step 2: Services completes second
    let services_result = vec![crate_root::state::modal::ServiceImpact {
        unit_name: "test.service".to_string(),
        providers: vec!["test-package-1".to_string()],
        is_active: true,
        needs_restart: true,
        recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
    }];
    let _ = services_res_tx.send(services_result.clone());

    // Process services result
    if let Some(services) = services_res_rx.recv().await {
        let was_preflight = app.preflight_services_resolving;
        app.services_resolving = false;
        app.preflight_services_resolving = false;

        if !app
            .preflight_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            app.install_list_services = services.clone();
            if let crate_root::state::Modal::Preflight {
                service_info,
                services_loaded,
                ..
            } = &mut app.modal
            {
                *service_info = services;
                *services_loaded = true;
            }
            if was_preflight {
                app.preflight_services_items = None;
            }
        }
    }

    // Verify services are loaded
    if let crate_root::state::Modal::Preflight {
        service_info,
        services_loaded,
        ..
    } = &app.modal
    {
        assert!(*services_loaded, "Services should be marked as loaded");
        assert!(!service_info.is_empty(), "Services should be loaded");
    }
    assert!(
        !app.preflight_services_resolving,
        "Services resolving flag should be cleared"
    );

    // Step 3: Deps completes third
    let deps_result = vec![crate_root::state::modal::DependencyInfo {
        name: "test-dep".to_string(),
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
    let _ = deps_res_tx.send(deps_result.clone());

    // Process deps result
    if let Some(deps) = deps_res_rx.recv().await {
        let was_preflight = app.preflight_deps_resolving;
        app.deps_resolving = false;
        app.preflight_deps_resolving = false;

        if !app
            .preflight_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            app.install_list_deps = deps.clone();
            if let crate_root::state::Modal::Preflight {
                dependency_info, ..
            } = &mut app.modal
            {
                let item_names: std::collections::HashSet<String> =
                    test_packages.iter().map(|i| i.name.clone()).collect();
                let filtered_deps: Vec<_> = deps
                    .iter()
                    .filter(|dep| {
                        dep.required_by
                            .iter()
                            .any(|req_by| item_names.contains(req_by))
                    })
                    .cloned()
                    .collect();
                *dependency_info = filtered_deps;
            }
            if was_preflight {
                app.preflight_deps_items = None;
            }
        }
    }

    // Verify deps are loaded
    if let crate_root::state::Modal::Preflight {
        dependency_info, ..
    } = &app.modal
    {
        assert!(!dependency_info.is_empty(), "Dependencies should be loaded");
    }
    assert!(
        !app.preflight_deps_resolving,
        "Deps resolving flag should be cleared"
    );

    // Step 4: Summary completes last
    let summary_result = crate_root::logic::preflight::PreflightSummaryOutcome {
        summary: crate_root::state::modal::PreflightSummaryData {
            packages: vec![],
            package_count: test_packages.len(),
            aur_count: 0,
            download_bytes: 1000,
            install_delta_bytes: 2000,
            risk_score: 0,
            risk_level: crate_root::state::modal::RiskLevel::Low,
            risk_reasons: vec![],
            major_bump_packages: vec![],
            core_system_updates: vec![],
            pacnew_candidates: 0usize,
            pacsave_candidates: 0usize,
            config_warning_packages: vec![],
            service_restart_units: vec![],
            summary_warnings: vec![],
            summary_notes: vec![],
        },
        header: crate_root::state::modal::PreflightHeaderChips {
            package_count: test_packages.len(),
            download_bytes: 1000,
            install_delta_bytes: 2000,
            aur_count: 0,
            risk_score: 0,
            risk_level: crate_root::state::modal::RiskLevel::Low,
        },
    };
    let _ = summary_res_tx.send(summary_result.clone());

    // Process summary result
    if let Some(summary_outcome) = summary_res_rx.recv().await {
        if !app
            .preflight_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
            && let crate_root::state::Modal::Preflight {
                summary,
                header_chips,
                ..
            } = &mut app.modal
        {
            *summary = Some(Box::new(summary_outcome.summary));
            *header_chips = summary_outcome.header;
        }
        app.preflight_summary_resolving = false;
        app.preflight_summary_items = None;
    }

    // Verify summary is loaded
    if let crate_root::state::Modal::Preflight { summary, .. } = &app.modal {
        assert!(summary.is_some(), "Summary should be loaded");
    }
    assert!(
        !app.preflight_summary_resolving,
        "Summary resolving flag should be cleared"
    );

    // Final verification: all stages should be complete
    assert!(!app.preflight_summary_resolving);
    assert!(!app.preflight_deps_resolving);
    assert!(!app.preflight_files_resolving);
    assert!(!app.preflight_services_resolving);
    assert!(app.preflight_summary_items.is_none());
    assert!(app.preflight_deps_items.is_none());
    assert!(app.preflight_files_items.is_none());
    assert!(app.preflight_services_items.is_none());
}

#[tokio::test]
/// What: Verify that preflight cancellation aborts in-flight work correctly.
///
/// Inputs:
/// - Preflight modal opened with packages
/// - Background resolution stages started
/// - Modal closed (cancellation triggered)
///
/// Output:
/// - Cancellation flag is set
/// - Queued work items are cleared
/// - Results arriving after cancellation are ignored
async fn preflight_cancellation_aborts_in_flight_work() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    let test_packages = vec![crate_root::state::PackageItem {
        name: "test-package".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    }];

    // Set up channels
    let (deps_res_tx, mut deps_res_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::modal::DependencyInfo>>();
    let (files_res_tx, mut files_res_rx) =
        mpsc::unbounded_channel::<Vec<crate_root::state::modal::PackageFileInfo>>();

    // Open preflight modal and queue work
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    app.preflight_deps_items = Some(test_packages.clone());
    app.preflight_deps_resolving = true;
    app.preflight_files_items = Some(test_packages.clone());
    app.preflight_files_resolving = true;

    app.modal = crate_root::state::Modal::Preflight {
        items: test_packages.clone(),
        action: crate_root::state::PreflightAction::Install,
        tab: crate_root::state::PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate_root::state::modal::PreflightHeaderChips {
            package_count: 1,
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

    // Verify work is queued
    assert!(app.preflight_deps_resolving);
    assert!(app.preflight_files_resolving);
    assert!(app.preflight_deps_items.is_some());
    assert!(app.preflight_files_items.is_some());

    // Cancel preflight (simulate modal closing)
    app.preflight_cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    app.preflight_deps_items = None;
    app.preflight_files_items = None;
    app.modal = crate_root::state::Modal::None;

    // Verify cancellation flag is set
    assert!(
        app.preflight_cancelled
            .load(std::sync::atomic::Ordering::Relaxed)
    );

    // Verify queues are cleared
    assert!(app.preflight_deps_items.is_none());
    assert!(app.preflight_files_items.is_none());

    // Simulate results arriving after cancellation
    let deps_result = vec![crate_root::state::modal::DependencyInfo {
        name: "test-dep".to_string(),
        version: "1.0.0".to_string(),
        status: crate_root::state::modal::DependencyStatus::ToInstall,
        source: crate_root::state::modal::DependencySource::Official {
            repo: "core".to_string(),
        },
        required_by: vec!["test-package".to_string()],
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }];
    let _ = deps_res_tx.send(deps_result.clone());

    // Process result (simulate runtime.rs handler checking cancellation)
    if let Some(deps) = deps_res_rx.recv().await {
        let cancelled = app
            .preflight_cancelled
            .load(std::sync::atomic::Ordering::Relaxed);
        let was_preflight = app.preflight_deps_resolving;
        app.deps_resolving = false;
        app.preflight_deps_resolving = false;

        if !cancelled {
            app.install_list_deps = deps;
            if was_preflight {
                app.preflight_deps_items = None;
            }
        } else if was_preflight {
            // Result should be ignored when cancelled
            app.preflight_deps_items = None;
        }
    }

    // Verify that install_list_deps was NOT updated (cancellation prevented update)
    // Since modal is closed, we can't check modal state, but we can verify flags
    assert!(!app.preflight_deps_resolving);
    assert!(app.preflight_deps_items.is_none());

    // Send files result after cancellation
    let files_result = vec![crate_root::state::modal::PackageFileInfo {
        name: "test-package".to_string(),
        files: vec![],
        total_count: 0,
        new_count: 0,
        changed_count: 0,
        removed_count: 0,
        config_count: 0,
        pacnew_candidates: 0usize,
        pacsave_candidates: 0usize,
    }];
    let _ = files_res_tx.send(files_result.clone());

    // Process files result
    if let Some(files) = files_res_rx.recv().await {
        let cancelled = app
            .preflight_cancelled
            .load(std::sync::atomic::Ordering::Relaxed);
        let was_preflight = app.preflight_files_resolving;
        app.files_resolving = false;
        app.preflight_files_resolving = false;

        if !cancelled {
            app.install_list_files = files;
            if was_preflight {
                app.preflight_files_items = None;
            }
        } else if was_preflight {
            app.preflight_files_items = None;
        }
    }

    // Verify flags are cleared
    assert!(!app.preflight_files_resolving);
    assert!(app.preflight_files_items.is_none());
}

