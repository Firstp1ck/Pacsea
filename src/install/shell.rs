use std::process::Command;

#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path};

#[cfg(not(target_os = "windows"))]
pub fn spawn_shell_commands_in_terminal(cmds: &[String]) {
    if cmds.is_empty() {
        return;
    }
    let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
    let joined = cmds.join(" && ");
    let cmd_str = format!("{joined}{hold}", hold = hold_tail);
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
        let _ = cmd.spawn();
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
                let _ = cmd.spawn();
                launched = true;
                break;
            }
        }
    }
    if !launched {
        let _ = Command::new("bash").args(["-lc", &cmd_str]).spawn();
    }
}

#[cfg(target_os = "windows")]
pub fn spawn_shell_commands_in_terminal(cmds: &[String]) {
    let msg = if cmds.is_empty() {
        "Nothing to run".to_string()
    } else {
        cmds.join(" && ")
    };
    let _ = Command::new("cmd")
        .args([
            "/C",
            "start",
            "Pacsea Update",
            "cmd",
            "/K",
            &format!("echo {msg}"),
        ])
        .spawn();
}
