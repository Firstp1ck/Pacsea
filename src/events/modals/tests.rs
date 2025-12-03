//! Tests for modal key event handling, particularly Esc key bug fixes.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{
    AppState, PackageItem, PreflightAction, PreflightTab, modal::PreflightHeaderChips,
};

use super::handle_modal_key;

/// What: Create a baseline `AppState` for modal tests.
///
/// Inputs:
/// - None
///
/// Output:
/// - Fresh `AppState` ready for modal testing
///
/// Details:
/// - Provides a clean starting state for each test case
fn new_app() -> AppState {
    AppState::default()
}

/// What: Create a key event with Press kind.
///
/// Inputs:
/// - `code`: Key code
/// - `modifiers`: Key modifiers
///
/// Output:
/// - `KeyEvent` with Press kind
fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    let mut ke = KeyEvent::new(code, modifiers);
    ke.kind = crossterm::event::KeyEventKind::Press;
    ke
}

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
/// What: Verify Esc key closes `PreflightExec` modal and doesn't restore it.
///
/// Inputs:
/// - `PreflightExec` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn preflight_exec_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PreflightExec {
        verbose: false,
        log_lines: vec![],
        abortable: true,
        items: vec![],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        success: None,
        header_chips: PreflightHeaderChips::default(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify 'q' key closes `PreflightExec` modal and doesn't restore it.
///
/// Inputs:
/// - `PreflightExec` modal
/// - 'q' key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests that 'q' also works to close the modal
fn preflight_exec_q_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PreflightExec {
        verbose: false,
        log_lines: vec![],
        abortable: true,
        items: vec![],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        success: None,
        header_chips: PreflightHeaderChips::default(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('q'), KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Esc key closes `PostSummary` modal and doesn't restore it.
///
/// Inputs:
/// - `PostSummary` modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc was being immediately restored
fn post_summary_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PostSummary {
        success: true,
        changed_files: 0,
        pacnew_count: 0,
        pacsave_count: 0,
        services_pending: vec![],
        snapshot_label: None,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes `PostSummary` modal and doesn't restore it.
///
/// Inputs:
/// - `PostSummary` modal
/// - Enter key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests that Enter also works to close the modal
fn post_summary_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::PostSummary {
        success: true,
        changed_files: 0,
        pacnew_count: 0,
        pacsave_count: 0,
        services_pending: vec![],
        snapshot_label: None,
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

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

#[test]
/// What: Verify Esc key closes News modal and doesn't restore it.
///
/// Inputs:
/// - News modal with test items
/// - Esc key event
///
/// Output:
/// - Modal is set to None and remains None (not restored)
///
/// Details:
/// - Tests the bug fix where Esc should close the modal
fn news_esc_closes_modal() {
    let mut app = new_app();
    let items = vec![
        crate::state::NewsItem {
            date: "2025-01-01".to_string(),
            title: "Test News 1".to_string(),
            url: "https://example.com/news1".to_string(),
        },
        crate::state::NewsItem {
            date: "2025-01-02".to_string(),
            title: "Test News 2".to_string(),
            url: "https://example.com/news2".to_string(),
        },
    ];
    app.modal = crate::state::Modal::News { items, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify navigation keys in News modal don't close it.
///
/// Inputs:
/// - News modal with multiple items
/// - Up/Down key events
///
/// Output:
/// - Modal remains open and selection changes
///
/// Details:
/// - Ensures other keys still work correctly after the Esc fix
fn news_navigation_preserves_modal() {
    let mut app = new_app();
    let items = vec![
        crate::state::NewsItem {
            date: "2025-01-01".to_string(),
            title: "Test News 1".to_string(),
            url: "https://example.com/news1".to_string(),
        },
        crate::state::NewsItem {
            date: "2025-01-02".to_string(),
            title: "Test News 2".to_string(),
            url: "https://example.com/news2".to_string(),
        },
        crate::state::NewsItem {
            date: "2025-01-03".to_string(),
            title: "Test News 3".to_string(),
            url: "https://example.com/news3".to_string(),
        },
    ];
    app.modal = crate::state::Modal::News { items, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    // Press Down - should move selection and keep modal open
    let ke_down = key_event(KeyCode::Down, KeyModifiers::empty());
    handle_modal_key(ke_down, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::News { selected, .. } => {
            assert_eq!(*selected, 1);
        }
        _ => panic!("Modal should remain News after Down key"),
    }

    // Press Down again - should move selection further
    let ke_down2 = key_event(KeyCode::Down, KeyModifiers::empty());
    handle_modal_key(ke_down2, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::News { selected, .. } => {
            assert_eq!(*selected, 2);
        }
        _ => panic!("Modal should remain News after second Down key"),
    }

    // Press Up - should move selection back
    let ke_up = key_event(KeyCode::Up, KeyModifiers::empty());
    handle_modal_key(ke_up, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::News { selected, .. } => {
            assert_eq!(*selected, 1);
        }
        _ => panic!("Modal should remain News after Up key"),
    }

    // Press Up at top - should stay at 0
    let ke_up2 = key_event(KeyCode::Up, KeyModifiers::empty());
    handle_modal_key(ke_up2, &mut app, &add_tx);

    match &app.modal {
        crate::state::Modal::News { selected, .. } => {
            assert_eq!(*selected, 0);
        }
        _ => panic!("Modal should remain News after Up key at top"),
    }
}

#[test]
/// What: Verify Enter key in News modal doesn't close it.
///
/// Inputs:
/// - News modal with items
/// - Enter key event
///
/// Output:
/// - Modal remains open (Enter opens URL but doesn't close modal)
///
/// Details:
/// - Ensures Enter key works correctly
/// - Cleans up browser tab opened by the test
fn news_enter_preserves_modal() {
    let mut app = new_app();
    let test_url = "https://example.com/news";
    let items = vec![crate::state::NewsItem {
        date: "2025-01-01".to_string(),
        title: "Test News".to_string(),
        url: test_url.to_string(),
    }];
    app.modal = crate::state::Modal::News { items, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();

    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    // Modal should remain open since Enter opens URL but doesn't close modal
    match &app.modal {
        crate::state::Modal::News { selected, .. } => {
            assert_eq!(*selected, 0);
        }
        _ => panic!("Modal should remain News after Enter key"),
    }

    // No cleanup needed - open_url is a no-op during tests
}

#[test]
/// What: Verify unhandled keys in News modal don't break state.
///
/// Inputs:
/// - News modal
/// - Unhandled key event (e.g., 'x')
///
/// Output:
/// - Modal remains open with unchanged state
///
/// Details:
/// - Ensures unhandled keys don't cause issues
fn news_unhandled_key_preserves_modal() {
    let mut app = new_app();
    let items = vec![crate::state::NewsItem {
        date: "2025-01-01".to_string(),
        title: "Test News".to_string(),
        url: "https://example.com/news".to_string(),
    }];
    app.modal = crate::state::Modal::News { items, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('x'), KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    // Modal should remain open since 'x' is not handled
    match &app.modal {
        crate::state::Modal::News { selected, .. } => {
            assert_eq!(*selected, 0);
        }
        _ => panic!("Modal should remain News for unhandled key"),
    }
}

#[test]
/// What: Verify Esc key closes Alert modal.
///
/// Inputs:
/// - Alert modal with message
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes Alert modal correctly
fn alert_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Alert {
        message: "Test alert message".to_string(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes Alert modal.
///
/// Inputs:
/// - Alert modal with message
/// - Enter key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Enter also closes Alert modal
fn alert_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Alert {
        message: "Test alert message".to_string(),
    };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

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
            source: crate::state::Source::Aur,
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
            source: crate::state::Source::Aur,
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
            source: crate::state::Source::Aur,
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
/// What: Verify Esc key closes Help modal.
///
/// Inputs:
/// - Help modal
/// - Esc key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Esc closes Help modal correctly
fn help_esc_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Help;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Esc, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

#[test]
/// What: Verify Enter key closes Help modal.
///
/// Inputs:
/// - Help modal
/// - Enter key event
///
/// Output:
/// - Modal is set to None
///
/// Details:
/// - Tests that Enter also closes Help modal
fn help_enter_closes_modal() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Help;

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Enter, KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    assert!(matches!(app.modal, crate::state::Modal::None));
}

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
        _ => panic!("Modal should remain VirusTotalSetup after Enter with empty input"),
    }

    // No cleanup needed - open_url is a no-op during tests
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
