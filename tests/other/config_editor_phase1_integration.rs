//! Integration tests for Phase 1 config-editor save flow.
//!
//! These tests drive the public event dispatcher while `AppMode::ConfigEditor`
//! is active and verify that saves update `settings.conf` on disk.

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
///
/// Details:
/// - Config path resolution uses process env vars, so this guard isolates tests
///   from developer machine config.
struct EnvGuard {
    original_home: Option<std::ffi::OsString>,
    original_xdg: Option<std::ffi::OsString>,
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
/// Inputs: None.
///
/// Output:
/// - Absolute path under system temp dir.
fn unique_test_home(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "pacsea_config_editor_phase1_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX epoch")
            .as_nanos()
    ))
}

/// What: Dispatch one key press through the public event loop.
///
/// Inputs:
/// - `app`: Mutable app state.
/// - `code`: Key code for this event.
/// - `mods`: Active modifiers.
/// - `channels`: Event dispatcher channels.
///
/// Output:
/// - None (mutates `app`).
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

/// Public event-loop channel bundle used by `handle_event`.
#[allow(clippy::struct_field_names)]
struct EventChannels {
    query_tx: mpsc::UnboundedSender<QueryInput>,
    details_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    preview_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    add_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    pkgb_tx: mpsc::UnboundedSender<pacsea::state::PackageItem>,
    comments_tx: mpsc::UnboundedSender<String>,
    pkgb_check_tx: mpsc::UnboundedSender<PkgbuildCheckRequest>,
}

/// What: Build no-op channels required by the top-level event dispatcher.
///
/// Inputs: None.
///
/// Output:
/// - Channel bundle suitable for key-driven integration tests.
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

#[test]
/// What: Save flow in config editor updates `settings.conf` for a bool key.
///
/// Inputs:
/// - Isolated HOME config with `show_install_pane = true`.
/// - Real key events through `events::handle_event` in `AppMode::ConfigEditor`.
///
/// Output:
/// - On-disk settings file contains `show_install_pane = false` after `Ctrl+S`.
///
/// Details:
/// - Exercises the integrated Phase 1 path: open settings key list, search key,
///   open popup, toggle bool, save.
fn config_editor_phase1_saves_boolean_setting_to_disk() {
    let home = unique_test_home("bool_save");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(
        &settings_path,
        "# isolated test config\nshow_install_pane = true\n",
    )
    .expect("must seed settings.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    // Enter from file list with empty query opens settings key list.
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    // Search directly for the target key so it becomes selected.
    for c in "show_install_pane".chars() {
        send_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE, &channels);
    }
    // Enter on filtered key list opens popup; Space toggles bool; Ctrl+S saves.
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Char(' '), KeyModifiers::NONE, &channels);
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&settings_path).expect("must read updated settings.conf");
    assert!(
        after.contains("show_install_pane = false"),
        "expected toggled bool in settings.conf, got: {after}"
    );
    let status = app
        .config_editor_state
        .status
        .as_deref()
        .unwrap_or_default()
        .to_string();
    assert!(
        status.contains("Saved show_install_pane"),
        "expected save status, got: {status}"
    );
}

#[test]
/// What: Save flow in config editor updates an enum setting on disk.
///
/// Inputs:
/// - Isolated HOME config with `sort_mode = best_matches`.
/// - Real key events through `events::handle_event` in `AppMode::ConfigEditor`.
///
/// Output:
/// - On-disk settings file contains `sort_mode = alphabetical` after save.
///
/// Details:
/// - Exercises non-bool Phase 1 popup flow using enum cycling (`Down`) then `Ctrl+S`.
fn config_editor_phase1_saves_enum_setting_to_disk() {
    let home = unique_test_home("enum_save");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(&settings_path, "sort_mode = best_matches\n").expect("must seed settings.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for c in "sort_mode".chars() {
        send_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Down, KeyModifiers::NONE, &channels);
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&settings_path).expect("must read updated settings.conf");
    assert!(
        after.contains("sort_mode = alphabetical"),
        "expected updated enum value in settings.conf, got: {after}"
    );
}

#[test]
/// What: Dry-run config-editor save reports action but leaves settings file untouched.
///
/// Inputs:
/// - Isolated HOME config with `show_install_pane = true`.
/// - `AppState { dry_run: true }`.
///
/// Output:
/// - Settings file remains unchanged after `Ctrl+S`.
/// - Status line reports dry-run save.
fn config_editor_phase1_dry_run_save_does_not_write_disk() {
    let home = unique_test_home("dry_run");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(&settings_path, "show_install_pane = true\n").expect("must seed settings.conf");
    let before = fs::read_to_string(&settings_path).expect("must read seed settings");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        dry_run: true,
        ..AppState::default()
    };
    let channels = build_event_channels();

    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for c in "show_install_pane".chars() {
        send_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Char(' '), KeyModifiers::NONE, &channels);
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&settings_path).expect("must read settings after dry-run");
    assert_eq!(before, after, "dry-run save must not modify settings.conf");
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
