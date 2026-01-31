//! Tests for `ScanConfig` and `VirusTotalSetup` modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

#[test]
/// What: Verify Esc key closes `ScanConfig` modal and doesn't restore it.
///
/// Inputs:
/// - `ScanConfig` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn scan_config_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::ScanConfig {
        do_clamav: false,
        do_trivy: false,
        do_semgrep: false,
        do_shellcheck: false,
        do_virustotal: false,
        do_custom: false,
        do_sleuth: false,
        cursor: 0,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Esc key closes `VirusTotalSetup` modal and doesn't restore it.
///
/// Inputs:
/// - `VirusTotalSetup` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn virustotal_setup_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::VirusTotalSetup {
        input: String::new(),
        cursor: 0,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key in `VirusTotalSetup` modal with empty input opens browser.
///
/// Inputs:
/// - `VirusTotalSetup` modal with empty input
/// - Enter key event
///
/// Output:
/// - Modal remains open and browser opens
///
/// Details:
/// - Ensures Enter key works correctly when input is empty
/// - Cleans up browser tab opened by the test
fn virustotal_setup_enter_opens_browser() {
    let mut app = new_app();
    app.modal = crate::state::Modal::VirusTotalSetup {
        input: String::new(),
        cursor: 0,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    // Modal should remain open since input is empty
    match &app.modal {
        crate::state::Modal::VirusTotalSetup { .. } => {}
        _ => panic!("Modal should remain `VirusTotalSetup` after Enter with empty input"),
    }

    // No cleanup needed - open_url is a no-op during tests
}

#[test]
/// What: Verify numpad Enter (carriage return) in `VirusTotalSetup` with empty input keeps modal open like main Enter.
///
/// Inputs:
/// - `VirusTotalSetup` modal with empty input
/// - `KeyCode::Char`('\r')
///
/// Output:
/// - Modal remains `VirusTotalSetup`
///
/// Details:
/// - Ensures numpad Enter handling does not break `VirusTotalSetup`; same outcome as main Enter
fn virus_total_setup_numpad_enter_carriage_return_empty_input_keeps_modal_open() {
    let mut app = new_app();
    app.modal = crate::state::Modal::VirusTotalSetup {
        input: String::new(),
        cursor: 0,
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    match &app.modal {
        crate::state::Modal::VirusTotalSetup { .. } => {}
        _ => panic!("Modal should remain `VirusTotalSetup` after numpad Enter with empty input"),
    }
}

#[test]
/// What: Verify numpad Enter (newline) in `VirusTotalSetup` with empty input keeps modal open like main Enter.
///
/// Inputs:
/// - `VirusTotalSetup` modal with empty input
/// - `KeyCode::Char`('\n')
///
/// Output:
/// - Modal remains `VirusTotalSetup`
///
/// Details:
/// - Ensures numpad Enter handling does not break `VirusTotalSetup`; same outcome as main Enter
fn virus_total_setup_numpad_enter_newline_empty_input_keeps_modal_open() {
    let mut app = new_app();
    app.modal = crate::state::Modal::VirusTotalSetup {
        input: String::new(),
        cursor: 0,
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    match &app.modal {
        crate::state::Modal::VirusTotalSetup { .. } => {}
        _ => panic!("Modal should remain `VirusTotalSetup` after numpad Enter with empty input"),
    }
}
