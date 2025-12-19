//! Tests for `SystemUpdate` modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes `SystemUpdate` modal and doesn't restore it.
///
/// Inputs:
/// - `SystemUpdate` modal with default settings
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn system_update_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::SystemUpdate {
        do_mirrors: false,
        do_pacman: false,
        force_sync: false,
        do_aur: false,
        do_cache: false,
        country_idx: 0,
        countries: vec!["US".to_string(), "DE".to_string()],
        mirror_count: 10,
        cursor: 0,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify navigation keys in `SystemUpdate` modal don't close it.
///
/// Inputs:
/// - `SystemUpdate` modal
/// - Up/Down key events
///
/// Output:
/// - Modal remains open and cursor position changes
///
/// Details:
/// - Ensures other keys still work correctly after the Esc fix
fn system_update_navigation_preserves_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::SystemUpdate {
        do_mirrors: false,
        do_pacman: false,
        force_sync: false,
        do_aur: false,
        do_cache: false,
        country_idx: 0,
        countries: vec!["US".to_string(), "DE".to_string()],
        mirror_count: 10,
        cursor: 0,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    // Press Down - should move cursor and keep modal open
    let ke_down = key_event(KeyCode::Down, KeyModifiers::empty());
    handle_modal_key(ke_down, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::SystemUpdate { cursor, .. } => {
            assert_eq!(*cursor, 1);
        }
        _ => panic!("Modal should remain SystemUpdate after Down key"),
    }

    // Press Up - should move cursor back and keep modal open
    let ke_up = key_event(KeyCode::Up, KeyModifiers::empty());
    handle_modal_key(ke_up, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::SystemUpdate { cursor, .. } => {
            assert_eq!(*cursor, 0);
        }
        _ => panic!("Modal should remain SystemUpdate after Up key"),
    }
}

#[test]
/// What: Verify unhandled keys in `SystemUpdate` modal don't break state.
///
/// Inputs:
/// - `SystemUpdate` modal
/// - Unhandled key event (e.g., 'z')
///
/// Output:
/// - Modal remains open with unchanged state
///
/// Details:
/// - Ensures unhandled keys don't cause issues
fn system_update_unhandled_key_preserves_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::SystemUpdate {
        do_mirrors: false,
        do_pacman: false,
        force_sync: false,
        do_aur: false,
        do_cache: false,
        country_idx: 0,
        countries: vec!["US".to_string(), "DE".to_string()],
        mirror_count: 10,
        cursor: 0,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('z'), KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    // Modal should remain open since 'z' is not handled
    match &app.modal {
        crate::state::Modal::SystemUpdate { cursor, .. } => {
            assert_eq!(*cursor, 0);
        }
        _ => panic!("Modal should remain SystemUpdate for unhandled key"),
    }
}

#[test]
/// What: Verify toggle keys in `SystemUpdate` modal work correctly.
///
/// Inputs:
/// - `SystemUpdate` modal
/// - Space key event to toggle options
///
/// Output:
/// - Modal remains open and flags are toggled
///
/// Details:
/// - Ensures toggle functionality still works after the Esc fix
fn system_update_toggle_works() {
    let mut app = new_app();
    app.modal = crate::state::Modal::SystemUpdate {
        do_mirrors: false,
        do_pacman: false,
        force_sync: false,
        do_aur: false,
        do_cache: false,
        country_idx: 0,
        countries: vec!["US".to_string(), "DE".to_string()],
        mirror_count: 10,
        cursor: 0,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    // Press Space to toggle the first option (do_mirrors)
    let ke_space = key_event(KeyCode::Char(' '), KeyModifiers::empty());
    handle_modal_key(ke_space, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::SystemUpdate {
            do_mirrors, cursor, ..
        } => {
            assert!(*do_mirrors);
            assert_eq!(*cursor, 0);
        }
        _ => panic!("Modal should remain SystemUpdate after Space key"),
    }
}
