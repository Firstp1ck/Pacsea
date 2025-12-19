//! Tests for global keybind blocking when modals are open.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{create_test_modals, key_event, new_app};
use super::handle_modal_key;

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
        crate::state::types::NewsFeedItem {
            id: "https://example.com/1".to_string(),
            date: "2025-01-01".to_string(),
            title: "News 1".to_string(),
            summary: None,
            url: Some("https://example.com/1".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        },
        crate::state::types::NewsFeedItem {
            id: "https://example.com/2".to_string(),
            date: "2025-01-02".to_string(),
            title: "News 2".to_string(),
            summary: None,
            url: Some("https://example.com/2".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        },
    ];
    app.modal = crate::state::Modal::News {
        items,
        selected: 0,
        scroll: 0,
    };

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
