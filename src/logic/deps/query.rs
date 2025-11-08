//! Package querying functions for dependency resolution.

use std::collections::HashSet;
use std::process::Command;

/// Get a set of upgradable package names.
pub(crate) fn get_upgradable_packages() -> HashSet<String> {
    tracing::debug!("Running: pacman -Qu");
    let output = Command::new("pacman")
        .args(["-Qu"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
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
                        if let Some(space_pos) = line.find(' ') {
                            Some(line[..space_pos].trim().to_string())
                        } else {
                            Some(line.to_string())
                        }
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

/// Get a set of installed package names.
pub(crate) fn get_installed_packages() -> HashSet<String> {
    tracing::debug!("Running: pacman -Qq");
    let output = Command::new("pacman")
        .args(["-Qq"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
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
