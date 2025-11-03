use std::process::Command;

#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
/// What: Spawn a terminal to remove all given packages with pacman.
///
/// Input: names slice of package names; dry_run prints the removal command instead of executing
/// Output: Launches a terminal (or bash) to run sudo pacman -Rns for the provided names.
///
/// Details: Prefers common terminals (GNOME Console/Terminal, kitty, alacritty, xterm, xfce4-terminal, etc.); falls back to bash. Appends a hold tail so the window remains open after command completion.
pub fn spawn_remove_all(names: &[String], dry_run: bool) {
    let names_str = names.join(" ");
    tracing::info!(names = %names_str, total = names.len(), dry_run, "spawning removal");
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
    let cmd_str = if dry_run {
        format!(
            "echo DRY RUN: sudo pacman -Rns --noconfirm {n}{hold}",
            n = names.join(" "),
            hold = hold_tail
        )
    } else {
        format!(
            "sudo pacman -Rns --noconfirm {n}{hold}",
            n = names.join(" "),
            hold = hold_tail
        )
    };
    // Prefer GNOME Terminal when running under GNOME desktop
    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .map(|v| v.to_uppercase().contains("GNOME"))
        .unwrap_or(false);
    let terms_gnome_first: &[(&str, &[&str], bool)] = &[
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("gnome-console", &["--", "bash", "-lc"], false),
        ("kgx", &["--", "bash", "-lc"], false),
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
        ("xfce4-terminal", &[], true),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
    let terms_default: &[(&str, &[&str], bool)] = &[
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("gnome-console", &["--", "bash", "-lc"], false),
        ("kgx", &["--", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
        ("xfce4-terminal", &[], true),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
    let terms = if is_gnome {
        terms_gnome_first
    } else {
        terms_default
    };
    let mut launched = false;
    if let Some(idx) = choose_terminal_index_prefer_path(terms) {
        let (term, args, needs_xfce_command) = terms[idx];
        let mut cmd = Command::new(term);
        if needs_xfce_command && term == "xfce4-terminal" {
            let quoted = shell_single_quote(&cmd_str);
            cmd.arg("--command").arg(format!("bash -lc {}", quoted));
        } else {
            cmd.args(args.iter().copied()).arg(&cmd_str);
        }
        if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
            if let Some(parent) = std::path::Path::new(&p).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            cmd.env("PACSEA_TEST_OUT", p);
        }
        if term == "konsole" && std::env::var_os("WAYLAND_DISPLAY").is_some() {
            cmd.env("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");
        }
        if term == "gnome-console" || term == "kgx" {
            cmd.env("GSK_RENDERER", "cairo");
            cmd.env("LIBGL_ALWAYS_SOFTWARE", "1");
        }
        let spawn_res = cmd.spawn();
        match spawn_res {
            Ok(_) => {
                tracing::info!(terminal = %term, names = %names_str, total = names.len(), dry_run, "launched terminal for removal")
            }
            Err(e) => {
                tracing::warn!(terminal = %term, error = %e, names = %names_str, "failed to spawn terminal, trying next");
            }
        }
        launched = true;
    } else {
        for (term, args, needs_xfce_command) in terms {
            if command_on_path(term) {
                let mut cmd = Command::new(term);
                if *needs_xfce_command && *term == "xfce4-terminal" {
                    let quoted = shell_single_quote(&cmd_str);
                    cmd.arg("--command").arg(format!("bash -lc {}", quoted));
                } else {
                    cmd.args(args.iter().copied()).arg(&cmd_str);
                }
                if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
                    if let Some(parent) = std::path::Path::new(&p).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    cmd.env("PACSEA_TEST_OUT", p);
                }
                if *term == "konsole" && std::env::var_os("WAYLAND_DISPLAY").is_some() {
                    cmd.env("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");
                }
                if *term == "gnome-console" || *term == "kgx" {
                    cmd.env("GSK_RENDERER", "cairo");
                    cmd.env("LIBGL_ALWAYS_SOFTWARE", "1");
                }
                let spawn_res = cmd.spawn();
                match spawn_res {
                    Ok(_) => {
                        tracing::info!(terminal = %term, names = %names_str, total = names.len(), dry_run, "launched terminal for removal")
                    }
                    Err(e) => {
                        tracing::warn!(terminal = %term, error = %e, names = %names_str, "failed to spawn terminal, trying next");
                        continue;
                    }
                }
                launched = true;
                break;
            }
        }
    }
    if !launched {
        let res = Command::new("bash").args(["-lc", &cmd_str]).spawn();
        if let Err(e) = res {
            tracing::error!(error = %e, names = %names_str, "failed to spawn bash to run removal command");
        } else {
            tracing::info!(names = %names_str, total = names.len(), dry_run, "launched bash for removal");
        }
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Ensure gnome-terminal is invoked with double dash for removals
    ///
    /// - Input: Fake gnome-terminal on PATH; spawn_remove_all with dry_run
    /// - Output: First args are "--", "bash", "-lc" (safe arg shape)
    fn remove_all_uses_gnome_terminal_double_dash() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_remove_gnome_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut out_path = dir.clone();
        out_path.push("args.txt");
        let mut term_path = dir.clone();
        term_path.push("gnome-terminal");
        let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
        fs::write(&term_path, script.as_bytes()).unwrap();
        let mut perms = fs::metadata(&term_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&term_path, perms).unwrap();

        let orig_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", dir.display().to_string());
            std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
        }

        let names = vec!["ripgrep".to_string(), "fd".to_string()];
        super::spawn_remove_all(&names, true);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
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
}

#[cfg(target_os = "windows")]
/// On Windows, open a shell window showing the intended remove action (no-op).
///
/// Inputs: same as Unix variant.
///
/// Output:
/// - Launches a `cmd` window with a message; actual removal is unsupported on Windows.
pub fn spawn_remove_all(names: &[String], dry_run: bool) {
    let mut names = names.to_vec();
    if names.is_empty() {
        names.push("nothing".into());
    }
    let names_str = names.join(" ");
    tracing::info!(names = %names_str, total = names.len(), dry_run, "spawning removal");
    let msg = if dry_run {
        format!("DRY RUN: remove {}", names.join(" "))
    } else {
        format!("Remove {} (not supported on Windows)", names.join(" "))
    };
    let _ = Command::new("cmd")
        .args([
            "/C",
            "start",
            "Pacsea Remove",
            "cmd",
            "/K",
            &format!("echo {msg}"),
        ])
        .spawn();
    tracing::info!(names = %names_str, total = names.len(), dry_run, "launched cmd for removal");
}
