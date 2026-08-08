//! Builds terminal shell commands from neutral arch-toolkit install plans.

use crate::state::{PackageItem, Source};

use super::utils::shell_single_quote;

/// Terminal tail appended after one install plan finishes.
const HOLD_TAIL: &str = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";

/// What: Build a shell command to install one package from a neutral toolkit plan.
///
/// Inputs:
/// - `item`: Package to install.
/// - `password`: Optional privilege password for Pacsea's existing sudo session policy.
/// - `dry_run`: Whether to display rather than execute the plan.
///
/// Output:
/// - Shell command and whether the operation uses a privilege tool.
///
/// # Errors
///
/// Returns an actionable error when package validation, toolkit planning, or privilege resolution
/// fails.
///
/// Details:
/// - arch-toolkit validates operands and inserts `--`; Pacsea retains privilege/password, dry-run,
///   PTY, hold-tail, and execution ownership. AUR helpers are never privilege-wrapped.
pub fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> Result<(String, bool), String> {
    let reinstall = crate::index::is_installed(&item.name);
    let neutral = crate::integrations::arch_toolkit::install::single_install(item, reinstall)?;
    match item.source {
        Source::Official { .. } => {
            let tool = crate::logic::privilege::active_tool()?;
            let privileged = password.filter(|value| !value.is_empty()).map_or_else(
                || crate::logic::privilege::build_privilege_command(tool, &neutral),
                |value| {
                    crate::logic::privilege::build_password_pipe(tool, value, &neutral)
                        .unwrap_or_else(|| {
                            crate::logic::privilege::build_privilege_command(tool, &neutral)
                        })
                },
            );
            Ok((render_terminal_command(&privileged, dry_run), true))
        }
        Source::Aur => Ok((render_terminal_command(&neutral, dry_run), false)),
    }
}

/// What: Add dry-run presentation and the external-terminal hold tail.
///
/// Inputs:
/// - `command`: Shell-safe neutral or privilege-wrapped command.
/// - `dry_run`: Whether to echo rather than execute it.
///
/// Output:
/// - Final shell string for Pacsea's terminal launcher.
///
/// Details:
/// - Dry-run quotes the complete command as one display operand and never executes it.
fn render_terminal_command(command: &str, dry_run: bool) -> String {
    let with_hold = format!("{command}{HOLD_TAIL}");
    if dry_run {
        format!("echo DRY RUN: {}", shell_single_quote(&with_hold))
    } else {
        with_hold
    }
}

#[cfg(test)]
mod tests {
    use super::build_install_command;
    use crate::state::{PackageItem, Source};

    /// What: Verify official toolkit plans retain Pacsea privilege and dry-run policy.
    ///
    /// Inputs:
    /// - One official package in normal and dry-run modes.
    ///
    /// Output:
    /// - Privileged pacman command with operand terminator and nonexecuting dry-run display.
    ///
    /// Details:
    /// - The command is built only and never spawned.
    #[test]
    fn official_install_uses_toolkit_plan() {
        let tool = crate::logic::privilege::active_tool().expect("privilege tool should exist");
        let item = package(Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        });
        let (command, privileged) =
            build_install_command(&item, None, false).expect("plan should build");
        assert!(privileged);
        assert!(command.contains(&format!(
            "{} pacman -S --needed --noconfirm -- ripgrep",
            tool.binary_name()
        )));
        assert!(command.contains("Press any key to close"));

        let (dry_run, _) =
            build_install_command(&item, None, true).expect("dry-run plan should build");
        assert!(dry_run.starts_with("echo DRY RUN: '"));
        assert!(dry_run.contains("pacman -S --needed --noconfirm -- ripgrep"));
    }

    /// What: Verify AUR toolkit planning remains unprivileged and fails when no helper exists.
    ///
    /// Inputs:
    /// - One AUR package.
    ///
    /// Output:
    /// - paru/yay fallback with `--aur`, operand terminator, and status-127 branch.
    ///
    /// Details:
    /// - The helper is selected only when the shell body later executes.
    #[test]
    fn aur_install_uses_unprivileged_toolkit_fallback() {
        let item = package(Source::Aur);
        let (command, privileged) =
            build_install_command(&item, None, false).expect("plan should build");
        assert!(!privileged);
        assert!(command.contains("paru -S --aur --needed --noconfirm -- ripgrep"));
        assert!(command.contains("yay -S --aur --needed --noconfirm -- ripgrep"));
        assert!(command.contains("exit 127"));
    }

    /// What: Create a deterministic package fixture.
    ///
    /// Inputs:
    /// - `source`: Desired package source.
    ///
    /// Output:
    /// - Pacsea package row named `ripgrep`.
    ///
    /// Details:
    /// - No host package metadata is read while constructing the fixture.
    fn package(source: Source) -> PackageItem {
        PackageItem {
            name: "ripgrep".to_string(),
            version: String::new(),
            description: String::new(),
            source,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }
    }
}
