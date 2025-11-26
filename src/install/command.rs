use crate::state::{PackageItem, Source};

#[cfg(not(target_os = "windows"))]
use super::utils::shell_single_quote;

/// What: Build the common AUR install body that prefers `paru` and falls back to `yay`.
///
/// Input:
/// - `flags`: Flag string forwarded to the helper (e.g. `-S --needed`).
/// - `n`: Space-separated package names to install.
///
/// Output:
/// - Parenthesised shell snippet `(if ... fi)` without the trailing hold suffix.
///
/// Details:
/// - Prefers `paru` if available, otherwise falls back to `yay`.
/// - Shows error message if no AUR helper is found.
#[must_use]
pub fn aur_install_body(flags: &str, n: &str) -> String {
    format!(
        "(if command -v paru >/dev/null 2>&1; then \
            paru {flags} {n}; \
          elif command -v yay >/dev/null 2>&1; then \
            yay {flags} {n}; \
          else \
            echo 'No AUR helper (paru/yay) found.'; \
          fi)"
    )
}

/// What: Build a shell command to install `item` and indicate whether `sudo` is used.
///
/// Input:
/// - `item`: Package to install (official via pacman, AUR via helper).
/// - `password`: Optional sudo password; when present, wires `sudo -S` with a pipe.
/// - `dry_run`: When `true`, prints the command instead of executing.
///
/// Output:
/// - Tuple `(command_string, uses_sudo)` with a shell-ready command and whether it requires sudo.
///
/// Details:
/// - Uses `--needed` flag for new installs, omits it for reinstalls.
/// - Adds a hold tail so spawned terminals remain open after completion.
#[must_use]
pub fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> (String, bool) {
    match &item.source {
        Source::Official { .. } => {
            let reinstall = crate::index::is_installed(&item.name);
            // For already installed packages, use -Syy to force refresh database and upgrade in one command
            // -Syy forces a full database refresh even if it was recently synced, ensuring we see the latest updates
            let base_cmd = if reinstall {
                format!("pacman -Syy --noconfirm {}", item.name)
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
                let escaped = shell_single_quote(pass);
                let pipe = format!("echo {escaped} | ");
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
                    "{body}{hold}",
                    body = aur_install_body(flags, &item.name),
                    hold = hold_tail
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
    /// What: Check the pacman command builder for official packages handles sudo, password prompts, and dry-run mode.
    ///
    /// Inputs:
    /// - Official package metadata.
    /// - Optional password string.
    /// - Dry-run flag toggled between `false` and `true`.
    ///
    /// Output:
    /// - Returns commands containing the expected pacman flags, optional `sudo -S` echo, and dry-run prefix.
    ///
    /// Details:
    /// - Ensures the hold-tail message persists and the helper flags remain in sync with UI behaviour.
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
        assert!(cmd1.contains("sudo pacman -S --needed --noconfirm ripgrep"));
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
    /// What: Verify AUR command construction selects the correct helper and respects dry-run output.
    ///
    /// Inputs:
    /// - AUR package metadata.
    /// - Dry-run flag toggled between `false` and `true`.
    ///
    /// Output:
    /// - Produces scripts that prefer `paru`, fall back to `yay`, and emit a dry-run echo when requested.
    ///
    /// Details:
    /// - Asserts the crafted shell script still includes the hold-tail prompt and missing-helper warning.
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
