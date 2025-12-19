//! Tests for `ConfirmInstall` and `ConfirmRemove` modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{PackageItem, Source};

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes `ConfirmInstall` modal.
///
/// Inputs:
/// - `ConfirmInstall` modal with items
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes `ConfirmInstall` modal correctly
fn confirm_install_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ConfirmInstall {
        items: vec![PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }],
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Esc key closes `ConfirmRemove` modal.
///
/// Inputs:
/// - `ConfirmRemove` modal with items
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes `ConfirmRemove` modal correctly
fn confirm_remove_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ConfirmRemove {
        items: vec![PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }],
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes `ConfirmRemove` modal.
///
/// Inputs:
/// - `ConfirmRemove` modal with items
/// - Enter key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Enter also closes `ConfirmRemove` modal
/// - Cleans up terminal window opened by the test
fn confirm_remove_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ConfirmRemove {
        items: vec![PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }],
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));

    // No cleanup needed - spawn_shell_commands_in_terminal is a no-op during tests
}
