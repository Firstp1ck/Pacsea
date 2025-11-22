use std::process::Command;

#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
/// What: Spawn a terminal to run a `&&`-joined series of shell commands with a hold tail.
///
/// Input:
/// - `cmds`: Ordered list of shell snippets to execute.
///
/// Output:
/// - Starts the preferred terminal (or `bash`) running the composed command sequence.
///
/// Details:
/// - Defers to `spawn_shell_commands_in_terminal_with_hold` to add the default hold tail.
/// - During tests, this is a no-op to avoid opening real terminal windows, unless `PACSEA_TEST_OUT` is set.
pub fn spawn_shell_commands_in_terminal(cmds: &[String]) {
    // Skip actual spawning during tests unless PACSEA_TEST_OUT is set (indicates a test with fake terminal)
    #[cfg(test)]
    if std::env::var("PACSEA_TEST_OUT").is_err() {
        return;
    }
    // Default wrapper keeps the terminal open after commands complete
    spawn_shell_commands_in_terminal_with_hold(cmds, true);
}

#[cfg(not(target_os = "windows"))]
/// What: Write a log message to terminal.log file.
///
/// Input:
/// - `message`: The log message to write.
///
/// Output:
/// - Writes the message to terminal.log, creating the log directory if needed.
///
/// Details:
/// - Silently ignores errors if the log file cannot be opened or written.
fn log_to_terminal_log(message: &str) {
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
        let _ = std::io::Write::write_all(&mut file, message.as_bytes());
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Configure environment variables for a terminal command based on terminal type and environment.
///
/// Input:
/// - `cmd`: The Command to configure.
/// - `term`: Terminal binary name.
/// - `is_wayland`: Whether running under Wayland.
///
/// Output:
/// - Modifies the command with appropriate environment variables.
///
/// Details:
/// - Sets `PACSEA_TEST_OUT` if present in environment.
/// - Suppresses `Konsole` `Wayland` warnings on `Wayland`.
/// - Forces software rendering for `GNOME Console` and `kgx`.
fn configure_terminal_env(cmd: &mut Command, term: &str, is_wayland: bool) {
    if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
        if let Some(parent) = std::path::Path::new(&p).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        cmd.env("PACSEA_TEST_OUT", p);
    }
    if term == "konsole" && is_wayland {
        cmd.env("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");
    }
    if term == "gnome-console" || term == "kgx" {
        cmd.env("GSK_RENDERER", "cairo");
        cmd.env("LIBGL_ALWAYS_SOFTWARE", "1");
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Build and spawn a terminal command with logging.
///
/// Input:
/// - `term`: Terminal binary name.
/// - `args`: Terminal arguments.
/// - `needs_xfce_command`: Whether to use xfce4-terminal special command format.
/// - `script_exec`: The script execution command string.
/// - `cmd_str`: The full command string for logging.
/// - `is_wayland`: Whether running under Wayland.
/// - `detach_stdio`: Whether to detach stdio streams.
///
/// Output:
/// - Returns `Ok(true)` if spawn succeeded, `Ok(false)` if it failed, or `Err` on error.
///
/// Details:
/// - Logs spawn attempt and result to terminal.log.
/// - Configures terminal-specific environment variables.
fn try_spawn_terminal(
    term: &str,
    args: &[&str],
    needs_xfce_command: bool,
    script_exec: &str,
    cmd_str: &str,
    is_wayland: bool,
    detach_stdio: bool,
) -> Result<bool, std::io::Error> {
    let mut cmd = Command::new(term);
    if needs_xfce_command && term == "xfce4-terminal" {
        let quoted = shell_single_quote(script_exec);
        cmd.arg("--command").arg(format!("bash -lc {quoted}"));
    } else {
        cmd.args(args.iter().copied()).arg(script_exec);
    }
    configure_terminal_env(&mut cmd, term, is_wayland);
    let cmd_len = cmd_str.len();
    log_to_terminal_log(&format!(
        "spawn term={} args={:?} xfce_mode={} cmd_len={}\n",
        term, args, needs_xfce_command, cmd_len
    ));
    if detach_stdio {
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    let res = cmd.spawn();
    match &res {
        Ok(child) => {
            log_to_terminal_log(&format!("spawn result: ok pid={}\n", child.id()));
        }
        Err(e) => {
            log_to_terminal_log(&format!("spawn result: err error={e}\n"));
        }
    }
    res.map(|_| true)
}

#[cfg(not(target_os = "windows"))]
/// What: Create a temporary script file with the command string.
///
/// Input:
/// - `cmd_str`: The command string to write to the script.
///
/// Output:
/// - Path to the created temporary script file.
///
/// Details:
/// - Creates a bash script with executable permissions.
fn create_temp_script(cmd_str: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    p.push(format!("pacsea_scan_{}_{}.sh", std::process::id(), ts));
    let _ = std::fs::write(&p, format!("#!/bin/bash\n{cmd_str}\n"));
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
}

#[cfg(not(target_os = "windows"))]
/// What: Persist the command string to a log file for debugging.
///
/// Input:
/// - `cmd_str`: The command string to log.
///
/// Output:
/// - None (writes to log file).
fn persist_command_to_log(cmd_str: &str) {
    let mut lp = crate::theme::logs_dir();
    lp.push("last_terminal_cmd.log");
    if let Some(parent) = lp.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&lp, format!("{cmd}\n", cmd = cmd_str));
}

#[cfg(not(target_os = "windows"))]
/// What: Build the list of terminal candidates with preference ordering.
///
/// Input:
/// - `is_gnome`: Whether running under GNOME desktop.
///
/// Output:
/// - Vector of terminal candidates with (`name`, `args`, `needs_xfce_command`) tuples.
///
/// Details:
/// - Prioritizes GNOME terminals when under GNOME, otherwise uses default order.
/// - Moves user-preferred terminal to the front if configured.
fn build_terminal_candidates(is_gnome: bool) -> Vec<(&'static str, &'static [&'static str], bool)> {
    let terms_gnome_first: &[(&str, &[&str], bool)] = &[
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("gnome-console", &["--", "bash", "-lc"], false),
        ("kgx", &["--", "bash", "-lc"], false),
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("ghostty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
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
        ("xfce4-terminal", &[], true),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
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
    terms_owned
}

#[cfg(not(target_os = "windows"))]
/// What: Attempt to spawn a terminal from the candidates list.
///
/// Input:
/// - `terms_owned`: List of terminal candidates.
/// - `script_exec`: Script execution command string.
/// - `cmd_str`: Full command string for logging.
/// - `is_wayland`: Whether running under Wayland.
///
/// Output:
/// - `true` if a terminal was successfully spawned, `false` otherwise.
fn attempt_terminal_spawn(
    terms_owned: &[(&str, &[&str], bool)],
    script_exec: &str,
    cmd_str: &str,
    is_wayland: bool,
) -> bool {
    if let Some(idx) = choose_terminal_index_prefer_path(terms_owned) {
        let (term, args, needs_xfce_command) = terms_owned[idx];
        return try_spawn_terminal(
            term,
            args,
            needs_xfce_command,
            script_exec,
            cmd_str,
            is_wayland,
            true,
        )
        .unwrap_or(false);
    }
    for (term, args, needs_xfce_command) in terms_owned.iter().copied() {
        if command_on_path(term)
            && try_spawn_terminal(
                term,
                args,
                needs_xfce_command,
                script_exec,
                cmd_str,
                is_wayland,
                false,
            )
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
/// What: Spawn a terminal to execute shell commands and optionally append a hold tail.
///
/// Input:
/// - `cmds`: Ordered list of shell snippets to execute.
/// - `hold`: When `true`, keeps the terminal open after command completion.
///
/// Output:
/// - Launches a terminal (or `bash`) running a temporary script that encapsulates the commands.
///
/// Details:
/// - Persists the command to a temp script to avoid argument-length issues.
/// - Prefers user-configured terminals, applies desktop-specific environment tweaks, and logs spawn attempts.
/// - During tests, this is a no-op to avoid opening real terminal windows.
pub fn spawn_shell_commands_in_terminal_with_hold(cmds: &[String], hold: bool) {
    // Skip actual spawning during tests unless PACSEA_TEST_OUT is set (indicates a test with fake terminal)
    #[cfg(test)]
    if std::env::var("PACSEA_TEST_OUT").is_err() {
        return;
    }

    if cmds.is_empty() {
        return;
    }
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
    let joined = cmds.join(" && ");
    let cmd_str = if hold {
        format!("{joined}{hold_tail}")
    } else {
        joined
    };
    let script_path = create_temp_script(&cmd_str);
    let script_path_str = script_path.to_string_lossy().to_string();
    let script_exec = format!("bash {}", shell_single_quote(&script_path_str));

    persist_command_to_log(&cmd_str);

    let desktop_env = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let is_gnome = desktop_env.to_uppercase().contains("GNOME");
    let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let terms_owned = build_terminal_candidates(is_gnome);

    log_to_terminal_log(&format!(
        "env desktop={} wayland={} script={} cmd_len={}\n",
        desktop_env,
        is_wayland,
        script_path_str,
        cmd_str.len()
    ));

    let launched = attempt_terminal_spawn(&terms_owned, &script_exec, &cmd_str, is_wayland);
    if !launched {
        log_to_terminal_log(&format!(
            "spawn term=bash args={:?} cmd_len={}\n",
            ["-lc"],
            cmd_str.len()
        ));
        let res = Command::new("bash").args(["-lc", &script_exec]).spawn();
        match &res {
            Ok(child) => {
                log_to_terminal_log(&format!("spawn result: ok pid={}\n", child.id()));
            }
            Err(e) => {
                log_to_terminal_log(&format!("spawn result: err error={e}\n"));
            }
        }
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Ensure `spawn_shell_commands_in_terminal` invokes GNOME Terminal with a double-dash separator.
    ///
    /// Inputs:
    /// - `cmds`: Single echo command executed via a temporary mock `gnome-terminal` script.
    ///
    /// Output:
    /// - Captured argv begins with `--`, `bash`, `-lc`, confirming safe argument ordering.
    ///
    /// Details:
    /// - Rewrites `PATH` to point at a fake executable that records arguments, then restores env vars
    ///   after spawning the terminal command.
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
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create test directory");
        let mut out_path = dir.clone();
        out_path.push("args.txt");
        let mut term_path = dir.clone();
        term_path.push("gnome-terminal");
        let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
        fs::write(&term_path, script.as_bytes()).expect("failed to write test terminal script");
        let mut perms = fs::metadata(&term_path)
            .expect("failed to read test terminal script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&term_path, perms)
            .expect("failed to set test terminal script permissions");

        let orig_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", dir.display().to_string());
            std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
        }

        let cmds = vec!["echo hi".to_string()];
        super::spawn_shell_commands_in_terminal(&cmds);
        // Wait for file to be created with retries
        let mut attempts = 0;
        while !out_path.exists() && attempts < 50 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            attempts += 1;
        }
        let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
        let lines: Vec<&str> = body.lines().collect();
        assert!(lines.len() >= 3, "expected at least 3 args, got: {body}");
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
/// What: Display the intended shell command sequence on Windows where execution is unsupported.
///
/// Input:
/// - `cmds`: Command fragments to present to the user.
///
/// Output:
/// - Launches a PowerShell window (if available and command contains "DRY RUN") for dry-run simulation, or `cmd` window otherwise.
///
/// Details:
/// - When commands contain "DRY RUN" and PowerShell is available, uses PowerShell to simulate the operation.
/// - Joins commands with `&&` for readability and uses `start` to detach the window.
pub fn spawn_shell_commands_in_terminal(cmds: &[String]) {
    let msg = if cmds.is_empty() {
        "Nothing to run".to_string()
    } else {
        cmds.join(" && ")
    };

    // Check if this is a dry-run operation (for downgrade, etc.)
    let is_dry_run = msg.contains("DRY RUN");

    if is_dry_run && super::utils::is_powershell_available() {
        // Use PowerShell to simulate the operation
        let escaped_msg = msg.replace("'", "''");
        let powershell_cmd = format!(
            "Write-Host '{}' -ForegroundColor Yellow; Write-Host ''; Write-Host 'Press any key to close...'; $null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')",
            escaped_msg
        );
        let _ = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &powershell_cmd])
            .spawn();
    } else {
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
}
