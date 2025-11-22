#[allow(unused_imports)]
use std::process::Command;

use crate::state::PackageItem;
#[cfg(not(target_os = "windows"))]
use crate::state::Source;

#[cfg(not(target_os = "windows"))]
use super::command::build_install_command;
#[cfg(all(target_os = "windows", not(test)))]
use super::command::build_install_command;
#[cfg(not(target_os = "windows"))]
use super::logging::log_installed;
#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
/// What: Attempt to spawn a terminal with the given command.
///
/// Input:
/// - `term`: Terminal binary name
/// - `args`: Arguments to pass to the terminal
/// - `needs_xfce_command`: Whether this terminal needs special xfce4-terminal command handling
/// - `cmd_str`: The install command to execute
/// - `item_name`: Package name for logging
/// - `src`: Source type ("official" or "aur") for logging
/// - `dry_run`: Whether this is a dry run
///
/// Output:
/// - `true` if the terminal was successfully spawned, `false` otherwise
///
/// Details:
/// - Handles `xfce4-terminal` special command format and sets up `PACSEA_TEST_OUT` environment variable if needed.
fn try_spawn_terminal(
    term: &str,
    args: &[&str],
    needs_xfce_command: bool,
    cmd_str: &str,
    item_name: &str,
    src: &str,
    dry_run: bool,
) -> bool {
    let mut cmd = Command::new(term);
    if needs_xfce_command && term == "xfce4-terminal" {
        let quoted = shell_single_quote(cmd_str);
        cmd.arg("--command").arg(format!("bash -lc {quoted}"));
    } else {
        cmd.args(args.iter().copied()).arg(cmd_str);
    }
    if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
        if let Some(parent) = std::path::Path::new(&p).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        cmd.env("PACSEA_TEST_OUT", p);
    }
    match cmd.spawn() {
        Ok(_) => {
            tracing::info!(
                terminal = %term,
                names = %item_name,
                total = 1,
                aur_count = (src == "aur") as usize,
                official_count = (src == "official") as usize,
                dry_run,
                "launched terminal for install"
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                terminal = %term,
                error = %e,
                names = %item_name,
                "failed to spawn terminal, trying next"
            );
            false
        }
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Get the terminal preference list based on desktop environment.
///
/// Input:
/// - None (reads `XDG_CURRENT_DESKTOP` environment variable)
///
/// Output:
/// - Slice of terminal tuples `(name, args, needs_xfce_command)` ordered by preference
///
/// Details:
/// - Prefers GNOME terminals when running under GNOME desktop, otherwise uses default ordering.
fn get_terminal_preferences() -> &'static [(&'static str, &'static [&'static str], bool)] {
    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .map(|v| v.to_uppercase().contains("GNOME"))
        .unwrap_or(false);
    if is_gnome {
        &[
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
        ]
    } else {
        &[
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
        ]
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Spawn a terminal to install a single package.
///
/// Input:
/// - item to install; password for sudo on official installs (optional); `dry_run` to print instead of execute
///
/// Output:
/// - Launches a terminal (or `bash`) running `pacman`/`paru`/`yay` to perform the install
///
/// Details:
/// - Prefers common terminals (`GNOME Console`/`Terminal`, `kitty`, `alacritty`, `xterm`, `xfce4-terminal`, etc.), falling back to `bash`. Uses `pacman` for official packages and `paru`/`yay` for AUR; appends a hold tail to keep the window open; logs installed names when not in `dry_run`.
/// - During tests, this is a no-op to avoid opening real terminal windows.
pub fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    // Skip actual spawning during tests unless PACSEA_TEST_OUT is set (indicates a test with fake terminal)
    #[cfg(test)]
    if std::env::var("PACSEA_TEST_OUT").is_err() {
        return;
    }

    let (cmd_str, uses_sudo) = build_install_command(item, password, dry_run);
    let src = match item.source {
        Source::Official { .. } => "official",
        Source::Aur => "aur",
    };
    tracing::info!(
        names = %item.name,
        total = 1,
        aur_count = (src == "aur") as usize,
        official_count = (src == "official") as usize,
        dry_run = dry_run,
        uses_sudo,
        "spawning install"
    );

    let terms = get_terminal_preferences();
    let mut launched = false;

    // Try preferred path-based selection first
    if let Some(idx) = choose_terminal_index_prefer_path(terms) {
        let (term, args, needs_xfce_command) = terms[idx];
        launched = try_spawn_terminal(
            term,
            args,
            needs_xfce_command,
            &cmd_str,
            &item.name,
            src,
            dry_run,
        );
    }

    // Fallback: try each terminal in order
    if !launched {
        for (term, args, needs_xfce_command) in terms {
            if command_on_path(term) {
                launched = try_spawn_terminal(
                    term,
                    args,
                    *needs_xfce_command,
                    &cmd_str,
                    &item.name,
                    src,
                    dry_run,
                );
                if launched {
                    break;
                }
            }
        }
    }

    // Final fallback: use bash directly
    if !launched {
        let res = Command::new("bash").args(["-lc", &cmd_str]).spawn();
        if let Err(e) = res {
            tracing::error!(error = %e, names = %item.name, "failed to spawn bash to run install command");
        } else {
            tracing::info!(
                names = %item.name,
                total = 1,
                aur_count = (src == "aur") as usize,
                official_count = (src == "official") as usize,
                dry_run = dry_run,
                "launched bash for install"
            );
        }
    }

    if !dry_run && let Err(e) = log_installed(std::slice::from_ref(&item.name)) {
        tracing::warn!(error = %e, names = %item.name, "failed to write install audit log");
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Confirm the single-install helper launches gnome-terminal with the expected separator arguments.
    ///
    /// Inputs:
    /// - Shim `gnome-terminal` placed first on `PATH` capturing its argv.
    /// - `spawn_install` invoked in dry-run mode for an official package.
    ///
    /// Output:
    /// - Captured arguments begin with `--`, `bash`, `-lc`, matching the safe invocation contract.
    ///
    /// Details:
    /// - Creates temporary directory to host the shim binary, exports `PACSEA_TEST_OUT`, then restores environment variables afterward.
    fn install_single_uses_gnome_terminal_double_dash() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_inst_single_gnome_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut out_path = dir.clone();
        out_path.push("args.txt");
        let mut term_path = dir.clone();
        term_path.push("gnome-terminal");
        let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
        fs::write(&term_path, script.as_bytes()).expect("Failed to write test terminal script");
        let mut perms = fs::metadata(&term_path)
            .expect("Failed to read test terminal script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&term_path, perms)
            .expect("Failed to set test terminal script permissions");

        let orig_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", dir.display().to_string());
            std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
        }

        let pkg = crate::state::PackageItem {
            name: "ripgrep".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        };
        super::spawn_install(&pkg, None, true);
        std::thread::sleep(std::time::Duration::from_millis(50));

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
/// What: Present a placeholder install message on Windows where pacman/AUR helpers are unavailable.
///
/// Input:
/// - `item`: Package metadata used to build the informational command.
/// - `password`: Ignored; included for API parity.
/// - `dry_run`: When `true`, uses PowerShell to simulate the install operation.
///
/// Output:
/// - Launches a detached PowerShell window (if available) for dry-run simulation, or `cmd` window otherwise.
///
/// Details:
/// - When `dry_run` is true and PowerShell is available, uses PowerShell to simulate the install with Write-Host.
/// - Logs the install attempt when not a dry run to keep audit behaviour consistent with Unix platforms.
/// - During tests, this is a no-op to avoid opening real terminal windows.
pub fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    #[cfg(not(test))]
    {
        let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);

        if dry_run && super::utils::is_powershell_available() {
            // Use PowerShell to simulate the install operation
            let powershell_cmd = format!(
                "Write-Host 'DRY RUN: Simulating install of {}' -ForegroundColor Yellow; Write-Host 'Command: {}' -ForegroundColor Cyan; Write-Host ''; Write-Host 'Press any key to close...'; $null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')",
                item.name,
                cmd_str.replace("'", "''")
            );
            let _ = Command::new("powershell.exe")
                .args(["-NoProfile", "-Command", &powershell_cmd])
                .spawn();
        } else {
            let _ = Command::new("cmd")
                .args(["/C", "start", "Pacsea Install", "cmd", "/K", &cmd_str])
                .spawn();
        }

        if !dry_run {
            let _ = super::logging::log_installed(std::slice::from_ref(&item.name));
        }
    }
}
