//! Package validation and categorization utilities.

use crate::args::i18n;

/// What: Check if a package exists in the official repositories using search.
///
/// Inputs:
/// - `package_name`: Name of the package to check.
///
/// Output:
/// - `true` if the package exists in official repos, `false` otherwise.
///
/// Details:
/// - Uses `sudo pacman -Ss` to search for the package and checks if exact match exists.
/// - Returns `false` if pacman is not available or the package is not found.
fn is_official_package_search(package_name: &str) -> bool {
    use std::process::{Command, Stdio};

    match Command::new("sudo")
        .args(["pacman", "-Ss", package_name])
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                return false;
            }
            // Check if the output contains the exact package name
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Look for exact package name match (format: "repo/package_name" or "package_name")
            output_str.lines().any(|line| {
                line.split_whitespace().next().is_some_and(|pkg_line| {
                    // Handle format like "repo/package_name" or just "package_name"
                    let pkg_part = pkg_line.split('/').next_back().unwrap_or(pkg_line);
                    pkg_part == package_name
                })
            })
        }
        Err(_) => false,
    }
}

/// What: Check if a package exists in the official repositories.
///
/// Inputs:
/// - `package_name`: Name of the package to check.
///
/// Output:
/// - `true` if the package exists in official repos, `false` otherwise.
///
/// Details:
/// - Uses `pacman -Si` to check if the package exists in official repositories.
/// - Returns `false` if pacman is not available or the package is not found.
fn is_official_package(package_name: &str) -> bool {
    use std::process::{Command, Stdio};

    match Command::new("pacman")
        .args(["-Si", package_name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// What: Check if an AUR package exists using search.
///
/// Inputs:
/// - `package_name`: Name of the package to check.
/// - `helper`: AUR helper to use ("paru" or "yay").
///
/// Output:
/// - `true` if the package exists in AUR, `false` otherwise.
///
/// Details:
/// - Uses `paru -Ss` or `yay -Ss` to search for the package and checks if exact match exists.
/// - Returns `false` if the helper is not available or the package is not found.
fn is_aur_package_search(package_name: &str, helper: &str) -> bool {
    use std::process::{Command, Stdio};

    match Command::new(helper)
        .args(["-Ss", package_name])
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                return false;
            }
            // Check if the output contains the exact package name
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Look for exact package name match (format: "aur/package_name" or "package_name")
            output_str.lines().any(|line| {
                line.split_whitespace().next().is_some_and(|pkg_line| {
                    // Handle format like "aur/package_name" or just "package_name"
                    let pkg_part = pkg_line.split('/').next_back().unwrap_or(pkg_line);
                    pkg_part == package_name
                })
            })
        }
        Err(_) => false,
    }
}

/// What: Check if an AUR package exists.
///
/// Inputs:
/// - `package_name`: Name of the package to check.
/// - `helper`: AUR helper to use ("paru" or "yay").
///
/// Output:
/// - `true` if the package exists in AUR, `false` otherwise.
///
/// Details:
/// - Uses `paru -Si` or `yay -Si` to check if the package exists in AUR.
/// - Returns `false` if the helper is not available or the package is not found.
fn is_aur_package(package_name: &str, helper: &str) -> bool {
    use std::process::{Command, Stdio};

    match Command::new(helper)
        .args(["-Si", package_name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// What: Validate and categorize packages into official, AUR, and invalid.
///
/// Inputs:
/// - `package_names`: Vector of package names to validate.
/// - `aur_helper`: Optional AUR helper name ("paru" or "yay").
///
/// Output:
/// - Tuple of (`official_packages`, `aur_packages`, `invalid_packages`).
///
/// Details:
/// - Checks each package against official repos and AUR (if helper available).
/// - Packages not found in either are marked as invalid.
pub fn validate_and_categorize_packages(
    package_names: &[String],
    aur_helper: Option<&str>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut official_packages = Vec::new();
    let mut aur_packages = Vec::new();
    let mut invalid_packages = Vec::new();

    for pkg in package_names {
        if is_official_package(pkg) {
            official_packages.push(pkg.clone());
        } else if let Some(helper) = aur_helper {
            if is_aur_package(pkg, helper) {
                aur_packages.push(pkg.clone());
            } else {
                invalid_packages.push(pkg.clone());
            }
        } else {
            // No AUR helper available, but package is not official
            // We can't verify AUR packages without a helper, so mark as invalid
            invalid_packages.push(pkg.clone());
        }
    }

    (official_packages, aur_packages, invalid_packages)
}

/// What: Validate packages using search commands and categorize them.
///
/// Inputs:
/// - `package_names`: Vector of package names to validate.
/// - `aur_helper`: Optional AUR helper name ("paru" or "yay").
///
/// Output:
/// - Tuple of (`official_packages`, `aur_packages`, `invalid_packages`).
///
/// Details:
/// - Checks each package using `sudo pacman -Ss` first.
/// - If not found, checks using `yay/paru -Ss` (if helper available).
/// - Packages not found in either are marked as invalid.
pub fn validate_and_categorize_packages_search(
    package_names: &[String],
    aur_helper: Option<&str>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut official_packages = Vec::new();
    let mut aur_packages = Vec::new();
    let mut invalid_packages = Vec::new();

    for pkg in package_names {
        if is_official_package_search(pkg) {
            official_packages.push(pkg.clone());
        } else if let Some(helper) = aur_helper {
            if is_aur_package_search(pkg, helper) {
                aur_packages.push(pkg.clone());
            } else {
                invalid_packages.push(pkg.clone());
            }
        } else {
            // No AUR helper available, but package is not official
            invalid_packages.push(pkg.clone());
        }
    }

    (official_packages, aur_packages, invalid_packages)
}

/// What: Handle invalid packages by warning user and asking for confirmation.
///
/// Inputs:
/// - `invalid_packages`: Vector of invalid package names.
/// - `aur_helper`: Optional AUR helper name.
/// - `official_packages`: Vector of valid official packages.
/// - `aur_packages`: Vector of valid AUR packages.
///
/// Output:
/// - `true` if user wants to continue, `false` if cancelled.
///
/// Details:
/// - Displays warning message listing invalid packages.
/// - Prompts user for confirmation to continue with valid packages.
/// - Exits with error if all packages are invalid.
pub fn handle_invalid_packages(
    invalid_packages: &[String],
    aur_helper: Option<&str>,
    official_packages: &[String],
    aur_packages: &[String],
) -> bool {
    use crate::args::utils;

    if invalid_packages.is_empty() {
        return true;
    }

    eprintln!("\n{}", i18n::t("app.cli.package.packages_not_found"));
    for pkg in invalid_packages {
        eprintln!("  - {pkg}");
    }
    if aur_helper.is_none() && !invalid_packages.is_empty() {
        eprintln!("\n{}", i18n::t("app.cli.package.no_aur_helper_note"));
        eprintln!("{}", i18n::t("app.cli.package.install_aur_helper"));
    }
    eprintln!();

    // If all packages are invalid, exit with error
    if official_packages.is_empty() && aur_packages.is_empty() {
        eprintln!("{}", i18n::t("app.cli.package.no_valid_packages"));
        tracing::error!("All packages are invalid");
        std::process::exit(1);
    }

    utils::prompt_user("Do you want to continue installing the remaining packages?")
}
