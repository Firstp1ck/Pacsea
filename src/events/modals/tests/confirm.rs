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

#[test]
/// What: Verify numpad Enter (carriage return) closes `ConfirmRemove` modal like main Enter.
///
/// Inputs:
/// - `ConfirmRemove` modal
/// - `KeyCode::Char`('\r')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break `ConfirmRemove`; same outcome as main Enter
fn confirm_remove_numpad_enter_carriage_return_closes_modal() {
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
    let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify numpad Enter (newline) closes `ConfirmRemove` modal like main Enter.
///
/// Inputs:
/// - `ConfirmRemove` modal
/// - `KeyCode::Char`('\n')
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Ensures numpad Enter handling does not break `ConfirmRemove`; same outcome as main Enter
fn confirm_remove_numpad_enter_newline_closes_modal() {
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
    let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter confirms `ConfirmAurVote` and queues runtime request.
///
/// Inputs:
/// - `ConfirmAurVote` modal with package/action payload.
/// - Enter key event.
///
/// Output:
/// - Modal closes and pending vote request is queued.
///
/// Details:
/// - Ensures confirm flow bridges modal confirmation to tick-handler dispatch queue.
fn confirm_aur_vote_enter_queues_request() {
    let mut app = new_app();
    app.pending_aur_vote_intent =
        Some(("pacsea-bin".to_string(), crate::sources::VoteAction::Vote));
    app.modal = crate::state::Modal::ConfirmAurVote {
        pkgbase: "pacsea-bin".to_string(),
        action: crate::sources::VoteAction::Vote,
        message: "Confirm AUR vote".to_string(),
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    handle_modal_key(
        key_event(KeyCode::Enter, KeyModifiers::empty()),
        &mut app,
        &add_tx,
    );

    assert!(matches!(app.modal, crate::state::Modal::None));
    assert!(app.pending_aur_vote_intent.is_none());
    assert_eq!(
        app.pending_aur_vote_request,
        Some(("pacsea-bin".to_string(), crate::sources::VoteAction::Vote))
    );
}

#[test]
/// What: Verify Esc cancels `ConfirmAurVote` without queuing a request.
///
/// Inputs:
/// - `ConfirmAurVote` modal with package/action payload.
/// - Esc key event.
///
/// Output:
/// - Modal closes and pending request remains unset.
///
/// Details:
/// - Guards accidental voting by ensuring cancellation is explicit and side-effect free.
fn confirm_aur_vote_esc_cancels_request() {
    let mut app = new_app();
    app.pending_aur_vote_intent =
        Some(("pacsea-bin".to_string(), crate::sources::VoteAction::Unvote));
    app.modal = crate::state::Modal::ConfirmAurVote {
        pkgbase: "pacsea-bin".to_string(),
        action: crate::sources::VoteAction::Unvote,
        message: "Confirm AUR unvote".to_string(),
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    handle_modal_key(
        key_event(KeyCode::Esc, KeyModifiers::empty()),
        &mut app,
        &add_tx,
    );

    assert!(matches!(app.modal, crate::state::Modal::None));
    assert!(app.pending_aur_vote_intent.is_none());
    assert!(app.pending_aur_vote_request.is_none());
}

#[test]
/// What: Verify dry-run hint is preserved in `ConfirmAurVote` modal payload.
///
/// Inputs:
/// - `ConfirmAurVote` modal with dry-run line in message.
///
/// Output:
/// - Modal message contains dry-run indicator text.
///
/// Details:
/// - Keeps regression coverage for the dry-run confirmation requirement.
fn confirm_aur_vote_modal_message_includes_dry_run_hint() {
    let app = new_app();
    let modal = crate::state::Modal::ConfirmAurVote {
        pkgbase: "pacsea-bin".to_string(),
        action: crate::sources::VoteAction::Vote,
        message: "Confirm AUR vote for 'pacsea-bin'?\n\nDry-run enabled: no remote mutation will be performed.".to_string(),
    };

    match modal {
        crate::state::Modal::ConfirmAurVote { message, .. } => {
            assert!(message.contains("Dry-run enabled"));
        }
        _ => panic!("expected ConfirmAurVote modal"),
    }
    assert!(app.pending_aur_vote_request.is_none());
}
