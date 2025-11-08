//! Dependency status determination and version checking.

use crate::state::modal::DependencyStatus;
use std::collections::HashSet;
use std::process::Command;

/// Determine the status of a dependency based on installation state.
pub(crate) fn determine_status(
    name: &str,
    version_req: &str,
    installed: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> DependencyStatus {
    if !installed.contains(name) {
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

/// Get the available version of a package from repositories.
pub(crate) fn get_available_version(name: &str) -> Option<String> {
    let output = Command::new("pacman")
        .args(["-Si", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
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

/// Get the installed version of a package.
pub(crate) fn get_installed_version(name: &str) -> Result<String, String> {
    let output = Command::new("pacman")
        .args(["-Q", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
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

/// Check if a version satisfies a requirement (simplified).
pub(crate) fn version_satisfies(installed: &str, requirement: &str) -> bool {
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
