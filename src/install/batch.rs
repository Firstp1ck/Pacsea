#[cfg(not(target_os = "windows"))]
use crate::state::Source;
#[allow(unused_imports)]
use std::process::Command;

use crate::state::PackageItem;

#[cfg(not(target_os = "windows"))]
use super::command::aur_install_body;
#[cfg(not(target_os = "windows"))]
use super::logging::log_installed;
#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
/// What: Build the shell command string for batch package installation.
///
/// Input:
/// - `items`: Packages to install
/// - `official`: Names of official packages
/// - `aur`: Names of AUR packages
/// - `dry_run`: When `true`, prints commands instead of executing
///
/// Output:
/// - Shell command string with hold tail appended
///
/// Details:
/// - Official packages are grouped into a single `pacman` invocation
/// - AUR packages are installed via `paru`/`yay` (prompts to install a helper if missing)
/// - Appends a "hold" tail so the terminal remains open after command completion
fn build_batch_install_command(
    items: &[PackageItem],
    official: &[String],
    aur: &[String],
    dry_run: bool,
) -> String {
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";

    if dry_run {
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
            format!("echo DRY RUN: nothing to install{hold_tail}")
        }
    } else if !aur.is_empty() {
        let all: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
        let n = all.join(" ");
        format!(
            "{body}{hold}",
            body = aur_install_body("-S --needed --noconfirm", &n),
            hold = hold_tail
        )
    } else if !official.is_empty() {
        format!(
            "(sudo pacman -S --needed --noconfirm {n} || (echo; echo 'Install failed.'; read -rp 'Retry with force database sync (-Syy)? [y/N]: ' ans; if [ \"$ans\" = \"y\" ] || [ \"$ans\" = \"Y\" ]; then sudo pacman -Syy && sudo pacman -S --needed --noconfirm {n}; fi)){hold}",
            n = official.join(" "),
            hold = hold_tail
        )
    } else {
        format!("echo nothing to install{hold_tail}")
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Attempt to spawn a terminal with the given command string.
///
/// Input:
/// - `term`: Terminal executable name
/// - `args`: Arguments for the terminal
/// - `needs_xfce_command`: Whether this terminal needs special xfce4-terminal command handling
/// - `cmd_str`: Command string to execute in the terminal
///
/// Output:
/// - `Ok(())` if the terminal was successfully spawned, `Err(())` otherwise
///
/// Details:
/// - Handles special cases for `konsole` (`Wayland`), `gnome-console`/`kgx` (rendering), and `xfce4-terminal` (command format)
/// - Sets up `PACSEA_TEST_OUT` environment variable if present
fn try_spawn_terminal(
    term: &str,
    args: &[&str],
    needs_xfce_command: bool,
    cmd_str: &str,
) -> Result<(), ()> {
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
    if term == "konsole" && std::env::var_os("WAYLAND_DISPLAY").is_some() {
        cmd.env("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");
    }
    if term == "gnome-console" || term == "kgx" {
        cmd.env("GSK_RENDERER", "cairo");
        cmd.env("LIBGL_ALWAYS_SOFTWARE", "1");
    }
    cmd.spawn().map(|_| ()).map_err(|_| ())
}

#[cfg(not(target_os = "windows"))]
/// What: Spawn a terminal to install a batch of packages.
///
/// Input:
/// - `items`: Packages to install
/// - `dry_run`: When `true`, prints commands instead of executing
///
/// Output:
/// - Launches a terminal (or falls back to `bash`) running the composed install commands.
///
/// Details:
/// - Official packages are grouped into a single `pacman` invocation
/// - AUR packages are installed via `paru`/`yay` (prompts to install a helper if missing)
/// - Prefers common terminals (GNOME Console/Terminal, kitty, alacritty, xterm, xfce4-terminal, etc.); falls back to `bash`
/// - Appends a "hold" tail so the terminal remains open after command completion
/// - During tests, this is a no-op to avoid opening real terminal windows.
pub fn spawn_install_all(_items: &[PackageItem], _dry_run: bool) {
    // Skip actual spawning during tests unless PACSEA_TEST_OUT is set (indicates a test with fake terminal)
    #[cfg(test)]
    if std::env::var("PACSEA_TEST_OUT").is_err() {
        return;
    }

    let mut official: Vec<String> = Vec::new();
    let mut aur: Vec<String> = Vec::new();
    for it in _items {
        match it.source {
            Source::Official { .. } => official.push(it.name.clone()),
            Source::Aur => aur.push(it.name.clone()),
        }
    }
    let names_vec: Vec<String> = _items.iter().map(|p| p.name.clone()).collect();
    tracing::info!(
        total = _items.len(),
        aur_count = aur.len(),
        official_count = official.len(),
        dry_run = _dry_run,
        names = %names_vec.join(" "),
        "spawning install"
    );

    let cmd_str = build_batch_install_command(_items, &official, &aur, _dry_run);

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
        match try_spawn_terminal(term, args, needs_xfce_command, &cmd_str) {
            Ok(()) => {
                tracing::info!(terminal = %term, total = _items.len(), aur_count = aur.len(), official_count = official.len(), dry_run = _dry_run, names = %names_vec.join(" "), "launched terminal for install");
                launched = true;
            }
            Err(()) => {
                tracing::warn!(terminal = %term, names = %names_vec.join(" "), "failed to spawn terminal, trying next");
            }
        }
    }

    if !launched {
        for (term, args, needs_xfce_command) in terms {
            if command_on_path(term) {
                match try_spawn_terminal(term, args, *needs_xfce_command, &cmd_str) {
                    Ok(()) => {
                        tracing::info!(terminal = %term, total = _items.len(), aur_count = aur.len(), official_count = official.len(), dry_run = _dry_run, names = %names_vec.join(" "), "launched terminal for install");
                        launched = true;
                        break;
                    }
                    Err(()) => {
                        tracing::warn!(terminal = %term, names = %names_vec.join(" "), "failed to spawn terminal, trying next");
                    }
                }
            }
        }
    }
    if !launched {
        let res = Command::new("bash").args(["-lc", &cmd_str]).spawn();
        if let Err(e) = res {
            tracing::error!(error = %e, names = %names_vec.join(" "), "failed to spawn bash to run install command");
        } else {
            tracing::info!(total = _items.len(), aur_count = aur.len(), official_count = official.len(), dry_run = _dry_run, names = %names_vec.join(" "), "launched bash for install");
        }
    }

    if !_dry_run {
        let names: Vec<String> = _items.iter().map(|p| p.name.clone()).collect();
        if !names.is_empty()
            && let Err(e) = log_installed(&names)
        {
            tracing::warn!(error = %e, count = names.len(), "failed to write install audit log");
        }
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Confirm batch installs launch gnome-terminal with the expected separator arguments.
    ///
    /// Inputs:
    /// - Shim `gnome-terminal` scripted to capture argv via `PACSEA_TEST_OUT`.
    /// - `spawn_install_all` invoked with two official packages in dry-run mode.
    ///
    /// Output:
    /// - Captured argument list starts with `--`, `bash`, `-lc`, validating safe command invocation.
    ///
    /// Details:
    /// - Overrides `PATH` and environment variables, then restores them to avoid leaking state across tests.
    fn install_batch_uses_gnome_terminal_double_dash() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_inst_batch_gnome_{}_{}",
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
        let mut perms = fs::metadata(&term_path).expect("Failed to read test terminal script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&term_path, perms).expect("Failed to set test terminal script permissions");

        let orig_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", dir.display().to_string());
            std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
        }

        let items = vec![
            crate::state::PackageItem {
                name: "rg".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            },
            crate::state::PackageItem {
                name: "fd".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            },
        ];
        super::spawn_install_all(&items, true);
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
/// What: Present an informational install message on Windows where package management is unsupported.
///
/// Input:
/// - `items`: Packages the user attempted to install.
/// - `dry_run`: When `true`, uses PowerShell to simulate the install operation.
///
/// Output:
/// - Launches a detached PowerShell window (if available) for dry-run simulation, or `cmd` window otherwise.
///
/// Details:
/// - When `dry_run` is true and PowerShell is available, uses PowerShell to simulate the batch install with Write-Host.
/// - Always logs install attempts when not in `dry_run` to remain consistent with Unix behaviour.
/// - During tests, this is a no-op to avoid opening real terminal windows.
pub fn spawn_install_all(_items: &[PackageItem], _dry_run: bool) {
    #[cfg(not(test))]
    {
        let mut names: Vec<String> = _items.iter().map(|p| p.name.clone()).collect();
        if names.is_empty() {
            names.push("nothing".into());
        }
        let names_str = names.join(" ");

        if _dry_run && super::utils::is_powershell_available() {
            // Use PowerShell to simulate the batch install operation
            let powershell_cmd = format!(
                "Write-Host 'DRY RUN: Simulating batch install of {}' -ForegroundColor Yellow; Write-Host 'Packages: {}' -ForegroundColor Cyan; Write-Host ''; Write-Host 'Press any key to close...'; $null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')",
                names.len(),
                names_str.replace("'", "''")
            );
            let _ = Command::new("powershell.exe")
                .args(["-NoProfile", "-Command", &powershell_cmd])
                .spawn();
        } else {
            let msg = if _dry_run {
                format!("DRY RUN: install {names_str}")
            } else {
                format!("Install {names_str} (not supported on Windows)")
            };
            let _ = Command::new("cmd")
                .args([
                    "/C",
                    "start",
                    "Pacsea Install",
                    "cmd",
                    "/K",
                    &format!("echo {msg}"),
                ])
                .spawn();
        }

        if !_dry_run {
            let _ = super::logging::log_installed(&names);
        }
    }
}
