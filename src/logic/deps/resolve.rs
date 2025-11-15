//! Core dependency resolution logic for individual packages.

use super::parse::{parse_dep_spec, parse_pacman_si_conflicts, parse_pacman_si_deps};
use super::source::{determine_dependency_source, is_system_package};
use super::srcinfo::{fetch_srcinfo, parse_srcinfo_conflicts, parse_srcinfo_deps};
use super::status::determine_status;
use crate::state::modal::DependencyInfo;
use crate::state::types::Source;
use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};

/// What: Batch fetch dependency lists for multiple official packages using `pacman -Si`.
///
/// Inputs:
/// - `names`: Package names to query (must be official packages, not local).
///
/// Output:
/// - HashMap mapping package name to its dependency list (Vec<String>).
///
/// Details:
/// - Batches queries into chunks of 50 to avoid command-line length limits.
/// - Parses multi-package `pacman -Si` output (packages separated by blank lines).
pub(crate) fn batch_fetch_official_deps(names: &[&str]) -> HashMap<String, Vec<String>> {
    const BATCH_SIZE: usize = 50;
    let mut result_map = HashMap::new();

    for chunk in names.chunks(BATCH_SIZE) {
        let mut args = vec!["-Si"];
        args.extend(chunk.iter().copied());
        match Command::new("pacman")
            .args(&args)
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
        {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                // Parse multi-package output: packages are separated by blank lines
                let mut package_blocks = Vec::new();
                let mut current_block = String::new();
                for line in text.lines() {
                    if line.trim().is_empty() {
                        if !current_block.is_empty() {
                            package_blocks.push(current_block.clone());
                            current_block.clear();
                        }
                    } else {
                        current_block.push_str(line);
                        current_block.push('\n');
                    }
                }
                if !current_block.is_empty() {
                    package_blocks.push(current_block);
                }

                // Parse each block to extract package name and dependencies
                for block in package_blocks {
                    let dep_names = parse_pacman_si_deps(&block);
                    // Extract package name from block
                    if let Some(name_line) =
                        block.lines().find(|l| l.trim_start().starts_with("Name"))
                        && let Some((_, name)) = name_line.split_once(':')
                    {
                        let pkg_name = name.trim().to_string();
                        result_map.insert(pkg_name, dep_names);
                    }
                }
            }
            _ => {
                // If batch fails, fall back to individual queries (but don't do it here to avoid recursion)
                // The caller will handle individual queries
                break;
            }
        }
    }
    result_map
}

