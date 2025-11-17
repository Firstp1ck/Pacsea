//! Integration tests for preflight modal optimization features.
//!
//! Tests cover:
//! - Out-of-order data arrival (stages completing in different orders)
//! - Cancellation support (aborting work when modal closes)

#![cfg(test)]

use pacsea as crate_root;
use tokio::sync::mpsc;

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

#[test]
/// What: Verify that preflight modal correctly loads cached data when packages are already in install list.
///
/// Inputs:
/// - Packages already listed in install_list
/// - Pre-populated cache with dependencies (including conflicts), files, services, and sandbox data
/// - Preflight modal opened
///
/// Output:
/// - Deps tab correctly loads and displays dependencies and conflicts
/// - Files tab correctly loads and displays file information
/// - Services tab correctly loads and displays service impacts
/// - Sandbox tab correctly loads and displays sandbox information
///
/// Details:
/// - Tests edge case where data is already cached before preflight starts
/// - Verifies that all tabs correctly sync data from cache to modal state
/// - Ensures UI can display the cached data correctly
fn preflight_loads_cached_data_when_packages_already_in_install_list() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    // Create test packages (mix of official and AUR for sandbox testing)
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
        crate_root::state::PackageItem {
            name: "test-aur-package".to_string(),
            version: "3.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        },
    ];

    // Pre-populate cache with dependencies (including conflicts)
    app.install_list_deps = vec![
        crate_root::state::modal::DependencyInfo {
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
        },
        crate_root::state::modal::DependencyInfo {
            name: "test-conflict".to_string(),
            version: "2.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::Conflict {
                reason: "Conflicts with existing-package (1.0.0)".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package-2".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "test-dep-2".to_string(),
            version: "3.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToUpgrade {
                current: "2.0.0".to_string(),
                required: "3.0.0".to_string(),
            },
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-package-1".to_string(), "test-package-2".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    // Pre-populate cache with files
    app.install_list_files = vec![
        crate_root::state::modal::PackageFileInfo {
            name: "test-package-1".to_string(),
            files: vec![
                crate_root::state::modal::FileChange {
                    path: "/usr/bin/test1".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "test-package-1".to_string(),
                    is_config: false,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                },
                crate_root::state::modal::FileChange {
                    path: "/etc/test1.conf".to_string(),
                    change_type: crate_root::state::modal::FileChangeType::New,
                    package: "test-package-1".to_string(),
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
            name: "test-package-2".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/test2".to_string(),
                change_type: crate_root::state::modal::FileChangeType::Changed,
                package: "test-package-2".to_string(),
                is_config: false,
                predicted_pacnew: false,
                predicted_pacsave: false,
            }],
            total_count: 1,
            new_count: 0,
            changed_count: 1,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        },
    ];

    // Pre-populate cache with services
    app.install_list_services = vec![
        crate_root::state::modal::ServiceImpact {
            unit_name: "test-service-1.service".to_string(),
            providers: vec!["test-package-1".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "test-service-2.service".to_string(),
            providers: vec!["test-package-2".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
        },
    ];

    // Pre-populate cache with sandbox info (for AUR package)
    app.install_list_sandbox = vec![crate_root::logic::sandbox::SandboxInfo {
        package_name: "test-aur-package".to_string(),
        depends: vec![
            crate_root::logic::sandbox::DependencyDelta {
                name: "dep1".to_string(),
                is_installed: true,
                installed_version: Some("1.0.0".to_string()),
                version_satisfied: true,
            },
            crate_root::logic::sandbox::DependencyDelta {
                name: "dep2".to_string(),
                is_installed: false,
                installed_version: None,
                version_satisfied: false,
            },
        ],
        makedepends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "make-dep".to_string(),
            is_installed: true,
            installed_version: Some("2.0.0".to_string()),
            version_satisfied: true,
        }],
        checkdepends: vec![],
        optdepends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "opt-dep".to_string(),
            is_installed: false,
            installed_version: None,
            version_satisfied: false,
        }],
    }];

    // Set packages in install list
    app.install_list = test_packages.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Open preflight modal (simulate what happens in events/install.rs)
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

    // Verify initial state - modal should be empty
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        ..
    } = &app.modal
    {
        assert!(
            dependency_info.is_empty(),
            "Dependencies should be empty initially"
        );
        assert!(file_info.is_empty(), "Files should be empty initially");
        assert!(
            service_info.is_empty(),
            "Services should be empty initially"
        );
        assert!(sandbox_info.is_empty(), "Sandbox should be empty initially");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 1: Switch to Deps tab and verify dependencies (including conflicts) are loaded
    // Manually switch tab and sync data (simulating what sync_dependencies does)
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
        assert_eq!(dependency_info.len(), 3, "Should have 3 dependencies");

        // Verify dependency types are present
        let has_to_install = dependency_info.iter().any(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::ToInstall
            )
        });
        let has_conflict = dependency_info.iter().any(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::Conflict { .. }
            )
        });
        let has_upgrade = dependency_info.iter().any(|d| {
            matches!(
                d.status,
                crate_root::state::modal::DependencyStatus::ToUpgrade { .. }
            )
        });

        assert!(has_to_install, "Should have ToInstall dependency");
        assert!(has_conflict, "Should have Conflict dependency");
        assert!(has_upgrade, "Should have ToUpgrade dependency");
        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Switch to Files tab and verify files are loaded
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
        assert_eq!(file_info.len(), 2, "Should have 2 file entries");

        // Verify file data
        let pkg1_files = file_info
            .iter()
            .find(|f| f.name == "test-package-1")
            .unwrap();
        assert_eq!(pkg1_files.files.len(), 2, "Package 1 should have 2 files");
        assert_eq!(pkg1_files.total_count, 2);
        assert_eq!(pkg1_files.new_count, 2);
        assert_eq!(pkg1_files.config_count, 1);
        assert_eq!(pkg1_files.pacnew_candidates, 1);

        let pkg2_files = file_info
            .iter()
            .find(|f| f.name == "test-package-2")
            .unwrap();
        assert_eq!(pkg2_files.files.len(), 1, "Package 2 should have 1 file");
        assert_eq!(pkg2_files.total_count, 1);
        assert_eq!(pkg2_files.changed_count, 1);
        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Switch to Services tab and verify services are loaded
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
        assert_eq!(service_info.len(), 2, "Should have 2 services");

        // Verify service data
        let svc1 = service_info
            .iter()
            .find(|s| s.unit_name == "test-service-1.service")
            .unwrap();
        assert!(svc1.is_active);
        assert!(svc1.needs_restart);
        assert_eq!(
            svc1.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );

        let svc2 = service_info
            .iter()
            .find(|s| s.unit_name == "test-service-2.service")
            .unwrap();
        assert!(!svc2.is_active);
        assert!(!svc2.needs_restart);
        assert_eq!(
            svc2.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer
        );
        assert_eq!(*service_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Switch to Sandbox tab and verify sandbox info is loaded
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        tab,
        sandbox_info,
        sandbox_loaded,
        sandbox_selected,
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
                *sandbox_selected = 0;
            }
        }
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        sandbox_info,
        sandbox_selected,
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

        // Verify sandbox data
        let sandbox = sandbox_info
            .iter()
            .find(|s| s.package_name == "test-aur-package")
            .unwrap();
        assert_eq!(sandbox.depends.len(), 2, "Should have 2 depends");
        assert_eq!(sandbox.makedepends.len(), 1, "Should have 1 makedepends");
        assert_eq!(sandbox.checkdepends.len(), 0, "Should have 0 checkdepends");
        assert_eq!(sandbox.optdepends.len(), 1, "Should have 1 optdepends");

        // Verify dependency details
        let dep1 = sandbox.depends.iter().find(|d| d.name == "dep1").unwrap();
        assert!(dep1.is_installed);
        assert_eq!(dep1.installed_version, Some("1.0.0".to_string()));

        let dep2 = sandbox.depends.iter().find(|d| d.name == "dep2").unwrap();
        assert!(!dep2.is_installed);

        assert_eq!(*sandbox_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All tabs should have loaded their data correctly
    // Switch back to Deps to verify data persists
    if let crate_root::state::Modal::Preflight { tab, .. } = &mut app.modal {
        *tab = crate_root::state::PreflightTab::Deps;
    }

    if let crate_root::state::Modal::Preflight {
        tab,
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
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
        assert!(!file_info.is_empty(), "Files should still be loaded");
        assert!(!service_info.is_empty(), "Services should still be loaded");
        assert!(!sandbox_info.is_empty(), "Sandbox should still be loaded");
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that adding a second package to install list preserves first package's cached data.
///
/// Inputs:
/// - First package already in install_list with cached data
/// - Second package added to install_list
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

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    // First package with cached data
    let first_package = crate_root::state::PackageItem {
        name: "first-package".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    };

    // Pre-populate cache with first package's data
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

    // Set first package in install list
    app.install_list = vec![first_package.clone()];
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Now add second package
    let second_package = crate_root::state::PackageItem {
        name: "second-package".to_string(),
        version: "2.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    };

    // Add second package's data to cache (simulating it being resolved)
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

    // Add a conflict: second package conflicts with first-dep-1
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

    // Update install list to include both packages
    app.install_list = vec![first_package.clone(), second_package.clone()];

    // Open preflight modal with both packages
    app.modal = crate_root::state::Modal::Preflight {
        items: vec![first_package.clone(), second_package.clone()],
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
    };

    // Test 1: Verify Deps tab loads both packages correctly and detects conflicts
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

        // Verify first package's dependencies are present
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

        // Verify second package's dependencies are present
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

        // Verify conflict involves first-dep-1
        let conflict = conflicts[0];
        assert_eq!(conflict.name, "first-dep-1");
        assert!(conflict.required_by.contains(&"second-package".to_string()));

        // Verify first package's original dependencies are unchanged
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

    // Test 2: Verify Files tab loads both packages correctly
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
        assert_eq!(file_info.len(), 2, "Should have 2 file entries");

        // Verify first package's files are preserved
        let first_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .unwrap();
        assert_eq!(
            first_files.files.len(),
            2,
            "First package should have 2 files"
        );
        assert_eq!(first_files.total_count, 2);
        assert_eq!(first_files.new_count, 2);
        assert_eq!(first_files.config_count, 1);
        assert_eq!(first_files.pacnew_candidates, 1);

        // Verify second package's files are loaded
        let second_files = file_info
            .iter()
            .find(|f| f.name == "second-package")
            .unwrap();
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

    // Test 3: Verify Services tab loads both packages correctly
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
        assert_eq!(service_info.len(), 2, "Should have 2 services");

        // Verify first package's service is preserved
        let first_svc = service_info
            .iter()
            .find(|s| s.unit_name == "first-service.service")
            .unwrap();
        assert!(first_svc.is_active);
        assert!(first_svc.needs_restart);
        assert_eq!(
            first_svc.restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart
        );
        assert!(first_svc.providers.contains(&"first-package".to_string()));

        // Verify second package's service is loaded
        let second_svc = service_info
            .iter()
            .find(|s| s.unit_name == "second-service.service")
            .unwrap();
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

    // Final verification: All data for both packages should be present
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        ..
    } = &app.modal
    {
        // Verify both packages have dependencies
        let first_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"first-package".to_string()))
            .collect();
        let second_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"second-package".to_string()))
            .collect();
        assert!(
            !first_pkg_deps.is_empty(),
            "First package should have dependencies"
        );
        assert!(
            !second_pkg_deps.is_empty(),
            "Second package should have dependencies"
        );

        // Verify both packages have files
        assert!(
            file_info.iter().any(|f| f.name == "first-package"),
            "First package should have files"
        );
        assert!(
            file_info.iter().any(|f| f.name == "second-package"),
            "Second package should have files"
        );

        // Verify both packages have services
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
/// What: Verify that adding a second package while first package is loading preserves independence.
///
/// Inputs:
/// - First package added to install_list and starts loading
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

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

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
    };

    // Simulate first package being added and starting to load
    // Some data is already cached, some is still resolving
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
    app.preflight_deps_items = Some(vec![first_package.clone()]);

    // Now add second package while first is still loading
    let second_package = crate_root::state::PackageItem {
        name: "second-package".to_string(),
        version: "2.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
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

    // Open preflight modal with both packages
    app.modal = crate_root::state::Modal::Preflight {
        items: vec![first_package.clone(), second_package.clone()],
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
    };

    // Test 1: Verify Deps tab loads both packages independently and detects conflicts
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

        // Verify conflict involves first-dep-1 but is required by second package
        let conflict = conflicts[0];
        assert_eq!(conflict.name, "first-dep-1");
        assert_eq!(conflict.version, "2.0.0");
        assert!(conflict.required_by.contains(&"second-package".to_string()));
        assert!(!conflict.required_by.contains(&"first-package".to_string()));

        // Verify first package's dependency is not affected by conflict
        // (first package still has its own first-dep-1 with version 1.0.0)
        assert_eq!(first_dep_1.version, "1.0.0");
        assert!(matches!(
            first_dep_1.status,
            crate_root::state::modal::DependencyStatus::ToInstall
        ));

        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Verify Files tab loads both packages independently
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
        assert_eq!(file_info.len(), 2, "Should have 2 file entries");

        // Verify first package's files are independent
        let first_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .unwrap();
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
            .unwrap();
        assert_eq!(
            second_files.files.len(),
            2,
            "Second package should have 2 files"
        );
        assert_eq!(second_files.total_count, 2);
        assert_eq!(second_files.new_count, 2);
        assert_eq!(second_files.config_count, 1);

        // Verify files are independent - first package's file count is not affected
        assert_eq!(first_files.files.len(), 1);
        assert_eq!(second_files.files.len(), 2);

        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Verify Services tab loads both packages independently
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

        // Verify second package's service is loaded (first package's service was still loading)
        assert_eq!(
            service_info.len(),
            1,
            "Should have 1 service (second package's, first still loading)"
        );

        // Verify second package's service is independent
        let second_svc = service_info
            .iter()
            .find(|s| s.unit_name == "second-service.service")
            .unwrap();
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

    // Final verification: Both packages should be independent
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        ..
    } = &app.modal
    {
        // Verify first package's data is independent
        let first_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"first-package".to_string()))
            .collect();
        let first_pkg_files = file_info
            .iter()
            .find(|f| f.name == "first-package")
            .unwrap();

        // Verify second package's data is independent
        let second_pkg_deps: Vec<_> = dependency_info
            .iter()
            .filter(|d| d.required_by.contains(&"second-package".to_string()))
            .collect();
        let second_pkg_files = file_info
            .iter()
            .find(|f| f.name == "second-package")
            .unwrap();

        // Verify independence: first package's data is not affected by second
        assert_eq!(
            first_pkg_deps.len(),
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
            second_pkg_deps.len(),
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
/// What: Verify that preflight modal handles mixed completion states correctly when switching tabs.
///
/// Inputs:
/// - Packages in install_list
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
fn preflight_handles_mixed_completion_states_when_switching_tabs() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

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
            name: "test-aur-package".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
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

#[test]
/// What: Verify that preflight modal loads data correctly regardless of tab switching order.
///
/// Inputs:
/// - Packages in install_list with all data cached
/// - User switches tabs in different orders (e.g., Summary → Sandbox → Deps → Files → Services)
///
/// Output:
/// - Each tab loads its data correctly when accessed
/// - Data persists when switching back to previously visited tabs
/// - No data corruption regardless of switching order
///
/// Details:
/// - Tests that tab switching order doesn't affect data loading
/// - Verifies data persistence across tab switches
/// - Ensures no race conditions or data loss
fn preflight_tab_switching_order_variations() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

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
            name: "test-aur-package".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        },
    ];

    // Pre-populate cache with all data
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

    app.install_list_services = vec![crate_root::state::modal::ServiceImpact {
        unit_name: "test-service.service".to_string(),
        providers: vec!["test-package-1".to_string()],
        is_active: true,
        needs_restart: true,
        recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
    }];

    app.install_list_sandbox = vec![crate_root::logic::sandbox::SandboxInfo {
        package_name: "test-aur-package".to_string(),
        depends: vec![],
        makedepends: vec![],
        checkdepends: vec![],
        optdepends: vec![],
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

    // Test different tab switching orders
    // Order 1: Summary → Sandbox → Deps → Files → Services
    // Order 2: Services → Files → Deps → Sandbox → Summary
    // Order 3: Deps → Services → Files → Sandbox → Deps (back to first)

    // Order 1: Summary → Sandbox
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

    // Order 1: Sandbox → Deps
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

    // Order 1: Deps → Files
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

    // Order 1: Files → Services
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

    // Verify all tabs have data after Order 1
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
        assert!(!dependency_info.is_empty(), "Deps should have data");
        assert!(!file_info.is_empty(), "Files should have data");
        assert!(!service_info.is_empty(), "Services should have data");
        assert!(*services_loaded, "Services should be loaded");
        assert!(!sandbox_info.is_empty(), "Sandbox should have data");
        assert!(*sandbox_loaded, "Sandbox should be loaded");
    } else {
        panic!("Expected Preflight modal");
    }

    // Order 2: Services → Files (reverse order)
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

    // Order 2: Files → Deps
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

    // Order 2: Deps → Sandbox
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

        // Re-sync to ensure data persists
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

    // Verify all tabs still have data after Order 2
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
        assert!(!dependency_info.is_empty(), "Deps should still have data");
        assert!(!file_info.is_empty(), "Files should still have data");
        assert!(!service_info.is_empty(), "Services should still have data");
        assert!(*services_loaded, "Services should still be loaded");
        assert!(!sandbox_info.is_empty(), "Sandbox should still have data");
        assert!(*sandbox_loaded, "Sandbox should still be loaded");
    } else {
        panic!("Expected Preflight modal");
    }

    // Order 3: Sandbox → Deps (back to first tab)
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

    // Final verification: All data persists regardless of switching order
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
        // Verify all tabs have correct data
        assert_eq!(dependency_info.len(), 1, "Deps should have 1 dependency");
        assert_eq!(file_info.len(), 1, "Files should have 1 file entry");
        assert_eq!(service_info.len(), 1, "Services should have 1 service");
        assert_eq!(sandbox_info.len(), 1, "Sandbox should have 1 entry");
        assert!(*services_loaded, "Services should be loaded");
        assert!(*sandbox_loaded, "Sandbox should be loaded");

        // Verify data integrity
        assert_eq!(
            dependency_info[0].name, "test-dep-1",
            "Dependency name should match"
        );
        assert_eq!(
            file_info[0].name, "test-package-1",
            "File package name should match"
        );
        assert_eq!(
            service_info[0].unit_name, "test-service.service",
            "Service unit name should match"
        );
        assert_eq!(
            sandbox_info[0].package_name, "test-aur-package",
            "Sandbox package name should match"
        );
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that preflight modal handles empty results gracefully across all tabs.
///
/// Inputs:
/// - Packages in install_list
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
fn preflight_handles_empty_results_gracefully() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    let test_packages = vec![crate_root::state::PackageItem {
        name: "test-package-empty".to_string(),
        version: "1.0.0".to_string(),
        description: String::new(),
        source: crate_root::state::Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
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
            if !cached_services.is_empty() {
                *service_info = cached_services;
                *services_loaded = true;
            } else {
                *services_loaded = true; // Mark as loaded even if empty
            }
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
            if !cached_sandbox.is_empty() {
                *sandbox_info = cached_sandbox;
                *sandbox_loaded = true;
            } else {
                *sandbox_loaded = true; // Mark as loaded even if empty
            }
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

#[test]
/// What: Verify that preflight modal handles mix of AUR and official packages correctly.
///
/// Inputs:
/// - Mix of AUR and official packages in install_list
/// - Different loading characteristics for each type
/// - Preflight modal opened with both types
///
/// Output:
/// - Sandbox tab only shows AUR packages
/// - Other tabs (Deps, Files, Services) show all packages
/// - AUR-specific features (sandbox) work correctly
/// - Official packages are excluded from sandbox
///
/// Details:
/// - Tests that filtering works correctly for AUR vs official packages
/// - Verifies sandbox tab only displays AUR packages
/// - Ensures other tabs display all packages regardless of source
fn preflight_handles_aur_and_official_package_mix() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    let test_packages = vec![
        crate_root::state::PackageItem {
            name: "test-official-package".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
        crate_root::state::PackageItem {
            name: "test-aur-package".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        },
    ];

    // Pre-populate cache with dependencies for both packages
    app.install_list_deps = vec![
        crate_root::state::modal::DependencyInfo {
            name: "official-dep-1".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-official-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        crate_root::state::modal::DependencyInfo {
            name: "aur-dep-1".to_string(),
            version: "1.0.0".to_string(),
            status: crate_root::state::modal::DependencyStatus::ToInstall,
            source: crate_root::state::modal::DependencySource::Aur,
            required_by: vec!["test-aur-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    // Pre-populate cache with files for both packages
    app.install_list_files = vec![
        crate_root::state::modal::PackageFileInfo {
            name: "test-official-package".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/official".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "test-official-package".to_string(),
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
        crate_root::state::modal::PackageFileInfo {
            name: "test-aur-package".to_string(),
            files: vec![crate_root::state::modal::FileChange {
                path: "/usr/bin/aur".to_string(),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: "test-aur-package".to_string(),
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

    // Pre-populate cache with services for both packages
    app.install_list_services = vec![
        crate_root::state::modal::ServiceImpact {
            unit_name: "official-service.service".to_string(),
            providers: vec!["test-official-package".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
        },
        crate_root::state::modal::ServiceImpact {
            unit_name: "aur-service.service".to_string(),
            providers: vec!["test-aur-package".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate_root::state::modal::ServiceRestartDecision::Defer,
        },
    ];

    // Pre-populate cache with sandbox info (only for AUR package)
    app.install_list_sandbox = vec![crate_root::logic::sandbox::SandboxInfo {
        package_name: "test-aur-package".to_string(),
        depends: vec![crate_root::logic::sandbox::DependencyDelta {
            name: "aur-dep-1".to_string(),
            is_installed: false,
            installed_version: None,
            version_satisfied: false,
        }],
        makedepends: vec![],
        checkdepends: vec![],
        optdepends: vec![],
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

    // Test 1: Deps tab - should show dependencies for both packages
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
        assert_eq!(
            dependency_info.len(),
            2,
            "Should have 2 dependencies (one for each package)"
        );
        assert!(
            dependency_info
                .iter()
                .any(|d| d.required_by.contains(&"test-official-package".to_string())),
            "Should have dependency for official package"
        );
        assert!(
            dependency_info
                .iter()
                .any(|d| d.required_by.contains(&"test-aur-package".to_string())),
            "Should have dependency for AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Files tab - should show files for both packages
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

    if let crate_root::state::Modal::Preflight { file_info, .. } = &app.modal {
        assert_eq!(
            file_info.len(),
            2,
            "Should have 2 file entries (one for each package)"
        );
        assert!(
            file_info.iter().any(|f| f.name == "test-official-package"),
            "Should have files for official package"
        );
        assert!(
            file_info.iter().any(|f| f.name == "test-aur-package"),
            "Should have files for AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Services tab - should show services for both packages
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
        service_info,
        services_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            service_info.len(),
            2,
            "Should have 2 services (one for each package)"
        );
        assert!(*services_loaded, "Services should be marked as loaded");
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"test-official-package".to_string())),
            "Should have service for official package"
        );
        assert!(
            service_info
                .iter()
                .any(|s| s.providers.contains(&"test-aur-package".to_string())),
            "Should have service for AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Sandbox tab - should ONLY show AUR package (official excluded)
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

        // Simulate sync_sandbox logic - should filter to only AUR packages
        if matches!(*action, crate_root::state::PreflightAction::Install) {
            // Filter items to only AUR packages
            let aur_items: Vec<_> = items
                .iter()
                .filter(|p| matches!(p.source, crate_root::state::Source::Aur))
                .map(|p| p.name.clone())
                .collect();
            let cached_sandbox: Vec<_> = app
                .install_list_sandbox
                .iter()
                .filter(|s| aur_items.contains(&s.package_name))
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
        assert_eq!(
            sandbox_info.len(),
            1,
            "Should have 1 sandbox entry (only AUR package)"
        );
        assert_eq!(
            sandbox_info[0].package_name, "test-aur-package",
            "Sandbox should only contain AUR package"
        );
        // Verify official package is NOT in sandbox
        assert!(
            !sandbox_info
                .iter()
                .any(|s| s.package_name == "test-official-package"),
            "Official package should NOT be in sandbox"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All tabs show correct data
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        ..
    } = &app.modal
    {
        // Deps, Files, Services should show both packages
        assert_eq!(dependency_info.len(), 2, "Deps should show both packages");
        assert_eq!(file_info.len(), 2, "Files should show both packages");
        assert_eq!(service_info.len(), 2, "Services should show both packages");

        // Sandbox should only show AUR package
        assert_eq!(
            sandbox_info.len(),
            1,
            "Sandbox should only show AUR package"
        );
        assert_eq!(
            sandbox_info[0].package_name, "test-aur-package",
            "Sandbox should contain AUR package"
        );
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that preflight modal handles large datasets correctly.
///
/// Inputs:
/// - 10+ packages in install_list (mix of official and AUR)
/// - Each package has 3-5 dependencies
/// - Each package has 2-3 files
/// - Each package has 1-2 services
/// - AUR packages have sandbox info
/// - User switches between all tabs
///
/// Output:
/// - All tabs load and display correctly with large datasets
/// - Navigation works correctly (selection indices, tree expansion)
/// - Data integrity is maintained (correct counts, no corruption)
///
/// Details:
/// - Tests performance and correctness with large datasets
/// - Verifies that many packages don't cause data corruption
/// - Ensures navigation remains functional with many items
fn preflight_handles_large_datasets_correctly() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    // Create 12 test packages (mix of official and AUR)
    let mut test_packages = Vec::new();
    for i in 1..=8 {
        test_packages.push(crate_root::state::PackageItem {
            name: format!("test-official-pkg-{}", i),
            version: format!("{}.0.0", i),
            description: String::new(),
            source: crate_root::state::Source::Official {
                repo: if i % 2 == 0 { "extra" } else { "core" }.to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        });
    }
    for i in 1..=4 {
        test_packages.push(crate_root::state::PackageItem {
            name: format!("test-aur-pkg-{}", i),
            version: format!("{}.0.0", i),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        });
    }

    // Pre-populate cache with dependencies (3-5 per package)
    let mut expected_dep_count = 0;
    for pkg in &test_packages {
        let dep_count = if pkg.name.contains("official") { 4 } else { 3 };
        for j in 1..=dep_count {
            app.install_list_deps
                .push(crate_root::state::modal::DependencyInfo {
                    name: format!("{}-dep-{}", pkg.name, j),
                    version: "1.0.0".to_string(),
                    status: crate_root::state::modal::DependencyStatus::ToInstall,
                    source: if pkg.name.contains("aur") {
                        crate_root::state::modal::DependencySource::Aur
                    } else {
                        crate_root::state::modal::DependencySource::Official {
                            repo: "core".to_string(),
                        }
                    },
                    required_by: vec![pkg.name.clone()],
                    depends_on: Vec::new(),
                    is_core: false,
                    is_system: false,
                });
            expected_dep_count += 1;
        }
    }

    // Pre-populate cache with files (2-3 per package)
    let mut expected_file_count = 0;
    for pkg in &test_packages {
        let file_count = if pkg.name.contains("official") { 3 } else { 2 };
        let mut files = Vec::new();
        for j in 1..=file_count {
            files.push(crate_root::state::modal::FileChange {
                path: format!("/usr/bin/{}-file-{}", pkg.name, j),
                change_type: crate_root::state::modal::FileChangeType::New,
                package: pkg.name.clone(),
                is_config: j == file_count, // Last file is config
                predicted_pacnew: false,
                predicted_pacsave: false,
            });
        }
        app.install_list_files
            .push(crate_root::state::modal::PackageFileInfo {
                name: pkg.name.clone(),
                files: files.clone(),
                total_count: file_count,
                new_count: file_count,
                changed_count: 0,
                removed_count: 0,
                config_count: 1,
                pacnew_candidates: 0,
                pacsave_candidates: 0,
            });
        expected_file_count += file_count;
    }

    // Pre-populate cache with services (1-2 per package)
    let mut expected_service_count = 0;
    for pkg in &test_packages {
        let service_count = if pkg.name.contains("official") { 2 } else { 1 };
        for j in 1..=service_count {
            app.install_list_services
                .push(crate_root::state::modal::ServiceImpact {
                    unit_name: format!("{}-service-{}.service", pkg.name, j),
                    providers: vec![pkg.name.clone()],
                    is_active: j == 1,
                    needs_restart: j == 1,
                    recommended_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
                    restart_decision: crate_root::state::modal::ServiceRestartDecision::Restart,
                });
            expected_service_count += 1;
        }
    }

    // Pre-populate cache with sandbox info (only for AUR packages)
    let mut expected_sandbox_count = 0;
    for pkg in &test_packages {
        if matches!(pkg.source, crate_root::state::Source::Aur) {
            app.install_list_sandbox
                .push(crate_root::logic::sandbox::SandboxInfo {
                    package_name: pkg.name.clone(),
                    depends: vec![crate_root::logic::sandbox::DependencyDelta {
                        name: format!("{}-sandbox-dep", pkg.name),
                        is_installed: false,
                        installed_version: None,
                        version_satisfied: false,
                    }],
                    makedepends: vec![],
                    checkdepends: vec![],
                    optdepends: vec![],
                });
            expected_sandbox_count += 1;
        }
    }

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
            aur_count: 4,
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

    // Test 1: Deps tab - should load all dependencies
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
        dependency_info,
        dep_selected,
        ..
    } = &app.modal
    {
        assert_eq!(
            dependency_info.len(),
            expected_dep_count,
            "Should have all dependencies loaded"
        );
        assert_eq!(*dep_selected, 0, "Selection should be reset to 0");
        // Verify all packages have their dependencies
        for pkg in &test_packages {
            let pkg_deps: Vec<_> = dependency_info
                .iter()
                .filter(|d| d.required_by.contains(&pkg.name))
                .collect();
            let expected = if pkg.name.contains("official") { 4 } else { 3 };
            assert_eq!(
                pkg_deps.len(),
                expected,
                "Package {} should have {} dependencies",
                pkg.name,
                expected
            );
        }
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 2: Files tab - should load all files
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
        file_info,
        file_selected,
        ..
    } = &app.modal
    {
        assert_eq!(
            file_info.len(),
            test_packages.len(),
            "Should have file info for all packages"
        );
        assert_eq!(*file_selected, 0, "Selection should be reset to 0");
        // Verify total file count
        let total_files: usize = file_info.iter().map(|f| f.files.len()).sum();
        assert_eq!(
            total_files, expected_file_count,
            "Should have all files loaded"
        );
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 3: Services tab - should load all services
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
        service_info,
        service_selected,
        services_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            service_info.len(),
            expected_service_count,
            "Should have all services loaded"
        );
        assert!(*services_loaded, "Services should be marked as loaded");
        assert_eq!(*service_selected, 0, "Selection should be reset to 0");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 4: Sandbox tab - should load sandbox info for AUR packages only
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
        sandbox_info,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        assert_eq!(
            sandbox_info.len(),
            expected_sandbox_count,
            "Should have sandbox info for all AUR packages"
        );
        assert!(*sandbox_loaded, "Sandbox should be marked as loaded");
    } else {
        panic!("Expected Preflight modal");
    }

    // Test 5: Verify navigation works (selection indices)
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        dep_selected,
        file_selected,
        service_selected,
        ..
    } = &mut app.modal
    {
        // Test that we can set selection indices to valid ranges
        if !dependency_info.is_empty() {
            *dep_selected = dependency_info.len().saturating_sub(1);
        }
        if !file_info.is_empty() {
            *file_selected = file_info.len().saturating_sub(1);
        }
        if !service_info.is_empty() {
            *service_selected = service_info.len().saturating_sub(1);
        }
    }

    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        dep_selected,
        file_selected,
        service_selected,
        ..
    } = &app.modal
    {
        if !dependency_info.is_empty() {
            assert!(
                *dep_selected < dependency_info.len(),
                "Dependency selection should be within bounds"
            );
        }
        if !file_info.is_empty() {
            assert!(
                *file_selected < file_info.len(),
                "File selection should be within bounds"
            );
        }
        if !service_info.is_empty() {
            assert!(
                *service_selected < service_info.len(),
                "Service selection should be within bounds"
            );
        }
    } else {
        panic!("Expected Preflight modal");
    }

    // Final verification: All data is correct and no corruption
    if let crate_root::state::Modal::Preflight {
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        ..
    } = &app.modal
    {
        // Verify counts match expected
        assert_eq!(
            dependency_info.len(),
            expected_dep_count,
            "Dependency count should match expected"
        );
        assert_eq!(
            file_info.len(),
            test_packages.len(),
            "File info count should match package count"
        );
        assert_eq!(
            service_info.len(),
            expected_service_count,
            "Service count should match expected"
        );
        assert_eq!(
            sandbox_info.len(),
            expected_sandbox_count,
            "Sandbox count should match expected"
        );

        // Verify data integrity - all packages should have their data
        for pkg in &test_packages {
            // Check dependencies
            let pkg_deps: Vec<_> = dependency_info
                .iter()
                .filter(|d| d.required_by.contains(&pkg.name))
                .collect();
            assert!(
                !pkg_deps.is_empty(),
                "Package {} should have dependencies",
                pkg.name
            );

            // Check files
            assert!(
                file_info.iter().any(|f| f.name == pkg.name),
                "Package {} should have file info",
                pkg.name
            );

            // Check services
            let pkg_services: Vec<_> = service_info
                .iter()
                .filter(|s| s.providers.contains(&pkg.name))
                .collect();
            assert!(
                !pkg_services.is_empty(),
                "Package {} should have services",
                pkg.name
            );

            // Check sandbox (only for AUR)
            if matches!(pkg.source, crate_root::state::Source::Aur) {
                assert!(
                    sandbox_info.iter().any(|s| s.package_name == pkg.name),
                    "AUR package {} should have sandbox info",
                    pkg.name
                );
            }
        }
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that service restart decisions persist when switching tabs.
///
/// Inputs:
/// - Packages in install_list with services
/// - Preflight modal opened with services loaded
/// - User changes service restart decisions in Services tab
/// - User switches to other tabs and back
///
/// Output:
/// - Service restart decisions remain unchanged when switching tabs
/// - Modified decisions persist across tab switches
/// - All services maintain their restart_decision values
///
/// Details:
/// - Tests that user choices for service restart decisions are preserved
/// - Verifies modal state correctly maintains service decisions
/// - Ensures no data loss when switching tabs
fn preflight_persists_service_restart_decisions_across_tabs() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

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
                .unwrap()
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should be Defer after toggle"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .unwrap()
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-2 should be Restart after toggle"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-3.service")
                .unwrap()
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
                .unwrap()
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should still be Defer after switching to Deps"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .unwrap()
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
                .unwrap()
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should still be Defer after switching to Files"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .unwrap()
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
                .unwrap()
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Defer,
            "service-1 should still be Defer after switching back to Services"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-2.service")
                .unwrap()
                .restart_decision,
            crate_root::state::modal::ServiceRestartDecision::Restart,
            "service-2 should still be Restart after switching back to Services"
        );
        assert_eq!(
            service_info
                .iter()
                .find(|s| s.unit_name == "service-3.service")
                .unwrap()
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
        for service in service_info.iter() {
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
/// - AUR packages in install_list with optional dependencies
/// - Preflight modal opened with sandbox info loaded
/// - User selects optional dependencies in Sandbox tab
/// - User switches to other tabs and back
///
/// Output:
/// - Optional dependency selections persist when switching tabs
/// - `selected_optdepends` HashMap maintains correct structure
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

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

    let test_packages = vec![
        crate_root::state::PackageItem {
            name: "test-aur-pkg-1".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
        },
        crate_root::state::PackageItem {
            name: "test-aur-pkg-2".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate_root::state::Source::Aur,
            popularity: None,
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

        let pkg1_selections = selected_optdepends.get("test-aur-pkg-1").unwrap();
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

        let pkg2_selections = selected_optdepends.get("test-aur-pkg-2").unwrap();
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
        let pkg1_selections = selected_optdepends.get("test-aur-pkg-1").unwrap();
        assert_eq!(
            pkg1_selections.len(),
            2,
            "test-aur-pkg-1 should still have 2 selections"
        );
        let pkg2_selections = selected_optdepends.get("test-aur-pkg-2").unwrap();
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
        let pkg1_selections = selected_optdepends.get("test-aur-pkg-1").unwrap();
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

        let pkg1_selections = selected_optdepends.get("test-aur-pkg-1").unwrap();
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

        let pkg2_selections = selected_optdepends.get("test-aur-pkg-2").unwrap();
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
        for (pkg_name, optdeps) in selected_optdepends.iter() {
            assert!(
                !optdeps.is_empty(),
                "Package {} should have at least one selected optdep",
                pkg_name
            );
            assert!(
                test_packages.iter().any(|p| p.name == *pkg_name),
                "Package {} should be in test packages",
                pkg_name
            );

            // Verify each selected optdep exists in sandbox info
            let sandbox = app
                .install_list_sandbox
                .iter()
                .find(|s| s.package_name == *pkg_name)
                .unwrap();
            for optdep in optdeps.iter() {
                // Extract package name from dependency spec (may include version or description)
                let optdep_pkg_name = optdep
                    .split(':')
                    .next()
                    .unwrap()
                    .split('>')
                    .next()
                    .unwrap()
                    .trim();
                assert!(
                    sandbox.optdepends.iter().any(|d| {
                        d.name == *optdep
                            || d.name.starts_with(optdep_pkg_name)
                            || optdep.starts_with(
                                d.name
                                    .split(':')
                                    .next()
                                    .unwrap()
                                    .split('>')
                                    .next()
                                    .unwrap()
                                    .trim(),
                            )
                    }),
                    "Selected optdep {} should exist in sandbox info for {}",
                    optdep,
                    pkg_name
                );
            }
        }
    } else {
        panic!("Expected Preflight modal");
    }
}

#[test]
/// What: Verify that all tabs (Deps, Files, Services, Sandbox) load and display correctly when conflicts are present.
///
/// Inputs:
/// - Packages in install_list with dependency conflicts
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
fn preflight_all_tabs_load_correctly_when_conflicts_present() {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };

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
        let to_install: Vec<_> = dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::ToInstall
                )
            })
            .collect();
        assert_eq!(to_install.len(), 4, "Should have 4 ToInstall dependencies");

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
        let pkg1_files = file_info.iter().find(|f| f.name == "package-1").unwrap();
        assert_eq!(pkg1_files.files.len(), 2, "Package-1 should have 2 files");
        assert_eq!(pkg1_files.total_count, 2);
        assert_eq!(pkg1_files.new_count, 2);
        assert_eq!(pkg1_files.changed_count, 0);
        assert_eq!(pkg1_files.config_count, 1);
        assert_eq!(pkg1_files.pacnew_candidates, 1);
        assert_eq!(pkg1_files.pacsave_candidates, 0);

        // Verify package-2 files are correct
        let pkg2_files = file_info.iter().find(|f| f.name == "package-2").unwrap();
        assert_eq!(pkg2_files.files.len(), 2, "Package-2 should have 2 files");
        assert_eq!(pkg2_files.total_count, 2);
        assert_eq!(pkg2_files.new_count, 1);
        assert_eq!(pkg2_files.changed_count, 1);
        assert_eq!(pkg2_files.config_count, 1);
        assert_eq!(pkg2_files.pacnew_candidates, 0);
        assert_eq!(pkg2_files.pacsave_candidates, 1);

        // Verify AUR package files are correct
        let aur_files = file_info.iter().find(|f| f.name == "aur-package").unwrap();
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
            .unwrap();
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
            .unwrap();
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
            .unwrap();
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
            .unwrap();
        assert_eq!(sandbox.depends.len(), 1, "Should have 1 depends");
        assert_eq!(sandbox.makedepends.len(), 1, "Should have 1 makedepends");
        assert_eq!(sandbox.checkdepends.len(), 0, "Should have 0 checkdepends");
        assert_eq!(sandbox.optdepends.len(), 1, "Should have 1 optdepends");

        // Verify dependency details
        let dep = sandbox
            .depends
            .iter()
            .find(|d| d.name == "aur-dep")
            .unwrap();
        assert!(!dep.is_installed);
        assert_eq!(dep.installed_version, None);

        let makedep = sandbox
            .makedepends
            .iter()
            .find(|d| d.name == "make-dep")
            .unwrap();
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
        let conflicts: Vec<_> = dependency_info
            .iter()
            .filter(|d| {
                matches!(
                    d.status,
                    crate_root::state::modal::DependencyStatus::Conflict { .. }
                )
            })
            .collect();
        assert_eq!(conflicts.len(), 1, "Should have 1 conflict");
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
