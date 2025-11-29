//! UI tests for security scan modals.
//!
//! Tests cover:
//! - ScanConfig modal structure
//! - VirusTotalSetup modal structure
//!
//! Note: These tests verify modal state structure rather than actual rendering.

#![cfg(test)]

use pacsea::state::{AppState, Modal};

#[test]
/// What: Test ScanConfig modal structure.
///
/// Inputs:
/// - ScanConfig modal with scanner options.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies ScanConfig modal can be created.
fn ui_scan_config_modal_structure() {
    let mut app = AppState::default();
    app.modal = Modal::ScanConfig {
        do_clamav: true,
        do_trivy: true,
        do_semgrep: false,
        do_shellcheck: true,
        do_virustotal: false,
        do_custom: false,
        do_sleuth: false,
        cursor: 3,
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
/// What: Test VirusTotalSetup modal structure.
///
/// Inputs:
/// - VirusTotalSetup modal with API key input.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies VirusTotalSetup modal can be created.
fn ui_virustotal_setup_modal_structure() {
    let mut app = AppState::default();
    app.modal = Modal::VirusTotalSetup {
        input: "test-api-key-12345".to_string(),
        cursor: 18,
    };

    match app.modal {
        Modal::VirusTotalSetup { input, cursor } => {
            assert_eq!(input, "test-api-key-12345");
            assert_eq!(cursor, 18);
        }
        _ => panic!("Expected VirusTotalSetup modal"),
    }
}

