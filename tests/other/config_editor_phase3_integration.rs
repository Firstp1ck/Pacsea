//! Integration tests for config-editor Phase 3 (theme tab) and Phase 4
//! (polish: stale-file warning, effective-config export).
//!
//! These tests drive the public event dispatcher while `AppMode::ConfigEditor`
//! is active and verify that theme edits validate the whole proposed file
//! before touching `theme.conf`, that concurrent on-disk edits trigger an
//! overwrite warning, and that `Ctrl+E` exports effective values.

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
        "pacsea_config_editor_phase3_{tag}_{}_{}",
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

/// What: Type a query string into the editor's middle pane.
fn type_query(app: &mut AppState, channels: &EventChannels, query: &str) {
    for c in query.chars() {
        send_key(app, KeyCode::Char(c), KeyModifiers::NONE, channels);
    }
}

/// What: Navigate to the theme file row (index 2) and open its key list.
fn enter_theme_file(app: &mut AppState, channels: &EventChannels) {
    send_key(app, KeyCode::Down, KeyModifiers::NONE, channels);
    send_key(app, KeyCode::Down, KeyModifiers::NONE, channels);
    send_key(app, KeyCode::Enter, KeyModifiers::NONE, channels);
}

/// What: Load the shipped complete theme config for seeding isolated homes.
fn shipped_theme_content() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("theme.conf");
    fs::read_to_string(&path).expect("shipped config/theme.conf must be readable")
}

/// What: Read the current status line from the editor state.
fn editor_status(app: &AppState) -> String {
    app.config_editor_state
        .status
        .as_deref()
        .unwrap_or_default()
        .to_string()
}

#[test]
/// What: Theme color save validates and persists to `theme.conf`, preserving other lines.
fn config_editor_phase3_saves_theme_color_to_disk() {
    let home = unique_test_home("theme_save");
    let _guard = EnvGuard::new(home.clone());
    let theme_path = home.join(".config").join("pacsea").join("theme.conf");
    fs::write(&theme_path, shipped_theme_content()).expect("must seed theme.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    enter_theme_file(&mut app, &channels);
    type_query(&mut app, &channels, "semantic_error");
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for _ in 0..16 {
        send_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE, &channels);
    }
    type_query(&mut app, &channels, "#123456");
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&theme_path).expect("must read updated theme.conf");
    assert!(
        after.contains("semantic_error = #123456"),
        "expected updated color in theme.conf, got: {after}"
    );
    assert!(
        after.contains("background_base = #1e1e2e"),
        "unrelated theme lines must be preserved, got: {after}"
    );
    let status = editor_status(&app);
    assert!(
        status.contains("Saved semantic_error"),
        "expected save status, got: {status}"
    );
}

#[test]
/// What: A theme edit that would leave the file invalid is rejected without writing.
///
/// Details:
/// - Seeds an incomplete `theme.conf`; the single-key patch proposal is then
///   still missing required keys, so whole-file validation must reject the
///   save and leave disk untouched.
fn config_editor_phase3_invalid_theme_rejected_without_write() {
    let home = unique_test_home("theme_invalid");
    let _guard = EnvGuard::new(home.clone());
    let theme_path = home.join(".config").join("pacsea").join("theme.conf");
    fs::write(&theme_path, "background_base = #1e1e2e\n").expect("must seed theme.conf");
    let before = fs::read_to_string(&theme_path).expect("read seed");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    enter_theme_file(&mut app, &channels);
    type_query(&mut app, &channels, "semantic_error");
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for _ in 0..16 {
        send_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE, &channels);
    }
    type_query(&mut app, &channels, "#123456");
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&theme_path).expect("read after rejected save");
    assert_eq!(before, after, "invalid theme must not modify theme.conf");
    let status = editor_status(&app);
    assert!(
        status.contains("Invalid theme rejected"),
        "expected rejection status, got: {status}"
    );
    assert!(
        app.config_editor_state.popup.is_some(),
        "popup must stay open after a rejected save"
    );
}

