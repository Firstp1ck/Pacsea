//! UI tests for optional dependencies modal.
//!
//! Tests cover:
//! - `OptionalDeps` modal structure
//! - Optional dependency row structure
//!
//! Note: These tests verify modal state structure rather than actual rendering.

#![cfg(test)]

use pacsea::state::{AppState, Modal, types::OptionalDepRow};

/// What: Create a test optional dependency row.
///
/// Inputs:
/// - `package`: Package name
/// - `installed`: Whether package is installed
/// - `selectable`: Whether row is selectable
///
/// Output:
/// - `OptionalDepRow` ready for testing
///
/// Details:
/// - Helper to create test optional dependency rows
fn create_test_row(package: &str, installed: bool, selectable: bool) -> OptionalDepRow {
    OptionalDepRow {
        label: format!("Test: {package}"),
        package: package.into(),
        installed,
        selectable,
        note: None,
    }
}

#[test]
/// What: Test `OptionalDeps` modal structure.
///
/// Inputs:
/// - `OptionalDeps` modal with dependency rows.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies `OptionalDeps` modal can be created.
fn ui_optional_deps_modal_structure() {
    let rows = vec![
        create_test_row("paru", false, true),
        create_test_row("yay", false, true),
        create_test_row("nvim", true, false),
        create_test_row("virustotal-setup", false, true),
    ];

    let app = AppState {
        modal: Modal::OptionalDeps { rows, selected: 1 },
        ..Default::default()
    };

    match app.modal {
        Modal::OptionalDeps {
            rows: ref modal_rows,
            selected,
        } => {
            assert_eq!(modal_rows.len(), 4);
            assert_eq!(selected, 1);
            assert_eq!(modal_rows[0].package, "paru");
            assert!(!modal_rows[0].installed);
            assert!(modal_rows[0].selectable);
            assert_eq!(modal_rows[2].package, "nvim");
            assert!(modal_rows[2].installed);
            assert!(!modal_rows[2].selectable);
        }
        _ => panic!("Expected OptionalDeps modal"),
    }
}
