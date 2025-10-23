#[cfg(not(target_os = "windows"))]
use crate::state::Source;
use std::process::Command;

use crate::state::PackageItem;

#[cfg(not(target_os = "windows"))]
use super::logging::log_installed;
#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

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
    let names_vec: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
    tracing::info!(
        total = items.len(),
        aur_count = aur.len(),
        official_count = official.len(),
        dry_run,
        names = %names_vec.join(" "),
        "spawning install"
    );
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
        ("xfce4-terminal", &[], true),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
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
        let spawn_res = cmd.spawn();
        match spawn_res {
            Ok(_) => {
                tracing::info!(terminal = %term, total = items.len(), aur_count = aur.len(), official_count = official.len(), dry_run, names = %names_vec.join(" "), "launched terminal for install");
            }
            Err(e) => {
                tracing::warn!(terminal = %term, error = %e, names = %names_vec.join(" "), "failed to spawn terminal, trying next");
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
                let spawn_res = cmd.spawn();
                match spawn_res {
                    Ok(_) => {
                        tracing::info!(terminal = %term, total = items.len(), aur_count = aur.len(), official_count = official.len(), dry_run, names = %names_vec.join(" "), "launched terminal for install");
                    }
                    Err(e) => {
                        tracing::warn!(terminal = %term, error = %e, names = %names_vec.join(" "), "failed to spawn terminal, trying next");
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
            tracing::error!(error = %e, names = %names_vec.join(" "), "failed to spawn bash to run install command");
        } else {
            tracing::info!(total = items.len(), aur_count = aur.len(), official_count = official.len(), dry_run, names = %names_vec.join(" "), "launched bash for install");
        }
    }

    if !dry_run {
        let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
        if !names.is_empty()
            && let Err(e) = log_installed(&names)
        {
            tracing::warn!(error = %e, count = names.len(), "failed to write install audit log");
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
