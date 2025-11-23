//! Command-line install functionality.

use crate::args::{i18n, package, utils};

/// What: Read and parse package names from a file.
///
/// Inputs:
/// - `file_path`: Path to the file containing package names.
///
/// Output:
/// - Vector of package names, or exits on error.
///
/// Details:
/// - Reads file line by line.
/// - Ignores empty lines.
/// - Ignores lines starting with "#" and ignores text after "#" in any line.
/// - Trims whitespace from package names.
fn read_packages_from_file(file_path: &str) -> Vec<String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "{}",
                i18n::t_fmt(
                    "app.cli.install.file_open_error",
                    &[&file_path as &dyn std::fmt::Display, &e]
                )
            );
            tracing::error!(file = %file_path, error = %e, "Failed to open file");
            std::process::exit(1);
        }
    };

    let reader = BufReader::new(file);
    let mut packages = Vec::new();
    let mut warnings = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let original_line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "{}",
                    i18n::t_fmt(
                        "app.cli.install.file_read_error",
                        &[
                            &(line_num + 1) as &dyn std::fmt::Display,
                            &file_path as &dyn std::fmt::Display,
                            &e,
                        ]
                    )
                );
                tracing::error!(
                    file = %file_path,
                    line = line_num + 1,
                    error = %e,
                    "Failed to read line from file"
                );
                continue;
            }
        };

        // Remove comments (everything after "#")
        let line = original_line.split('#').next().unwrap_or("").trim();

        // Skip empty lines and lines starting with "#"
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check if line contains spaces between words (package names should not have spaces)
        if line.contains(' ') {
            warnings.push((line_num + 1, original_line.trim().to_string()));
            tracing::warn!(
                file = %file_path,
                line = line_num + 1,
                content = %original_line.trim(),
                "Line contains spaces between words"
            );
            continue;
        }

        packages.push(line.to_string());
    }

    // Display warnings if any
    if !warnings.is_empty() {
        eprintln!("\n{}", i18n::t("app.cli.install.lines_with_spaces"));
        for (line_num, content) in &warnings {
            eprintln!(
                "{}",
                i18n::t_fmt2("app.cli.install.line_item", line_num, content)
            );
        }
        eprintln!();
    }

    packages
}

/// What: Install official packages via pacman.
///
/// Inputs:
/// - `packages`: Vector of official package names.
///
/// Output:
/// - `Ok(())` on success, exits the process on failure.
///
/// Details:
/// - Executes `sudo pacman -S` with the package names.
/// - Exits on failure.
fn install_official_packages(packages: &[String]) {
    use std::process::Command;

    if packages.is_empty() {
        return;
    }

    tracing::info!(packages = ?packages, "Installing official packages");
    let status = Command::new("sudo")
        .arg("pacman")
        .arg("-S")
        .args(packages)
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            tracing::info!("Official packages installed successfully");
        }
        Ok(exit_status) => {
            eprintln!("{}", i18n::t("app.cli.install.official_failed"));
            tracing::error!(
                exit_code = exit_status.code(),
                "Failed to install official packages"
            );
            std::process::exit(exit_status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("{}", i18n::t_fmt1("app.cli.install.pacman_exec_failed", &e));
            tracing::error!(error = %e, "Failed to execute pacman");
            std::process::exit(1);
        }
    }
}

/// What: Install AUR packages via paru or yay.
///
/// Inputs:
/// - `packages`: Vector of AUR package names.
/// - `helper`: AUR helper name ("paru" or "yay").
///
/// Output:
/// - `Ok(())` on success, exits the process on failure.
///
/// Details:
/// - Executes helper `-S` with the package names.
/// - Exits on failure.
fn install_aur_packages(packages: &[String], helper: &str) {
    use std::process::Command;

    if packages.is_empty() {
        return;
    }

    tracing::info!(helper = %helper, packages = ?packages, "Installing AUR packages");
    let status = Command::new(helper).arg("-S").args(packages).status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            tracing::info!("AUR packages installed successfully");
        }
        Ok(exit_status) => {
            eprintln!("{}", i18n::t("app.cli.install.aur_failed"));
            tracing::error!(
                exit_code = exit_status.code(),
                "Failed to install AUR packages"
            );
            std::process::exit(exit_status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!(
                "{}",
                i18n::t_fmt2("app.cli.install.aur_helper_exec_failed", helper, &e)
            );
            tracing::error!(error = %e, helper = %helper, "Failed to execute AUR helper");
            std::process::exit(1);
        }
    }
}

