//! Installation command construction and launching utilities.
//!
//! This module is responsible for turning a `PackageItem` into an executable
//! command-line and spawning an external terminal to perform the installation.
//!
//! Key behaviors:
//! - On Unix-like systems, installation happens in a new terminal (Alacritty,
//!   Kitty, Xterm, GNOME Terminal, Konsole, Xfce Terminal, Tilix, or MATE
//!   Terminal), falling back to invoking `bash -lc` if none is available.
//! - `pacman` is used for official packages. AUR installs prefer `paru`, then
//!   `yay` if `paru` is missing.
//! - Commands end with a resilient "hold" tail to keep the terminal open and
//!   prompt the user to press a key before closing.
//! - In dry-run mode, commands print what would happen without making changes.
//! - On Windows, installation is not implemented; a `cmd` window is opened to
//!   display the intended action for visibility.
//! - Completed (non-dry-run) installs are appended to `install_log.txt` with a
//!   timestamp for simple auditability.
use std::process::Command;

use crate::state::{PackageItem, Source};

// Helper: check whether a command exists on PATH (Unix-aware exec bit)
#[cfg(not(target_os = "windows"))]
/// Check whether `cmd` exists on the current `PATH`.
///
/// This function understands platform nuances:
/// - If `cmd` contains a path separator, it checks that path directly.
/// - On Unix, the file must be present and have at least one execute bit set.
/// - On Windows, any regular file is accepted, and `PATHEXT` is honored when
///   probing candidates without extensions.
fn command_on_path(cmd: &str) -> bool {
    use std::path::Path;

    fn is_exec(p: &std::path::Path) -> bool {
        if !p.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(p) {
                return meta.permissions().mode() & 0o111 != 0;
            }
            false
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    // If path contains a separator, check directly
    if cmd.contains(std::path::MAIN_SEPARATOR) {
        return is_exec(Path::new(cmd));
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(cmd);
            if is_exec(&candidate) {
                return true;
            }
            #[cfg(windows)]
            {
                if let Some(pathext) = std::env::var_os("PATHEXT") {
                    for ext in pathext.to_string_lossy().split(';') {
                        let candidate = dir.join(format!("{}{}", cmd, ext));
                        if candidate.is_file() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
/// Spawn a new terminal to install a single `PackageItem` on Unix-like systems.
///
/// - `password`: Optional sudo password. If provided, it is piped to `sudo -S`.
///   If omitted, `sudo` will prompt interactively on the spawned TTY.
/// - `dry_run`: When true, prints the intended command instead of performing it.
///
/// This tries a list of known terminal emulators and falls back to `bash -lc`.
/// After launching, it logs the package name to `install_log.txt` when not in
/// dry-run mode.
pub fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);
    // Try common terminals
    let terms: &[(&str, &[&str], bool)] = &[
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
        ("xfce4-terminal", &["-e", "bash", "-lc"], false),
        ("tilix", &["-e", "bash", "-lc"], false),
        ("mate-terminal", &["-e", "bash", "-lc"], false),
    ];
    let mut launched = false;
    for (term, args, _hold) in terms {
        if command_on_path(term) {
            let _ = Command::new(term)
                .args(args.iter().copied())
                .arg(&cmd_str)
                .spawn();
            launched = true;
            break;
        }
    }
    if !launched {
        let _ = Command::new("bash").args(["-lc", &cmd_str]).spawn();
    }
    if !dry_run {
        let _ = log_installed(std::slice::from_ref(&item.name));
    }
}

#[cfg(target_os = "windows")]
/// Open a `cmd` window to display and run the install command on Windows.
///
/// Windows is not supported for actual Arch-based installation; this function
/// opens a terminal with the constructed command to inform the user. When not
/// in dry-run mode, it still records the package as "installed" in the
/// `install_log.txt` for parity with Unix behavior.
pub fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);
    let _ = Command::new("cmd")
        .args(["/C", "start", "Pacsea Install", "cmd", "/K", &cmd_str])
        .spawn();
    if !dry_run {
        let _ = log_installed(&[item.name.clone()]);
    }
}

/// Build a shell command to install `item` and indicate whether `sudo` is used.
///
/// Returns `(command_string, uses_sudo)` where:
/// - `command_string` is a bash-compatible line that includes a terminal hold
///   tail to keep the window open after completion.
/// - `uses_sudo` is true for official packages that use `pacman`.
///
/// Behavior by source:
/// - Official: `pacman -S --needed <name>` under `sudo`. If `password` is
///   provided, it is quoted and piped to `sudo -S`.
/// - AUR: Prefer `paru`, fall back to `yay`; error message if neither exists.
///
/// In `dry_run` mode the command echoes the intended action rather than running
/// it.
pub fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> (String, bool) {
    match &item.source {
        Source::Official { .. } => {
            let base_cmd = format!("pacman -S --needed --noconfirm {}", item.name);
            // Robust hold tail that works even if read fails (e.g., no TTY)
            let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            if dry_run {
                let bash = format!("echo DRY RUN: sudo {base_cmd}{hold}", hold = hold_tail);
                return (bash, true);
            }
            // Escape password for bash
            let pass = password.unwrap_or("");
            if pass.is_empty() {
                // Interactive sudo without -S so it prompts on the TTY
                let bash = format!("sudo {base_cmd}{hold}", hold = hold_tail);
                (bash, true)
            } else {
                let escaped = pass.replace('\'', "'\"'\"'\''");
                let pipe = format!("echo '{escaped}' | ");
                let bash = format!("{pipe}sudo -S {base_cmd}{hold}", hold = hold_tail);
                (bash, true)
            }
        }
        Source::Aur => {
            let hold_tail = "; echo; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            let aur_cmd = if dry_run {
                format!(
                    "echo DRY RUN: paru -S --needed --noconfirm {n} || yay -S --needed --noconfirm {n}{hold}",
                    n = item.name,
                    hold = hold_tail
                )
            } else {
                format!(
                    "(command -v paru >/dev/null 2>&1 && paru -S --needed --noconfirm {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed --noconfirm {n}) || echo 'No AUR helper (paru/yay) found.'{hold}",
                    n = item.name,
                    hold = hold_tail
                )
            };
            (aur_cmd, false)
        }
    }
}

#[cfg(not(target_os = "windows"))]
/// Spawn a single terminal to install multiple `PackageItem`s on Unix-like systems.
///
/// If any AUR package is present, prefer installing the entire list via an AUR
/// helper (`paru` or `yay`) so that dependencies across sources are resolved
/// consistently. Otherwise, install all items via `pacman`. The window is kept
/// open with a hold tail.
///
/// In `dry_run` mode, the terminal prints the commands that would be executed.
/// On success (non-dry-run), appends all names to `install_log.txt`.
pub fn spawn_install_all(items: &[PackageItem], dry_run: bool) {
    let mut official: Vec<String> = Vec::new();
    let mut aur: Vec<String> = Vec::new();
    for it in items {
        match it.source {
            Source::Official { .. } => official.push(it.name.clone()),
            Source::Aur => aur.push(it.name.clone()),
        }
    }
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";

    let cmd_str = if dry_run {
        if !aur.is_empty() {
            let all: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
            format!(
                "echo DRY RUN: (paru -S --needed --noconfirm {n} || yay -S --needed --noconfirm {n}){hold}",
                n = all.join(" "),
                hold = hold_tail
            )
        } else if !official.is_empty() {
            format!(
                "echo DRY RUN: sudo pacman -S --needed --noconfirm {n}{hold}",
                n = official.join(" "),
                hold = hold_tail
            )
        } else {
            format!("echo DRY RUN: nothing to install{hold}", hold = hold_tail)
        }
    } else {
        if !aur.is_empty() {
            let all: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
            let n = all.join(" ");
            format!(
                "(command -v paru >/dev/null 2>&1 && paru -S --needed --noconfirm {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed --noconfirm {n}) || echo 'No AUR helper (paru/yay) found.'{hold}",
                n = n,
                hold = hold_tail
            )
        } else if !official.is_empty() {
            format!(
                "sudo pacman -S --needed --noconfirm {n}{hold}",
                n = official.join(" "),
                hold = hold_tail
            )
        } else {
            format!("echo nothing to install{hold}", hold = hold_tail)
        }
    };

    // Spawn terminal once
    let terms: &[(&str, &[&str], bool)] = &[
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
        ("xfce4-terminal", &["-e", "bash", "-lc"], false),
        ("tilix", &["-e", "bash", "-lc"], false),
        ("mate-terminal", &["-e", "bash", "-lc"], false),
    ];
    let mut launched = false;
    for (term, args, _hold) in terms {
        if command_on_path(term) {
            let _ = Command::new(term)
                .args(args.iter().copied())
                .arg(&cmd_str)
                .spawn();
            launched = true;
            break;
        }
    }
    if !launched {
        let _ = Command::new("bash").args(["-lc", &cmd_str]).spawn();
    }

    // Log installs
    if !dry_run {
        let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
        if !names.is_empty() {
            let _ = log_installed(&names);
        }
    }
}

#[cfg(target_os = "windows")]
/// Open a `cmd` window to describe batch installation on Windows.
///
/// This does not perform installation; it echoes a message for visibility. When
/// not in `dry_run` mode, names are nevertheless recorded to `install_log.txt`.
pub fn spawn_install_all(items: &[PackageItem], dry_run: bool) {
    let mut names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
    if names.is_empty() {
        names.push("nothing".into());
    }
    let msg = if dry_run {
        format!("DRY RUN: install {}", names.join(" "))
    } else {
        format!("Install {} (not supported on Windows)", names.join(" "))
    };
    let _ = Command::new("cmd")
        .args([
            "/C",
            "start",
            "Pacsea Install",
            "cmd",
            "/K",
            &format!("echo {}", msg),
        ])
        .spawn();
    if !dry_run {
        let _ = log_installed(&names);
    }
}

/// Append installed package names with a timestamp to `install_log.txt` in the
/// XDG state directory.
///
/// Each line is formatted as `<YYYY-MM-DD HH:MM:SS> <name>`. Errors are
/// propagated to the caller. This function creates the log file if missing and
/// appends to it otherwise.
fn log_installed(names: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    let mut path = crate::theme::state_dir();
    path.push("install_log.txt");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .ok();
    let when = crate::util::ts_to_date(now);
    for n in names {
        writeln!(f, "{} {}", when, n)?;
    }
    Ok(())
}
