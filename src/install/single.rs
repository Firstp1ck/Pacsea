use std::process::Command;

use crate::state::PackageItem;
#[cfg(not(target_os = "windows"))]
use crate::state::Source;

use super::command::build_install_command;
#[cfg(not(target_os = "windows"))]
use super::logging::log_installed;
#[cfg(not(target_os = "windows"))]
use super::utils::command_on_path;

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
        ("xfce4-terminal", &["--", "bash", "-lc"], false),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
    let mut launched = false;
    for (term, args, _hold) in terms {
        if command_on_path(term) {
            let spawn_res = Command::new(term)
                .args(args.iter().copied())
                .arg(&cmd_str)
                .spawn();
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
