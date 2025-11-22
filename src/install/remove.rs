#[allow(unused_imports)]
use std::process::Command;

use crate::state::modal::CascadeMode;

/// What: Check for configuration directories in `$HOME/PACKAGE_NAME` and `$HOME/.config/PACKAGE_NAME`.
///
/// Inputs:
/// - `package_name`: Name of the package to check for config directories.
/// - `home`: Home directory path.
///
/// Output:
/// - Vector of found config directory paths.
///
/// Details:
/// - Checks both `$HOME/PACKAGE_NAME` and `$HOME/.config/PACKAGE_NAME`.
/// - Only returns directories that actually exist.
#[must_use]
pub fn check_config_directories(package_name: &str, home: &str) -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut found_dirs = Vec::new();

    // Check $HOME/PACKAGE_NAME
    let home_pkg_dir = PathBuf::from(home).join(package_name);
    if home_pkg_dir.exists() && home_pkg_dir.is_dir() {
        found_dirs.push(home_pkg_dir);
    }

    // Check $HOME/.config/PACKAGE_NAME
    let config_pkg_dir = PathBuf::from(home).join(".config").join(package_name);
    if config_pkg_dir.exists() && config_pkg_dir.is_dir() {
        found_dirs.push(config_pkg_dir);
    }

    found_dirs
}

