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
/// What: Verify Ctrl+R in News modal triggers mark all read instead of config reload.
///
/// Inputs:
/// - News modal with multiple items
/// - Ctrl+R key event
///
/// Output:
/// - All news items are marked as read
/// - Modal remains open
/// - Config reload does NOT happen
///
/// Details:
/// - Ensures that when News modal is active, Ctrl+R triggers news action (mark all read)
///   instead of the global config reload action
fn news_ctrl_r_mark_all_read_not_config_reload() {
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
    let ke = key_event(KeyCode::Char('r'), KeyModifiers::CONTROL);

    // Verify initial state - no items marked as read
    assert!(app.news_read_urls.is_empty());
    assert!(!app.news_read_dirty);

    handle_modal_key(ke, &mut app, &add_tx);

    // Verify all items are marked as read
    assert!(app.news_read_urls.contains("https://example.com/news1"));
    assert!(app.news_read_urls.contains("https://example.com/news2"));
    assert!(app.news_read_urls.contains("https://example.com/news3"));
    assert_eq!(app.news_read_urls.len(), 3);
    assert!(app.news_read_dirty);

    // Verify modal remains open
    match &app.modal {
        crate::state::Modal::News { selected, .. } => {
            assert_eq!(*selected, 0);
        }
        _ => panic!("Modal should remain News after Ctrl+R"),
    }

    // Verify config reload did NOT happen (no toast message about config reload)
    // Config reload would set a toast message, but mark all read doesn't
    assert!(app.toast_message.is_none());
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

// ============================================================================
// Global Keybind Blocking Tests
// ============================================================================
// These tests verify that when a modal is open, global keybinds are blocked
// and do not trigger their global actions. Only exit (Ctrl+C) should work globally.

/// What: Create test modals for global keybind blocking tests.
///
/// Inputs:
/// - None
///
/// Output:
/// - Vector of (modal, name) tuples for testing
///
/// Details:
/// - Returns all modals that should block global keybinds (excludes None and Preflight)
fn create_test_modals() -> Vec<(crate::state::Modal, &'static str)> {
    vec![
        (
            crate::state::Modal::Alert {
                message: "Test alert".to_string(),
            },
            "Alert",
        ),
        (
            crate::state::Modal::Loading {
                message: "Loading...".to_string(),
            },
            "Loading",
        ),
        (
            crate::state::Modal::ConfirmInstall { items: vec![] },
            "ConfirmInstall",
        ),
        (
            crate::state::Modal::ConfirmReinstall {
                items: vec![],
                all_items: vec![],
                header_chips: PreflightHeaderChips::default(),
            },
            "ConfirmReinstall",
        ),
        (
            crate::state::Modal::ConfirmBatchUpdate {
                items: vec![],
                dry_run: false,
            },
            "ConfirmBatchUpdate",
        ),
        (
            crate::state::Modal::PreflightExec {
                items: vec![],
                action: PreflightAction::Install,
                tab: PreflightTab::Summary,
                verbose: false,
                log_lines: vec![],
                abortable: false,
                header_chips: PreflightHeaderChips::default(),
                success: None,
            },
            "PreflightExec",
        ),
        (
            crate::state::Modal::PostSummary {
                success: true,
                changed_files: 0,
                pacnew_count: 0,
                pacsave_count: 0,
                services_pending: vec![],
                snapshot_label: None,
            },
            "PostSummary",
        ),
        (crate::state::Modal::Help, "Help"),
        (
            crate::state::Modal::ConfirmRemove { items: vec![] },
            "ConfirmRemove",
        ),
        (
            crate::state::Modal::SystemUpdate {
                do_mirrors: false,
                do_pacman: false,
                force_sync: false,
                do_aur: false,
                do_cache: false,
                country_idx: 0,
                countries: vec!["US".to_string()],
                mirror_count: 10,
                cursor: 0,
            },
            "SystemUpdate",
        ),
        (
            crate::state::Modal::News {
                items: vec![crate::state::NewsItem {
                    date: "2025-01-01".to_string(),
                    title: "Test".to_string(),
                    url: "https://example.com".to_string(),
                }],
                selected: 0,
            },
            "News",
        ),
        (
            crate::state::Modal::Updates {
                entries: vec![("pkg".to_string(), "1.0".to_string(), "2.0".to_string())],
                scroll: 0,
                selected: 0,
            },
            "Updates",
        ),
        (
            crate::state::Modal::OptionalDeps {
                rows: vec![],
                selected: 0,
            },
            "OptionalDeps",
        ),
        (
            crate::state::Modal::ScanConfig {
                do_clamav: false,
                do_trivy: false,
                do_semgrep: false,
                do_shellcheck: false,
                do_virustotal: false,
                do_custom: false,
                do_sleuth: false,
                cursor: 0,
            },
            "ScanConfig",
        ),
        (
            crate::state::Modal::VirusTotalSetup {
                input: String::new(),
                cursor: 0,
            },
            "VirusTotalSetup",
        ),
        (
            crate::state::Modal::PasswordPrompt {
                purpose: crate::state::modal::PasswordPurpose::Install,
                items: vec![],
                input: String::new(),
                cursor: 0,
                error: None,
            },
            "PasswordPrompt",
        ),
        (
            crate::state::Modal::GnomeTerminalPrompt,
            "GnomeTerminalPrompt",
        ),
        (crate::state::Modal::ImportHelp, "ImportHelp"),
    ]
}

#[test]
/// What: Verify Ctrl+R (reload config) is blocked when modals are open.
///
/// Inputs:
/// - Each modal type (except None and Preflight)
/// - Ctrl+R key event
///
/// Output:
/// - Config reload does NOT trigger
/// - Modal remains open or handles 'r' key per its own logic
///
/// Details:
/// - Tests that global keybind is blocked for all modals
/// - Note: Some modals (like `PostSummary`) use 'r' for their own actions (rollback)
///   which is expected behavior - the modal keybind takes priority
fn global_keybind_ctrl_r_blocked_in_all_modals() {
    // Config reload toast message pattern (from i18n)
    let config_reload_patterns = ["config", "reload", "Config", "Reload"];

    for (modal, name) in create_test_modals() {
        let mut app = new_app();
        app.modal = modal.clone();

        let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
        let ke = key_event(KeyCode::Char('r'), KeyModifiers::CONTROL);

        handle_modal_key(ke, &mut app, &add_tx);

        // If there's a toast message, verify it's NOT a config reload message
        // (some modals like PostSummary use 'r' for their own actions like rollback)
        if let Some(ref msg) = app.toast_message {
            let is_config_reload = config_reload_patterns.iter().any(|p| msg.contains(p));
            assert!(
                !is_config_reload,
                "{name}: Ctrl+R should not trigger config reload, got toast: {msg}"
            );
        }

        // Modal should still be open (or closed by its own handler, but NOT by global keybind)
        // We just verify no global side effects occurred
    }
}

#[test]
/// What: Verify Ctrl+X (PKGBUILD toggle) is blocked when modals are open.
///
/// Inputs:
/// - Each modal type (except None and Preflight)
/// - Ctrl+X key event
///
/// Output:
/// - PKGBUILD visibility does NOT change
///
/// Details:
/// - Tests that global keybind is blocked for all modals
fn global_keybind_ctrl_x_blocked_in_all_modals() {
    for (modal, name) in create_test_modals() {
        let mut app = new_app();
        app.modal = modal.clone();
        app.pkgb_visible = false;

        let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
        let ke = key_event(KeyCode::Char('x'), KeyModifiers::CONTROL);

        handle_modal_key(ke, &mut app, &add_tx);

        assert!(
            !app.pkgb_visible,
            "{name}: Ctrl+X should be blocked, pkgb_visible should remain false"
        );
    }
}

#[test]
/// What: Verify Ctrl+S (change sort) is blocked when modals are open.
///
/// Inputs:
/// - Each modal type (except None and Preflight)
/// - Ctrl+S key event
///
/// Output:
/// - Sort order does NOT change
///
/// Details:
/// - Tests that global keybind is blocked for all modals
fn global_keybind_ctrl_s_blocked_in_all_modals() {
    for (modal, name) in create_test_modals() {
        let mut app = new_app();
        app.modal = modal.clone();
        let original_sort = app.sort_mode;

        let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
        let ke = key_event(KeyCode::Char('s'), KeyModifiers::CONTROL);

        handle_modal_key(ke, &mut app, &add_tx);

        assert_eq!(
            app.sort_mode, original_sort,
            "{name}: Ctrl+S should be blocked, sort_mode should remain unchanged"
        );
    }
}

#[test]
/// What: Verify F1 (help overlay) is blocked when modals are open.
///
/// Inputs:
/// - Each modal type (except None, Preflight, and Help itself)
/// - F1 key event
///
/// Output:
/// - Help modal does NOT open (no nested Help modal)
///
/// Details:
/// - Tests that global keybind is blocked for all modals
fn global_keybind_f1_blocked_in_all_modals() {
    for (modal, name) in create_test_modals() {
        // Skip Help modal itself - F1 doesn't make sense there
        if matches!(modal, crate::state::Modal::Help) {
            continue;
        }

        let mut app = new_app();
        app.modal = modal.clone();

        let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
        let ke = key_event(KeyCode::F(1), KeyModifiers::empty());

        handle_modal_key(ke, &mut app, &add_tx);

        // Should NOT have opened Help modal (would replace current modal)
        // The modal should either be unchanged or closed by its own Esc/Enter handling
        // but NOT replaced by Help
        assert!(
            !matches!(app.modal, crate::state::Modal::Help)
                || matches!(modal, crate::state::Modal::Help),
            "{name}: F1 should be blocked, Help modal should not open"
        );
    }
}

#[test]
/// What: Verify Ctrl+T (comments toggle) is blocked when modals are open.
///
/// Inputs:
/// - Each modal type (except None and Preflight)
/// - Ctrl+T key event
///
/// Output:
/// - Comments visibility does NOT change
///
/// Details:
/// - Tests that global keybind is blocked for all modals
fn global_keybind_ctrl_t_blocked_in_all_modals() {
    for (modal, name) in create_test_modals() {
        let mut app = new_app();
        app.modal = modal.clone();
        let original_comments_visible = app.comments_visible;

        let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
        let ke = key_event(KeyCode::Char('t'), KeyModifiers::CONTROL);

        handle_modal_key(ke, &mut app, &add_tx);

        assert_eq!(
            app.comments_visible, original_comments_visible,
            "{name}: Ctrl+T should be blocked, comments_visible should remain unchanged"
        );
    }
}

#[test]
/// What: Verify all global keybinds work when no modal is open.
///
/// Inputs:
/// - `Modal::None`
/// - Various global keybind key events
///
/// Output:
/// - Global keybinds should work normally (changes state)
///
/// Details:
/// - Baseline test to ensure global keybinds work when expected
fn global_keybinds_work_when_no_modal_open() {
    // Test Ctrl+S changes sort mode when no modal is open
    let mut app = new_app();
    app.modal = crate::state::Modal::None;
    let original_sort = app.sort_mode;

    // Note: handle_modal_key returns early for Modal::None,
    // so global keybinds are handled by handle_global_key in mod.rs
    // This test verifies the modal handler doesn't interfere
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('s'), KeyModifiers::CONTROL);

    let result = handle_modal_key(ke, &mut app, &add_tx);

    // Modal handler should return false for Modal::None (not handled)
    assert!(!result, "Modal::None should return false (not handled)");
    // Sort mode should be unchanged by modal handler (global handler would change it)
    assert_eq!(
        app.sort_mode, original_sort,
        "Modal handler should not change sort_mode for Modal::None"
    );
}

