//! UI tests for downgrade process.
//!
//! Tests cover:
//! - Downgrade list state structure
//! - Downgrade pane focus state
//!
//! Note: These tests verify state structure rather than actual rendering.

#![cfg(test)]

use pacsea::state::{AppState, PackageItem, RightPaneFocus, Source};

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
/// What: Test downgrade list state structure.
///
/// Inputs:
/// - `AppState` with downgrade list and focus.
///
/// Output:
/// - State is correctly structured.
///
/// Details:
/// - Verifies downgrade list and focus state.
fn ui_downgrade_list_state() {
    let mut app = AppState {
        installed_only_mode: true,
        right_pane_focus: RightPaneFocus::Downgrade,
        ..Default::default()
    };

    let pkg1 = create_test_package(
        "pkg1",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );
    let pkg2 = create_test_package(
        "pkg2",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    app.downgrade_list.push(pkg1);
    app.downgrade_list.push(pkg2);
    app.downgrade_state.select(Some(0));

    assert!(app.installed_only_mode);
    assert_eq!(app.right_pane_focus, RightPaneFocus::Downgrade);
    assert_eq!(app.downgrade_list.len(), 2);
    assert_eq!(app.downgrade_state.selected(), Some(0));
}
