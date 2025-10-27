#[cfg(not(target_os = "windows"))]
use crate::state::Source;
use std::process::Command;

use crate::state::PackageItem;

#[cfg(not(target_os = "windows"))]
use super::logging::log_installed;
#[cfg(not(target_os = "windows"))]
use super::utils::{choose_terminal_index_prefer_path, command_on_path, shell_single_quote};

#[cfg(not(target_os = "windows"))]
/// Spawn a terminal to install a batch of packages.
///
/// Inputs:
/// - `items`: Packages to install (official are grouped for pacman, AUR via paru/yay).
/// - `dry_run`: When `true`, prints commands instead of executing.
///
/// Output:
/// - Launches a terminal (or falls back to `bash`) running the composed install commands.
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
            "(command -v paru >/dev/null 2>&1 && paru -S --needed --noconfirm {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed --noconfirm {n}) || (echo 'No AUR helper (paru/yay) found.'; echo; echo 'Choose AUR helper to install:'; echo '  1) paru'; echo '  2) yay'; echo '  3) cancel'; read -rp 'Enter 1/2/3: ' choice; case \"$choice\" in 1) git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si ;; 2) git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si ;; *) echo 'Cancelled.'; exit 1 ;; esac; if command -v paru >/dev/null 2>&1; then paru -S --needed --noconfirm {n}; elif command -v yay >/dev/null 2>&1; then yay -S --needed --noconfirm {n}; else echo 'AUR helper installation failed or was cancelled.'; exit 1; fi){hold}",
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

    // Prefer GNOME Terminal when running under GNOME desktop
    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .map(|v| v.to_uppercase().contains("GNOME"))
        .unwrap_or(false);
    let terms_gnome_first: &[(&str, &[&str], bool)] = &[
        ("gnome-terminal", &["--", "bash", "-lc"], false),
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
        ("konsole", &["-e", "bash", "-lc"], false),
        ("xfce4-terminal", &[], true),
        ("tilix", &["--", "bash", "-lc"], false),
        ("mate-terminal", &["--", "bash", "-lc"], false),
    ];
    let terms = if is_gnome { terms_gnome_first } else { terms_default };
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

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Ensure gnome-terminal is invoked with double dash for batch install
    ///
    /// - Input: Fake gnome-terminal on PATH; spawn_install_all with dry_run
    /// - Output: First args are "--", "bash", "-lc" (safe arg shape)
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
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut out_path = dir.clone();
        out_path.push("args.txt");
        let mut term_path = dir.clone();
        term_path.push("gnome-terminal");
        let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
        fs::write(&term_path, script.as_bytes()).unwrap();
        let mut perms = fs::metadata(&term_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&term_path, perms).unwrap();

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
        assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
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
