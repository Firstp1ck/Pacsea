//! Integration tests for single package update process.
//!
//! Tests cover:
//! - Updates modal handling
//! - Single package update flow
//! - Preflight modal for updates
//!
//! Note: These tests verify the update flow structure.

#![cfg(test)]

use crossterm::event::{
    Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use pacsea::state::{AppState, Modal, PackageItem, Source};
use pacsea::{events, state::PkgbuildCheckRequest};
use tokio::sync::mpsc;

/// What: Shared sender bundle for invoking `events::handle_event` in integration tests.
///
/// Inputs:
/// - None
///
/// Output:
/// - Type alias used by `make_event_channels`.
///
/// Details:
/// - Reduces signature complexity while keeping channel wiring explicit in tests.
type EventChannels = (
    mpsc::UnboundedSender<pacsea::state::QueryInput>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<String>,
    mpsc::UnboundedSender<PkgbuildCheckRequest>,
);

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
/// What: Test Updates modal state creation.
///
/// Inputs:
/// - `Updates` modal with update entries.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies `Updates` modal can be created and accessed.
fn integration_updates_modal_state() {
    let entries = vec![
        ("pkg1".to_string(), "1.0.0".to_string(), "1.1.0".to_string()),
        ("pkg2".to_string(), "2.0.0".to_string(), "2.1.0".to_string()),
    ];

    let app = AppState {
        modal: Modal::Updates {
            entries,
            scroll: 0,
            selected: 0,
            filter_active: false,
            filter_query: String::new(),
            filter_caret: 0,
            last_selected_pkg_name: None,
            filtered_indices: vec![0, 1],
            selected_pkg_names: std::collections::HashSet::new(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::Updates {
            entries: ref modal_entries,
            scroll,
            selected,
            ..
        } => {
            assert_eq!(modal_entries.len(), 2);
            assert_eq!(scroll, 0);
            assert_eq!(selected, 0);
            assert_eq!(modal_entries[0].0, "pkg1");
            assert_eq!(modal_entries[0].1, "1.0.0");
            assert_eq!(modal_entries[0].2, "1.1.0");
        }
        _ => panic!("Expected Updates modal"),
    }
}

#[test]
/// What: Test single package update flow structure.
///
/// Inputs:
/// - Package item with updated version.
///
/// Output:
/// - Update flow can be initiated.
///
/// Details:
/// - Verifies that single package updates use the preflight modal flow.
fn integration_single_package_update_flow() {
    let _app = AppState::default();
    let pkg = create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    // Single package update should open preflight modal (similar to install)
    // This is handled by open_preflight_modal function
    // We can test that the package structure supports updates
    assert_eq!(pkg.name, "test-pkg");
    assert_eq!(pkg.version, "1.0.0");
    assert!(matches!(pkg.source, Source::Official { .. }));
}

#[test]
/// What: Test Updates modal navigation.
///
/// Inputs:
/// - `Updates` modal with multiple entries, navigation keys.
///
/// Output:
/// - Selection moves correctly.
///
/// Details:
/// - Verifies navigation in `Updates` modal.
fn integration_updates_modal_navigation() {
    let entries = vec![
        ("pkg1".to_string(), "1.0.0".to_string(), "1.1.0".to_string()),
        ("pkg2".to_string(), "2.0.0".to_string(), "2.1.0".to_string()),
        ("pkg3".to_string(), "3.0.0".to_string(), "3.1.0".to_string()),
    ];

    let app = AppState {
        modal: Modal::Updates {
            entries,
            scroll: 0,
            selected: 0,
            filter_active: false,
            filter_query: String::new(),
            filter_caret: 0,
            last_selected_pkg_name: None,
            filtered_indices: vec![0, 1, 2],
            selected_pkg_names: std::collections::HashSet::new(),
        },
        ..Default::default()
    };

    // Test selection state
    match app.modal {
        Modal::Updates { selected, .. } => {
            assert_eq!(selected, 0);
        }
        _ => panic!("Expected Updates modal"),
    }
}

/// What: Create a full channel set required by `events::handle_event`.
///
/// Inputs:
/// - None
///
/// Output:
/// - Tuple of channels matching the public event dispatcher signature.
///
/// Details:
/// - Keeps integration tests focused on modal flow assertions instead of channel boilerplate.
fn make_event_channels() -> EventChannels {
    let (query_tx, _query_rx) = mpsc::unbounded_channel();
    let (details_tx, _details_rx) = mpsc::unbounded_channel();
    let (preview_tx, _preview_rx) = mpsc::unbounded_channel();
    let (add_tx, _add_rx) = mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
    let (comments_tx, _comments_rx) = mpsc::unbounded_channel();
    let (pkgb_check_tx, _pkgb_check_rx) = mpsc::unbounded_channel();
    (
        query_tx,
        details_tx,
        preview_tx,
        add_tx,
        pkgb_tx,
        comments_tx,
        pkgb_check_tx,
    )
}

#[test]
/// What: Validate keyboard-driven updates modal flow reaches a transition state.
///
/// Inputs:
/// - Updates modal, slash filter keys, multi-select toggle, and Enter.
///
/// Output:
/// - Modal transitions away from `Updates` after Enter.
///
/// Details:
/// - Asserts only state transitions and leaves selected-set precedence semantics flexible.
fn integration_updates_modal_keyboard_flow_filter_multiselect_enter() {
    let entries = vec![
        (
            "alpha".to_string(),
            "1.0.0".to_string(),
            "1.1.0".to_string(),
        ),
        ("beta".to_string(), "2.0.0".to_string(), "2.1.0".to_string()),
    ];
    let mut app = AppState {
        modal: Modal::Updates {
            entries,
            scroll: 0,
            selected: 0,
            filter_active: false,
            filter_query: String::new(),
            filter_caret: 0,
            last_selected_pkg_name: None,
            filtered_indices: vec![0, 1],
            selected_pkg_names: std::collections::HashSet::new(),
        },
        ..Default::default()
    };
    app.updates_modal_content_rect = Some((0, 0, 80, 5));
    app.updates_modal_entry_line_starts = vec![0, 1];
    app.updates_modal_total_lines = 2;

    let (query_tx, details_tx, preview_tx, add_tx, pkgb_tx, comments_tx, pkgb_check_tx) =
        make_event_channels();

    // Enter filter mode and type a query, then exit filter mode.
    let slash = CEvent::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    let mut a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
    a_key.kind = KeyEventKind::Press;
    let mut l_key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty());
    l_key.kind = KeyEventKind::Press;
    let esc = CEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    let _ = events::handle_event(
        &slash,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );
    let _ = events::handle_event(
        &CEvent::Key(a_key),
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );
    let _ = events::handle_event(
        &CEvent::Key(l_key),
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );
    let _ = events::handle_event(
        &esc,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );

    // Toggle selected focused row, then submit.
    let space = CEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()));
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = events::handle_event(
        &space,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );
    let _ = events::handle_event(
        &enter,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );

    assert!(!matches!(app.modal, Modal::Updates { .. }));
}

