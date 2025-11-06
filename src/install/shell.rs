use std::process::Command;

#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
/// Spawn a terminal to run a sequence of shell commands joined by `&&` with a hold tail.
///
/// Inputs:
/// - `cmds`: List of shell command strings to execute in order.
///
/// Output:
/// - Launches a terminal (or `bash`) executing the composite command.
pub fn spawn_shell_commands_in_terminal(cmds: &[String]) {
    // Default wrapper keeps the terminal open after commands complete
    spawn_shell_commands_in_terminal_with_hold(cmds, true);
}

#[cfg(not(target_os = "windows"))]
pub fn spawn_shell_commands_in_terminal_with_hold(cmds: &[String], hold: bool) {
    if cmds.is_empty() {
        return;
    }
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
    let joined = cmds.join(" && ");
    let cmd_str = if hold {
        format!("{joined}{hold}", hold = hold_tail)
    } else {
        joined.clone()
    };
    // Write a temporary script to avoid terminal argument length/quoting issues
    let script_path = {
        let mut p = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        p.push(format!("pacsea_scan_{}_{}.sh", std::process::id(), ts));
        let _ = std::fs::write(&p, format!("#!/bin/bash\n{}\n", cmd_str));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&p) {
                let mut perms = meta.permissions();
                perms.set_mode(0o700);
                let _ = std::fs::set_permissions(&p, perms);
            }
        }
        p
    };
    let script_path_str = script_path.to_string_lossy().to_string();
    let script_exec = format!("bash {}", shell_single_quote(&script_path_str));

    // Persist the full command for debugging/repro
    {
        let mut lp = crate::theme::logs_dir();
        lp.push("last_terminal_cmd.txt");
        if let Some(parent) = lp.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&lp, format!("{cmd}\n", cmd = &cmd_str));
    }

    // Prefer GNOME Terminal when running under GNOME desktop
    let desktop_env = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let is_gnome = desktop_env.to_uppercase().contains("GNOME");
    let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();

    // (binary, args, needs_xfce_command)
    let terms_gnome_first: &[(&str, &[&str], bool)] = &[
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("gnome-console", &["--", "bash", "-lc"], false),
        ("kgx", &["--", "bash", "-lc"], false),
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("ghostty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
        // For xfce4-terminal, use --command "bash -lc '<cmd>'" to avoid -lc being parsed by terminal
        ("xfce4-terminal", &[], true),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
    let terms_default: &[(&str, &[&str], bool)] = &[
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("ghostty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("gnome-console", &["--", "bash", "-lc"], false),
        ("kgx", &["--", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
        // For xfce4-terminal, use --command "bash -lc '<cmd>'" to avoid -lc being parsed by terminal
        ("xfce4-terminal", &[], true),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
    // Build terminal candidates and optionally prioritize user-preferred terminal
    let mut terms_owned: Vec<(&str, &[&str], bool)> = if is_gnome {
        terms_gnome_first.to_vec()
    } else {
        terms_default.to_vec()
    };
    let preferred = crate::theme::settings()
        .preferred_terminal
        .trim()
        .to_string();
    if !preferred.is_empty()
        && let Some(pos) = terms_owned
            .iter()
            .position(|(name, _, _)| *name == preferred)
    {
        let entry = terms_owned.remove(pos);
        terms_owned.insert(0, entry);
    }

    // Log environment context once per invocation
    {
        let mut lp = crate::theme::logs_dir();
        lp.push("terminal.log");
        if let Some(parent) = lp.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&lp)
        {
            let _ = std::io::Write::write_all(
                &mut file,
                format!(
                    "env desktop={} wayland={} script={} cmd_len={}\n",
                    desktop_env,
                    is_wayland,
                    script_path_str,
                    cmd_str.len()
                )
                .as_bytes(),
            );
        }
    }

    let mut launched = false;
    if let Some(idx) = choose_terminal_index_prefer_path(&terms_owned) {
        let (term, args, needs_xfce_command) = terms_owned[idx];
        let mut cmd = Command::new(term);
        if needs_xfce_command && term == "xfce4-terminal" {
            let quoted = shell_single_quote(&script_exec);
            cmd.arg("--command").arg(format!("bash -lc {}", quoted));
        } else {
            cmd.args(args.iter().copied()).arg(&script_exec);
        }
        if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
            if let Some(parent) = std::path::Path::new(&p).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            cmd.env("PACSEA_TEST_OUT", p);
        }
        // Suppress Konsole Wayland textinput warnings on Wayland
        if term == "konsole" && is_wayland {
            cmd.env("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");
        }
        // Force software/cairo rendering for GNOME Console to avoid GPU/Vulkan errors
        if term == "gnome-console" || term == "kgx" {
            cmd.env("GSK_RENDERER", "cairo");
            cmd.env("LIBGL_ALWAYS_SOFTWARE", "1");
        }
        let cmd_len = cmd_str.len();
        {
            let mut lp = crate::theme::logs_dir();
            lp.push("terminal.log");
            if let Some(parent) = lp.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&lp)
            {
                let _ = std::io::Write::write_all(
                    &mut file,
                    format!(
                        "spawn term={} args={:?} xfce_mode={} cmd_len={}\n",
                        term, args, needs_xfce_command, cmd_len
                    )
                    .as_bytes(),
                );
            }
        }
        // Detach stdio to prevent terminal logs (e.g., Ghostty info/warnings) from overlapping the TUI
        let res = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        {
            let mut lp = crate::theme::logs_dir();
            lp.push("terminal.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&lp)
            {
                match res {
                    Ok(ref child) => {
                        let _ = std::io::Write::write_all(
                            &mut file,
                            format!("spawn result: ok pid={}\n", child.id()).as_bytes(),
                        );
                    }
                    Err(ref e) => {
                        let _ = std::io::Write::write_all(
                            &mut file,
                            format!("spawn result: err error={}\n", e).as_bytes(),
                        );
                    }
                }
            }
        }
        if res.is_ok() {
            launched = true;
        }
    } else {
        for (term, args, needs_xfce_command) in terms_owned.iter().copied() {
            if command_on_path(term) {
                let mut cmd = Command::new(term);
                if needs_xfce_command && term == "xfce4-terminal" {
                    let quoted = shell_single_quote(&script_exec);
                    cmd.arg("--command").arg(format!("bash -lc {}", quoted));
                } else {
                    cmd.args(args.iter().copied()).arg(&script_exec);
                }
                if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
                    if let Some(parent) = std::path::Path::new(&p).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    cmd.env("PACSEA_TEST_OUT", p);
                }
                // Suppress Konsole Wayland textinput warnings on Wayland
                if term == "konsole" && is_wayland {
                    cmd.env("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");
                }
                // Force software/cairo rendering for GNOME Console to avoid GPU/Vulkan errors
                if term == "gnome-console" || term == "kgx" {
                    cmd.env("GSK_RENDERER", "cairo");
                    cmd.env("LIBGL_ALWAYS_SOFTWARE", "1");
                }
                let cmd_len = cmd_str.len();
                {
                    let mut lp = crate::theme::logs_dir();
                    lp.push("terminal.log");
                    if let Some(parent) = lp.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&lp)
                    {
                        let _ = std::io::Write::write_all(
                            &mut file,
                            format!(
                                "spawn term={} args={:?} xfce_mode={} cmd_len={}\n",
                                term, args, needs_xfce_command, cmd_len
                            )
                            .as_bytes(),
                        );
                    }
                }
                let res = cmd.spawn();
                {
                    let mut lp = crate::theme::logs_dir();
                    lp.push("terminal.log");
                    if let Ok(mut file) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&lp)
                    {
                        match res {
                            Ok(ref child) => {
                                let _ = std::io::Write::write_all(
                                    &mut file,
                                    format!("spawn result: ok pid={}\n", child.id()).as_bytes(),
                                );
                            }
                            Err(ref e) => {
                                let _ = std::io::Write::write_all(
                                    &mut file,
                                    format!("spawn result: err error={}\n", e).as_bytes(),
                                );
                            }
                        }
                    }
                }
                if res.is_ok() {
                    launched = true;
                    break;
                }
            }
        }
    }
    if !launched {
        let cmd_len = cmd_str.len();
        {
            let mut lp = crate::theme::logs_dir();
            lp.push("terminal.log");
            if let Some(parent) = lp.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&lp)
            {
                let _ = std::io::Write::write_all(
                    &mut file,
                    format!("spawn term=bash args={:?} cmd_len={}\n", ["-lc"], cmd_len).as_bytes(),
                );
            }
        }
        let res = Command::new("bash").args(["-lc", &script_exec]).spawn();
        {
            let mut lp = crate::theme::logs_dir();
            lp.push("terminal.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&lp)
            {
                match res {
                    Ok(ref child) => {
                        let _ = std::io::Write::write_all(
                            &mut file,
                            format!("spawn result: ok pid={}\n", child.id()).as_bytes(),
                        );
                    }
                    Err(ref e) => {
                        let _ = std::io::Write::write_all(
                            &mut file,
                            format!("spawn result: err error={}\n", e).as_bytes(),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Ensure gnome-terminal is invoked with double dash for shell commands
    ///
    /// - Input: Fake gnome-terminal on PATH; spawn_shell_commands_in_terminal
    /// - Output: First args are "--", "bash", "-lc" (safe arg shape)
    fn shell_uses_gnome_terminal_double_dash() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_shell_gnome_{}_{}",
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

        let cmds = vec!["echo hi".to_string()];
        super::spawn_shell_commands_in_terminal(&cmds);
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
/// On Windows, open a shell window echoing the provided command sequence.
///
/// Inputs: `cmds` to display.
///
/// Output: Launches a `cmd` window displaying the message.
pub fn spawn_shell_commands_in_terminal(cmds: &[String]) {
    let msg = if cmds.is_empty() {
        "Nothing to run".to_string()
    } else {
        cmds.join(" && ")
    };
    let _ = Command::new("cmd")
        .args([
            "/C",
            "start",
            "Pacsea Update",
            "cmd",
            "/K",
            &format!("echo {msg}"),
        ])
        .spawn();
}
