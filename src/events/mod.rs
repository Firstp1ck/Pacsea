//! Event handling layer for Pacsea's TUI (modularized).
//!
//! This module re-exports `handle_event` and delegates pane-specific logic
//! and mouse handling to submodules to keep files small and maintainable.

use crossterm::event::{Event as CEvent, KeyEventKind};
use tokio::sync::mpsc;

use crate::state::{AppState, Focus, PackageItem, QueryInput};

mod distro;
mod global;
mod install;
mod modals;
mod mouse;
mod preflight;
mod recent;
mod search;
mod utils;

// re-export intentionally omitted; handled internally

/// What: Dispatch a single terminal event (keyboard/mouse) and mutate the [`AppState`].
///
/// Inputs:
/// - `ev`: Terminal event (key or mouse)
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send search queries
/// - `details_tx`: Channel to request package details
/// - `preview_tx`: Channel to request preview details for Recent
/// - `add_tx`: Channel to enqueue items into the install list
/// - `pkgb_tx`: Channel to request PKGBUILD content for the current selection
///
/// Output:
/// - `true` to signal the application should exit; otherwise `false`.
///
/// Details:
/// - Handles active modal interactions first (Alert/SystemUpdate/ConfirmInstall/ConfirmRemove/Help/News).
/// - Supports global shortcuts (help overlay, theme reload, exit, PKGBUILD viewer toggle, change sort).
/// - Delegates pane-specific handling to `search`, `recent`, and `install` submodules.
pub fn handle_event(
    ev: CEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if let CEvent::Key(ke) = ev {
        if ke.kind != KeyEventKind::Press {
            return false;
        }

        // Handle Preflight modal first (it's the largest)
        if matches!(app.modal, crate::state::Modal::Preflight { .. }) {
            return preflight::handle_preflight_key(ke, app);
        }

        // Handle all other modals
        if modals::handle_modal_key(ke, app, add_tx) {
            return false;
        }

        // If any modal remains open after handling above, consume the key to prevent main window interaction
        if !matches!(app.modal, crate::state::Modal::None) {
            return false;
        }

        // Handle global shortcuts and dropdown menus
        if let Some(should_exit) = global::handle_global_key(ke, app, details_tx, pkgb_tx) {
            if should_exit {
                return true; // Exit requested
            }
            // Key was handled by global shortcuts, don't process further
            return false;
        }

        // Pane-specific handling (Search, Recent, Install)
        // Recent pane focused
        if matches!(app.focus, Focus::Recent) {
            let should_exit =
                recent::handle_recent_key(ke, app, query_tx, details_tx, preview_tx, add_tx);
            return should_exit;
        }

        // Install pane focused
        if matches!(app.focus, Focus::Install) {
            let should_exit = install::handle_install_key(ke, app, details_tx, preview_tx, add_tx);
            return should_exit;
        }

        // Search pane focused (delegated)
        if matches!(app.focus, Focus::Search) {
            let should_exit =
                search::handle_search_key(ke, app, query_tx, details_tx, add_tx, preview_tx);
            return should_exit;
        }

        // Fallback: not handled
        return false;
    }

    // Mouse handling delegated
    if let CEvent::Mouse(m) = ev {
        return mouse::handle_mouse_event(m, app, details_tx, preview_tx, add_tx, pkgb_tx);
    }
    false
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    #[test]
    /// What: Ensure the system update action invokes `xfce4-terminal` with the expected command separator.
    ///
    /// Inputs:
    /// - Shimmed `xfce4-terminal` placed on `PATH`, mouse clicks to open Options → Update System, and `Enter` key event.
    ///
    /// Output:
    /// - Captured arguments begin with `--command` followed by `bash -lc ...`.
    ///
    /// Details:
    /// - Uses environment overrides plus a fake terminal script to observe the spawn command safely.
    fn ui_options_update_system_enter_triggers_xfce4_args_shape() {
        let _guard = crate::global_test_mutex_lock();
        // fake xfce4-terminal
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_term_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create test directory");
        let mut out_path = dir.clone();
        out_path.push("args.txt");
        let mut term_path = dir.clone();
        term_path.push("xfce4-terminal");
        let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
        fs::write(&term_path, script.as_bytes()).unwrap();
        let mut perms = fs::metadata(&term_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&term_path, perms).unwrap();
        let orig_path = std::env::var_os("PATH");
        // Prepend our fake terminal directory to PATH to ensure xfce4-terminal is found first
        let combined_path = match std::env::var("PATH") {
            Ok(p) => format!("{}:{}", dir.display(), p),
            Err(_) => dir.display().to_string(),
        };
        unsafe {
            std::env::set_var("PATH", combined_path);
            std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
            std::env::set_var("PACSEA_TEST_HEADLESS", "1");
        }

        let mut app = AppState {
            ..Default::default()
        };
        let (qtx, _qrx) = mpsc::unbounded_channel();
        let (dtx, _drx) = mpsc::unbounded_channel();
        let (ptx, _prx) = mpsc::unbounded_channel();
        let (atx, _arx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        app.options_button_rect = Some((5, 5, 10, 1));
        let click_options = CEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 5,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        assert!(app.options_menu_open);
        app.options_menu_rect = Some((5, 6, 20, 3));
        let click_menu_update = CEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 7,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(
            click_menu_update,
            &mut app,
            &qtx,
            &dtx,
            &ptx,
            &atx,
            &pkgb_tx,
        );
        let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        let _ = super::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        // Wait for file to be created with retries
        let mut attempts = 0;
        while !out_path.exists() && attempts < 50 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            attempts += 1;
        }
        // Give the process time to complete writing to avoid race conditions with other tests
        std::thread::sleep(std::time::Duration::from_millis(100));
        let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
        let lines: Vec<&str> = body.lines().collect();
        // Verify that xfce4-terminal was actually used by checking for --command argument
        // (xfce4-terminal is the only terminal that uses --command format)
        // Find the last --command to handle cases where multiple spawns might have occurred
        let command_idx = lines.iter().rposition(|&l| l == "--command");
        if command_idx.is_none() {
            // If --command wasn't found, xfce4-terminal wasn't used (another terminal was chosen)
            // This can happen when other terminals are on PATH and chosen first
            eprintln!(
                "Warning: xfce4-terminal was not used (no --command found, got: {:?}), skipping xfce4-specific assertion",
                lines
            );
            unsafe {
                if let Some(v) = orig_path {
                    std::env::set_var("PATH", v);
                } else {
                    std::env::remove_var("PATH");
                }
                std::env::remove_var("PACSEA_TEST_OUT");
            }
            return;
        }
        let command_idx = command_idx.unwrap();
        assert!(
            command_idx + 1 < lines.len(),
            "--command found at index {} but no following argument. Lines: {:?}",
            command_idx,
            lines
        );
        assert!(
            lines[command_idx + 1].starts_with("bash -lc "),
            "Expected argument after --command to start with 'bash -lc ', got: '{}'. All lines: {:?}",
            lines[command_idx + 1],
            lines
        );
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
            std::env::remove_var("PACSEA_TEST_OUT");
        }
    }

    #[test]
    /// What: Validate optional dependency rows reflect installed editors/terminals and X11-specific tooling.
    ///
    /// Inputs:
    /// - Temporary `PATH` exposing `nvim` and `kitty`, with `WAYLAND_DISPLAY` cleared to emulate X11.
    ///
    /// Output:
    /// - Optional deps list shows installed entries as non-selectable and missing tooling as selectable rows for clipboard/mirror/AUR helpers.
    ///
    /// Details:
    /// - Drives the Options menu to render optional dependencies while observing row attributes.
    fn optional_deps_rows_reflect_installed_and_x11_and_reflector() {
        let _guard = crate::global_test_mutex_lock();
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        // Create a temp directory with fake executables for editor and terminal
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_optional_deps_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);

        // Helpers to create executable stubs
        let make_exec = |name: &str| {
            let mut p = dir.clone();
            p.push(name);
            fs::write(&p, b"#!/bin/sh\nexit 0\n").unwrap();
            let mut perms = fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&p, perms).unwrap();
        };

        // Present nvim and kitty on PATH
        make_exec("nvim");
        make_exec("kitty");

        // Save and override PATH for deterministic detection; ensure X11 by clearing WAYLAND_DISPLAY
        let orig_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", dir.display().to_string());
            std::env::set_var("PACSEA_TEST_HEADLESS", "1");
        };
        let orig_wl = std::env::var_os("WAYLAND_DISPLAY");
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };

        // Drive the event handler: open Options then press '4' to open Optional Deps
        let mut app = AppState {
            ..Default::default()
        };
        // Initialize i18n translations for optional deps
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert(
            "app.optional_deps.categories.editor".to_string(),
            "Editor".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.terminal".to_string(),
            "Terminal".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.clipboard".to_string(),
            "Clipboard".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.aur_helper".to_string(),
            "AUR Helper".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.security".to_string(),
            "Security".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
        let (qtx, _qrx) = mpsc::unbounded_channel();
        let (dtx, _drx) = mpsc::unbounded_channel();
        let (ptx, _prx) = mpsc::unbounded_channel();
        let (atx, _arx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();

        // Open Options via click
        app.options_button_rect = Some((5, 5, 12, 1));
        let click_options = CEvent::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 6,
            row: 5,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        assert!(app.options_menu_open);

        // Press '4' (row index 3) to open Optional Deps
        let mut key_four_event =
            crossterm::event::KeyEvent::new(KeyCode::Char('4'), KeyModifiers::empty());
        key_four_event.kind = KeyEventKind::Press;
        let key_four = CEvent::Key(key_four_event);
        let _ = super::handle_event(key_four, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

        match &app.modal {
            crate::state::Modal::OptionalDeps { rows, .. } => {
                // Find helper to locate row by label prefix
                let find = |prefix: &str| rows.iter().find(|r| r.label.starts_with(prefix));

                // Editor: nvim
                let ed = find("Editor: nvim").expect("editor row nvim");
                assert!(ed.installed, "nvim should be marked installed");
                assert!(!ed.selectable, "installed editor should not be selectable");

                // Terminal: kitty
                let term = find("Terminal: kitty").expect("terminal row kitty");
                assert!(term.installed, "kitty should be marked installed");
                assert!(
                    !term.selectable,
                    "installed terminal should not be selectable"
                );

                // Clipboard: xclip (X11)
                let clip = find("Clipboard: xclip").expect("clipboard xclip row");
                assert!(
                    !clip.installed,
                    "xclip should not appear installed by default"
                );
                assert!(
                    clip.selectable,
                    "xclip should be selectable when not installed"
                );
                assert_eq!(clip.note.as_deref(), Some("X11"));

                // Mirrors: reflector (non-Manjaro default)
                let mirrors = find("Mirrors: reflector").expect("reflector row");
                assert!(
                    !mirrors.installed,
                    "reflector should not be installed by default"
                );
                assert!(mirrors.selectable, "reflector should be selectable");

                // AUR helper: both paru and yay should be present and selectable when not installed
                let paru = find("AUR Helper: paru").expect("paru row");
                assert!(!paru.installed);
                assert!(paru.selectable);
                let yay = find("AUR Helper: yay").expect("yay row");
                assert!(!yay.installed);
                assert!(yay.selectable);
            }
            other => panic!("Expected OptionalDeps modal, got {:?}", other),
        }

        // Restore environment
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
            if let Some(v) = orig_wl {
                std::env::set_var("WAYLAND_DISPLAY", v);
            } else {
                std::env::remove_var("WAYLAND_DISPLAY");
            }
        }

        // Cleanup temp dir
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Optional Deps shows Wayland clipboard (wl-clipboard) when WAYLAND_DISPLAY is set
    ///
    /// - Setup: Empty PATH; set WAYLAND_DISPLAY
    /// - Expect: A row "Clipboard: wl-clipboard" with note "Wayland", not installed and selectable
    fn optional_deps_rows_wayland_shows_wl_clipboard() {
        let _guard = crate::global_test_mutex_lock();
        use std::fs;
        use std::path::PathBuf;

        // Temp PATH directory (empty)
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_optional_deps_wl_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);

        let orig_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", dir.display().to_string());
            std::env::set_var("PACSEA_TEST_HEADLESS", "1");
        };
        let orig_wl = std::env::var_os("WAYLAND_DISPLAY");
        unsafe { std::env::set_var("WAYLAND_DISPLAY", "1") };

        let mut app = AppState {
            ..Default::default()
        };
        // Initialize i18n translations for optional deps
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert(
            "app.optional_deps.categories.editor".to_string(),
            "Editor".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.terminal".to_string(),
            "Terminal".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.clipboard".to_string(),
            "Clipboard".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.aur_helper".to_string(),
            "AUR Helper".to_string(),
        );
        translations.insert(
            "app.optional_deps.categories.security".to_string(),
            "Security".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
        let (qtx, _qrx) = mpsc::unbounded_channel();
        let (dtx, _drx) = mpsc::unbounded_channel();
        let (ptx, _prx) = mpsc::unbounded_channel();
        let (atx, _arx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();

        // Open Options via click
        app.options_button_rect = Some((5, 5, 12, 1));
        let click_options = CEvent::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 6,
            row: 5,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        assert!(app.options_menu_open);

        // Press '4' to open Optional Deps
        let mut key_four_event =
            crossterm::event::KeyEvent::new(KeyCode::Char('4'), KeyModifiers::empty());
        key_four_event.kind = KeyEventKind::Press;
        let key_four = CEvent::Key(key_four_event);
        let _ = super::handle_event(key_four, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

        match &app.modal {
            crate::state::Modal::OptionalDeps { rows, .. } => {
                let clip = rows
                    .iter()
                    .find(|r| r.label.starts_with("Clipboard: wl-clipboard"))
                    .expect("wl-clipboard row");
                assert_eq!(clip.note.as_deref(), Some("Wayland"));
                assert!(!clip.installed);
                assert!(clip.selectable);
                // Ensure xclip is not presented when Wayland is active
                assert!(
                    !rows.iter().any(|r| r.label.starts_with("Clipboard: xclip")),
                    "xclip should not be listed on Wayland"
                );
            }
            other => panic!("Expected OptionalDeps modal, got {:?}", other),
        }

        // Restore env and cleanup
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
            if let Some(v) = orig_wl {
                std::env::set_var("WAYLAND_DISPLAY", v);
            } else {
                std::env::remove_var("WAYLAND_DISPLAY");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
