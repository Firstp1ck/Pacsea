//! Command-line remove functionality.

use crate::args::{i18n, utils};

/// What: Check for configuration directories in $HOME/PACKAGE_NAME and $HOME/.config/PACKAGE_NAME.
///
/// Inputs:
/// - `package_name`: Name of the package to check for config directories.
/// - `home`: Home directory path.
///
/// Output:
/// - Vector of found config directory paths.
///
/// Details:
/// - Checks both $HOME/PACKAGE_NAME and $HOME/.config/PACKAGE_NAME.
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
/// - Shows warning about removal and no backup.
/// - Prompts user with [y/N] (No is default).
/// - Executes `sudo pacman -Rns` to remove packages.
/// - After removal, checks for config directories in $HOME/PACKAGE_NAME and $HOME/.config/PACKAGE_NAME.
/// - Shows found config directories in a list.
/// - Exits immediately after removal (doesn't launch TUI).
pub fn handle_remove(packages: &[String]) -> ! {
    use std::process::Command;

    tracing::info!(packages = ?packages, "Remove mode requested from CLI");

    let package_names = utils::parse_package_names(packages);
    if package_names.is_empty() {
        eprintln!("{}", i18n::t("app.cli.remove.no_packages"));
        tracing::error!("No packages specified for removal");
        std::process::exit(1);
    }

    // Show warning message
    eprintln!("\n{}", i18n::t("app.cli.remove.warning"));
    eprintln!("\n{}", i18n::t("app.cli.remove.packages_to_remove"));
    for pkg in &package_names {
        eprintln!("{}", i18n::t_fmt1("app.cli.remove.package_item", pkg));
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
    let status = Command::new("sudo")
        .arg("pacman")
        .arg("-Rns")
        .args(&package_names)
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            tracing::info!("Packages removed successfully");
            println!("\n{}", i18n::t("app.cli.remove.success"));

            // Check for config directories after removal
            if let Ok(home) = std::env::var("HOME") {
                let mut found_configs = Vec::new();
                for pkg in &package_names {
                    let config_dirs = check_config_directories(pkg, &home);
                    for dir in config_dirs {
                        found_configs.push((pkg.clone(), dir));
                    }
                }

                if !found_configs.is_empty() {
                    println!("\n{}", i18n::t("app.cli.remove.config_dirs_found"));
                    for (pkg, dir) in &found_configs {
                        println!("{}", i18n::t_fmt2("app.cli.remove.config_dir_item", pkg, dir.display()));
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
