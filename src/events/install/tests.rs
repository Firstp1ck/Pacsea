use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

use super::handle_install_key;

/// What: Produce a baseline `AppState` tailored for install-pane tests without repeating setup boilerplate.
///
/// Inputs:
/// - None (relies on `Default::default()` for deterministic initial state).
///
/// Output:
/// - Fresh `AppState` ready for mutation inside individual test cases.
///
/// Details:
/// - Keeps test bodies concise while ensuring each case starts from a clean copy.
fn new_app() -> AppState {
    AppState {
        ..Default::default()
    }
}

#[test]
/// What: Confirm pressing Enter opens the preflight modal when installs are pending.
///
/// Inputs:
/// - Install list seeded with a single package and `Enter` key event.
///
/// Output:
/// - Modal transitions to `Preflight` with one item, `Install` action, and `Summary` tab active.
///
/// Details:
/// - Uses mock channels to satisfy handler requirements without observing downstream messages.
/// - Sets up temporary config directory to ensure `skip_preflight = false` regardless of user config.
fn install_enter_opens_confirm_install() {
    let _guard = crate::theme::test_mutex().lock().unwrap();
    let orig_home = std::env::var_os("HOME");
    let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let base = std::env::temp_dir().join(format!(
        "pacsea_test_install_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cfg = base.join(".config").join("pacsea");
    let _ = std::fs::create_dir_all(&cfg);
    unsafe { std::env::set_var("HOME", base.display().to_string()) };
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };

    // Write settings.conf with skip_preflight = false
    let settings_path = cfg.join("settings.conf");
    std::fs::write(&settings_path, "skip_preflight = false\n").unwrap();

    let mut app = new_app();
    app.install_list = vec![PackageItem {
        name: "rg".into(),
        version: "1".into(),
        description: String::new(),
        source: crate::state::Source::Aur,
        popularity: None,
    }];
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let _ = handle_install_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        &mut app,
        &dtx,
        &ptx,
        &atx,
    );
    match app.modal {
        crate::state::Modal::Preflight {
            ref items,
            action,
            tab,
            summary: _,
            summary_scroll: _,
            header_chips: _,
            dependency_info: _,
            dep_selected: _,
            dep_tree_expanded: _,
            deps_error: _,
            file_info: _,
            file_selected: _,
            file_tree_expanded: _,
            files_error: _,
            service_info: _,
            service_selected: _,
            services_loaded: _,
            services_error: _,
            sandbox_info: _,
            sandbox_selected: _,
            sandbox_tree_expanded: _,
            sandbox_loaded: _,
            sandbox_error: _,
            selected_optdepends: _,
            cascade_mode: _,
        } => {
            assert_eq!(items.len(), 1);
            assert_eq!(action, crate::state::PreflightAction::Install);
            assert_eq!(tab, crate::state::PreflightTab::Summary);
        }
        _ => panic!("Preflight modal not opened"),
    }

    unsafe {
        if let Some(v) = orig_home {
            std::env::set_var("HOME", v);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(v) = orig_xdg {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
/// What: Placeholder ensuring default behaviour still opens the preflight modal when `skip_preflight` remains false.
///
/// Inputs:
/// - Single official package queued for install with `Enter` key event.
///
/// Output:
/// - Modal remains `Preflight`, matching current default configuration.
///
/// Details:
/// - Documents intent for future skip-preflight support while asserting existing flow stays intact.
/// - Sets up temporary config directory to ensure `skip_preflight = false` regardless of user config.
fn install_enter_bypasses_preflight_with_skip_flag() {
    let _guard = crate::theme::test_mutex().lock().unwrap();
    let orig_home = std::env::var_os("HOME");
    let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
    let base = std::env::temp_dir().join(format!(
        "pacsea_test_install_skip_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cfg = base.join(".config").join("pacsea");
    let _ = std::fs::create_dir_all(&cfg);
    unsafe { std::env::set_var("HOME", base.display().to_string()) };
    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };

    // Write settings.conf with skip_preflight = false
    let settings_path = cfg.join("settings.conf");
    std::fs::write(&settings_path, "skip_preflight = false\n").unwrap();

    // Verify the setting is false
    assert!(
        !crate::theme::settings().skip_preflight,
        "skip_preflight unexpectedly true by default"
    );

    let mut app = new_app();
    app.install_list = vec![PackageItem {
        name: "ripgrep".into(),
        version: "1".into(),
        description: String::new(),
        source: crate::state::Source::Official {
            repo: "core".into(),
            arch: "x86_64".into(),
        },
        popularity: None,
    }];
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let _ = handle_install_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        &mut app,
        &dtx,
        &ptx,
        &atx,
    );
    // Behavior remains preflight when flag false; placeholder ensures future refactor retains compatibility.
    match app.modal {
        crate::state::Modal::Preflight {
            summary: _,
            summary_scroll: _,
            header_chips: _,
            dependency_info: _,
            dep_selected: _,
            dep_tree_expanded: _,
            file_info: _,
            file_selected: _,
            file_tree_expanded: _,
            files_error: _,
            service_info: _,
            service_selected: _,
            services_loaded: _,
            cascade_mode: _,
            ..
        } => {}
        _ => panic!("Expected Preflight when skip_preflight=false"),
    }

    unsafe {
        if let Some(v) = orig_home {
            std::env::set_var("HOME", v);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(v) = orig_xdg {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
/// What: Verify the Delete key removes the selected install item.
///
/// Inputs:
/// - Install list with two entries, selection on the first, and `Delete` key event.
///
/// Output:
/// - List shrinks to one entry, confirming removal logic.
///
/// Details:
/// - Channels are stubbed to satisfy handler signature while focusing on list mutation.
fn install_delete_removes_item() {
    let mut app = new_app();
    app.install_list = vec![
        PackageItem {
            name: "rg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        },
        PackageItem {
            name: "fd".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        },
    ];
    app.install_state.select(Some(0));
    let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
    let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
    let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
    let _ = handle_install_key(
        KeyEvent::new(KeyCode::Delete, KeyModifiers::empty()),
        &mut app,
        &dtx,
        &ptx,
        &atx,
    );
    assert_eq!(app.install_list.len(), 1);
}
