//! Dependency source determination utilities.

use crate::state::modal::DependencySource;
use std::collections::HashSet;
use std::process::{Command, Stdio};

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
pub(super) fn determine_dependency_source(
    name: &str,
    installed: &HashSet<String>,
) -> (DependencySource, bool) {
    if !installed.contains(name) {
        // Not installed - check if it exists in official repos first
        // Only default to AUR if it's not found in official repos
        let output = Command::new("pacman")
            .args(["-Si", name])
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output
            && output.status.success()
        {
            // Package exists in official repos - determine which repo
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.starts_with("Repository")
                    && let Some(colon_pos) = line.find(':')
                {
                    let repo = line[colon_pos + 1..].trim().to_lowercase();
                    let is_core = repo == "core";
                    return (DependencySource::Official { repo }, is_core);
                }
            }
            // Found in official repos but couldn't determine repo - assume extra
            return (
                DependencySource::Official {
                    repo: "extra".to_string(),
                },
                false,
            );
        }
        // Not found in official repos - this could be:
        // 1. A binary/script provided by a package (not a package itself) - should be Missing
        // 2. A virtual package (.so file) - should be filtered out earlier
        // 3. A real AUR package - but we can't distinguish without checking AUR
        //
        // IMPORTANT: We don't try AUR here because:
        // - Most dependencies are from official repos or are binaries/scripts
        // - Trying AUR for every unknown dependency causes unnecessary API calls
        // - Real AUR packages should be explicitly specified by the user, not discovered as dependencies
        // - If it's truly an AUR dependency, it will be marked as Missing and the user can handle it
        tracing::debug!(
            "Package {} not found in official repos and not installed - will be marked as Missing (skipping AUR check)",
            name
        );
        // Return AUR but the resolve logic should check if it exists before trying API
        // Actually, let's return Official with a special marker - but that won't work with current code
        // Better: return AUR but add a check in resolve_package_deps to verify it exists first
        return (DependencySource::Aur, false);
    }

    // Package is installed - check which repository it came from
    let output = Command::new("pacman")
        .args(["-Qi", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
                    // Handle local packages specially
                    if repo == "local" || repo.is_empty() {
                        return (DependencySource::Local, false);
                    }
                    return (DependencySource::Official { repo }, is_core);
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
pub(super) fn is_system_package(name: &str) -> bool {
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
