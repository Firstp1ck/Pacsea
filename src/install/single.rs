use std::process::Command;

use crate::state::PackageItem;
#[cfg(not(target_os = "windows"))]
use crate::state::Source;

use super::command::build_install_command;
#[cfg(not(target_os = "windows"))]
use super::logging::log_installed;
#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
pub fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, uses_sudo) = build_install_command(item, password, dry_run);
    let src = match item.source {
        Source::Official { .. } => "official",
        Source::Aur => "aur",
    };
    tracing::info!(names = %item.name, total = 1, aur_count = (src == "aur") as usize, official_count = (src == "official") as usize, dry_run, uses_sudo, "spawning install");
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
                tracing::info!(terminal = %term, names = %item.name, total = 1, aur_count = (src == "aur") as usize, official_count = (src == "official") as usize, dry_run, "launched terminal for install");
            }
            Err(e) => {
                tracing::warn!(terminal = %term, error = %e, names = %item.name, "failed to spawn terminal, trying next");
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
                        tracing::info!(terminal = %term, names = %item.name, total = 1, aur_count = (src == "aur") as usize, official_count = (src == "official") as usize, dry_run, "launched terminal for install");
                    }
                    Err(e) => {
                        tracing::warn!(terminal = %term, error = %e, names = %item.name, "failed to spawn terminal, trying next");
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
            tracing::error!(error = %e, names = %item.name, "failed to spawn bash to run install command");
        } else {
            tracing::info!(names = %item.name, total = 1, aur_count = (src == "aur") as usize, official_count = (src == "official") as usize, dry_run, "launched bash for install");
        }
    }
    if !dry_run && let Err(e) = log_installed(std::slice::from_ref(&item.name)) {
        tracing::warn!(error = %e, names = %item.name, "failed to write install audit log");
    }
}

#[cfg(target_os = "windows")]
pub fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);
    let _ = Command::new("cmd")
        .args(["/C", "start", "Pacsea Install", "cmd", "/K", &cmd_str])
        .spawn();
    if !dry_run {
        let _ = super::logging::log_installed(&[item.name.clone()]);
    }
}