#[test]
/// What: Verify modal keybinds take priority over global keybinds.
///
/// Inputs:
/// - News modal with items
/// - Ctrl+R key event (global: reload config, modal: mark all read)
///
/// Output:
/// - Modal action triggers (mark all read)
/// - Global action does NOT trigger (no toast)
///
/// Details:
/// - Comprehensive test for the original issue (Ctrl+R conflict in News modal)
fn modal_keybinds_priority_over_global_ctrl_r_in_news() {
    let mut app = new_app();
    let items = vec![
        crate::state::NewsItem {
            date: "2025-01-01".to_string(),
            title: "News 1".to_string(),
            url: "https://example.com/1".to_string(),
        },
        crate::state::NewsItem {
            date: "2025-01-02".to_string(),
            title: "News 2".to_string(),
            url: "https://example.com/2".to_string(),
        },
    ];
    app.modal = crate::state::Modal::News { items, selected: 0 };

    // Verify initial state
    assert!(app.news_read_urls.is_empty());
    assert!(!app.news_read_dirty);
    assert!(app.toast_message.is_none());

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('r'), KeyModifiers::CONTROL);

    handle_modal_key(ke, &mut app, &add_tx);

    // Modal action should have triggered: mark all read
    assert_eq!(
        app.news_read_urls.len(),
        2,
        "All news items should be marked as read"
    );
    assert!(app.news_read_dirty, "news_read_dirty should be true");

    // Global action should NOT have triggered: no config reload toast
    assert!(
        app.toast_message.is_none(),
        "Config reload toast should NOT appear (global keybind blocked)"
    );

    // Modal should remain open
    assert!(
        matches!(app.modal, crate::state::Modal::News { .. }),
        "News modal should remain open"
    );
}

#[test]
/// What: Verify 'r' (mark single read) works in News modal without conflict.
///
/// Inputs:
/// - News modal with items
/// - 'r' key event (no modifiers)
///
/// Output:
/// - Single item marked as read
///
/// Details:
/// - Tests that lowercase 'r' works for mark single read
fn news_modal_lowercase_r_marks_single_read() {
    let mut app = new_app();
    let items = vec![
        crate::state::NewsItem {
            date: "2025-01-01".to_string(),
            title: "News 1".to_string(),
            url: "https://example.com/1".to_string(),
        },
        crate::state::NewsItem {
            date: "2025-01-02".to_string(),
            title: "News 2".to_string(),
            url: "https://example.com/2".to_string(),
        },
    ];
    app.modal = crate::state::Modal::News { items, selected: 0 };

    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('r'), KeyModifiers::empty());

    handle_modal_key(ke, &mut app, &add_tx);

    // Only first item should be marked as read
    assert_eq!(
        app.news_read_urls.len(),
        1,
        "Only selected item should be marked as read"
    );
    assert!(
        app.news_read_urls.contains("https://example.com/1"),
        "First item URL should be marked as read"
    );
    assert!(app.news_read_dirty);
}
