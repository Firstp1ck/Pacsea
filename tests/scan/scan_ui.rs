//! UI tests for security scan modals.
//!
//! Tests cover:
//! - `ScanConfig` modal structure
//! - `VirusTotalSetup` modal structure
//! - `PreflightExec` modal for integrated scan execution
//! - Modal state transitions
//!
//! Note: These tests verify modal state structure rather than actual rendering.

#![cfg(test)]

use pacsea::state::{AppState, Modal};

#[test]
/// What: Test `ScanConfig` modal structure.
///
/// Inputs:
/// - `ScanConfig` modal with scanner options.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies `ScanConfig` modal can be created.
fn ui_scan_config_modal_structure() {
    let app = AppState {
        modal: Modal::ScanConfig {
            do_clamav: true,
            do_trivy: true,
            do_semgrep: false,
            do_shellcheck: true,
            do_virustotal: false,
            do_custom: false,
            do_sleuth: false,
            cursor: 3,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ScanConfig {
            do_clamav,
            do_trivy,
            do_semgrep,
            do_shellcheck,
            do_virustotal,
            do_custom,
            do_sleuth,
            cursor,
        } => {
            assert!(do_clamav);
            assert!(do_trivy);
            assert!(!do_semgrep);
            assert!(do_shellcheck);
            assert!(!do_virustotal);
            assert!(!do_custom);
            assert!(!do_sleuth);
            assert_eq!(cursor, 3);
        }
        _ => panic!("Expected ScanConfig modal"),
    }
}

#[test]
/// What: Test `VirusTotalSetup` modal structure.
///
/// Inputs:
/// - `VirusTotalSetup` modal with API key input.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies `VirusTotalSetup` modal can be created.
fn ui_virustotal_setup_modal_structure() {
    let app = AppState {
        modal: Modal::VirusTotalSetup {
            input: "test-api-key-12345".to_string(),
            cursor: 18,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::VirusTotalSetup { input, cursor } => {
            assert_eq!(input, "test-api-key-12345");
            assert_eq!(cursor, 18);
        }
        _ => panic!("Expected VirusTotalSetup modal"),
    }
}

#[test]
/// What: Test ``PreflightExec`` modal structure for integrated scan execution.
///
/// Inputs:
/// - ``PreflightExec`` modal after scan configuration confirmation.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies ``PreflightExec`` modal can be created for scan execution.
fn ui_scan_preflight_exec_structure() {
    use pacsea::install::ExecutorRequest;
    use pacsea::state::{PackageItem, PreflightAction, PreflightTab, Source};

    let item = PackageItem {
        name: "test-pkg".to_string(),
        version: String::new(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    };

    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![item],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: pacsea::state::modal::PreflightHeaderChips::default(),
            success: None,
        },
        pending_executor_request: Some(ExecutorRequest::Scan {
            package: "test-pkg".to_string(),
            do_clamav: true,
            do_trivy: true,
            do_semgrep: false,
            do_shellcheck: false,
            do_virustotal: false,
            do_custom: false,
            dry_run: false,
        }),
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec {
            items,
            action,
            tab,
            verbose,
            log_lines,
            abortable,
            ..
        } => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].name, "test-pkg");
            assert_eq!(action, PreflightAction::Install);
            assert_eq!(tab, PreflightTab::Summary);
            assert!(!verbose);
            assert!(log_lines.is_empty());
            assert!(!abortable);
        }
        _ => panic!("Expected PreflightExec modal"),
    }

    match app.pending_executor_request {
        Some(ExecutorRequest::Scan {
            package,
            do_clamav,
            do_trivy,
            do_semgrep,
            do_shellcheck,
            do_virustotal,
            do_custom,
            ..
        }) => {
            assert_eq!(package, "test-pkg");
            assert!(do_clamav);
            assert!(do_trivy);
            assert!(!do_semgrep);
            assert!(!do_shellcheck);
            assert!(!do_virustotal);
            assert!(!do_custom);
        }
        _ => panic!("Expected Scan executor request"),
    }
}

#[test]
/// What: Test modal transition from ``ScanConfig`` to ``PreflightExec`` for integrated scan.
///
/// Inputs:
/// - ``ScanConfig`` modal with non-sleuth scanners enabled.
///
/// Output:
/// - Modal transitions to ``PreflightExec``.
/// - Executor request is created with ``ExecutorRequest::Scan``.
///
/// Details:
/// - Verifies modal state transition flow for integrated scan execution.
fn ui_scan_modal_transition() {
    use pacsea::install::ExecutorRequest;
    use pacsea::state::{PackageItem, PreflightAction, PreflightTab, Source};

    let item = PackageItem {
        name: "test-pkg".to_string(),
        version: String::new(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    };

    let mut app = AppState {
        modal: Modal::ScanConfig {
            do_clamav: true,
            do_trivy: true,
            do_semgrep: false,
            do_shellcheck: false,
            do_virustotal: false,
            do_custom: false,
            do_sleuth: false,
            cursor: 0,
        },
        pending_install_names: Some(vec!["test-pkg".to_string()]),
        dry_run: false,
        ..Default::default()
    };

    // Simulate scan configuration confirmation
    let package = "test-pkg";
    let do_clamav = true;
    let do_trivy = true;
    let do_semgrep = false;
    let do_shellcheck = false;
    let do_virustotal = false;
    let do_custom = false;
    #[allow(clippy::no_effect_underscore_binding)]
    let _do_sleuth = false;

    // Transition to PreflightExec
    app.modal = Modal::PreflightExec {
        items: vec![item],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: vec![],
        abortable: false,
        success: None,
        header_chips: pacsea::state::modal::PreflightHeaderChips::default(),
    };

    app.pending_executor_request = Some(ExecutorRequest::Scan {
        package: package.to_string(),
        do_clamav,
        do_trivy,
        do_semgrep,
        do_shellcheck,
        do_virustotal,
        do_custom,
        dry_run: app.dry_run,
    });

    // Verify transition to PreflightExec
    assert!(matches!(app.modal, Modal::PreflightExec { .. }));
    assert!(app.pending_executor_request.is_some());
    match app.pending_executor_request {
        Some(ExecutorRequest::Scan { .. }) => {}
        _ => panic!("Expected Scan executor request"),
    }
}
