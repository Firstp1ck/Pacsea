//! Builds shell commands for installing packages via pacman or AUR helpers.

use crate::state::{PackageItem, Source};

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
            let tool = crate::logic::privilege::active_tool();
            let reinstall = crate::index::is_installed(&item.name);
            let base_cmd = if reinstall {
                format!("pacman -S --noconfirm {}", item.name)
            } else {
                format!("pacman -S --needed --noconfirm {}", item.name)
            };
            let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            if dry_run {
                let cmd = format!(
                    "{}{hold_tail}",
                    crate::logic::privilege::build_privilege_command(tool, &base_cmd)
                );
                let quoted = shell_single_quote(&cmd);
                let bash = format!("echo DRY RUN: {quoted}");
                return (bash, true);
            }
            let pass = password.unwrap_or("");
            if pass.is_empty() {
                let bash = format!(
                    "{}{hold_tail}",
                    crate::logic::privilege::build_privilege_command(tool, &base_cmd)
                );
                (bash, true)
            } else {
                let piped = crate::logic::privilege::build_password_pipe(tool, pass, &base_cmd);
                let priv_cmd = piped.unwrap_or_else(|| {
                    crate::logic::privilege::build_privilege_command(tool, &base_cmd)
                });
                let bash = format!("{priv_cmd}{hold_tail}");
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
                let cmd = format!(
                    "paru {flags} {n} || yay {flags} {n}{hold}",
                    n = item.name,
                    hold = hold_tail,
                    flags = flags
                );
                let quoted = shell_single_quote(&cmd);
                format!("echo DRY RUN: {quoted}")
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
    /// What: Check the pacman command builder for official packages handles privilege tool,
    /// password prompts, and dry-run mode.
    ///
    /// Inputs:
    /// - Official package metadata.
    /// - Optional password string.
    /// - Dry-run flag toggled between `false` and `true`.
    ///
    /// Output:
    /// - Returns commands containing the expected pacman flags, optional piped password,
    ///   and dry-run prefix.
    ///
    /// Details:
    /// - Ensures the hold-tail message persists and the helper flags remain in sync with UI behaviour.
    /// - Uses privilege abstraction so output adapts to active tool (sudo or doas).
    fn install_build_install_command_official_variants() {
        let tool = crate::logic::privilege::active_tool();
        let bin = tool.binary_name();

        let pkg = PackageItem {
            name: "ripgrep".into(),
            version: "14".into(),
            description: String::new(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };

        let (cmd1, uses_sudo1) = build_install_command(&pkg, None, false);
        assert!(uses_sudo1);
        assert!(
            cmd1.contains(&format!("{bin} pacman -S --needed --noconfirm ripgrep")),
            "expected '{bin} pacman -S --needed --noconfirm ripgrep' in: {cmd1}"
        );
        assert!(cmd1.contains("Press any key to close"));

        let (cmd2, uses_sudo2) = build_install_command(&pkg, Some("pa's"), false);
        assert!(uses_sudo2);
        if tool.capabilities().supports_stdin_password {
            assert!(
                cmd2.contains(&format!("{bin} -S pacman -S --needed --noconfirm ripgrep")),
                "expected '{bin} -S pacman ...' in: {cmd2}"
            );
        } else {
            assert!(
                cmd2.contains(&format!("{bin} pacman -S --needed --noconfirm ripgrep")),
                "doas fallback should use plain command: {cmd2}"
            );
        }

        let (cmd3, uses_sudo3) = build_install_command(&pkg, None, true);
        assert!(uses_sudo3);
        assert!(cmd3.starts_with("echo DRY RUN: '"));
        assert!(
            cmd3.contains(&format!("{bin} pacman -S --needed --noconfirm ripgrep")),
            "expected '{bin} pacman ...' in dry-run: {cmd3}"
        );
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
            out_of_date: None,
            orphaned: false,
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
        // Dry-run commands are now properly quoted to avoid syntax errors
        assert!(cmd2.starts_with("echo DRY RUN: '"));
        assert!(cmd2.contains("paru -S --needed --noconfirm yay-bin"));
    }
}
