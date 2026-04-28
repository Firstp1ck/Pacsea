//! Integration tests for Phase 2 config-editor save flow on `keybinds.conf`.
//!
//! These tests drive the public event dispatcher while `AppMode::ConfigEditor`
//! is active and verify that keybind edits update `keybinds.conf` on disk and
//! that conflict detection rejects collisions before any write happens.

#![cfg(not(target_os = "windows"))]

use crossterm::event::{Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use pacsea::events::handle_event;
use pacsea::state::{AppState, PkgbuildCheckRequest, QueryInput, types::AppMode};
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// What: Guard process-wide HOME/XDG overrides for config-editor integration tests.
///
/// Inputs:
/// - `home_root`: Temporary directory used as test HOME root.
///
/// Output:
/// - Guard that restores original `HOME` and `XDG_CONFIG_HOME` on drop.
struct EnvGuard {
    /// Original `HOME` value, restored on drop.
    original_home: Option<std::ffi::OsString>,
    /// Original `XDG_CONFIG_HOME` value, restored on drop.
    original_xdg: Option<std::ffi::OsString>,
    /// Temp HOME root cleaned up on drop.
    home_root: PathBuf,
}

impl EnvGuard {
    /// What: Apply isolated HOME/XDG environment for a test run.
    ///
    /// Inputs:
    /// - `home_root`: Temporary HOME path.
    ///
    /// Output:
    /// - Initialized guard.
    fn new(home_root: PathBuf) -> Self {
        let original_home = std::env::var_os("HOME");
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let config_root = home_root.join(".config").join("pacsea");
        fs::create_dir_all(&config_root).expect("must create isolated config root");
        unsafe {
            std::env::set_var("HOME", &home_root);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        Self {
            original_home,
            original_xdg,
            home_root,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(home) = self.original_home.as_ref() {
                std::env::set_var("HOME", home);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(xdg) = self.original_xdg.as_ref() {
                std::env::set_var("XDG_CONFIG_HOME", xdg);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
        let _ = fs::remove_dir_all(&self.home_root);
    }
}

/// What: Build a unique temporary HOME directory for an integration test.
///
/// Inputs:
/// - `tag`: Short identifier embedded in the directory name.
///
/// Output:
/// - Absolute path under system temp dir.
fn unique_test_home(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "pacsea_config_editor_phase2_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos()
    ))
}

/// What: Public event-loop channel bundle used by `handle_event`.
#[allow(clippy::struct_field_names)]
struct EventChannels {
    /// Search query channel sender.
    query_tx: mpsc::UnboundedSender<QueryInput>,
    /// Details preview channel sender.
    details_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    /// Preview channel sender.
    preview_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    /// Add-to-install channel sender.
    add_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    /// PKGBUILD channel sender.
    pkgb_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    /// Comments channel sender.
    comments_tx: mpsc::UnboundedSender<String>,
    /// PKGBUILD-check channel sender.
    pkgb_check_tx: mpsc::UnboundedSender<PkgbuildCheckRequest>,
}

/// What: Build no-op channels required by the top-level event dispatcher.
fn build_event_channels() -> EventChannels {
    let (query_tx, _query_rx) = mpsc::unbounded_channel::<QueryInput>();
    let (details_tx, _details_rx) = mpsc::unbounded_channel::<pacsea::state::PackageItem>();
    let (preview_tx, _preview_rx) = mpsc::unbounded_channel::<pacsea::state::PackageItem>();
    let (add_tx, _add_rx) = mpsc::unbounded_channel::<pacsea::state::PackageItem>();
    let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<pacsea::state::PackageItem>();
    let (comments_tx, _comments_rx) = mpsc::unbounded_channel::<String>();
    let (pkgb_check_tx, _pkgb_check_rx) = mpsc::unbounded_channel::<PkgbuildCheckRequest>();
    EventChannels {
        query_tx,
        details_tx,
        preview_tx,
        add_tx,
        pkgb_tx,
        comments_tx,
        pkgb_check_tx,
    }
}

/// What: Dispatch one key press through the public event loop.
fn send_key(app: &mut AppState, code: KeyCode, mods: KeyModifiers, channels: &EventChannels) {
    let mut key = KeyEvent::new(code, mods);
    key.kind = KeyEventKind::Press;
    let ev = CEvent::Key(key);
    let _ = handle_event(
        &ev,
        app,
        &channels.query_tx,
        &channels.details_tx,
        &channels.preview_tx,
        &channels.add_tx,
        &channels.pkgb_tx,
        &channels.comments_tx,
        &channels.pkgb_check_tx,
    );
}

/// What: Navigate to the keybind file row and open its key list.
fn enter_keybinds_file(app: &mut AppState, channels: &EventChannels) {
    // File list cursor starts at 0 (settings); press Down once to reach
    // keybinds.conf, then Enter on empty query opens its key list.
    send_key(app, KeyCode::Down, KeyModifiers::NONE, channels);
    send_key(app, KeyCode::Enter, KeyModifiers::NONE, channels);
}

/// What: Type a query string into the editor's middle pane.
fn type_query(app: &mut AppState, channels: &EventChannels, query: &str) {
    for c in query.chars() {
        send_key(app, KeyCode::Char(c), KeyModifiers::NONE, channels);
    }
}

#[test]
/// What: Save flow in config editor updates a keybind value in `keybinds.conf`.
fn config_editor_phase2_saves_keybind_to_disk() {
    let home = unique_test_home("save");
    let _guard = EnvGuard::new(home.clone());
    let kb_path = home.join(".config").join("pacsea").join("keybinds.conf");
    fs::write(&kb_path, "keybind_reload_config = CTRL+R\n").expect("must seed keybinds.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    enter_keybinds_file(&mut app, &channels);
    type_query(&mut app, &channels, "keybind_reload_config");
    // Open popup, edit text buffer to "F9", save.
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    // Clear the prefilled buffer ("Ctrl+r") and type the new value.
    for _ in 0..16 {
        send_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE, &channels);
    }
    type_query(&mut app, &channels, "F9");
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&kb_path).expect("must read updated keybinds.conf");
    assert!(
        after.contains("keybind_reload_config = F9"),
        "expected updated keybind in keybinds.conf, got: {after}"
    );
    let status = app
        .config_editor_state
        .status
        .as_deref()
        .unwrap_or_default()
        .to_string();
    assert!(
        status.contains("Saved keybind_reload_config"),
        "expected save status, got: {status}"
    );
}

#[test]
/// What: Dry-run keybind save reports action but leaves `keybinds.conf` untouched.
fn config_editor_phase2_dry_run_save_does_not_write_disk() {
    let home = unique_test_home("dry_run");
    let _guard = EnvGuard::new(home.clone());
    let kb_path = home.join(".config").join("pacsea").join("keybinds.conf");
    fs::write(&kb_path, "keybind_reload_config = CTRL+R\n").expect("must seed keybinds.conf");
    let before = fs::read_to_string(&kb_path).expect("read seed");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        dry_run: true,
        ..AppState::default()
    };
    let channels = build_event_channels();

    enter_keybinds_file(&mut app, &channels);
    type_query(&mut app, &channels, "keybind_reload_config");
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for _ in 0..16 {
        send_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE, &channels);
    }
    type_query(&mut app, &channels, "F9");
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&kb_path).expect("read after dry-run");
    assert_eq!(before, after, "dry-run save must not modify keybinds.conf");
    let status = app
        .config_editor_state
        .status
        .as_deref()
        .unwrap_or_default()
        .to_string();
    assert!(
        status.contains("Dry-run: would update"),
        "expected dry-run status, got: {status}"
    );
}

