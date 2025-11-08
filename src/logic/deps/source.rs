//! Dependency source determination utilities.

use crate::state::modal::DependencySource;
use std::collections::HashSet;
use std::process::Command;

/// Determine the source repository for a dependency package.
///
/// Returns (source, is_core) tuple.
pub(crate) fn determine_dependency_source(
    name: &str,
    installed: &HashSet<String>,
) -> (DependencySource, bool) {
    if !installed.contains(name) {
        // Not installed - could be AUR or official, default to AUR
        return (DependencySource::Aur, false);
    }

    // Package is installed - check which repository it came from
    let output = Command::new("pacman")
        .args(["-Qi", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            // Look for "Repository" field in pacman -Qi output
            for line in text.lines() {
                if line.starts_with("Repository")
                    && let Some(colon_pos) = line.find(':')
                {
                    let repo = line[colon_pos + 1..].trim().to_lowercase();
                    let is_core = repo == "core";
                    return (
                        DependencySource::Official {
                            repo: if repo.is_empty() {
                                "unknown".to_string()
                            } else {
                                repo
                            },
                        },
                        is_core,
                    );
                }
            }
        }
        _ => {
            // Fallback: try pacman -Q to see if it's installed
            // If we can't determine repo, assume it's from an official repo
            tracing::debug!(
                "Could not determine repository for {}, assuming official",
                name
            );
        }
    }

    // Default: assume official repository (most installed packages are)
    let is_core = is_system_package(name);
    (
        DependencySource::Official {
            repo: if is_core {
                "core".to_string()
            } else {
                "extra".to_string()
            },
        },
        is_core,
    )
}

/// Check if a package is a critical system package.
pub(crate) fn is_system_package(name: &str) -> bool {
    // List of critical system packages
    let system_packages = [
        "glibc",
        "linux",
        "systemd",
        "pacman",
        "bash",
        "coreutils",
        "gcc",
        "binutils",
        "filesystem",
        "util-linux",
        "shadow",
        "sed",
        "grep",
    ];
    system_packages.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_system_package_detects_core() {
        assert!(is_system_package("glibc"));
        assert!(is_system_package("linux"));
        assert!(!is_system_package("firefox"));
    }
}