/// What: Handle command-line install from file mode by installing packages via pacman or AUR helper.
///
/// Inputs:
/// - `file_path`: Path to file containing package names (one per line).
///
/// Output:
/// - Exits the process with the command's exit code or 1 on error.
///
/// Details:
/// - Reads package names from file (one per line).
/// - Ignores empty lines and lines starting with "#".
/// - Ignores text after "#" in any line.
/// - Checks each package using `sudo pacman -Ss`, then `yay/paru -Ss` if not found.
/// - Warns user if any packages don't exist and asks for confirmation (Yes default).
/// - Determines if packages are official or AUR.
/// - Installs official packages via `sudo pacman -S`.
/// - Installs AUR packages via `paru -S` or `yay -S` (prefers paru).
/// - Exits immediately after installation (doesn't launch TUI).
pub fn handle_install_from_file(file_path: &str) -> ! {
    tracing::info!(file = %file_path, "Install from file requested from CLI");

    // Read packages from file
    let package_names = read_packages_from_file(file_path);
    if package_names.is_empty() {
        eprintln!(
            "{}",
            i18n::t_fmt1("app.cli.install.no_packages_in_file", file_path)
        );
        tracing::error!(file = %file_path, "No packages found in file");
        std::process::exit(1);
    }

    tracing::info!(
        file = %file_path,
        package_count = package_names.len(),
        "Read packages from file"
    );

    // Get AUR helper early to check AUR packages
    let aur_helper = utils::get_aur_helper();

    // Validate and categorize packages using search commands
    let (official_packages, aur_packages, invalid_packages) =
        package::validate_and_categorize_packages_search(&package_names, aur_helper);

    // Handle invalid packages
    if !invalid_packages.is_empty() {
        eprintln!("\n{}", i18n::t("app.cli.install.packages_not_found"));
        for pkg in &invalid_packages {
            eprintln!("  - {pkg}");
        }
        if aur_helper.is_none() && !invalid_packages.is_empty() {
            eprintln!("\n{}", i18n::t("app.cli.install.no_aur_helper_note"));
            eprintln!("{}", i18n::t("app.cli.install.install_aur_helper"));
        }
        eprintln!();

        // If all packages are invalid, exit with error
        if official_packages.is_empty() && aur_packages.is_empty() {
            eprintln!("{}", i18n::t("app.cli.install.no_valid_packages"));
            tracing::error!("All packages are invalid");
            std::process::exit(1);
        }

        // Ask user if they want to continue (Yes default)
        if !utils::prompt_user(&i18n::t("app.cli.install.continue_prompt")) {
            tracing::info!("User cancelled installation due to invalid packages");
            println!("{}", i18n::t("app.cli.install.cancelled"));
            std::process::exit(0);
        }
    }

    // If no valid packages remain after filtering, exit
    if official_packages.is_empty() && aur_packages.is_empty() {
        eprintln!("Error: No valid packages to install.");
        tracing::error!("No valid packages after validation");
        std::process::exit(1);
    }

    // Install official packages
    install_official_packages(&official_packages);

    // Install AUR packages
    if !aur_packages.is_empty() {
        let Some(helper) = aur_helper else {
            eprintln!(
                "Error: Neither paru nor yay is available. Please install one of them to install AUR packages."
            );
            tracing::error!("Neither paru nor yay is available for AUR packages");
            std::process::exit(1);
        };
        install_aur_packages(&aur_packages, helper);
    }

    tracing::info!("All packages installed successfully");
    std::process::exit(0);
}

/// What: Handle command-line install mode by installing packages via pacman or AUR helper.
///
/// Inputs:
/// - `packages`: Vector of package names (comma-separated or space-separated).
///
/// Output:
/// - Exits the process with the command's exit code or 1 on error.
///
/// Details:
/// - Parses package names (handles comma-separated and space-separated).
/// - Checks each package to verify it exists before installation.
/// - Warns user if any packages don't exist and asks for confirmation.
/// - Determines if packages are official or AUR.
/// - Installs official packages via `sudo pacman -S`.
/// - Installs AUR packages via `paru -S` or `yay -S` (prefers paru).
/// - Exits immediately after installation (doesn't launch TUI).
pub fn handle_install(packages: &[String]) -> ! {
    tracing::info!(packages = ?packages, "Install mode requested from CLI");

    let package_names = utils::parse_package_names(packages);
    if package_names.is_empty() {
        eprintln!("{}", i18n::t("app.cli.install.no_packages_specified"));
        tracing::error!("No packages specified for installation");
        std::process::exit(1);
    }

    // Get AUR helper early to check AUR packages
    let aur_helper = utils::get_aur_helper();

    // Validate and categorize packages
    let (official_packages, aur_packages, invalid_packages) =
        package::validate_and_categorize_packages(&package_names, aur_helper);

    // Handle invalid packages
    if !package::handle_invalid_packages(
        &invalid_packages,
        aur_helper,
        &official_packages,
        &aur_packages,
    ) {
        tracing::info!("User cancelled installation due to invalid packages");
        println!("{}", i18n::t("app.cli.install.cancelled"));
        std::process::exit(0);
    }

    // If no valid packages remain after filtering, exit
    if official_packages.is_empty() && aur_packages.is_empty() {
        eprintln!("{}", i18n::t("app.cli.install.no_valid_packages"));
        tracing::error!("No valid packages after validation");
        std::process::exit(1);
    }

    // Install official packages
    install_official_packages(&official_packages);

    // Install AUR packages
    if !aur_packages.is_empty() {
        let Some(helper) = aur_helper else {
            eprintln!("{}", i18n::t("app.cli.install.neither_helper_available"));
            tracing::error!("Neither paru nor yay is available for AUR packages");
            std::process::exit(1);
        };
        install_aur_packages(&aur_packages, helper);
    }

    tracing::info!("All packages installed successfully");
    std::process::exit(0);
}
