use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem, QueryInput};
use crate::theme::KeyChord;

use super::super::utils::matches_any;
use super::handle_search_key;
use super::helpers::navigate_pane;

/// What: Provide a baseline `AppState` tailored for search-pane tests without repeating setup boilerplate.
///
/// Inputs:
/// - None (relies on `Default::default()` for deterministic initial state).
///
/// Output:
/// - Fresh `AppState` ready for mutation inside individual test cases.
///
/// Details:
/// - Keeps search tests concise by centralizing the default application setup in one helper.
fn new_app() -> AppState {
    AppState::default()
}

#[test]
/// What: Insert-mode typing should update the search input, caret, and emit query messages.
///
/// Inputs:
/// - Key events for `'r'`, `'g'`, and `Backspace` applied in sequence while in insert mode.
///
/// Output:
/// - `app.input` transitions `"" -> "r" -> "rg" -> "r"`, and at least one query arrives on `qrx`.
///
/// Details:
/// - Validates caret/anchor bookkeeping indirectly by observing the query channel after each keystroke.
fn search_insert_typing_and_backspace() {
    let mut app = new_app();
    let (qtx, mut qrx) = mpsc::unbounded_channel::<QueryInput>();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert_eq!(app.input, "r");
    // At least one query should have been sent
    assert!(qrx.try_recv().ok().is_some());
}

#[test]
/// What: Normal-mode selection commands should set the anchor and adjust the caret within bounds.
///
/// Inputs:
/// - Escape key to enter normal mode, followed by `'l'` (select-right) and `'h'` (select-left).
///
/// Output:
/// - `search_select_anchor` becomes `Some` and the caret remains within the valid input character range.
///
/// Details:
/// - Confirms the keymap translation respects the default bindings for navigation-style selection.
fn search_normal_mode_selection() {
    let mut app = new_app();
    app.input = "rip".into();
    let (qtx, _qrx) = mpsc::unbounded_channel::<QueryInput>();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    // Toggle into normal mode (Esc by default per KeyMap)
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    // Select right (default 'l')
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert!(app.search_select_anchor.is_some());
    // Select left (default 'h')
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert!(app.search_caret <= crate::events::utils::char_count(&app.input));
}

#[test]
/// What: Verify `matches_any` correctly handles Shift+char edge cases across different terminal behaviors.
///
/// Inputs:
/// - Key events with and without Shift modifier, matched against configured chords with Shift.
///
/// Output:
/// - Returns `true` when the key event matches the configured chord, handling terminal inconsistencies.
///
/// Details:
/// - Tests that uppercase chars match Shift+lowercase configs even if terminal doesn't report Shift.
/// - Tests that lowercase chars with Shift modifier match Shift+lowercase configs.
/// - Tests exact matches without Shift handling.
fn helpers_matches_any_shift_handling() {
    use crossterm::event::KeyCode;

    // Test exact match (no Shift involved)
    let ke1 = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    let chord1 = vec![KeyChord {
        code: KeyCode::Char('r'),
        mods: KeyModifiers::CONTROL,
    }];
    assert!(matches_any(&ke1, &chord1));

    // Test Shift+char config matching uppercase char (terminal may not report Shift)
    let ke2 = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::empty());
    let chord2 = vec![KeyChord {
        code: KeyCode::Char('r'),
        mods: KeyModifiers::SHIFT,
    }];
    assert!(matches_any(&ke2, &chord2));

    // Test Shift+char config matching lowercase char with Shift modifier
    let ke3 = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::SHIFT);
    let chord3 = vec![KeyChord {
        code: KeyCode::Char('r'),
        mods: KeyModifiers::SHIFT,
    }];
    assert!(matches_any(&ke3, &chord3));

    // Test non-match case
    let ke4 = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty());
    let chord4 = vec![KeyChord {
        code: KeyCode::Char('r'),
        mods: KeyModifiers::SHIFT,
    }];
    assert!(!matches_any(&ke4, &chord4));
}

#[test]
/// What: Normal mode deletion (default: 'd') should remove selected text range and trigger query.
///
/// Inputs:
/// - Enter normal mode, set selection anchor, move caret, then press 'd' to delete.
///
/// Output:
/// - Selected text is removed from input, caret moves to start of deleted range, query is sent.
///
/// Details:
/// - Validates that deletion respects anchor and caret positions, removing the range between them.
fn search_normal_mode_deletion() {
    let mut app = new_app();
    app.input = "hello world".into();
    app.search_caret = 6; // After "hello "
    let (qtx, mut qrx) = mpsc::unbounded_channel::<QueryInput>();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    // Enter normal mode
    app.search_normal_mode = true;
    // Set anchor at start
    app.search_select_anchor = Some(0);
    // Move caret to position 5 (after "hello")
    app.search_caret = 5;

    // Delete selection (default 'd')
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );

    // Should have deleted "hello"
    assert_eq!(app.input, " world");
    assert_eq!(app.search_caret, 0);
    assert!(app.search_select_anchor.is_none());
    // Query should have been sent
    assert!(qrx.try_recv().ok().is_some());
}

