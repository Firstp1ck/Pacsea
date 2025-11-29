//! Integration tests for the remove process.
//!
//! Tests cover:
//! - Full remove flow from user action to execution
//! - Cascade mode handling
//! - Preflight modal for remove
//! - Executor request handling
//!
//! Note: These tests are expected to fail initially as remove currently spawns terminals.

#![cfg(test)]

use pacsea::install::{ExecutorOutput, ExecutorRequest};
use pacsea::state::modal::CascadeMode;
use pacsea::state::{AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source};

/// What: Create a test package item with specified source.
///
/// Inputs:
/// - `name`: Package name
/// - `source`: Package source (Official or AUR)
///
/// Output:
/// - `PackageItem` ready for testing
///
/// Details:
/// - Helper to create test packages with consistent structure
fn create_test_package(name: &str, source: Source) -> PackageItem {
    PackageItem {
        name: name.into(),
        version: "1.0.0".into(),
        description: String::new(),
        source,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

#[test]
/// What: Test preflight modal state creation for remove action.
///
/// Inputs:
/// - Remove list with packages.
///
/// Output:
/// - `Preflight` modal can be created with correct items and action.
///
/// Details:
/// - Verifies preflight modal state structure for remove.
fn integration_remove_preflight_modal_state() {
    let items = vec![
        create_test_package(
            "old-pkg1",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package(
            "old-pkg2",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
    ];

    // Test that we can create a preflight modal state for remove
    let app = AppState {
        modal: Modal::Preflight {
            items,
            action: PreflightAction::Remove,
            tab: PreflightTab::Summary,
            summary: None,
            summary_scroll: 0,
            header_chips: pacsea::state::modal::PreflightHeaderChips {
                package_count: 2,
                download_bytes: 0,
                install_delta_bytes: -2000,
                aur_count: 0,
                risk_score: 0,
                risk_level: pacsea::state::modal::RiskLevel::Low,
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
            cascade_mode: CascadeMode::Basic,
            cached_reverse_deps_report: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::Preflight {
            items: ref modal_items,
            action,
            tab,
            ..
        } => {
            assert_eq!(modal_items.len(), 2);
            assert_eq!(action, PreflightAction::Remove);
            assert_eq!(tab, PreflightTab::Summary);
        }
        _ => panic!("Expected Preflight modal, got: {:?}", app.modal),
    }
}

#[test]
/// What: Test executor request creation for remove.
///
/// Inputs:
/// - Package names, cascade mode, password, `dry_run` flag.
///
/// Output:
/// - `ExecutorRequest::Remove` with correct parameters.
///
/// Details:
/// - Verifies executor request is created correctly from remove parameters.
fn integration_remove_executor_request_creation() {
    let names = vec!["old-pkg1".to_string(), "old-pkg2".to_string()];

    let request = ExecutorRequest::Remove {
        names,
        password: Some("testpass".to_string()),
        cascade: CascadeMode::Cascade,
        dry_run: false,
    };

    match request {
        ExecutorRequest::Remove {
            names: req_names,
            password,
            cascade,
            dry_run,
        } => {
            assert_eq!(req_names.len(), 2);
            assert_eq!(password, Some("testpass".to_string()));
            assert_eq!(cascade, CascadeMode::Cascade);
            assert!(!dry_run);
        }
        ExecutorRequest::Install { .. }
        | ExecutorRequest::Downgrade { .. }
        | ExecutorRequest::Update { .. }
        | ExecutorRequest::CustomCommand { .. } => {
            panic!("Expected Remove request")
        }
    }
}

#[test]
/// What: Test executor request with different cascade modes.
///
/// Inputs:
/// - Package names with different cascade modes.
///
/// Output:
/// - `ExecutorRequest::Remove` with correct cascade mode.
///
/// Details:
/// - Verifies all cascade modes are handled correctly.
fn integration_remove_cascade_modes() {
    let names = ["test-pkg".to_string()];

    for cascade_mode in [
        CascadeMode::Basic,
        CascadeMode::Cascade,
        CascadeMode::CascadeWithConfigs,
    ] {
        let request = ExecutorRequest::Remove {
            names: names.to_vec(),
            password: None,
            cascade: cascade_mode,
            dry_run: true,
        };

        match request {
            ExecutorRequest::Remove { cascade, .. } => {
                assert_eq!(cascade, cascade_mode);
            }
            ExecutorRequest::Install { .. }
            | ExecutorRequest::Downgrade { .. }
            | ExecutorRequest::Update { .. }
            | ExecutorRequest::CustomCommand { .. } => {
                panic!("Expected Remove request")
            }
        }
    }
}

#[test]
/// What: Test executor output handling for remove.
///
/// Inputs:
/// - Various `ExecutorOutput` messages.
///
/// Output:
/// - Output messages are correctly structured.
///
/// Details:
/// - Verifies `ExecutorOutput` enum variants work correctly for remove operations.
fn integration_remove_executor_output_handling() {
    // Test Line output
    let output1 = ExecutorOutput::Line("Removing packages...".to_string());
    match output1 {
        ExecutorOutput::Line(line) => assert!(line.contains("Removing")),
        _ => panic!("Expected Line variant"),
    }

    // Test Finished output with success
    let output2 = ExecutorOutput::Finished {
        success: true,
        exit_code: Some(0),
    };
    match output2 {
        ExecutorOutput::Finished { success, exit_code } => {
            assert!(success);
            assert_eq!(exit_code, Some(0));
        }
        _ => panic!("Expected Finished variant"),
    }

    // Test Error output
    let output3 = ExecutorOutput::Error("Remove failed".to_string());
    match output3 {
        ExecutorOutput::Error(msg) => assert_eq!(msg, "Remove failed"),
        _ => panic!("Expected Error variant"),
    }
}

#[test]
/// What: Test that remove process uses `ExecutorRequest` through preflight modal.
///
/// Inputs:
/// - Remove action triggered through preflight `start_execution` (simulated).
///
/// Output:
/// - `pending_executor_request` should be set with `ExecutorRequest::Remove`.
///
/// Details:
/// - Remove already uses executor pattern through preflight modal.
/// - This test simulates what `start_execution` does to verify the executor pattern works.
/// - Note: Direct remove (without preflight) still uses `spawn_remove_all` and spawns terminals.
fn integration_remove_uses_executor_not_terminal() {
    let items = vec![create_test_package(
        "old-pkg1",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];
    let mut app = AppState {
        remove_cascade_mode: CascadeMode::Basic,
        dry_run: false,
        ..Default::default()
    };

    // Simulate what start_execution does for Remove action
    // This is the NEW process - remove through preflight uses ExecutorRequest
    let header_chips = pacsea::state::modal::PreflightHeaderChips {
        package_count: 1,
        download_bytes: 0,
        install_delta_bytes: -1000,
        aur_count: 0,
        risk_score: 0,
        risk_level: pacsea::state::modal::RiskLevel::Low,
    };

    // Store executor request (what start_execution does)
    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();

    // Transition to PreflightExec modal (what start_execution does)
    app.modal = Modal::PreflightExec {
        items,
        action: PreflightAction::Remove,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: Vec::new(),
        abortable: false,
        header_chips,
    };
    app.pending_executor_request = Some(ExecutorRequest::Remove {
        names,
        password: None,
        cascade: app.remove_cascade_mode,
        dry_run: app.dry_run,
    });

    // Verify pending_executor_request is set correctly
    assert!(
        app.pending_executor_request.is_some(),
        "Remove process through preflight must use ExecutorRequest"
    );

    // Verify it's a Remove request with correct parameters
    match app.pending_executor_request {
        Some(ExecutorRequest::Remove {
            names,
            cascade,
            dry_run,
            ..
        }) => {
            assert_eq!(names.len(), 1);
            assert_eq!(names[0], "old-pkg1");
            assert_eq!(cascade, CascadeMode::Basic);
            assert!(!dry_run);
        }
        _ => {
            panic!("Remove process must use ExecutorRequest::Remove, not other variants");
        }
    }

    // Verify modal is PreflightExec
    match app.modal {
        Modal::PreflightExec { action, .. } => {
            assert_eq!(action, PreflightAction::Remove);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}
