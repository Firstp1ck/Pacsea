//! Package querying functions for dependency resolution.

use std::collections::HashSet;
use std::process::{Command, Stdio};

/// What: Collect names of packages that have upgrades available via pacman.
///
/// Inputs:
/// - (none): Reads upgrade information by invoking `pacman -Qu`.
///
/// Output:
/// - Returns a set containing package names that pacman reports as upgradable.
///
/// Details:
/// - Trims each line from the command output and extracts the leading package token before version metadata.
/// - Gracefully handles command failures by returning an empty set to avoid blocking dependency checks.
pub(super) fn get_upgradable_packages() -> HashSet<String> {
    tracing::debug!("Running: pacman -Qu");
    let output = Command::new("pacman")
        .args(["-Qu"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                // pacman -Qu outputs "name old-version -> new-version" or just "name" for AUR packages
                let packages: HashSet<String> = text
                    .lines()
                    .filter_map(|line| {
                        let line = line.trim();
                        if line.is_empty() {
                            return None;
                        }
                        // Extract package name (everything before space or "->")
                        line.find(' ').map_or_else(
                            || line.to_string(),
                            |space_pos| line[..space_pos].trim().to_string(),
                        )
                    })
                    .collect();
                tracing::debug!(
                    "Successfully retrieved {} upgradable packages",
                    packages.len()
                );
                packages
            } else {
                // No upgradable packages or error - return empty set
                HashSet::new()
            }
        }
        Err(e) => {
            tracing::debug!("Failed to execute pacman -Qu: {} (assuming no upgrades)", e);
            HashSet::new()
        }
    }
}

/// What: Enumerate all currently installed packages on the system.
///
/// Inputs:
/// - (none): Invokes `pacman -Qq` to query the local database.
///
/// Output:
/// - Returns a set of package names installed on the machine; empty on failure.
///
/// Details:
/// - Uses pacman's quiet format to obtain trimmed names and logs errors where available for diagnostics.
pub fn get_installed_packages() -> HashSet<String> {
    tracing::debug!("Running: pacman -Qq");
    let output = Command::new("pacman")
        .args(["-Qq"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let packages: HashSet<String> =
                    text.lines().map(|s| s.trim().to_string()).collect();
                tracing::debug!(
                    "Successfully retrieved {} installed packages",
                    packages.len()
                );
                packages
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(
                    "pacman -Qq failed with status {:?}: {}",
                    output.status.code(),
                    stderr
                );
                HashSet::new()
            }
        }
        Err(e) => {
            tracing::error!("Failed to execute pacman -Qq: {}", e);
            HashSet::new()
        }
    }
}

/// What: Check if a specific package name is provided by any installed package (lazy check).
///
/// Inputs:
/// - `name`: Package name to check.
/// - `installed`: Set of installed package names (used to optimize search).
///
/// Output:
/// - Returns `Some(package_name)` if the name is provided by an installed package, `None` otherwise.
///
/// Details:
/// - Uses `pacman -Qqo` to efficiently check if any installed package provides the name.
/// - This is much faster than querying all packages upfront.
/// - Returns the name of the providing package for debugging purposes.
fn check_if_provided(name: &str, _installed: &HashSet<String>) -> Option<String> {
    // Use pacman -Qqo to check which package provides this name
    // This is efficient - pacman does the lookup internally
    let output = Command::new("pacman")
        .args(["-Qqo", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let providing_pkg = text.lines().next().map(|s| s.trim().to_string());
            if providing_pkg.is_some() {
                tracing::debug!(
                    "{} is provided by {}",
                    name,
                    providing_pkg
                        .as_ref()
                        .expect("providing_pkg should be Some after is_some() check")
                );
            }
            providing_pkg
        }
        _ => None,
    }
}

/// What: Build an empty provides set (for API compatibility).
///
/// Inputs:
/// - `installed`: Set of installed package names (unused, kept for API compatibility).
///
/// Output:
/// - Returns an empty set (provides are now checked lazily).
///
/// Details:
/// - This function is kept for API compatibility but no longer builds the full provides set.
/// - Provides are now checked on-demand using `check_if_provided()` for better performance.
#[must_use]
pub fn get_provided_packages(_installed: &HashSet<String>) -> HashSet<String> {
    // Return empty set - provides are now checked lazily on-demand
    // This avoids querying all installed packages upfront, which was very slow
    HashSet::new()
}

/// What: Check if a package is installed or provided by an installed package.
///
/// Inputs:
/// - `name`: Package name to check.
/// - `installed`: Set of directly installed package names.
/// - `provided`: Set of package names provided by installed packages (unused, kept for API compatibility).
///
/// Output:
/// - Returns `true` if the package is directly installed or provided by an installed package.
///
/// Details:
/// - First checks if the package is directly installed.
/// - Then lazily checks if it's provided by any installed package using `pacman -Qqo`.
/// - This handles cases like `rustup` providing `rust` efficiently without querying all packages upfront.
#[must_use]
pub fn is_package_installed_or_provided(
    name: &str,
    installed: &HashSet<String>,
    _provided: &HashSet<String>,
) -> bool {
    // First check if directly installed
    if installed.contains(name) {
        return true;
    }

    // Lazy check if provided by any installed package (much faster than building full set upfront)
    check_if_provided(name, installed).is_some()
}
