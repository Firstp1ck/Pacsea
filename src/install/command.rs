use crate::state::{PackageItem, Source};

/// What: Build a shell command to install `item` and indicate whether `sudo` is used.
///
/// Inputs:
/// - `item`: Package to install (Official uses pacman; AUR uses paru/yay)
/// - `password`: Optional sudo password (when provided, uses `sudo -S` pipe)
/// - `dry_run`: When `true`, prints the command instead of executing
///
/// Output:
/// - Tuple `(command_string, uses_sudo)` with a shell-ready command and whether it requires sudo.
pub fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> (String, bool) {
    match &item.source {
        Source::Official { .. } => {
            let reinstall = crate::index::is_installed(&item.name);
            let base_cmd = if reinstall {
                format!("pacman -S --noconfirm {}", item.name)
            } else {
                format!("pacman -S --needed --noconfirm {}", item.name)
            };
            let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            if dry_run {
                let bash = format!("echo DRY RUN: sudo {base_cmd}{hold_tail}");
                return (bash, true);
            }
            let pass = password.unwrap_or("");
            if pass.is_empty() {
                let bash = format!("sudo {base_cmd}{hold_tail}");
                (bash, true)
            } else {
                let escaped = pass.replace('\'', "'\"'\"'\''");
                let pipe = format!("echo '{escaped}' | ");
                let bash = format!("{pipe}sudo -S {base_cmd}{hold_tail}");
                (bash, true)
            }
        }
        Source::Aur => {
            let hold_tail = "; echo; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            let reinstall = crate::index::is_installed(&item.name);
            let flags = if reinstall {
                "-S --noconfirm"
            } else {
                "-S --needed --noconfirm"
            };
            let aur_cmd = if dry_run {
                format!(
                    "echo DRY RUN: paru {flags} {n} || yay {flags} {n}{hold}",
                    n = item.name,
                    hold = hold_tail,
                    flags = flags
                )
            } else {
                format!(
                    "(if command -v paru >/dev/null 2>&1; then paru {flags} {n}; elif command -v yay >/dev/null 2>&1; then yay {flags} {n}; else echo 'No AUR helper (paru/yay) found.'; echo; echo 'Choose AUR helper to install:'; echo '  1) paru'; echo '  2) yay'; echo '  3) cancel'; read -rp 'Enter 1/2/3: ' choice; case \"$choice\" in 1) git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si ;; 2) git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si ;; *) echo 'Cancelled.'; exit 1 ;; esac; if command -v paru >/dev/null 2>&1; then paru {flags} {n}; elif command -v yay >/dev/null 2>&1; then yay {flags} {n}; else echo 'AUR helper installation failed or was cancelled.'; exit 1; fi; fi){hold}",
                    n = item.name,
                    hold = hold_tail,
                    flags = flags
                )
            };
            (aur_cmd, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Build pacman command for official package across variants
    ///
    /// - Input: Official pkg, with/without password, dry-run toggle
    /// - Output: sudo pacman command shape, sudo -S when password, DRY RUN prefix
    fn install_build_install_command_official_variants() {
        let pkg = PackageItem {
            name: "ripgrep".into(),
            version: "14".into(),
            description: String::new(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        };

        let (cmd1, uses_sudo1) = build_install_command(&pkg, None, false);
        assert!(uses_sudo1);
        assert!(cmd1.starts_with("sudo pacman -S --needed --noconfirm ripgrep"));
        assert!(cmd1.contains("Press any key to close"));

        let (cmd2, uses_sudo2) = build_install_command(&pkg, Some("pa's"), false);
        assert!(uses_sudo2);
        assert!(cmd2.contains("echo "));
        assert!(cmd2.contains("sudo -S pacman -S --needed --noconfirm ripgrep"));

        let (cmd3, uses_sudo3) = build_install_command(&pkg, None, true);
        assert!(uses_sudo3);
        assert!(cmd3.starts_with("echo DRY RUN: sudo pacman -S --needed --noconfirm ripgrep"));
    }

    #[test]
    /// What: Build AUR install command with helper preference and dry-run
    ///
    /// - Input: AUR pkg, dry-run false/true
    /// - Output: Paru preferred with yay fallback; DRY RUN echoes helper command
    fn install_build_install_command_aur_variants() {
        let pkg = PackageItem {
            name: "yay-bin".into(),
            version: "1".into(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
        };

        let (cmd1, uses_sudo1) = build_install_command(&pkg, None, false);
        assert!(!uses_sudo1);
        assert!(cmd1.contains("command -v paru"));
        assert!(cmd1.contains("paru -S --needed --noconfirm yay-bin"));
        assert!(cmd1.contains("elif command -v yay"));
        assert!(cmd1.contains("No AUR helper"));
        assert!(cmd1.contains("Press any key to close"));

        let (cmd2, uses_sudo2) = build_install_command(&pkg, None, true);
        assert!(!uses_sudo2);
        assert!(cmd2.starts_with("echo DRY RUN: paru -S --needed --noconfirm yay-bin"));
    }
}
