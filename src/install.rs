use std::fs;
use std::process::Command;

use crate::state::{PackageItem, Source};

// Helper: check whether a command exists on PATH (Unix-aware exec bit)
fn command_on_path(cmd: &str) -> bool {
    use std::path::Path;

    fn is_exec(p: &std::path::Path) -> bool {
        if !p.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(p) {
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
pub fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);
    let _ = Command::new("cmd")
        .args(["/C", "start", "Pacsea Install", "cmd", "/K", &cmd_str])
        .spawn();
    if !dry_run {
        let _ = log_installed(&[item.name.clone()]);
    }
}

pub fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> (String, bool) {
    match &item.source {
        Source::Official { .. } => {
            let base_cmd = format!("pacman -S --needed {}", item.name);
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
                    "echo DRY RUN: paru -S --needed {n} || yay -S --needed {n}{hold}",
                    n = item.name,
                    hold = hold_tail
                )
            } else {
                format!(
                    "(command -v paru >/dev/null 2>&1 && paru -S --needed {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed {n}) || echo 'No AUR helper (paru/yay) found.'{hold}",
                    n = item.name,
                    hold = hold_tail
                )
            };
            (aur_cmd, false)
        }
    }
}

#[cfg(not(target_os = "windows"))]
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
    let mut parts: Vec<String> = Vec::new();
    if dry_run {
        if !official.is_empty() {
            parts.push(format!(
                "echo DRY RUN: sudo pacman -S --needed {}",
                official.join(" ")
            ));
        }
        if !aur.is_empty() {
            parts.push(format!(
                "echo DRY RUN: (paru -S --needed {} || yay -S --needed {})",
                aur.join(" "),
                aur.join(" ")
            ));
        }
        if parts.is_empty() {
            parts.push("echo DRY RUN: nothing to install".to_string());
        }
        let cmd_str = format!("{}{}", parts.join("; "), hold_tail);
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
        return;
    }

    if !official.is_empty() {
        parts.push(format!("sudo pacman -S --needed {}", official.join(" ")));
    }
    if !aur.is_empty() {
        let names = aur.join(" ");
        parts.push(format!("(command -v paru >/dev/null 2>&1 && paru -S --needed {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed {n}) || echo 'No AUR helper (paru/yay) found.'", n = names));
    }
    if parts.is_empty() {
        parts.push("echo nothing to install".to_string());
    }
    let cmd_str = format!("{}{}", parts.join("; "), hold_tail);

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
    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
    if !names.is_empty() {
        let _ = log_installed(&names);
    }
}

#[cfg(target_os = "windows")]
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

fn log_installed(names: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    let mut path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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
