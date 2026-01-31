//! Tests for other modal key event handling (`GnomeTerminalPrompt`, `ImportHelp`).

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes `GnomeTerminalPrompt` modal.
///
/// Inputs:
/// - `GnomeTerminalPrompt` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes `GnomeTerminalPrompt` modal correctly
fn gnome_terminal_prompt_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::GnomeTerminalPrompt;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key in `GnomeTerminalPrompt` modal spawns terminal.
///
/// Inputs:
/// - `GnomeTerminalPrompt` modal
/// - Enter key event
///
/// Output:
/// - Modal closes and terminal spawns
///
/// Details:
/// - Ensures Enter key works correctly
/// - Cleans up terminal window opened by the test
fn gnome_terminal_prompt_enter_spawns_terminal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::GnomeTerminalPrompt;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));

    // No cleanup needed - spawn_shell_commands_in_terminal is a no-op during tests
}

#[test]
/// What: Verify numpad Enter (carriage return) closes GnomeTerminalPrompt like main Enter.
///
/// Inputs:
/// - GnomeTerminalPrompt modal
/// - KeyCode::Char('\r')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break GnomeTerminalPrompt; same outcome as main Enter
fn gnome_terminal_prompt_numpad_enter_carriage_return_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::GnomeTerminalPrompt;
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify numpad Enter (newline) closes GnomeTerminalPrompt like main Enter.
///
/// Inputs:
/// - GnomeTerminalPrompt modal
/// - KeyCode::Char('\n')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break GnomeTerminalPrompt; same outcome as main Enter
fn gnome_terminal_prompt_numpad_enter_newline_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::GnomeTerminalPrompt;
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Esc key closes `ImportHelp` modal.
///
/// Inputs:
/// - `ImportHelp` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes `ImportHelp` modal correctly
fn import_help_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ImportHelp;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes `ImportHelp` modal.
///
/// Inputs:
/// - `ImportHelp` modal
/// - Enter key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Enter also closes `ImportHelp` modal
/// - Cleans up file picker window opened by the test
fn import_help_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ImportHelp;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));

    // No cleanup needed - file picker is a no-op during tests (see events/modals/import.rs)
}

#[test]
/// What: Verify numpad Enter (carriage return) closes ImportHelp modal like main Enter.
///
/// Inputs:
/// - ImportHelp modal
/// - KeyCode::Char('\r')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break ImportHelp; same outcome as main Enter
fn import_help_numpad_enter_carriage_return_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ImportHelp;
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify numpad Enter (newline) closes ImportHelp modal like main Enter.
///
/// Inputs:
/// - ImportHelp modal
/// - KeyCode::Char('\n')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break ImportHelp; same outcome as main Enter
fn import_help_numpad_enter_newline_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ImportHelp;
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}