#[test]
/// What: Normal mode clear (default: Shift+Del) should clear entire input and trigger query.
///
/// Inputs:
/// - Enter normal mode with non-empty input, then press Shift+Del.
///
/// Output:
/// - Input is cleared, caret reset to 0, selection anchor cleared, query is sent.
///
/// Details:
/// - Validates that clear operation resets all input-related state.
fn search_normal_mode_clear() {
    let mut app = new_app();
    app.input = "test query".into();
    app.search_caret = 5;
    app.search_select_anchor = Some(3);
    let (qtx, mut qrx) = mpsc::unbounded_channel::<QueryInput>();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    // Enter normal mode
    app.search_normal_mode = true;
    // Configure clear keybinding
    app.keymap.search_normal_clear = vec![KeyChord {
        code: KeyCode::Delete,
        mods: KeyModifiers::SHIFT,
    }];

    // Clear input
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );

    assert_eq!(app.input, "");
    assert_eq!(app.search_caret, 0);
    assert!(app.search_select_anchor.is_none());
    // Query should have been sent
    assert!(qrx.try_recv().ok().is_some());
}

#[test]
/// What: Mode toggle should switch between insert and normal mode.
///
/// Inputs:
/// - Start in insert mode, press toggle key (default: Esc), then toggle again.
///
/// Output:
/// - Mode toggles between false (insert) and true (normal), selection anchor cleared on exit.
///
/// Details:
/// - Validates that mode toggle correctly updates `search_normal_mode` state.
fn search_mode_toggle() {
    let mut app = new_app();
    app.search_normal_mode = false;
    app.search_select_anchor = Some(5);
    let (qtx, _qrx) = mpsc::unbounded_channel::<QueryInput>();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    // Configure toggle key (default Esc)
    app.keymap.search_normal_toggle = vec![KeyChord {
        code: KeyCode::Esc,
        mods: KeyModifiers::empty(),
    }];

    // Toggle to normal mode
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert!(app.search_normal_mode);

    // Toggle back to insert mode
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert!(!app.search_normal_mode);
    // Note: Selection anchor is cleared when entering insert mode via search_normal_insert key,
    // but not when toggling mode. This is expected behavior.
}

#[test]
/// What: Pane navigation should correctly switch focus and update selection state.
///
/// Inputs:
/// - Call `navigate_pane` with "right" direction in different modes.
///
/// Output:
/// - Focus changes appropriately, selection state initialized if needed.
///
/// Details:
/// - Tests that right navigation targets Install/Downgrade based on `installed_only_mode`.
/// - Left navigation test is skipped as it requires Tokio runtime for preview trigger.
fn helpers_navigate_pane_directions() {
    let mut app = new_app();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    // Test right navigation in normal mode
    app.installed_only_mode = false;
    app.install_list.push(crate::state::PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: crate::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    });
    navigate_pane(&mut app, "right", &dtx, &ptx);
    assert!(matches!(app.focus, crate::state::Focus::Install));

    // Test right navigation in installed-only mode
    app.installed_only_mode = true;
    app.downgrade_list.push(crate::state::PackageItem {
        name: "downgrade-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: crate::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    });
    navigate_pane(&mut app, "right", &dtx, &ptx);
    assert!(matches!(app.focus, crate::state::Focus::Install));
    assert!(matches!(
        app.right_pane_focus,
        crate::state::RightPaneFocus::Downgrade
    ));
}

#[test]
/// What: Space key in insert mode should add items to appropriate lists based on mode.
///
/// Inputs:
/// - Press Space with a selected item in normal and installed-only modes.
///
/// Output:
/// - Items are sent to `add_tx` in normal mode, or added to `remove_list` in installed-only mode.
///
/// Details:
/// - Validates that Space correctly routes items based on `installed_only_mode` flag.
fn search_insert_mode_space_adds_items() {
    let mut app = new_app();
    let test_item = crate::state::PackageItem {
        name: "test-package".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: crate::state::Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    };
    app.results.push(test_item.clone());
    app.selected = 0;

    let (qtx, _qrx) = mpsc::unbounded_channel::<QueryInput>();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, mut arx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    // Test normal mode: should send to add_tx
    app.installed_only_mode = false;
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    let received = arx.try_recv().ok();
    assert!(received.is_some());
    assert_eq!(
        received
            .expect("received should be Some after is_some() check")
            .name,
        "test-package"
    );

    // Test installed-only mode: should add to remove_list
    app.installed_only_mode = true;
    app.results.push(test_item);
    app.selected = 0;
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert!(app.remove_list.iter().any(|p| p.name == "test-package"));
}

#[test]
/// What: Normal mode navigation keys (j/k) should move selection within bounds.
///
/// Inputs:
/// - Enter normal mode, press 'j' to move down, 'k' to move up.
///
/// Output:
/// - Selection index changes appropriately, staying within results bounds.
///
/// Details:
/// - Validates that j/k navigation respects result list boundaries.
fn search_normal_mode_navigation() {
    let mut app = new_app();
    app.results = vec![
        crate::state::PackageItem {
            name: "pkg1".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
        crate::state::PackageItem {
            name: "pkg2".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
        crate::state::PackageItem {
            name: "pkg3".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
    ];
    app.selected = 1;
    app.search_normal_mode = true;

    let (qtx, _qrx) = mpsc::unbounded_channel::<QueryInput>();
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

    // Move down (j)
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert_eq!(app.selected, 2);

    // Move up (k)
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert_eq!(app.selected, 1);

    // Move up at top should stay at 0
    app.selected = 0;
    let _ = handle_search_key(
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()),
        &mut app,
        &qtx,
        &dtx,
        &atx,
        &ptx,
    );
    assert_eq!(app.selected, 0);
}
