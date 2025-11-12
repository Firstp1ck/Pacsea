//! Core dependency resolution logic for individual packages.

use super::parse::{parse_dep_spec, parse_pacman_si_deps, parse_pacman_si_optdeps};
use super::source::{determine_dependency_source, is_system_package};
use super::srcinfo::{fetch_srcinfo, parse_srcinfo_deps};
use super::status::determine_status;
use crate::state::modal::DependencyInfo;
use crate::state::types::Source;
use std::collections::HashSet;
use std::process::{Command, Stdio};

/// What: Resolve direct dependency metadata for a single package.
///
/// Inputs:
/// - `name`: Package identifier whose dependencies should be enumerated.
/// - `source`: Source enum describing whether the package is official or AUR.
/// - `installed`: Set of locally installed packages for status determination.
/// - `upgradable`: Set of packages flagged for upgrades, used to detect stale dependencies.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records or an error string when resolution fails.
///
/// Details:
/// - Invokes pacman or AUR helpers depending on source, filtering out virtual entries and self references.
pub(crate) fn resolve_package_deps(
    name: &str,
    source: &Source,
    installed: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Result<Vec<DependencyInfo>, String> {
    let mut deps = Vec::new();

    match source {
        Source::Official { repo, .. } => {
            // Handle local packages specially - use pacman -Qi instead of -Si
            if repo == "local" {
                tracing::debug!("Running: pacman -Qi {} (local package)", name);
                let output = Command::new("pacman")
                    .args(["-Qi", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .map_err(|e| {
                        tracing::error!("Failed to execute pacman -Qi {}: {}", name, e);
                        format!("pacman -Qi failed: {}", e)
                    })?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!(
                        "pacman -Qi {} failed with status {:?}: {}",
                        name,
                        output.status.code(),
                        stderr
                    );
                    // Local package might not exist anymore, return empty deps
                    return Ok(Vec::new());
                }

                let text = String::from_utf8_lossy(&output.stdout);
                tracing::debug!("pacman -Qi {} output ({} bytes)", name, text.len());

                // Parse "Depends On" field from pacman -Qi output (same format as -Si)
                let dep_names = parse_pacman_si_deps(&text);
                let opt_dep_names = parse_pacman_si_optdeps(&text);
                tracing::debug!(
                    "Parsed {} dependency names and {} optional dependency names from pacman -Qi output",
                    dep_names.len(),
                    opt_dep_names.len()
                );

                // Process dependencies (same logic as official packages)
                let required_dep_names: HashSet<String> = dep_names
                    .iter()
                    .map(|d| {
                        let (n, _) = parse_dep_spec(d);
                        n
                    })
                    .collect();

                for dep_spec in dep_names {
                    let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                    if pkg_name == name {
                        tracing::debug!("Skipping self-reference: {} == {}", pkg_name, name);
                        continue;
                    }
                    if pkg_name.ends_with(".so")
                        || pkg_name.contains(".so.")
                        || pkg_name.contains(".so=")
                    {
                        tracing::debug!("Filtering out virtual package: {}", pkg_name);
                        continue;
                    }

                    let status = determine_status(&pkg_name, &version_req, installed, upgradable);
                    let (source, is_core) = determine_dependency_source(&pkg_name, installed);
                    let is_system = is_core || is_system_package(&pkg_name);

                    deps.push(DependencyInfo {
                        name: pkg_name,
                        version: version_req,
                        status,
                        source,
                        required_by: vec![name.to_string()],
                        depends_on: Vec::new(),
                        is_core,
                        is_system,
                    });
                }

                for dep_spec in opt_dep_names {
                    let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                    if pkg_name == name {
                        tracing::debug!(
                            "Skipping self-reference in optdeps: {} == {}",
                            pkg_name,
                            name
                        );
                        continue;
                    }
                    if pkg_name.ends_with(".so")
                        || pkg_name.contains(".so.")
                        || pkg_name.contains(".so=")
                    {
                        tracing::debug!("Filtering out virtual package in optdeps: {}", pkg_name);
                        continue;
                    }

                    if required_dep_names.contains(&pkg_name) {
                        tracing::debug!(
                            "Skipping optional dep {} (already in required deps)",
                            pkg_name
                        );
                        continue;
                    }

                    let status = determine_status(&pkg_name, &version_req, installed, upgradable);
                    let (source, is_core) = determine_dependency_source(&pkg_name, installed);
                    let is_system = is_core || is_system_package(&pkg_name);

                    deps.push(DependencyInfo {
                        name: pkg_name,
                        version: version_req,
                        status,
                        source,
                        required_by: vec![name.to_string()],
                        depends_on: Vec::new(),
                        is_core,
                        is_system,
                    });
                }
                return Ok(deps);
            }

            // Use pacman -Si to get dependency list (shows all deps, not just ones to download)
            // Note: pacman -Si doesn't need repo prefix - it will find the package in any repo
            // Using repo prefix can cause failures if repo is incorrect (e.g., core package marked as extra)
            tracing::debug!("Running: pacman -Si {} (repo: {})", name, repo);
            let output = Command::new("pacman")
                .args(["-Si", name])
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| {
                    tracing::error!("Failed to execute pacman -Si {}: {}", name, e);
                    format!("pacman -Si failed: {}", e)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(
                    "pacman -Si {} failed with status {:?}: {}",
                    name,
                    output.status.code(),
                    stderr
                );
                return Err(format!("pacman -Si failed for {}: {}", name, stderr));
            }

            let text = String::from_utf8_lossy(&output.stdout);
            tracing::debug!("pacman -Si {} output ({} bytes)", name, text.len());

            // Parse "Depends On" field from pacman -Si output
            let dep_names = parse_pacman_si_deps(&text);
            // Also parse "Optional Deps" for completeness (though they're optional)
            let opt_dep_names = parse_pacman_si_optdeps(&text);
            tracing::debug!(
                "Parsed {} dependency names and {} optional dependency names from pacman -Si output",
                dep_names.len(),
                opt_dep_names.len()
            );

            // Collect required dependency names for deduplication with optional deps
            let required_dep_names: HashSet<String> = dep_names
                .iter()
                .map(|d| {
                    let (n, _) = parse_dep_spec(d);
                    n
                })
                .collect();

            // Process required dependencies
            for dep_spec in dep_names {
                let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                // Skip if this dependency is the package itself (shouldn't happen, but be safe)
                if pkg_name == name {
                    tracing::debug!("Skipping self-reference: {} == {}", pkg_name, name);
                    continue;
                }
                // Filter out .so files (virtual packages) - safety check in case filtering in parse_pacman_si_deps missed something
                if pkg_name.ends_with(".so")
                    || pkg_name.contains(".so.")
                    || pkg_name.contains(".so=")
                {
                    tracing::debug!("Filtering out virtual package: {}", pkg_name);
                    continue;
                }

                let status = determine_status(&pkg_name, &version_req, installed, upgradable);
                let (source, is_core) = determine_dependency_source(&pkg_name, installed);
                let is_system = is_core || is_system_package(&pkg_name);

                deps.push(DependencyInfo {
                    name: pkg_name,
                    version: version_req,
                    status,
                    source,
                    required_by: vec![name.to_string()],
                    depends_on: Vec::new(),
                    is_core,
                    is_system,
                });
            }

            // Process optional dependencies (for completeness, but mark them differently if needed)
            // Note: Optional deps are not strictly required, but we include them for visibility
            for dep_spec in opt_dep_names {
                let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                // Skip if this dependency is the package itself
                if pkg_name == name {
                    tracing::debug!(
                        "Skipping self-reference in optdeps: {} == {}",
                        pkg_name,
                        name
                    );
                    continue;
                }
                // Filter out .so files (virtual packages)
                if pkg_name.ends_with(".so")
                    || pkg_name.contains(".so.")
                    || pkg_name.contains(".so=")
                {
                    tracing::debug!("Filtering out virtual package in optdeps: {}", pkg_name);
                    continue;
                }

                // Check if we already have this as a required dependency
                if required_dep_names.contains(&pkg_name) {
                    tracing::debug!(
                        "Skipping optional dep {} (already in required deps)",
                        pkg_name
                    );
                    continue;
                }

                let status = determine_status(&pkg_name, &version_req, installed, upgradable);
                let (source, is_core) = determine_dependency_source(&pkg_name, installed);
                let is_system = is_core || is_system_package(&pkg_name);

                deps.push(DependencyInfo {
                    name: pkg_name,
                    version: version_req,
                    status,
                    source,
                    required_by: vec![name.to_string()],
                    depends_on: Vec::new(),
                    is_core,
                    is_system,
                });
            }
        }
        Source::Aur => {
            // For AUR packages, first verify it actually exists in AUR before trying to resolve
            // This prevents unnecessary API calls for binaries/scripts that aren't packages
            // Quick check: if pacman -Si failed, it's likely not a real package
            // We'll still try AUR but only if paru/yay is available (faster than API)
            tracing::debug!(
                "Attempting to resolve AUR package: {} (will skip if not found)",
                name
            );

            // Check if paru exists
            let has_paru = Command::new("paru")
                .args(["--version"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .is_ok();

            // Check if yay exists
            let has_yay = Command::new("yay")
                .args(["--version"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .is_ok();

            // Try paru/yay first, but fall back to API if they fail
            // Use -Si to get all dependencies (similar to pacman -Si)
            let mut used_helper = false;

            if has_paru {
                tracing::debug!("Trying paru -Si {} for dependency resolution", name);
                match Command::new("paru")
                    .args(["-Si", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let text = String::from_utf8_lossy(&output.stdout);
                            tracing::debug!("paru -Si {} output ({} bytes)", name, text.len());
                            let dep_names = parse_pacman_si_deps(&text);
                            if !dep_names.is_empty() {
                                tracing::info!("Using paru to resolve dependencies for {}", name);
                                used_helper = true;
                                for dep_spec in dep_names {
                                    let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                                    // Skip if this dependency is the package itself
                                    if pkg_name == name {
                                        tracing::debug!(
                                            "Skipping self-reference: {} == {}",
                                            pkg_name,
                                            name
                                        );
                                        continue;
                                    }
                                    // Filter out .so files (virtual packages)
                                    if pkg_name.ends_with(".so")
                                        || pkg_name.contains(".so.")
                                        || pkg_name.contains(".so=")
                                    {
                                        tracing::debug!(
                                            "Filtering out virtual package: {}",
                                            pkg_name
                                        );
                                        continue;
                                    }

                                    let status = determine_status(
                                        &pkg_name,
                                        &version_req,
                                        installed,
                                        upgradable,
                                    );
                                    let (source, is_core) =
                                        determine_dependency_source(&pkg_name, installed);
                                    let is_system = is_core || is_system_package(&pkg_name);

                                    deps.push(DependencyInfo {
                                        name: pkg_name,
                                        version: version_req,
                                        status,
                                        source,
                                        required_by: vec![name.to_string()],
                                        depends_on: Vec::new(),
                                        is_core,
                                        is_system,
                                    });
                                }
                            }
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            tracing::debug!(
                                "paru -Si {} failed (will try yay or API): {}",
                                name,
                                stderr.trim()
                            );
                        }
                    }
                    Err(_) => {
                        // paru not available, continue to try yay or API
                    }
                }
            }

            if !used_helper && has_yay {
                tracing::debug!("Trying yay -Si {} for dependency resolution", name);
                match Command::new("yay")
                    .args(["-Si", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let text = String::from_utf8_lossy(&output.stdout);
                            tracing::debug!("yay -Si {} output ({} bytes)", name, text.len());
                            let dep_names = parse_pacman_si_deps(&text);
                            if !dep_names.is_empty() {
                                tracing::info!("Using yay to resolve dependencies for {}", name);
                                used_helper = true;
                                for dep_spec in dep_names {
                                    let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                                    // Skip if this dependency is the package itself
                                    if pkg_name == name {
                                        tracing::debug!(
                                            "Skipping self-reference: {} == {}",
                                            pkg_name,
                                            name
                                        );
                                        continue;
                                    }
                                    // Filter out .so files (virtual packages)
                                    if pkg_name.ends_with(".so")
                                        || pkg_name.contains(".so.")
                                        || pkg_name.contains(".so=")
                                    {
                                        tracing::debug!(
                                            "Filtering out virtual package: {}",
                                            pkg_name
                                        );
                                        continue;
                                    }

                                    let status = determine_status(
                                        &pkg_name,
                                        &version_req,
                                        installed,
                                        upgradable,
                                    );
                                    let (source, is_core) =
                                        determine_dependency_source(&pkg_name, installed);
                                    let is_system = is_core || is_system_package(&pkg_name);

                                    deps.push(DependencyInfo {
                                        name: pkg_name,
                                        version: version_req,
                                        status,
                                        source,
                                        required_by: vec![name.to_string()],
                                        depends_on: Vec::new(),
                                        is_core,
                                        is_system,
                                    });
                                }
                            }
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            tracing::debug!(
                                "yay -Si {} failed (will use API): {}",
                                name,
                                stderr.trim()
                            );
                        }
                    }
                    Err(_) => {
                        // yay not available, continue to API fallback
                    }
                }
            }

            // Skip AUR API fallback - if paru/yay failed, the package likely doesn't exist
            // This prevents unnecessary API calls for binaries/scripts that aren't packages
            // The dependency will be marked as Missing by the status determination logic
            if !used_helper {
                tracing::debug!(
                    "Skipping AUR API for {} - paru/yay failed or not available (likely not a real package)",
                    name
                );
                // Return empty deps - the dependency will be marked as Missing
                // This is better than making unnecessary API calls
            }

            // Try to fetch and parse .SRCINFO to get makedepends/checkdepends and enhance dependency list
            // This complements the helper/API results with build-time dependencies
            match fetch_srcinfo(name) {
                Ok(srcinfo_text) => {
                    tracing::debug!("Successfully fetched .SRCINFO for {}", name);
                    let (
                        srcinfo_depends,
                        srcinfo_makedepends,
                        srcinfo_checkdepends,
                        srcinfo_optdepends,
                    ) = parse_srcinfo_deps(&srcinfo_text);

                    tracing::debug!(
                        "Parsed .SRCINFO: {} depends, {} makedepends, {} checkdepends, {} optdepends",
                        srcinfo_depends.len(),
                        srcinfo_makedepends.len(),
                        srcinfo_checkdepends.len(),
                        srcinfo_optdepends.len()
                    );

                    // Merge depends from .SRCINFO (may have additional entries not in helper/API)
                    let existing_dep_names: HashSet<String> =
                        deps.iter().map(|d| d.name.clone()).collect();

                    // Add missing depends from .SRCINFO
                    for dep_spec in srcinfo_depends {
                        let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                        if pkg_name == name {
                            continue;
                        }
                        if pkg_name.ends_with(".so")
                            || pkg_name.contains(".so.")
                            || pkg_name.contains(".so=")
                        {
                            continue;
                        }

                        if !existing_dep_names.contains(&pkg_name) {
                            let status =
                                determine_status(&pkg_name, &version_req, installed, upgradable);
                            let (source, is_core) =
                                determine_dependency_source(&pkg_name, installed);
                            let is_system = is_core || is_system_package(&pkg_name);

                            deps.push(DependencyInfo {
                                name: pkg_name.clone(),
                                version: version_req,
                                status,
                                source,
                                required_by: vec![name.to_string()],
                                depends_on: Vec::new(),
                                is_core,
                                is_system,
                            });
                        }
                    }

                    // Add makedepends (build-time dependencies)
                    for dep_spec in srcinfo_makedepends {
                        let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                        if pkg_name == name {
                            continue;
                        }
                        if pkg_name.ends_with(".so")
                            || pkg_name.contains(".so.")
                            || pkg_name.contains(".so=")
                        {
                            continue;
                        }

                        if !existing_dep_names.contains(&pkg_name) {
                            let status =
                                determine_status(&pkg_name, &version_req, installed, upgradable);
                            let (source, is_core) =
                                determine_dependency_source(&pkg_name, installed);
                            let is_system = is_core || is_system_package(&pkg_name);

                            deps.push(DependencyInfo {
                                name: pkg_name.clone(),
                                version: version_req,
                                status,
                                source,
                                required_by: vec![name.to_string()],
                                depends_on: Vec::new(),
                                is_core,
                                is_system,
                            });
                        }
                    }

                    // Add checkdepends (test dependencies)
                    for dep_spec in srcinfo_checkdepends {
                        let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                        if pkg_name == name {
                            continue;
                        }
                        if pkg_name.ends_with(".so")
                            || pkg_name.contains(".so.")
                            || pkg_name.contains(".so=")
                        {
                            continue;
                        }

                        if !existing_dep_names.contains(&pkg_name) {
                            let status =
                                determine_status(&pkg_name, &version_req, installed, upgradable);
                            let (source, is_core) =
                                determine_dependency_source(&pkg_name, installed);
                            let is_system = is_core || is_system_package(&pkg_name);

                            deps.push(DependencyInfo {
                                name: pkg_name.clone(),
                                version: version_req,
                                status,
                                source,
                                required_by: vec![name.to_string()],
                                depends_on: Vec::new(),
                                is_core,
                                is_system,
                            });
                        }
                    }

                    // Add optdepends (optional dependencies)
                    for dep_spec in srcinfo_optdepends {
                        // optdepends format: "package: description" or just "package"
                        let dep_spec_clean = if let Some((pkg_part, _)) = dep_spec.split_once(':') {
                            pkg_part.trim()
                        } else {
                            dep_spec.trim()
                        };

                        let (pkg_name, version_req) = parse_dep_spec(dep_spec_clean);
                        if pkg_name == name {
                            continue;
                        }
                        if pkg_name.ends_with(".so")
                            || pkg_name.contains(".so.")
                            || pkg_name.contains(".so=")
                        {
                            continue;
                        }

                        if !existing_dep_names.contains(&pkg_name) {
                            let status =
                                determine_status(&pkg_name, &version_req, installed, upgradable);
                            let (source, is_core) =
                                determine_dependency_source(&pkg_name, installed);
                            let is_system = is_core || is_system_package(&pkg_name);

                            deps.push(DependencyInfo {
                                name: pkg_name.clone(),
                                version: version_req,
                                status,
                                source,
                                required_by: vec![name.to_string()],
                                depends_on: Vec::new(),
                                is_core,
                                is_system,
                            });
                        }
                    }

                    tracing::info!(
                        "Enhanced dependency list with .SRCINFO data: total {} dependencies",
                        deps.len()
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        "Could not fetch .SRCINFO for {}: {} (continuing without it)",
                        name,
                        e
                    );
                }
            }
        }
    }

    tracing::debug!("Resolved {} dependencies for package {}", deps.len(), name);
    Ok(deps)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    struct PathGuard {
        original: Option<String>,
    }

    impl PathGuard {
        fn push(dir: &std::path::Path) -> Self {
            let original = std::env::var("PATH").ok();
            let mut new_path = dir.display().to_string();
            if let Some(ref orig) = original {
                new_path.push(':');
                new_path.push_str(orig);
            }
            unsafe {
                std::env::set_var("PATH", &new_path);
            }
            Self { original }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            if let Some(ref orig) = self.original {
                unsafe {
                    std::env::set_var("PATH", orig);
                }
            } else {
                unsafe {
                    std::env::remove_var("PATH");
                }
            }
        }
    }

    fn write_executable(dir: &std::path::Path, name: &str, body: &str) {
        let path = dir.join(name);
        let mut file = fs::File::create(&path).expect("create stub");
        file.write_all(body.as_bytes()).expect("write stub");
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod stub");
    }

    #[test]
    /// What: Confirm official dependency resolution consumes the pacman stub output and filters virtual entries.
    ///
    /// Inputs:
    /// - Staged `pacman` shell script that prints a crafted `-Si` response including `.so` and versioned dependencies.
    ///
    /// Output:
    /// - Dependency vector contains only the real packages with preserved version requirements and `required_by` set.
    ///
    /// Details:
    /// - Guards against regressions in parsing logic for the pacman path while isolating the function from system binaries via PATH overrides.
    fn resolve_official_uses_pacman_si_stub() {
        let dir = tempdir().expect("tempdir");
        let _test_guard = crate::logic::test_mutex().lock().unwrap();
        let _guard = PathGuard::push(dir.path());
        write_executable(
            dir.path(),
            "pacman",
            r#"#!/bin/sh
if [ "$1" = "-Si" ]; then
cat <<'EOF'
Name            : pkg
Depends On      : dep1 libplaceholder.so other>=1.2
EOF
exit 0
fi
exit 1
"#,
        );

        let installed = HashSet::new();
        let upgradable = HashSet::new();
        let deps = resolve_package_deps(
            "pkg",
            &Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            &installed,
            &upgradable,
        )
        .expect("resolve succeeds");

        assert_eq!(deps.len(), 2);
        let mut names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["dep1", "other"]);

        let other = deps
            .iter()
            .find(|d| d.name == "other")
            .expect("other present");
        assert_eq!(other.version, ">=1.2");
        assert_eq!(other.required_by, vec!["pkg".to_string()]);
    }

    #[test]
    /// What: Verify the AUR branch leverages the helper stub output and skips self-referential dependencies.
    ///
    /// Inputs:
    /// - PATH-injected `paru` script responding to `--version` and `-Si`, plus inert stubs for `yay` and `pacman`.
    ///
    /// Output:
    /// - Dependency list reflects helper-derived entries while omitting the package itself.
    ///
    /// Details:
    /// - Ensures helper discovery short-circuits the API fallback and that parsing behaves consistently for AUR responses.
    fn resolve_aur_prefers_paru_stub_and_skips_self() {
        let dir = tempdir().expect("tempdir");
        let _test_guard = crate::logic::test_mutex().lock().unwrap();
        let _guard = PathGuard::push(dir.path());
        write_executable(
            dir.path(),
            "paru",
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
exit 0
fi
if [ "$1" = "-Si" ]; then
cat <<'EOF'
Name            : pkg
Depends On      : pkg helper extra>=2.0
EOF
exit 0
fi
exit 1
"#,
        );
        write_executable(dir.path(), "yay", "#!/bin/sh\nexit 1\n");
        write_executable(dir.path(), "pacman", "#!/bin/sh\nexit 1\n");
        write_executable(dir.path(), "curl", "#!/bin/sh\nexit 1\n");

        let installed = HashSet::new();
        let upgradable = HashSet::new();
        let deps = resolve_package_deps("pkg", &Source::Aur, &installed, &upgradable)
            .expect("resolve succeeds");

        assert_eq!(deps.len(), 2);
        let mut names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["extra", "helper"]);
        let extra = deps
            .iter()
            .find(|d| d.name == "extra")
            .expect("extra present");
        assert_eq!(extra.version, ">=2.0");
        assert_eq!(extra.required_by, vec!["pkg".to_string()]);
    }
}
