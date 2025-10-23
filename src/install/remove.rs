use std::process::Command;

#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path};

#[cfg(not(target_os = "windows"))]
pub fn spawn_remove_all(names: &[String], dry_run: bool) {
    let names_str = names.join(" ");
    tracing::info!(names = %names_str, total = names.len(), dry_run, "spawning removal");
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
    let cmd_str = if dry_run {
        format!(
            "echo DRY RUN: sudo pacman -Rns --noconfirm {n}{hold}",
            n = names.join(" "),
            hold = hold_tail
        )
    } else {
        format!(
            "sudo pacman -Rns --noconfirm {n}{hold}",
            n = names.join(" "),
            hold = hold_tail
        )
    };
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
    if let Some(idx) = choose_terminal_index_prefer_path(terms) {
        let (term, args, _hold) = terms[idx];
        let mut cmd = Command::new(term);
        cmd.args(args.iter().copied()).arg(&cmd_str);
        if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
            if let Some(parent) = std::path::Path::new(&p).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            cmd.env("PACSEA_TEST_OUT", p);
        }
        let spawn_res = cmd.spawn();
        match spawn_res {
            Ok(_) => {
                tracing::info!(terminal = %term, names = %names_str, total = names.len(), dry_run, "launched terminal for removal")
            }
            Err(e) => {
                tracing::warn!(terminal = %term, error = %e, names = %names_str, "failed to spawn terminal, trying next");
            }
        }
        launched = true;
    } else {
        for (term, args, _hold) in terms {
            if command_on_path(term) {
                let mut cmd = Command::new(term);
                cmd.args(args.iter().copied()).arg(&cmd_str);
                if let Ok(p) = std::env::var("PACSEA_TEST_OUT") {
                    if let Some(parent) = std::path::Path::new(&p).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    cmd.env("PACSEA_TEST_OUT", p);
                }
                let spawn_res = cmd.spawn();
                match spawn_res {
                    Ok(_) => {
                        tracing::info!(terminal = %term, names = %names_str, total = names.len(), dry_run, "launched terminal for removal")
                    }
                    Err(e) => {
                        tracing::warn!(terminal = %term, error = %e, names = %names_str, "failed to spawn terminal, trying next");
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
            tracing::error!(error = %e, names = %names_str, "failed to spawn bash to run removal command");
        } else {
            tracing::info!(names = %names_str, total = names.len(), dry_run, "launched bash for removal");
        }
    }
}

#[cfg(target_os = "windows")]
pub fn spawn_remove_all(names: &[String], dry_run: bool) {
    let mut names = names.to_vec();
    if names.is_empty() {
        names.push("nothing".into());
    }
    let names_str = names.join(" ");
    tracing::info!(names = %names_str, total = names.len(), dry_run, "spawning removal");
    let msg = if dry_run {
        format!("DRY RUN: remove {}", names.join(" "))
    } else {
        format!("Remove {} (not supported on Windows)", names.join(" "))
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
    tracing::info!(names = %names_str, total = names.len(), dry_run, "launched cmd for removal");
}
