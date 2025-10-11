use std::process::Command;

#[cfg(not(target_os = "windows"))]
use super::utils::command_on_path;

#[cfg(not(target_os = "windows"))]
pub fn spawn_remove_all(names: &[String], dry_run: bool) {
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
}

#[cfg(target_os = "windows")]
pub fn spawn_remove_all(names: &[String], dry_run: bool) {
    let mut names = names.to_vec();
    if names.is_empty() {
        names.push("nothing".into());
    }
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
}
