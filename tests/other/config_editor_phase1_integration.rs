//! Integration tests for Phase 1 config-editor save flow.
//!
//! These tests drive the public event dispatcher while `AppMode::ConfigEditor`
//! is active and verify that saves update `settings.conf` on disk.

#![cfg(not(target_os = "windows"))]

use crate::env_guard::EnvGuard;
use crossterm::event::{Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use pacsea::events::handle_event;
use pacsea::state::{AppState, PkgbuildCheckRequest, QueryInput, types::AppMode};
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc;

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
/// What: Save a shipped locale through the enum popup to settings.conf.
///
/// Inputs:
/// - Isolated settings.conf with locale = en-US and keyboard events selecting de-DE.
///
/// Output:
/// - The saved settings.conf contains locale = de-DE.
///
/// Details:
/// - The popup must be an enum, not a text input, and Down selects the next choice.
fn config_editor_phase1_saves_locale_enum_to_disk() {
    let home = unique_test_home("locale_save");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(&settings_path, "locale = en-US\n").expect("must seed settings.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();
    for c in "locale".chars() {
        send_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    assert!(matches!(
        app.config_editor_state
            .popup
            .as_ref()
            .map(|popup| &popup.kind),
        Some(pacsea::state::config_editor::EditPopupKind::Enum { index: 1, .. })
    ));
    send_key(&mut app, KeyCode::Down, KeyModifiers::NONE, &channels);
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&settings_path).expect("must read updated settings.conf");
    assert!(
        after.contains("locale = de-DE"),
        "unexpected settings.conf: {after}"
    );
}

#[test]
/// What: Save the locale auto-detect option through keyboard navigation.
///
/// Inputs:
/// - Isolated settings.conf with locale = de-DE.
/// - Two Up events from the stored choice and Ctrl+S.
///
/// Output:
/// - The saved locale line has an empty value.
///
/// Details:
/// - The empty option round-trips as the runtime's system-locale auto-detect value.
fn config_editor_phase1_saves_auto_detect_locale_to_disk() {
    let home = unique_test_home("locale_auto_save");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(&settings_path, "locale = de-DE\n").expect("must seed settings.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();
    for c in "locale".chars() {
        send_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Up, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Up, KeyModifiers::NONE, &channels);
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&settings_path).expect("must read updated settings.conf");
    assert!(
        after.lines().any(|line| line
            .strip_prefix("locale =")
            .is_some_and(|value| value.trim().is_empty())),
        "unexpected settings.conf: {after}"
    );
    assert!(!after.contains("de-DE"));
}

#[test]
/// What: Save a noncanonical locale spelling as its preselected canonical choice.
///
/// Inputs:
/// - Isolated settings.conf with `locale = de_DE`; Ctrl+S without cycling.
///
/// Output:
/// - The saved locale is de-DE.
///
/// Details:
/// - Popup selection normalizes the candidate, while retaining the raw original.
fn config_editor_phase1_normalizes_locale_on_save() {
    let home = unique_test_home("locale_normalized_save");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config").join("pacsea").join("settings.conf");
    fs::write(&settings_path, "locale = de_DE\n").expect("must seed settings.conf");

    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();
    for c in "locale".chars() {
        send_key(&mut app, KeyCode::Char(c), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    let popup = app
        .config_editor_state
        .popup
        .as_ref()
        .expect("locale popup");
    assert!(matches!(
        popup.kind,
        pacsea::state::config_editor::EditPopupKind::Enum { index: 2, .. }
    ));
    assert_eq!(popup.original, "de_DE");
    send_key(
        &mut app,
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
        &channels,
    );

    let after = fs::read_to_string(&settings_path).expect("must read updated settings.conf");
    assert!(
        after.contains("locale = de-DE"),
        "unexpected settings.conf: {after}"
    );
    assert!(!after.contains("de_DE"));
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

/// Popup fields own printable help and uppercase menu shortcuts.
#[test]
fn config_editor_popup_printable_precedence() {
    let home = unique_test_home("printable");
    let _guard = EnvGuard::new(home);
    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let entry = pacsea::theme::find_setting("clipboard_suffix").expect("setting");
    app.config_editor_state.popup = Some(pacsea::state::EditPopupState::from_current(entry, ""));
    let channels = build_event_channels();
    for (ch, mods) in [
        ('?', KeyModifiers::NONE),
        ('C', KeyModifiers::SHIFT),
        ('O', KeyModifiers::SHIFT),
        ('P', KeyModifiers::SHIFT),
    ] {
        send_key(&mut app, KeyCode::Char(ch), mods, &channels);
    }
    assert_eq!(
        app.config_editor_state
            .popup
            .as_ref()
            .expect("popup")
            .buffer,
        "?COP"
    );
    assert!(matches!(app.modal, pacsea::state::Modal::None));
    assert!(!app.config_menu_open && !app.options_menu_open && !app.panels_menu_open);
}

/// Tabs, Starred editing, recents, and the Esc ladder work through public routing.
#[test]
fn config_editor_tabs_starred_recents_and_escape() {
    use pacsea::state::config_editor::ConfigEditorTab;
    let home = unique_test_home("tabs_starred");
    let _guard = EnvGuard::new(home);
    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        dry_run: true,
        ..AppState::default()
    };
    app.config_editor_state.bookmarked_keys.clear();
    app.config_editor_state.recent_queries.clear();
    let channels = build_event_channels();
    for ch in "show_install_pane".chars() {
        send_key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE, &channels);
    }
    send_key(
        &mut app,
        KeyCode::Char('b'),
        KeyModifiers::CONTROL,
        &channels,
    );
    assert!(app.config_editor_state.is_bookmarked("show_install_pane"));
    send_key(&mut app, KeyCode::BackTab, KeyModifiers::SHIFT, &channels);
    assert_eq!(
        app.config_editor_state.active_tab(),
        ConfigEditorTab::Starred
    );
    assert_eq!(app.config_editor_state.query, "show_install_pane");
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    assert!(app.config_editor_state.popup.is_some());
    send_key(&mut app, KeyCode::Char(' '), KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    assert!(app.config_editor_state.popup.is_none());
    send_key(
        &mut app,
        KeyCode::Char('r'),
        KeyModifiers::CONTROL,
        &channels,
    );
    assert_eq!(app.config_editor_state.recent_dropdown, Some(0));
    send_key(&mut app, KeyCode::Esc, KeyModifiers::NONE, &channels);
    assert_ne!(app.config_editor_state.query.len(), 0);
    send_key(&mut app, KeyCode::Esc, KeyModifiers::NONE, &channels);
    assert_eq!(app.config_editor_state.query.len(), 0);
    send_key(&mut app, KeyCode::Esc, KeyModifiers::NONE, &channels);
    assert_eq!(app.app_mode, AppMode::Package);
}

/// Text suggestion selection saves its effective value and CSV completion preserves earlier tokens.
#[test]
fn config_editor_text_suggestion_and_comma_completion_save() {
    let home = unique_test_home("suggestion_save");
    let _guard = EnvGuard::new(home.clone());
    let settings_path = home.join(".config/pacsea/settings.conf");
    fs::write(
        &settings_path,
        "preferred_terminal = kitty\nselected_countries = Germany\n",
    )
    .expect("seed");
    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();
    for ch in "preferred_terminal".chars() {
        send_key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for _ in 0..16 {
        send_key(&mut app, KeyCode::Backspace, KeyModifiers::NONE, &channels);
    }
    for ch in "ala".chars() {
        send_key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE, &channels);
    }
    assert_eq!(
        app.config_editor_state
            .popup
            .as_ref()
            .expect("popup")
            .highlight,
        None
    );
    send_key(&mut app, KeyCode::Down, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    assert!(
        fs::read_to_string(&settings_path)
            .expect("saved")
            .contains("preferred_terminal = alacritty")
    );
    send_key(&mut app, KeyCode::Esc, KeyModifiers::NONE, &channels);
    for ch in "selected_countries".chars() {
        send_key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    for ch in ", Fr".chars() {
        send_key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Tab, KeyModifiers::NONE, &channels);
    assert_eq!(
        app.config_editor_state
            .popup
            .as_ref()
            .expect("popup")
            .buffer,
        "Germany, France"
    );
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    assert!(
        fs::read_to_string(settings_path)
            .expect("saved")
            .contains("selected_countries = Germany, France")
    );
}

/// Restart-bound saves retain the existing status prefix and append the restart note.
#[test]
fn config_editor_restart_note_after_save() {
    let home = unique_test_home("restart_note");
    let _guard = EnvGuard::new(home.clone());
    fs::write(
        home.join(".config/pacsea/settings.conf"),
        "pi_scan_thinking = low\n",
    )
    .expect("seed");
    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    let channels = build_event_channels();
    for ch in "pi_scan_thinking".chars() {
        send_key(&mut app, KeyCode::Char(ch), KeyModifiers::NONE, &channels);
    }
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Down, KeyModifiers::NONE, &channels);
    send_key(&mut app, KeyCode::Enter, KeyModifiers::NONE, &channels);
    let status = app
        .config_editor_state
        .status
        .as_deref()
        .expect("saved status");
    assert!(status.starts_with("Saved pi_scan_thinking → "));
    assert!(status.ends_with("(restart required)"));
}

/// What: Persistence loads explicitly at startup and when reopening the editor.
///
/// Inputs: Isolated JSON lists with whitespace, duplicates, and an empty entry.
///
/// Output: Normalized history and bookmarks at both initialization boundaries.
///
/// Details: Cheap default state neither creates the lists directory nor reads existing lists.
#[test]
fn config_editor_explicit_persistence_loading() {
    let home = unique_test_home("persisted_lists");
    let _guard = EnvGuard::new(home.clone());
    let lists = home.join(".config/pacsea/lists");
    let transient = pacsea::state::ConfigEditorState::default();
    assert!(!lists.exists());
    assert!(transient.recent_queries_path.as_os_str().is_empty());
    fs::create_dir_all(&lists).expect("lists directory");
    fs::write(
        lists.join("config_editor_recent_searches.json"),
        r#"[" mirror ", "", "mirror", "news"]"#,
    )
    .expect("recent list");
    fs::write(
        lists.join("config_editor_bookmarks.json"),
        r#"[" sort_mode ", "sort_mode"]"#,
    )
    .expect("bookmarks");
    let transient = pacsea::state::ConfigEditorState::default();
    assert_eq!(transient.recent_queries.len(), 0);
    assert_eq!(transient.bookmarked_keys.len(), 0);
    let mut app = AppState::default();
    assert_eq!(app.config_editor_state.recent_queries, ["mirror", "news"]);
    assert_eq!(app.config_editor_state.bookmarked_keys, ["sort_mode"]);
    assert_eq!(
        app.config_editor_state.recent_queries_path,
        lists.join("config_editor_recent_searches.json")
    );
    app.config_editor_state.recent_queries.clear();
    app.config_menu_open = true;
    let channels = build_event_channels();
    send_key(&mut app, KeyCode::Char('5'), KeyModifiers::NONE, &channels);
    assert_eq!(app.app_mode, AppMode::ConfigEditor);
    assert_eq!(app.config_editor_state.recent_queries, ["mirror", "news"]);
    assert_eq!(app.config_editor_state.bookmarked_keys, ["sort_mode"]);
}