#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
/// What: Configure terminal-specific environment variables for a command.
///
/// Input:
/// - `cmd`: Command builder to configure.
/// - `term`: Terminal name to check for special handling.
///
/// Output:
/// - Modifies `cmd` with environment variables for konsole, gnome-console, or kgx if needed.
///
/// Details:
/// - Sets Wayland-specific environment for konsole when running under Wayland.
/// - Sets rendering environment for gnome-console and kgx to ensure compatibility.
fn configure_terminal_env(cmd: &mut Command, term: &str) {
    if term == "konsole" && std::env::var_os("WAYLAND_DISPLAY").is_some() {
        cmd.env("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");
    }
    if term == "gnome-console" || term == "kgx" {
        cmd.env("GSK_RENDERER", "cairo");
        cmd.env("LIBGL_ALWAYS_SOFTWARE", "1");
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Configure test output environment variable for a command.
///
/// Input:
/// - `cmd`: Command builder to configure.
///
/// Output:
/// - Sets `PACSEA_TEST_OUT` environment variable if present, creating parent directory if needed.
///
/// Details:
/// - Only applies when `PACSEA_TEST_OUT` is set in the environment.
fn configure_test_env(cmd: &mut Command) {
    if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
        if let Some(parent) = std::path::Path::new(&p).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        cmd.env("PACSEA_TEST_OUT", p);
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Logging context for terminal spawning operations.
///
/// Details:
/// - Groups related logging parameters to reduce function argument count.
struct SpawnContext<'a> {
    names_str: &'a str,
    names_len: usize,
    dry_run: bool,
    cascade_mode: CascadeMode,
}

#[cfg(not(target_os = "windows"))]
/// What: Attempt to spawn a terminal with the given configuration.
///
/// Input:
/// - `term`: Terminal executable name.
/// - `args`: Arguments to pass before the command string.
/// - `needs_xfce_command`: Whether this terminal needs special xfce4-terminal command format.
/// - `cmd_str`: The command string to execute.
/// - `ctx`: Logging context for the operation.
///
/// Output:
/// - `true` if the terminal was successfully spawned, `false` otherwise.
///
/// Details:
/// - Configures command arguments based on terminal type.
/// - Sets up environment variables and test output handling.
/// - Logs success or failure appropriately.
fn try_spawn_terminal(
    term: &str,
    args: &[&str],
    needs_xfce_command: bool,
    cmd_str: &str,
    ctx: &SpawnContext<'_>,
) -> bool {
    let mut cmd = Command::new(term);
    if needs_xfce_command && term == "xfce4-terminal" {
        let quoted = shell_single_quote(cmd_str);
        cmd.arg("--command").arg(format!("bash -lc {quoted}"));
    } else {
        cmd.args(args.iter().copied()).arg(cmd_str);
    }
    configure_test_env(&mut cmd);
    configure_terminal_env(&mut cmd, term);

    match cmd.spawn() {
        Ok(_) => {
            tracing::info!(
                terminal = %term,
                names = %ctx.names_str,
                total = ctx.names_len,
                dry_run = ctx.dry_run,
                mode = ?ctx.cascade_mode,
                "launched terminal for removal"
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                terminal = %term,
                error = %e,
                names = %ctx.names_str,
                "failed to spawn terminal, trying next"
            );
            false
        }
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Spawn a terminal to remove all given packages with pacman.
///
/// Input:
/// - names slice of package names; `dry_run` prints the removal command instead of executing
///
/// Output:
/// - Launches a terminal (or bash) to run sudo pacman -Rns for the provided names.
///
/// Details:
/// - Prefers common terminals (GNOME Console/Terminal, kitty, alacritty, xterm, xfce4-terminal, etc.); falls back to bash. Appends a hold tail so the window remains open after command completion.
/// - During tests, this is a no-op to avoid opening real terminal windows.
pub fn spawn_remove_all(names: &[String], dry_run: bool, cascade_mode: CascadeMode) {
    // Skip actual spawning during tests unless PACSEA_TEST_OUT is set (indicates a test with fake terminal)
    #[cfg(test)]
    if std::env::var("PACSEA_TEST_OUT").is_err() {
        return;
    }

    let names_str = names.join(" ");
    tracing::info!(
        names = %names_str,
        total = names.len(),
        dry_run = dry_run,
        mode = ?cascade_mode,
        "spawning removal"
    );
    let flag = cascade_mode.flag();
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
    let cmd_str = if dry_run {
        format!(
            "echo DRY RUN: sudo pacman {flag} --noconfirm {n}{hold}",
            flag = flag,
            n = names.join(" "),
            hold = hold_tail
        )
    } else {
        format!(
            "sudo pacman {flag} --noconfirm {n}{hold}",
            flag = flag,
            n = names.join(" "),
            hold = hold_tail
        )
    };

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

    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .is_some_and(|v| v.to_uppercase().contains("GNOME"));
    let terms = if is_gnome {
        terms_gnome_first
    } else {
        terms_default
    };

    let ctx = SpawnContext {
        names_str: &names_str,
        names_len: names.len(),
        dry_run: dry_run,
        cascade_mode: cascade_mode,
    };

    let mut launched = false;
    if let Some(idx) = choose_terminal_index_prefer_path(terms) {
        let (term, args, needs_xfce_command) = terms[idx];
        launched = try_spawn_terminal(term, args, needs_xfce_command, &cmd_str, &ctx);
    }

    if !launched {
        for (term, args, needs_xfce_command) in terms {
            if command_on_path(term) {
                launched = try_spawn_terminal(term, args, *needs_xfce_command, &cmd_str, &ctx);
                if launched {
                    break;
                }
            }
        }
    }

    if !launched {
        let res = Command::new("bash").args(["-lc", &cmd_str]).spawn();
        if let Err(e) = res {
            tracing::error!(error = %e, names = %names_str, "failed to spawn bash to run removal command");
        } else {
            tracing::info!(
                names = %names_str,
                total = names.len(),
                dry_run = dry_run,
                mode = ?cascade_mode,
                "launched bash for removal"
            );
        }
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Verify the removal helper prefers gnome-terminal and passes the expected dash handling.
    ///
    /// Inputs:
    /// - Fake `gnome-terminal` script injected into `PATH`.
    /// - `spawn_remove_all` invoked in dry-run cascade mode with two package names.
    ///
    /// Output:
    /// - Captured invocation arguments start with `--`, `bash`, `-lc` to ensure safe command parsing.
    ///
    /// Details:
    /// - Redirects `PACSEA_TEST_OUT` so the shim terminal records arguments, then restores environment variables.
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
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
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

        let names = vec!["ripgrep".to_string(), "fd".to_string()];
        super::spawn_remove_all(
            &names,
            true,
            crate::state::modal::CascadeMode::CascadeWithConfigs,
        );
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
/// What: Present a placeholder removal message on Windows where pacman is unavailable.
///
/// Input:
/// - `names`: Packages the user requested to remove.
/// - `dry_run`: When `true`, uses PowerShell to simulate the removal operation.
/// - `cascade_mode`: Removal mode used for display consistency.
///
/// Output:
/// - Launches a detached PowerShell window (if available) for dry-run simulation, or `cmd` window otherwise.
///
/// Details:
/// - When `dry_run` is true and PowerShell is available, uses PowerShell to simulate the removal with Write-Host.
/// - Mirrors Unix logging by emitting an info trace, but performs no package operations.
/// - During tests, this is a no-op to avoid opening real terminal windows.
pub fn spawn_remove_all(names: &[String], dry_run: bool, cascade_mode: CascadeMode) {
    #[cfg(not(test))]
    {
        let mut names = names.to_vec();
        if names.is_empty() {
            names.push("nothing".into());
        }
        let names_str = names.join(" ");
        tracing::info!(
            names = %names_str,
            total = names.len(),
            dry_run = dry_run,
            mode = ?cascade_mode,
            "spawning removal"
        );
        let flag = cascade_mode.flag();
        let cmd = format!("pacman {flag} --noconfirm {}", names.join(" "));

        if dry_run && super::utils::is_powershell_available() {
            // Use PowerShell to simulate the removal operation
            let powershell_cmd = format!(
                "Write-Host 'DRY RUN: Simulating removal of {}' -ForegroundColor Yellow; Write-Host 'Command: {}' -ForegroundColor Cyan; Write-Host ''; Write-Host 'Press any key to close...'; $null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')",
                names_str,
                cmd.replace("'", "''")
            );
            let _ = Command::new("powershell.exe")
                .args(["-NoProfile", "-Command", &powershell_cmd])
                .spawn();
            tracing::info!(
                names = %names_str,
                total = names.len(),
                dry_run = dry_run,
                mode = ?cascade_mode,
                "launched PowerShell for removal"
            );
        } else {
            let msg = if dry_run {
                format!("DRY RUN: {cmd}")
            } else {
                format!(
                    "Remove {} with pacman {flag} (not supported on Windows)",
                    names.join(" ")
                )
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
            tracing::info!(
                names = %names_str,
                total = names.len(),
                dry_run = _dry_run,
                mode = ?_cascade_mode,
                "launched cmd for removal"
            );
        }
    }
}
