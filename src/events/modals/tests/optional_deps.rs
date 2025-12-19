//! Tests for `OptionalDeps` modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes `OptionalDeps` modal and doesn't restore it.
///
/// Inputs:
/// - `OptionalDeps` modal with test rows
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn optional_deps_esc_closes_modal() {
    let mut app = new_app();
    let rows = vec![crate::state::types::OptionalDepRow {
        label: "Test".to_string(),
        package: "test-pkg".to_string(),
        installed: false,
        selectable: true,
        note: None,
    }];
    app.modal = crate::state::Modal::OptionalDeps { rows, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify navigation keys in `OptionalDeps` modal don't close it.
///
/// Inputs:
/// - `OptionalDeps` modal with multiple rows
/// - Up/Down key events
///
/// Output:
/// - Modal remains open and selection changes
///
/// Details:
/// - Ensures other keys still work correctly after the Esc fix
fn optional_deps_navigation_preserves_modal() {
    let mut app = new_app();
    let rows = vec![
        crate::state::types::OptionalDepRow {
            label: "Test 1".to_string(),
            package: "test-pkg-1".to_string(),
            installed: false,
            selectable: true,
            note: None,
        },
        crate::state::types::OptionalDepRow {
            label: "Test 2".to_string(),
            package: "test-pkg-2".to_string(),
            installed: false,
            selectable: true,
            note: None,
        },
    ];
    app.modal = crate::state::Modal::OptionalDeps { rows, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    // Press Down - should move selection and keep modal open
    let ke_down = key_event(KeyCode::Down, KeyModifiers::empty());
    handle_modal_key(ke_down, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::OptionalDeps { selected, .. } => {
            assert_eq!(*selected, 1);
        }
        _ => panic!("Modal should remain OptionalDeps after Down key"),
    }

    // Press Up - should move selection back and keep modal open
    let ke_up = key_event(KeyCode::Up, KeyModifiers::empty());
    handle_modal_key(ke_up, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::OptionalDeps { selected, .. } => {
            assert_eq!(*selected, 0);
        }
        _ => panic!("Modal should remain OptionalDeps after Up key"),
    }
}

#[test]
/// What: Verify unhandled keys in `OptionalDeps` modal don't break state.
///
/// Inputs:
/// - `OptionalDeps` modal
/// - Unhandled key event (e.g., 'x')
///
/// Output:
/// - Modal remains open with unchanged state
///
/// Details:
/// - Ensures unhandled keys don't cause issues
fn optional_deps_unhandled_key_preserves_modal() {
    let mut app = new_app();
    let rows = vec![crate::state::types::OptionalDepRow {
        label: "Test".to_string(),
        package: "test-pkg".to_string(),
        installed: false,
        selectable: true,
        note: None,
    }];
    app.modal = crate::state::Modal::OptionalDeps { rows, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('x'), KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    // Modal should remain open since 'x' is not handled
    match &app.modal {
        crate::state::Modal::OptionalDeps { selected, .. } => {
            assert_eq!(*selected, 0);
        }
        _ => panic!("Modal should remain OptionalDeps for unhandled key"),
    }
}
