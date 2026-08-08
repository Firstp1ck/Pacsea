//! Safe install-planning adapters.

use crate::state::modal::CascadeMode;
use crate::state::{PackageItem, Source};

/// What: Build a neutral install plan for one Pacsea package through arch-toolkit.
///
/// Inputs:
/// - `item`: Package selected for installation.
/// - `reinstall`: Whether to omit `--needed`.
///
/// Output:
/// - Shell-safe neutral command body without privilege, password, dry-run, or terminal policy.
///
/// Details:
/// - AUR plans select paru/yay at shell runtime and fail with status 127 when neither exists.
pub fn single_install(item: &PackageItem, reinstall: bool) -> Result<String, String> {
    let options = install_options(reinstall);
    match item.source {
        Source::Official { .. } => {
            arch_toolkit::install::build_pacman_install(std::slice::from_ref(&item.name), &options)
                .map(|spec| spec.to_shell_string())
                .map_err(|error| error.to_string())
        }
        Source::Aur => arch_toolkit::install::aur_install_shell_fallback(
            std::slice::from_ref(&item.name),
            &options,
        )
        .map_err(|error| error.to_string()),
    }
}

/// What: Build neutral grouped install command bodies through arch-toolkit.
///
/// Inputs:
/// - `official`: Official package names.
/// - `aur`: AUR package names.
/// - `official_reinstall`: Whether the official group contains an installed package.
/// - `aur_reinstall`: Whether the AUR group contains an installed package.
///
/// Output:
/// - Optional official and AUR command bodies in execution order.
///
/// Details:
/// - Pacsea remains responsible for privilege wrapping, short-circuit chaining, dry-run, and hold tails.
pub fn batch_install(
    official: &[String],
    aur: &[String],
    official_reinstall: bool,
    aur_reinstall: bool,
) -> Result<(Option<String>, Option<String>), String> {
    let official_command = if official.is_empty() {
        None
    } else {
        Some(
            arch_toolkit::install::build_pacman_install(
                official,
                &install_options(official_reinstall),
            )
            .map_err(|error| error.to_string())?
            .to_shell_string(),
        )
    };
    let aur_command = if aur.is_empty() {
        None
    } else {
        Some(
            arch_toolkit::install::aur_install_shell_fallback(aur, &install_options(aur_reinstall))
                .map_err(|error| error.to_string())?,
        )
    };
    Ok((official_command, aur_command))
}

/// What: Build a neutral removal command through arch-toolkit.
///
/// Inputs:
/// - `names`: Package names selected for removal.
/// - `cascade`: Pacsea removal cascade mode.
///
/// Output:
/// - Shell-safe pacman command without privilege or password policy.
///
/// Details:
/// - The toolkit emits a `--` operand terminator after validating every name.
pub fn remove(names: &[String], cascade: CascadeMode) -> Result<String, String> {
    arch_toolkit::install::build_remove_command(names, cascade_mode(cascade), true)
        .map(|spec| spec.to_shell_string())
        .map_err(|error| error.to_string())
}

/// What: Build a neutral full-system update command through arch-toolkit.
///
/// Inputs:
/// - `force_sync`: Whether to force-refresh sync databases.
///
/// Output:
/// - Shell-safe pacman command without privilege policy.
///
/// Details:
/// - AUR updates remain a distinct helper plan so Pacsea can short-circuit after pacman failure.
pub fn official_update(force_sync: bool) -> String {
    if force_sync {
        arch_toolkit::install::build_force_sync_update_command(None, true).to_shell_string()
    } else {
        arch_toolkit::install::build_update_command(None, true).to_shell_string()
    }
}

/// What: Build a shell-time AUR update fallback through arch-toolkit.
///
/// Inputs:
/// - None.
///
/// Output:
/// - paru/yay fallback body that exits 127 when no helper is installed.
///
/// Details:
/// - Never privilege-wraps AUR helpers.
pub fn aur_update() -> String {
    arch_toolkit::install::aur_update_shell_fallback(true)
}

/// What: Validate package operands through arch-toolkit.
///
/// Inputs:
/// - `names`: Package names.
/// - `context`: Human-readable planning context.
///
/// Output:
/// - Success or toolkit validation error text.
///
/// Details:
/// - Rejects empty, uppercase, leading `-`/`.`, and disallowed-character names.
#[cfg(test)]
pub fn validate_names(names: &[String], context: &str) -> Result<(), String> {
    arch_toolkit::install::validate_package_names(names, context).map_err(|error| error.to_string())
}

/// What: Build toolkit install options preserving Pacsea reinstall behavior.
///
/// Inputs:
/// - `reinstall`: Whether the target group contains an installed package.
///
/// Output:
/// - Non-interactive AUR-restricted options.
///
/// Details:
/// - `needed` is disabled only for explicit reinstall paths.
const fn install_options(reinstall: bool) -> arch_toolkit::InstallOptions {
    arch_toolkit::InstallOptions {
        needed: !reinstall,
        noconfirm: true,
        aur_only: true,
    }
}

/// What: Convert Pacsea removal cascade policy into toolkit planning policy.
///
/// Inputs:
/// - `mode`: Pacsea cascade mode.
///
/// Output:
/// - Equivalent toolkit cascade mode.
///
/// Details:
/// - All variants map one-to-one.
const fn cascade_mode(mode: CascadeMode) -> arch_toolkit::CascadeMode {
    match mode {
        CascadeMode::Basic => arch_toolkit::CascadeMode::Basic,
        CascadeMode::Cascade => arch_toolkit::CascadeMode::Cascade,
        CascadeMode::CascadeWithConfigs => arch_toolkit::CascadeMode::CascadeWithConfigs,
    }
}

#[cfg(test)]
mod tests {
    use crate::state::modal::CascadeMode;
    use crate::state::{PackageItem, Source};

    /// What: Verify official install planning uses argv-safe operand separation.
    ///
    /// Inputs:
    /// - One official package and fresh-install policy.
    ///
    /// Output:
    /// - pacman plan with `--needed`, `--noconfirm`, and `--`.
    ///
    /// Details:
    /// - No command is executed.
    #[test]
    fn official_plan_contains_operand_terminator() {
        let item = PackageItem {
            name: "ripgrep".to_string(),
            version: String::new(),
            description: String::new(),
            source: Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };
        assert_eq!(
            super::single_install(&item, false).expect("plan should build"),
            "pacman -S --needed --noconfirm -- ripgrep"
        );
    }

    /// What: Verify leading-option and hidden package names fail before planning.
    ///
    /// Inputs:
    /// - Names beginning with `-` and `.`.
    ///
    /// Output:
    /// - Validation errors for both.
    ///
    /// Details:
    /// - Regression coverage for option-injection and hidden-name confusion.
    #[test]
    fn rejects_leading_option_and_hidden_names() {
        assert!(super::validate_names(&["-Syu".to_string()], "test").is_err());
        assert!(super::validate_names(&[".hidden".to_string()], "test").is_err());
    }

    /// What: Verify removal planning maps cascade policy and operand separation.
    ///
    /// Inputs:
    /// - One package and config-pruning cascade mode.
    ///
    /// Output:
    /// - Neutral pacman removal command.
    ///
    /// Details:
    /// - No privilege wrapper or command execution occurs in the adapter.
    #[test]
    fn removal_plan_maps_cascade() {
        assert_eq!(
            super::remove(&["ripgrep".to_string()], CascadeMode::CascadeWithConfigs)
                .expect("plan should build"),
            "pacman -Rns --noconfirm -- ripgrep"
        );
    }
}
