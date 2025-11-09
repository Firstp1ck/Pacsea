//! Dependency source determination utilities.

use crate::state::modal::DependencySource;
use std::collections::HashSet;
use std::process::Command;

/// What: Infer the origin repository for a dependency currently under analysis.
///
/// Inputs:
/// - `name`: Candidate dependency package name.
/// - `installed`: Set of locally installed package names used to detect presence.
///
/// Output:
/// - Returns a tuple with the determined `DependencySource` and a flag indicating core membership.
///
/// Details:
/// - Prefers inspecting `pacman -Qi` metadata when the package is installed; otherwise defaults to heuristics.
/// - Downgrades gracefully to official classifications when the repository field cannot be read.
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

/// What: Identify whether a dependency belongs to a curated list of critical system packages.
///
/// Inputs:
/// - `name`: Package name to compare against the predefined system set.
///
/// Output:
/// - `true` when the package is considered a core system component; otherwise `false`.
///
/// Details:
/// - Used to highlight packages whose removal or downgrade should be discouraged.
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
    /// What: Confirm `is_system_package` recognizes curated critical packages.
    ///
    /// Inputs:
    /// - `names`: Sample package names including system and non-system entries.
    ///
    /// Output:
    /// - Returns `true` for known core packages and `false` for unrelated software.
    ///
    /// Details:
    /// - Exercises both positive (glibc, linux) and negative (firefox) cases to validate membership.
    fn is_system_package_detects_core() {
        assert!(is_system_package("glibc"));
        assert!(is_system_package("linux"));
        assert!(!is_system_package("firefox"));
    }
}
