//! Tests for News modal key event handling.

use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::PackageItem;

use super::common::{key_event, new_app};
use super::handle_modal_key;

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
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news1".to_string(),
            date: "2025-01-01".to_string(),
            title: "Test News 1".to_string(),
            summary: None,
            url: Some("https://example.com/news1".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        },
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news2".to_string(),
            date: "2025-01-02".to_string(),
            title: "Test News 2".to_string(),
            summary: None,
            url: Some("https://example.com/news2".to_string()),
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
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news1".to_string(),
            date: "2025-01-01".to_string(),
            title: "Test News 1".to_string(),
            summary: None,
            url: Some("https://example.com/news1".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        },
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news2".to_string(),
            date: "2025-01-02".to_string(),
            title: "Test News 2".to_string(),
            summary: None,
            url: Some("https://example.com/news2".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        },
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news3".to_string(),
            date: "2025-01-03".to_string(),
            title: "Test News 3".to_string(),
            summary: None,
            url: Some("https://example.com/news3".to_string()),
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
    let items = vec![crate::state::types::NewsFeedItem {
        id: test_url.to_string(),
        date: "2025-01-01".to_string(),
        title: "Test News".to_string(),
        summary: None,
        url: Some(test_url.to_string()),
        source: crate::state::types::NewsFeedSource::ArchNews,
        severity: None,
        packages: Vec::new(),
    }];
    app.modal = crate::state::Modal::News {
        items,
        selected: 0,
        scroll: 0,
    };

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
/// What: Verify numpad Enter (carriage return) in News modal has same effect as main Enter.
///
/// Inputs:
/// - News modal with item
/// - KeyCode::Char('\r')
///
/// Output:
/// - Modal remains News, selected unchanged
///
/// Details:
/// - Ensures numpad Enter handling does not break News; same outcome as main Enter
fn news_numpad_enter_carriage_return_keeps_modal_open() {
    let items = vec![crate::state::types::NewsFeedItem {
        id: "https://example.com/1".to_string(),
        date: "2025-01-01".to_string(),
        title: "Test".to_string(),
        summary: None,
        url: Some("https://example.com".to_string()),
        source: crate::state::types::NewsFeedSource::ArchNews,
        severity: None,
        packages: Vec::new(),
    }];
    let mut app = new_app();
    app.modal = crate::state::Modal::News {
        items: items.clone(),
        selected: 0,
        scroll: 0,
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    match &app.modal {
        crate::state::Modal::News { selected, .. } => assert_eq!(*selected, 0),
        _ => panic!("Modal should remain News after numpad Enter"),
    }
}

#[test]
/// What: Verify numpad Enter (newline) in News modal has same effect as main Enter.
///
/// Inputs:
/// - News modal with item
/// - KeyCode::Char('\n')
///
/// Output:
/// - Modal remains News, selected unchanged
///
/// Details:
/// - Ensures numpad Enter handling does not break News; same outcome as main Enter
fn news_numpad_enter_newline_keeps_modal_open() {
    let items = vec![crate::state::types::NewsFeedItem {
        id: "https://example.com/1".to_string(),
        date: "2025-01-01".to_string(),
        title: "Test".to_string(),
        summary: None,
        url: Some("https://example.com".to_string()),
        source: crate::state::types::NewsFeedSource::ArchNews,
        severity: None,
        packages: Vec::new(),
    }];
    let mut app = new_app();
    app.modal = crate::state::Modal::News {
        items: items.clone(),
        selected: 0,
        scroll: 0,
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
    handle_modal_key(ke, &mut app, &add_tx);
    match &app.modal {
        crate::state::Modal::News { selected, .. } => assert_eq!(*selected, 0),
        _ => panic!("Modal should remain News after numpad Enter"),
    }
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
    let items = vec![crate::state::types::NewsFeedItem {
        id: "https://example.com/news".to_string(),
        date: "2025-01-01".to_string(),
        title: "Test News".to_string(),
        summary: None,
        url: Some("https://example.com/news".to_string()),
        source: crate::state::types::NewsFeedSource::ArchNews,
        severity: None,
        packages: Vec::new(),
    }];
    app.modal = crate::state::Modal::News {
        items,
        selected: 0,
        scroll: 0,
    };

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
/// - Only works in normal mode
fn news_ctrl_r_mark_all_read_not_config_reload() {
    let mut app = new_app();
    app.search_normal_mode = true; // Must be in normal mode
    let items = vec![
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news1".to_string(),
            date: "2025-01-01".to_string(),
            title: "Test News 1".to_string(),
            summary: None,
            url: Some("https://example.com/news1".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        },
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news2".to_string(),
            date: "2025-01-02".to_string(),
            title: "Test News 2".to_string(),
            summary: None,
            url: Some("https://example.com/news2".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        },
        crate::state::types::NewsFeedItem {
            id: "https://example.com/news3".to_string(),
            date: "2025-01-03".to_string(),
            title: "Test News 3".to_string(),
            summary: None,
            url: Some("https://example.com/news3".to_string()),
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
/// - Only works in normal mode
fn news_modal_lowercase_r_marks_single_read() {
    let mut app = new_app();
    app.search_normal_mode = true; // Must be in normal mode
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