#[test]
/// What: Conflict detection blocks saves that would collide within the same scope.
///
/// Details:
/// - Uses `keybind_search_normal_select_left` (default `h`) to flag the
///   conflict when `keybind_search_normal_select_right` is rebound to `h`.
/// - These actions live in the `search_normal` scope and their default chords
///   are not intercepted by any global keybind handler, so `h` reaches the
///   editor popup buffer reliably during a unit-test key drive.
fn config_editor_phase2_conflict_blocks_save_and_leaves_file_unchanged() {
    let home = unique_test_home("conflict");
    let _guard = EnvGuard::new(home.clone());
    let kb_path = home.join(".config").join("pacsea").join("keybinds.conf");
    // Empty keybinds.conf keeps default keymap so `select_left = h` is in
    // effect when conflict detection runs.
    fs::write(&kb_path, "# isolated keybinds.conf\n").expect("must seed keybinds.conf");
    let before = fs::read_to_string(&kb_path).expect("read seed");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    enter_keybinds_file(&mut app, &channels);
    type_query(&mut app, &channels, "keybind_search_normal_select_right");
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    // Buffer prefilled with current value (default `l`); clear and retype.
    for _ in 0..8 {
        send_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE, &channels);
    }
    type_query(&mut app, &channels, "h");
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&kb_path).expect("read after conflict");
    assert_eq!(before, after, "conflict must not modify keybinds.conf");
    let status = app
        .config_editor_state
        .status
        .as_deref()
        .unwrap_or_default()
        .to_string();
    assert!(
        status.contains("Conflict") && status.contains("keybind_search_normal_select_left"),
        "expected conflict status naming keybind_search_normal_select_left, got: {status}"
    );
}
