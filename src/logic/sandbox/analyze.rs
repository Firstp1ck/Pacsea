//! Analysis functions for comparing dependencies against host environment.

use crate::logic::sandbox::parse::parse_pkgbuild_deps;
use crate::logic::sandbox::parse::parse_srcinfo_deps;
use crate::logic::sandbox::types::{DependencyDelta, SandboxInfo};
use std::collections::HashSet;
use std::process::{Command, Stdio};

/// What: Analyze package dependencies from .SRCINFO content.
///
/// Inputs:
/// - `package_name`: AUR package name.
/// - `srcinfo_text`: .SRCINFO content.
/// - `installed`: Set of installed package names.
/// - `provided`: Set of package names provided by installed packages.
///
/// Output:
/// - `SandboxInfo` with dependency deltas.
pub(super) fn analyze_package_from_srcinfo(
    package_name: &str,
    srcinfo_text: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> SandboxInfo {
    let (depends, makedepends, checkdepends, optdepends) = parse_srcinfo_deps(srcinfo_text);

    // Analyze each dependency against host environment
    let depends_delta = analyze_dependencies(&depends, installed, provided);
    let makedepends_delta = analyze_dependencies(&makedepends, installed, provided);
    let checkdepends_delta = analyze_dependencies(&checkdepends, installed, provided);
    let optdepends_delta = analyze_dependencies(&optdepends, installed, provided);

    SandboxInfo {
        package_name: package_name.to_string(),
        depends: depends_delta,
        makedepends: makedepends_delta,
        checkdepends: checkdepends_delta,
        optdepends: optdepends_delta,
    }
}

/// What: Analyze package dependencies from PKGBUILD content.
///
/// Inputs:
/// - `package_name`: AUR package name.
/// - `pkgbuild_text`: PKGBUILD content.
/// - `installed`: Set of installed package names.
/// - `provided`: Set of package names provided by installed packages.
///
/// Output:
/// - `SandboxInfo` with dependency deltas.
pub(super) fn analyze_package_from_pkgbuild(
    package_name: &str,
    pkgbuild_text: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> SandboxInfo {
    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild_text);

    // Analyze each dependency against host environment
    let depends_delta = analyze_dependencies(&depends, installed, provided);
    let makedepends_delta = analyze_dependencies(&makedepends, installed, provided);
    let checkdepends_delta = analyze_dependencies(&checkdepends, installed, provided);
    let optdepends_delta = analyze_dependencies(&optdepends, installed, provided);

    SandboxInfo {
        package_name: package_name.to_string(),
        depends: depends_delta,
        makedepends: makedepends_delta,
        checkdepends: checkdepends_delta,
        optdepends: optdepends_delta,
    }
}

/// What: Analyze dependencies against the host environment.
///
/// Inputs:
/// - `deps`: Vector of dependency specifications.
/// - `installed`: Set of installed package names.
///
/// Output:
/// - Vector of `DependencyDelta` entries showing status of each dependency.
///
/// Details:
/// - Skips local packages entirely.
pub(super) fn analyze_dependencies(
    deps: &[String],
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> Vec<DependencyDelta> {
    deps.iter()
        .filter_map(|dep_spec| {
            // Extract package name (may include version requirements)
            let pkg_name = extract_package_name(dep_spec);
            // Check if package is installed or provided by an installed package
            let is_installed = crate::logic::deps::is_package_installed_or_provided(
                &pkg_name, installed, provided,
            );

            // Skip local packages - they're not relevant for sandbox analysis
            if is_installed && is_local_package(&pkg_name) {
                return None;
            }

            // Try to get installed version
            let installed_version = if is_installed {
                crate::logic::deps::get_installed_version(&pkg_name).ok()
            } else {
                None
            };

            // Check if version requirement is satisfied
            let version_satisfied = installed_version
                .as_ref()
                .is_some_and(|version| crate::logic::deps::version_satisfies(version, dep_spec));

            Some(DependencyDelta {
                name: dep_spec.clone(),
                is_installed,
                installed_version,
                version_satisfied,
            })
        })
        .collect()
}

/// What: Extract package name from a dependency specification.
///
/// Inputs:
/// - `dep_spec`: Dependency specification (e.g., "foo>=1.2", "bar", "baz: description").
///
/// Output:
/// - Package name without version requirements or description.
#[must_use]
pub fn extract_package_name(dep_spec: &str) -> String {
    // Handle optdepends format: "package: description"
    let name = dep_spec
        .find(':')
        .map_or_else(|| dep_spec, |colon_pos| &dep_spec[..colon_pos]);

    // Remove version operators: >=, <=, ==, >, <
    name.trim()
        .split(">=")
        .next()
        .unwrap_or(name)
        .split("<=")
        .next()
        .unwrap_or(name)
        .split("==")
        .next()
        .unwrap_or(name)
        .split('>')
        .next()
        .unwrap_or(name)
        .split('<')
        .next()
        .unwrap_or(name)
        .trim()
        .to_string()
}

/// What: Check if a package is a local package.
///
/// Inputs:
/// - `name`: Package name to check.
///
/// Output:
/// - `true` if the package is local, `false` otherwise.
fn is_local_package(name: &str) -> bool {
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
                    return repo == "local" || repo.is_empty();
                }
            }
        }
        _ => {
            // If we can't determine, assume it's not local
            return false;
        }
    }

    false
}

/// What: Get the set of installed packages.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Set of installed package names.
pub(super) fn get_installed_packages() -> std::collections::HashSet<String> {
    crate::logic::deps::get_installed_packages()
}