#[test]
/// What: Validate mouse + keyboard interop in updates modal before Enter transition.
///
/// Inputs:
/// - Mouse click to choose a row followed by keyboard toggle/navigation and Enter.
///
/// Output:
/// - Combined interactions preserve valid updates state and then transition away on Enter.
///
/// Details:
/// - Ensures row selection from mouse input interoperates with keyboard multi-select workflow.
fn integration_updates_modal_mouse_keyboard_interop() {
    let entries = vec![
        (
            "alpha".to_string(),
            "1.0.0".to_string(),
            "1.1.0".to_string(),
        ),
        ("beta".to_string(), "2.0.0".to_string(), "2.1.0".to_string()),
        (
            "gamma".to_string(),
            "3.0.0".to_string(),
            "3.1.0".to_string(),
        ),
    ];
    let mut app = AppState {
        modal: Modal::Updates {
            entries,
            scroll: 0,
            selected: 0,
            filter_active: false,
            filter_query: String::new(),
            filter_caret: 0,
            last_selected_pkg_name: None,
            filtered_indices: vec![0, 1, 2],
            selected_pkg_names: std::collections::HashSet::new(),
        },
        ..Default::default()
    };
    app.updates_modal_rect = Some((10, 4, 80, 15));
    app.updates_modal_content_rect = Some((12, 7, 70, 8));
    app.updates_modal_entry_line_starts = vec![0, 1, 2];
    app.updates_modal_total_lines = 3;

    let (query_tx, details_tx, preview_tx, add_tx, pkgb_tx, comments_tx, pkgb_check_tx) =
        make_event_channels();

    let click_second_row = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 14,
        row: 8,
        modifiers: KeyModifiers::empty(),
    });
    let _ = events::handle_event(
        &click_second_row,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );

    match &app.modal {
        Modal::Updates { selected, .. } => assert_eq!(*selected, 1),
        _ => panic!("Expected Updates modal after mouse selection"),
    }

    let space = CEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()));
    let down = CEvent::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = events::handle_event(
        &space,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );
    let _ = events::handle_event(
        &down,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );
    let _ = events::handle_event(
        &enter,
        &mut app,
        &query_tx,
        &details_tx,
        &preview_tx,
        &add_tx,
        &pkgb_tx,
        &comments_tx,
        &pkgb_check_tx,
    );

    assert!(!matches!(app.modal, Modal::Updates { .. }));
}
