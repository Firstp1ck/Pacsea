//! Dependency status determination and version checking.

use crate::logic::deps::is_package_installed_or_provided;
use crate::state::modal::DependencyStatus;
use std::collections::HashSet;
use std::process::{Command, Stdio};

/// What: Evaluate a dependency's installation status relative to required versions.
///
/// Inputs:
/// - `name`: Dependency package identifier.
/// - `version_req`: Optional version constraint string (e.g., `>=1.2`).
/// - `installed`: Set of names currently installed on the system.
/// - `upgradable`: Set of names pacman reports as upgradable.
///
/// Output:
/// - Returns a `DependencyStatus` describing whether installation, upgrade, or no action is needed.
///
/// Details:
/// - Combines local database queries with helper functions to capture upgrade requirements and conflicts.
pub(crate) fn determine_status(
    name: &str,
    version_req: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> DependencyStatus {
    // Check if package is installed or provided by an installed package
    if !is_package_installed_or_provided(name, installed, provided) {
        return DependencyStatus::ToInstall;
    }

    // Check if package is upgradable (even without version requirement)
    let is_upgradable = upgradable.contains(name);

    // If version requirement is specified, check if it matches
    if !version_req.is_empty() {
        // Try to get installed version
        if let Ok(installed_version) = get_installed_version(name) {
            // Simple version comparison (basic implementation)
            if !version_satisfies(&installed_version, version_req) {
                return DependencyStatus::ToUpgrade {
                    current: installed_version,
                    required: version_req.to_string(),
                };
            }
            // Version requirement satisfied, but check if package is upgradable anyway
            if is_upgradable {
                // Get available version from pacman -Si if possible
                let available_version =
                    get_available_version(name).unwrap_or_else(|| "newer".to_string());
                return DependencyStatus::ToUpgrade {
                    current: installed_version,
                    required: available_version,
                };
            }
            return DependencyStatus::Installed {
                version: installed_version,
            };
        }
    }

    // Installed but no version check needed - check if upgradable
    if is_upgradable {
        match get_installed_version(name) {
            Ok(current_version) => {
                let available_version =
                    get_available_version(name).unwrap_or_else(|| "newer".to_string());
                return DependencyStatus::ToUpgrade {
                    current: current_version,
                    required: available_version,
                };
            }
            Err(_) => {
                return DependencyStatus::ToUpgrade {
                    current: "installed".to_string(),
                    required: "newer".to_string(),
                };
            }
        }
    }

    // Installed and up-to-date - get actual version
    match get_installed_version(name) {
        Ok(version) => DependencyStatus::Installed { version },
        Err(_) => DependencyStatus::Installed {
            version: "installed".to_string(),
        },
    }
}

/// What: Query the repositories for the latest available version of a package.
///
/// Inputs:
/// - `name`: Package name looked up via `pacman -Si`.
///
/// Output:
/// - Returns the version string advertised in the repositories, or `None` on failure.
///
/// Details:
/// - Strips revision suffixes (e.g., `-1`) so comparisons focus on the base semantic version.
pub(crate) fn get_available_version(name: &str) -> Option<String> {
    let output = Command::new("pacman")
        .args(["-Si", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.starts_with("Version")
            && let Some(colon_pos) = line.find(':')
        {
            let version = line[colon_pos + 1..].trim();
            // Remove revision suffix if present
            let version = version.split('-').next().unwrap_or(version);
            return Some(version.to_string());
        }
    }
    None
}

/// What: Retrieve the locally installed version of a package.
///
/// Inputs:
/// - `name`: Package to query via `pacman -Q`.
///
/// Output:
/// - Returns the installed version string on success; otherwise an error message.
///
/// Details:
/// - Normalizes versions by removing revision suffixes to facilitate requirement comparisons.
pub fn get_installed_version(name: &str) -> Result<String, String> {
    let output = Command::new("pacman")
        .args(["-Q", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("pacman -Q failed: {}", e))?;

    if !output.status.success() {
        return Err("Package not found".to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = text.lines().next() {
        // Format: "name version" or "name version-revision"
        if let Some(space_pos) = line.find(' ') {
            let version = line[space_pos + 1..].trim();
            // Remove revision suffix if present (e.g., "1.2.3-1" -> "1.2.3")
            let version = version.split('-').next().unwrap_or(version);
            return Ok(version.to_string());
        }
    }

    Err("Could not parse version".to_string())
}

/// What: Perform a simplified comparison between an installed version and a requirement expression.
///
/// Inputs:
/// - `installed`: Version string currently present on the system.
/// - `requirement`: Comparison expression such as `>=1.2` or `=2.0`.
///
/// Output:
/// - `true` when the expression evaluates in favor of the installed version; otherwise `false`.
///
/// Details:
/// - Uses straightforward string comparisons rather than full semantic version parsing, matching pacman's format.
pub fn version_satisfies(installed: &str, requirement: &str) -> bool {
    // This is a simplified version checker
    // For production, use a proper version comparison library
    if let Some(req_ver) = requirement.strip_prefix(">=") {
        installed >= req_ver
    } else if let Some(req_ver) = requirement.strip_prefix("<=") {
        installed <= req_ver
    } else if let Some(req_ver) = requirement.strip_prefix("=") {
        installed == req_ver
    } else if let Some(req_ver) = requirement.strip_prefix(">") {
        installed > req_ver
    } else if let Some(req_ver) = requirement.strip_prefix("<") {
        installed < req_ver
    } else {
        // No version requirement, assume satisfied
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Ensure relational comparison operators behave according to the simplified string checks.
    ///
    /// Inputs:
    /// - `>=`, `<=`, `>`, `<`, and `=` requirements evaluated against representative version strings.
    ///
    /// Output:
    /// - Verifies truthiness for matching cases and falseness for mismatched comparisons.
    ///
    /// Details:
    /// - Confirms the helper remains stable for the ordering relied upon by dependency diagnostics.
    fn version_satisfies_relational_operators() {
        assert!(version_satisfies("2.0", ">=1.5"));
        assert!(!version_satisfies("1.0", ">=1.5"));
        assert!(version_satisfies("1.5", "<=1.5"));
        assert!(version_satisfies("1.6", ">1.5"));
        assert!(!version_satisfies("1.4", ">1.5"));
        assert!(version_satisfies("1.5", "=1.5"));
        assert!(!version_satisfies("1.6", "<1.5"));
    }

    #[test]
    /// What: Confirm the helper defaults to success when no requirement string is provided.
    ///
    /// Inputs:
    /// - Empty and non-operator requirement strings.
    ///
    /// Output:
    /// - Returns `true`, indicating no additional comparison is enforced.
    ///
    /// Details:
    /// - Guards the fallback branch used by callers that lack explicit version constraints.
    fn version_satisfies_defaults_to_true_without_constraint() {
        assert!(version_satisfies("2.0", ""));
        assert!(version_satisfies("2.0", "n/a"));
    }
}