/// What: Resolve direct dependency metadata for a single package.
///
/// Inputs:
/// - `name`: Package identifier whose dependencies should be enumerated.
/// - `source`: Source enum describing whether the package is official or AUR.
/// - `installed`: Set of locally installed packages for status determination.
/// - `provided`: Set of package names provided by installed packages.
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
    provided: &HashSet<String>,
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
                tracing::debug!(
                    "Parsed {} dependency names from pacman -Qi output",
                    dep_names.len()
                );

                // Process runtime dependencies only
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

                    let status =
                        determine_status(&pkg_name, &version_req, installed, provided, upgradable);
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

                // Skip optional dependencies - only show runtime dependencies
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
            tracing::debug!(
                "Parsed {} dependency names from pacman -Si output",
                dep_names.len()
            );

            // Process runtime dependencies (depends)
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

                let status =
                    determine_status(&pkg_name, &version_req, installed, provided, upgradable);
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

            // Skip optional dependencies - only show runtime dependencies (depends)
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
                            // Note: paru -Si only returns runtime dependencies (depends), not makedepends/checkdepends
                            // We'll still fetch .SRCINFO later to get build-time dependencies
                            if !dep_names.is_empty() {
                                tracing::info!(
                                    "Using paru to resolve runtime dependencies for {} (will fetch .SRCINFO for build-time deps)",
                                    name
                                );
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
                                        provided,
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
                            // Note: yay -Si only returns runtime dependencies (depends), not makedepends/checkdepends
                            // We'll still fetch .SRCINFO later to get build-time dependencies
                            if !dep_names.is_empty() {
                                tracing::info!(
                                    "Using yay to resolve runtime dependencies for {} (will fetch .SRCINFO for build-time deps)",
                                    name
                                );
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
                                        provided,
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

            // Always try to fetch and parse .SRCINFO to get makedepends/checkdepends and enhance dependency list
            // This is critical because paru/yay -Si only returns runtime dependencies (depends),
            // not build-time dependencies (makedepends/checkdepends)
            // Even if paru/yay succeeded, we still need .SRCINFO for complete dependency information
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
                            let status = determine_status(
                                &pkg_name,
                                &version_req,
                                installed,
                                provided,
                                upgradable,
                            );
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

                    // Skip makedepends, checkdepends, and optdepends - only show runtime dependencies (depends)

                    tracing::info!(
                        "Enhanced dependency list with .SRCINFO data: total {} dependencies",
                        deps.len()
                    );
                }
                Err(e) => {
                    // Log as warning since missing .SRCINFO means we won't have makedepends/checkdepends
                    // This is important for AUR packages as build-time dependencies won't be shown
                    tracing::warn!(
                        "Could not fetch .SRCINFO for {}: {} (build-time dependencies will be missing)",
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

/// What: Fetch conflicts for a package from pacman or AUR sources.
///
/// Inputs:
/// - `name`: Package identifier.
/// - `source`: Source enum describing whether the package is official or AUR.
///
/// Output:
/// - Returns a vector of conflicting package names, or empty vector on error.
///
/// Details:
/// - For official packages, uses `pacman -Si` to get conflicts.
/// - For AUR packages, tries paru/yay first, then falls back to .SRCINFO.
pub(crate) fn fetch_package_conflicts(name: &str, source: &Source) -> Vec<String> {
    match source {
        Source::Official { repo, .. } => {
            // Handle local packages specially - use pacman -Qi instead of -Si
            if repo == "local" {
                tracing::debug!("Running: pacman -Qi {} (local package, conflicts)", name);
                if let Ok(output) = Command::new("pacman")
                    .args(["-Qi", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    return parse_pacman_si_conflicts(&text);
                }
                return Vec::new();
            }

            // Use pacman -Si to get conflicts
            tracing::debug!("Running: pacman -Si {} (conflicts)", name);
            if let Ok(output) = Command::new("pacman")
                .args(["-Si", name])
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                && output.status.success()
            {
                let text = String::from_utf8_lossy(&output.stdout);
                return parse_pacman_si_conflicts(&text);
            }
            Vec::new()
        }
        Source::Aur => {
            // Try paru/yay first
            let has_paru = Command::new("paru")
                .args(["--version"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .is_ok();

            let has_yay = Command::new("yay")
                .args(["--version"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .is_ok();

            if has_paru {
                tracing::debug!("Trying paru -Si {} for conflicts", name);
                if let Ok(output) = Command::new("paru")
                    .args(["-Si", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let conflicts = parse_pacman_si_conflicts(&text);
                    if !conflicts.is_empty() {
                        return conflicts;
                    }
                }
            }

            if has_yay {
                tracing::debug!("Trying yay -Si {} for conflicts", name);
                if let Ok(output) = Command::new("yay")
                    .args(["-Si", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let conflicts = parse_pacman_si_conflicts(&text);
                    if !conflicts.is_empty() {
                        return conflicts;
                    }
                }
            }

            // Fall back to .SRCINFO
            if let Ok(srcinfo_text) = fetch_srcinfo(name) {
                tracing::debug!("Using .SRCINFO for conflicts of {}", name);
                return parse_srcinfo_conflicts(&srcinfo_text);
            }

            Vec::new()
        }
    }
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
        let _test_guard = crate::global_test_mutex_lock();
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
        let provided = HashSet::new();
        let deps = resolve_package_deps(
            "pkg",
            &Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            &installed,
            &provided,
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
        let _test_guard = crate::global_test_mutex_lock();
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
        let provided = HashSet::new();
        let deps = resolve_package_deps("pkg", &Source::Aur, &installed, &provided, &upgradable)
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
