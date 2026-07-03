//! Command-line remove functionality.

use crate::args::{guardrails, i18n, utils};
use pacsea::logic::preflight::guardrails::GuardrailOperation;

/// What: Check for configuration directories in `$HOME/PACKAGE_NAME` and `$HOME/.config/PACKAGE_NAME`.
///
/// Inputs:
/// - `package_name`: Name of the package to check for config directories.
/// - `home`: Home directory path.
///
/// Output:
/// - Vector of found config directory paths.
///
/// Details:
/// - Checks both `$HOME/PACKAGE_NAME` and `$HOME/.config/PACKAGE_NAME`.
/// - Only returns directories that actually exist.
fn check_config_directories(package_name: &str, home: &str) -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut found_dirs = Vec::new();

    // Check $HOME/PACKAGE_NAME
    let home_pkg_dir = PathBuf::from(home).join(package_name);
    if home_pkg_dir.exists() && home_pkg_dir.is_dir() {
        found_dirs.push(home_pkg_dir);
    }

    // Check $HOME/.config/PACKAGE_NAME
    let config_pkg_dir = PathBuf::from(home).join(".config").join(package_name);
    if config_pkg_dir.exists() && config_pkg_dir.is_dir() {
        found_dirs.push(config_pkg_dir);
    }

    found_dirs
}

/// What: Handle command-line remove mode by removing packages via pacman.
///
/// Inputs:
/// - `packages`: Vector of package names (comma-separated or space-separated).
///
/// Output:
/// - Exits the process with the command's exit code or 1 on error.
///
/// Details:
/// - Parses package names (handles comma-separated and space-separated).
/// - Delegates to [`remove_packages`] for confirmation and execution.
pub fn handle_remove(packages: &[String]) -> ! {
    tracing::info!(packages = ?packages, "Remove mode requested from CLI");

    let package_names = utils::parse_package_names(packages);
    if package_names.is_empty() {
        eprintln!("{}", i18n::t("app.cli.remove.no_packages"));
        tracing::error!("No packages specified for removal");
        std::process::exit(1);
    }

    remove_packages(&package_names)
}

/// What: Handle command-line remove-from-file mode by removing packages listed in a file.
///
/// Inputs:
/// - `file_path`: Path to a file containing package names (one per line).
///
/// Output:
/// - Exits the process with the command's exit code or 1 on error.
///
/// Details:
/// - Reads package names from file (one per line); ignores empty lines,
///   `#` comments, and lines containing spaces (parity with `-I`).
/// - Delegates to [`remove_packages`] for confirmation and execution.
pub fn handle_remove_from_file(file_path: &str) -> ! {
    tracing::info!(file = %file_path, "Remove from file requested from CLI");

    let package_names = utils::read_packages_from_file(file_path);
    if package_names.is_empty() {
        eprintln!(
            "{}",
            i18n::t_fmt1("app.cli.remove.no_packages_in_file", file_path)
        );
        tracing::error!(file = %file_path, "No packages found in file");
        std::process::exit(1);
    }

    tracing::info!(
        file = %file_path,
        package_count = package_names.len(),
        "Read packages from file"
    );

    remove_packages(&package_names)
}

/// What: Confirm and remove the given packages via pacman, then exit.
///
/// Inputs:
/// - `package_names`: Already-parsed package names to remove (non-empty).
///
/// Output:
/// - Exits the process with the command's exit code or 1 on error.
///
/// Details:
/// - Shows warning about removal and no backup.
/// - Prompts user with [y/N] (No is default).
/// - Executes `sudo pacman -Rns` to remove packages.
/// - After removal, checks for config directories in `$HOME/PACKAGE_NAME` and `$HOME/.config/PACKAGE_NAME`.
/// - Shows found config directories in a list.
/// - Exits immediately after removal (doesn't launch TUI).
fn remove_packages(package_names: &[String]) -> ! {
    use std::process::Command;

    // Guardrails: db lock aborts before prompting
    guardrails::enforce(GuardrailOperation::Remove);

    // Show warning message
    eprintln!("\n{}", i18n::t("app.cli.remove.warning"));
    eprintln!("\n{}", i18n::t("app.cli.remove.packages_to_remove"));
    for pkg in package_names {
        eprintln!("  - {pkg}");
    }
    eprintln!();

    // Prompt user for confirmation (defaults to No)
    if !utils::prompt_user_no_default(&i18n::t("app.cli.remove.prompt")) {
        tracing::info!("User cancelled removal");
        println!("{}", i18n::t("app.cli.remove.cancelled"));
        std::process::exit(0);
    }

    // Execute sudo pacman -Rns
    tracing::info!(packages = ?package_names, "Removing packages");
    let tool = match pacsea::logic::privilege::active_tool() {
        Ok(t) => t,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };
    let status = Command::new(tool.binary_name())
        .arg("pacman")
        .arg("-Rns")
        .args(package_names)
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            tracing::info!("Packages removed successfully");
            println!("\n{}", i18n::t("app.cli.remove.success"));

            // Check for config directories after removal
            if let Ok(home) = std::env::var("HOME") {
                let mut found_configs = Vec::new();
                for pkg in package_names {
                    let config_dirs = check_config_directories(pkg, &home);
                    for dir in config_dirs {
                        found_configs.push((pkg.clone(), dir));
                    }
                }

                if !found_configs.is_empty() {
                    println!("\n{}", i18n::t("app.cli.remove.config_dirs_found"));
                    for (pkg, dir) in &found_configs {
                        println!("  - {}: {}", pkg, dir.display());
                    }
                    println!("\n{}", i18n::t("app.cli.remove.config_dirs_note"));
                }
            }

            std::process::exit(0);
        }
        Ok(exit_status) => {
            eprintln!("\n{}", i18n::t("app.cli.remove.failed"));
            tracing::error!(exit_code = exit_status.code(), "Failed to remove packages");
            std::process::exit(exit_status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("\n{}", i18n::t_fmt1("app.cli.remove.exec_failed", &e));
            tracing::error!(error = %e, "Failed to execute pacman");
            std::process::exit(1);
        }
    }
}
