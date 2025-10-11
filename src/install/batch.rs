use std::process::Command;

use crate::state::{PackageItem, Source};

#[cfg(not(target_os = "windows"))]
use super::logging::log_installed;
#[cfg(not(target_os = "windows"))]
use super::utils::command_on_path;

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
    } else if !aur.is_empty() {
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
    };

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
        let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
        if !names.is_empty() {
            let _ = log_installed(&names);
        }
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
            &format!("echo {msg}"),
        ])
        .spawn();
    if !dry_run {
        let _ = super::logging::log_installed(&names);
    }
}
