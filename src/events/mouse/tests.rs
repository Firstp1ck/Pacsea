//! Tests for mouse event handling.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tokio::sync::mpsc;

use super::handle_mouse_event;
use crate::state::{AppState, PackageItem};

/// What: Provide a fresh `AppState` tailored for mouse-event tests without repeated boilerplate.
///
/// Inputs:
/// - None (relies on `Default::default()` for deterministic initial state).
///
/// Output:
/// - New `AppState` ready for mutation inside individual mouse-event scenarios.
///
/// Details:
/// - Keeps mouse tests concise by centralizing the default setup in a single helper.
/// - Sets `PACSEA_TEST_HEADLESS` to prevent mouse escape sequences in test output.
fn new_app() -> AppState {
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }
    AppState::default()
}

#[test]
/// What: Clicking the PKGBUILD toggle should open the viewer and request content.
///
/// Inputs:
/// - `app`: State seeded with one selected result and `pkgb_button_rect` coordinates.
/// - `ev`: Left-click positioned inside the PKGBUILD button rectangle.
///
/// Output:
/// - Returns `false` while mutating `app` so `pkgb_visible` is `true` and the selection is sent on `pkgb_tx`.
///
/// Details:
/// - Captures the message from `pkgb_tx` to ensure the handler enqueues a fetch when opening the pane.
fn click_pkgb_toggle_opens() {
    let mut app = new_app();
    app.results = vec![crate::state::PackageItem {
        name: "rg".into(),
        version: "1".into(),
        description: String::new(),
        source: crate::state::Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.pkgb_button_rect = Some((10, 10, 5, 1));
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_tx, mut pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 10,
        modifiers: KeyModifiers::empty(),
    };
    let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.pkgb_visible);
    assert!(pkgb_rx.try_recv().ok().is_some());
}

#[test]
/// What: Clicking the toggle while the PKGBUILD viewer is open should close it and reset cached state.
///
/// Inputs:
/// - `app`: State with viewer visible, scroll offset/non-empty text, and `pkgb_button_rect` populated.
/// - `ev`: Left-click inside the PKGBUILD toggle rectangle.
///
/// Output:
/// - Returns `false` after mutating `app` so the viewer is hidden, text cleared, scroll set to zero, and rect unset.
///
/// Details:
/// - Ensures the handler cleans up PKGBUILD state to avoid stale scroll positions on the next open.
fn click_pkgb_toggle_closes_and_resets() {
    let mut app = new_app();
    app.results = vec![crate::state::PackageItem {
        name: "rg".into(),
        version: "1".into(),
        description: String::new(),
        source: crate::state::Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.pkgb_button_rect = Some((10, 10, 5, 1));
    // Pre-set PKGBUILD viewer as open with state to be reset
    app.pkgb_visible = true;
    app.pkgb_text = Some("dummy".into());
    app.pkgb_scroll = 7;
    app.pkgb_rect = Some((50, 50, 20, 5));

    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
    // Click inside the toggle area to hide
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 11,
        row: 10,
        modifiers: KeyModifiers::empty(),
    };
    let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);

    assert!(!app.pkgb_visible);
    assert!(app.pkgb_text.is_none());
    assert_eq!(app.pkgb_scroll, 0);
    assert!(app.pkgb_rect.is_none());
}

#[test]
/// What: Clicking an AUR filter toggle should invert its state and apply filters.
///
/// Inputs:
/// - `app`: State with `results_filter_show_aur` initially `false` and `results_filter_aur_rect` set.
/// - `ev`: Left-click positioned inside the AUR filter rectangle.
///
/// Output:
/// - Returns `false` while mutating `app` so `results_filter_show_aur` becomes `true`.
///
/// Details:
/// - Verifies that filter toggles work correctly and trigger filter application.
fn click_aur_filter_toggles() {
    let mut app = new_app();
    app.results_filter_show_aur = false;
    app.results_filter_aur_rect = Some((5, 2, 10, 1));

    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 7,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };

    let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.results_filter_show_aur);
}