#[test]
/// What: Dry-run theme save reports the action but leaves `theme.conf` untouched.
fn config_editor_phase3_dry_run_theme_save_does_not_write_disk() {
    let home = unique_test_home("theme_dry_run");
    let _guard = EnvGuard::new(home.clone());
    let theme_path = home.join(".config").join("pacsea").join("theme.conf");
    fs::write(&theme_path, shipped_theme_content()).expect("must seed theme.conf");
    let before = fs::read_to_string(&theme_path).expect("read seed");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        dry_run: true,
        ..AppState::default()
    };
    let channels = build_event_channels();

    enter_theme_file(&mut app, &channels);
    type_query(&mut app, &channels, "semantic_error");
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for _ in 0..16 {
        send_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE, &channels);
    }
    type_query(&mut app, &channels, "#123456");
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&theme_path).expect("read after dry-run");
    assert_eq!(before, after, "dry-run save must not modify theme.conf");
    let status = editor_status(&app);
    assert!(
        status.contains("Dry-run: would update"),
        "expected dry-run status, got: {status}"
    );
}

#[test]
/// What: A file changed on disk while editing warns first, then overwrites on repeat save.
fn config_editor_phase4_stale_file_warns_then_overwrites() {
    let home = unique_test_home("stale_mtime");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(&settings_path, "sort_mode = best_matches\n").expect("must seed settings.conf");
    let opened_mtime = fs::metadata(&settings_path)
        .and_then(|m| m.modified())
        .expect("seed mtime");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    // Open settings.conf (file cursor starts at 0) and the sort_mode popup.
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    type_query(&mut app, &channels, "sort_mode");
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    assert!(app.config_editor_state.popup.is_some(), "popup must open");

    // Simulate an external edit; retry until the mtime is observably newer to
    // tolerate coarse filesystem timestamp granularity.
    let external = "sort_mode = official_first\n# external edit\n";
    for _ in 0..40 {
        fs::write(&settings_path, external).expect("external edit");
        let new_mtime = fs::metadata(&settings_path)
            .and_then(|m| m.modified())
            .expect("external mtime");
        if new_mtime != opened_mtime {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Pick a different enum value so the save is a real change.
    send_key(&mut app, KeyCode::Down, KeyModifiers::NONE, &channels);
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );
    let status = editor_status(&app);
    assert!(
        status.contains("changed on disk"),
        "expected stale-file warning, got: {status}"
    );
    let mid = fs::read_to_string(&settings_path).expect("read after warning");
    assert_eq!(mid, external, "warned save must not modify the file");

    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );
    let status = editor_status(&app);
    assert!(
        status.contains("Saved sort_mode"),
        "expected save status after explicit overwrite, got: {status}"
    );
    let after = fs::read_to_string(&settings_path).expect("read after overwrite");
    assert!(
        after.contains("# external edit"),
        "line-preserving patch must keep the external comment, got: {after}"
    );
    assert!(
        !after.contains("sort_mode = official_first"),
        "sort_mode must be rewritten by the overwrite, got: {after}"
    );
}

#[test]
/// What: `Ctrl+E` exports effective values for the selected file with secrets redacted.
fn config_editor_phase4_export_writes_effective_values_with_redaction() {
    let home = unique_test_home("export");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(
        &settings_path,
        "sort_mode = alphabetical\nvirustotal_api_key = super_secret_value\n",
    )
    .expect("must seed settings.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();

    // Open settings.conf, then export.
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    send_key(
        &mut app,
        KeyCode::Char('e'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let export_path = home
        .join(".config")
        .join("pacsea")
        .join("lists")
        .join("effective_settings.conf");
    let exported = fs::read_to_string(&export_path).expect("export file must exist");
    assert!(
        exported.contains("sort_mode = alphabetical"),
        "export must contain effective values, got: {exported}"
    );
    assert!(
        exported.contains("virustotal_api_key = [REDACTED]"),
        "secrets must be redacted in exports, got: {exported}"
    );
    assert!(
        !exported.contains("super_secret_value"),
        "raw secret must never appear in exports"
    );
    let status = editor_status(&app);
    assert!(
        status.contains("Exported"),
        "expected export status, got: {status}"
    );
}
