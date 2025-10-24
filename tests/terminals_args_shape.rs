#![cfg(not(target_os = "windows"))]

use crossterm::event::{
    Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use pacsea as crate_root;

fn write_fake(term_name: &str, dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let mut out_path = dir.to_path_buf();
    out_path.push("args.txt");
    let mut term_path = dir.to_path_buf();
    term_path.push(term_name);
    let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
    std::fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = std::fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&term_path, perms).unwrap();
    (term_path, out_path)
}

#[test]
/// What: tilix arg shape uses "--", "bash", "-lc"
///
/// - Input: Fake tilix on PATH and SystemUpdate Enter
/// - Output: First three args are ["--", "bash", "-lc"]
fn ui_options_update_system_enter_triggers_tilix_args_shape() {
    use std::path::PathBuf;
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_tilix_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let (_term_path, out_path) = write_fake("tilix", &dir);
    let orig_path = std::env::var_os("PATH");
    let combined_path = match std::env::var("PATH") {
        Ok(p) => format!("{}:{}", dir.display(), p),
        Err(_) => dir.display().to_string(),
    };
    unsafe {
        std::env::set_var("PATH", combined_path);
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let body = std::fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");
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
/// What: mate-terminal arg shape uses "--", "bash", "-lc"
///
/// - Input: Fake mate-terminal on PATH and SystemUpdate Enter
/// - Output: First three args are ["--", "bash", "-lc"]
fn ui_options_update_system_enter_triggers_mate_terminal_args_shape() {
    use std::path::PathBuf;
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_mate_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let (_term_path, out_path) = write_fake("mate-terminal", &dir);
    let orig_path = std::env::var_os("PATH");
    let combined_path = match std::env::var("PATH") {
        Ok(p) => format!("{}:{}", dir.display(), p),
        Err(_) => dir.display().to_string(),
    };
    unsafe {
        std::env::set_var("PATH", combined_path);
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let body = std::fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");
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
/// What: gnome-terminal arg shape uses "--", "bash", "-lc"
///
/// - Input: Fake gnome-terminal PATH isolated; SystemUpdate Enter
/// - Output: First three args are ["--", "bash", "-lc"]
fn ui_options_update_system_enter_triggers_gnome_terminal_args_shape() {
    use std::path::PathBuf;
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_gnome_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let (_term_path, out_path) = write_fake("gnome-terminal", &dir);
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let body = std::fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");
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
/// What: konsole arg shape uses "-e", "bash", "-lc"
///
/// - Input: Fake konsole PATH isolated; SystemUpdate Enter
/// - Output: First three args are ["-e", "bash", "-lc"]
fn ui_options_update_system_enter_triggers_konsole_args_shape() {
    use std::path::PathBuf;
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_konsole_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let (_term_path, out_path) = write_fake("konsole", &dir);
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let body = std::fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "-e");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");
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
/// What: alacritty arg shape uses "-e", "bash", "-lc"
///
/// - Input: Fake alacritty PATH isolated; SystemUpdate Enter
/// - Output: First three args are ["-e", "bash", "-lc"]
fn ui_options_update_system_enter_triggers_alacritty_args_shape() {
    use std::path::PathBuf;
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_alacritty_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let (_term_path, out_path) = write_fake("alacritty", &dir);
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let body = std::fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "-e");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");
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
/// What: kitty arg shape uses "bash", "-lc"
///
/// - Input: Fake kitty PATH isolated; SystemUpdate Enter
/// - Output: First two args are ["bash", "-lc"]
fn ui_options_update_system_enter_triggers_kitty_args_shape() {
    use std::path::PathBuf;
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_kitty_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let (_term_path, out_path) = write_fake("kitty", &dir);
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let body = std::fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 2, "expected at least 2 args, got: {}", body);
    assert_eq!(lines[0], "bash");
    assert_eq!(lines[1], "-lc");
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
/// What: xterm arg shape uses "-hold", "-e", "bash", "-lc"
///
/// - Input: Fake xterm PATH isolated; SystemUpdate Enter
/// - Output: First four args are ["-hold", "-e", "bash", "-lc"]
fn ui_options_update_system_enter_triggers_xterm_args_shape() {
    use std::path::PathBuf;
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_xterm_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let (_term_path, out_path) = write_fake("xterm", &dir);
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    let mut app = crate_root::state::AppState {
        ..Default::default()
    };
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let body = std::fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 4, "expected at least 4 args, got: {}", body);
    assert_eq!(lines[0], "-hold");
    assert_eq!(lines[1], "-e");
    assert_eq!(lines[2], "bash");
    assert_eq!(lines[3], "-lc");
    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}