#[test]
/// What: Clicking the Artix filter when all individual filters are on should turn all off.
///
/// Inputs:
/// - `app`: State with all Artix repo filters enabled and individual filter rects visible.
/// - `ev`: Left-click positioned inside the main Artix filter rectangle.
///
/// Output:
/// - Returns `false` while mutating `app` so all Artix filters become `false`.
///
/// Details:
/// - Tests the "all on -> all off" toggle behavior for Artix filters.
fn click_artix_filter_all_on_turns_all_off() {
    let mut app = new_app();
    app.results_filter_show_artix_omniverse = true;
    app.results_filter_show_artix_universe = true;
    app.results_filter_show_artix_lib32 = true;
    app.results_filter_show_artix_galaxy = true;
    app.results_filter_show_artix_world = true;
    app.results_filter_show_artix_system = true;
    app.results_filter_show_artix = true;
    // Set individual rects to indicate they're visible (not in dropdown mode)
    app.results_filter_artix_omniverse_rect = Some((10, 2, 5, 1));
    app.results_filter_artix_rect = Some((5, 2, 8, 1));

    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 7,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };

    let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(!app.results_filter_show_artix_omniverse);
    assert!(!app.results_filter_show_artix_universe);
    assert!(!app.results_filter_show_artix_lib32);
    assert!(!app.results_filter_show_artix_galaxy);
    assert!(!app.results_filter_show_artix_world);
    assert!(!app.results_filter_show_artix_system);
    assert!(!app.results_filter_show_artix);
}

#[test]
/// What: Clicking the Artix filter when some filters are off should turn all on.
///
/// Inputs:
/// - `app`: State with some Artix repo filters disabled and individual filter rects visible.
/// - `ev`: Left-click positioned inside the main Artix filter rectangle.
///
/// Output:
/// - Returns `false` while mutating `app` so all Artix filters become `true`.
///
/// Details:
/// - Tests the "some off -> all on" toggle behavior for Artix filters.
fn click_artix_filter_some_off_turns_all_on() {
    let mut app = new_app();
    app.results_filter_show_artix_omniverse = true;
    app.results_filter_show_artix_universe = false; // One is off
    app.results_filter_show_artix_lib32 = true;
    app.results_filter_show_artix_galaxy = true;
    app.results_filter_show_artix_world = true;
    app.results_filter_show_artix_system = true;
    app.results_filter_show_artix = true;
    // Set individual rects to indicate they're visible (not in dropdown mode)
    app.results_filter_artix_omniverse_rect = Some((10, 2, 5, 1));
    app.results_filter_artix_rect = Some((5, 2, 8, 1));

    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 7,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };

    let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.results_filter_show_artix_omniverse);
    assert!(app.results_filter_show_artix_universe);
    assert!(app.results_filter_show_artix_lib32);
    assert!(app.results_filter_show_artix_galaxy);
    assert!(app.results_filter_show_artix_world);
    assert!(app.results_filter_show_artix_system);
    assert!(app.results_filter_show_artix);
}

#[test]
/// What: Clicking the Artix filter when in dropdown mode should toggle the dropdown menu.
///
/// Inputs:
/// - `app`: State with individual Artix filter rects hidden (dropdown mode) and menu closed.
/// - `ev`: Left-click positioned inside the main Artix filter rectangle.
///
/// Output:
/// - Returns `false` while mutating `app` so `artix_filter_menu_open` becomes `true`.
///
/// Details:
/// - Tests that clicking the main Artix filter toggles the dropdown when individual filters are hidden.
fn click_artix_filter_toggles_dropdown() {
    let mut app = new_app();
    app.artix_filter_menu_open = false;
    app.results_filter_artix_rect = Some((5, 2, 8, 1));
    // Individual rects are None, indicating dropdown mode
    app.results_filter_artix_omniverse_rect = None;
    app.results_filter_artix_universe_rect = None;

    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 7,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };

    let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.artix_filter_menu_open);
}
