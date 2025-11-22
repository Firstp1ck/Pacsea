//! Core dependency resolution logic for individual packages.

use super::parse::{parse_dep_spec, parse_pacman_si_conflicts, parse_pacman_si_deps};
use super::source::{determine_dependency_source, is_system_package};
use super::srcinfo::{fetch_srcinfo, parse_srcinfo_conflicts, parse_srcinfo_deps};
use super::status::determine_status;
use crate::logic::files::get_pkgbuild_from_cache;
use crate::logic::sandbox::parse_pkgbuild_deps;
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
/// - `HashMap` mapping package name to its dependency list (`Vec<String>`).
///
/// Details:
/// - Batches queries into chunks of 50 to avoid command-line length limits.
/// - Parses multi-package `pacman -Si` output (packages separated by blank lines).
pub(super) fn batch_fetch_official_deps(names: &[&str]) -> HashMap<String, Vec<String>> {
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

/// What: Check if a command is available in PATH.
///
/// Inputs:
/// - `cmd`: Command name to check.
///
/// Output:
/// - Returns true if the command exists and can be executed.
///
/// Details:
/// - Uses a simple version check to verify command availability.
fn is_command_available(cmd: &str) -> bool {
    Command::new(cmd)
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

/// What: Check if a package name should be filtered out (virtual package or self-reference).
///
/// Inputs:
/// - `pkg_name`: Package name to check.
/// - `parent_name`: Name of the parent package (to detect self-references).
///
/// Output:
/// - Returns true if the package should be filtered out.
///
/// Details:
/// - Filters out .so files (virtual packages) and self-references.
fn should_filter_dependency(pkg_name: &str, parent_name: &str) -> bool {
    pkg_name == parent_name
        || pkg_name.ends_with(".so")
        || pkg_name.contains(".so.")
        || pkg_name.contains(".so=")
}

/// What: Convert a dependency spec into a `DependencyInfo` record.
///
/// Inputs:
/// - `dep_spec`: Dependency specification string (may include version requirements).
/// - `parent_name`: Name of the package that requires this dependency.
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns Some(DependencyInfo) if the dependency should be included, None if filtered.
///
/// Details:
/// - Parses the dependency spec, filters out virtual packages and self-references,
///   and determines status, source, and system package flags.
fn process_dependency_spec(
    dep_spec: &str,
    parent_name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Option<DependencyInfo> {
    let (pkg_name, version_req) = parse_dep_spec(dep_spec);

    if should_filter_dependency(&pkg_name, parent_name) {
        if pkg_name == parent_name {
            tracing::debug!("Skipping self-reference: {} == {}", pkg_name, parent_name);
        } else {
            tracing::debug!("Filtering out virtual package: {}", pkg_name);
        }
        return None;
    }

    let status = determine_status(&pkg_name, &version_req, installed, provided, upgradable);
    let (source, is_core) = determine_dependency_source(&pkg_name, installed);
    let is_system = is_core || is_system_package(&pkg_name);

    Some(DependencyInfo {
        name: pkg_name,
        version: version_req,
        status,
        source,
        required_by: vec![parent_name.to_string()],
        depends_on: Vec::new(),
        is_core,
        is_system,
    })
}

/// What: Process a list of dependency specs into `DependencyInfo` records.
///
/// Inputs:
/// - `dep_specs`: Vector of dependency specification strings.
/// - `parent_name`: Name of the package that requires these dependencies.
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records (filtered).
///
/// Details:
/// - Processes each dependency spec and collects valid dependencies.
fn process_dependency_specs(
    dep_specs: Vec<String>,
    parent_name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Vec<DependencyInfo> {
    dep_specs
        .into_iter()
        .filter_map(|dep_spec| {
            process_dependency_spec(&dep_spec, parent_name, installed, provided, upgradable)
        })
        .collect()
}

/// What: Resolve dependencies for a local package using pacman -Qi.
///
/// Inputs:
/// - `name`: Package name.
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records or an error string.
///
/// Details:
/// - Uses pacman -Qi to get dependency information for locally installed packages.
fn resolve_local_package_deps(
    name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Result<Vec<DependencyInfo>, String> {
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
            format!("pacman -Qi failed: {e}")
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            "pacman -Qi {} failed with status {:?}: {}",
            name,
            output.status.code(),
            stderr
        );
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    tracing::debug!("pacman -Qi {} output ({} bytes)", name, text.len());

    let dep_names = parse_pacman_si_deps(&text);
    tracing::debug!(
        "Parsed {} dependency names from pacman -Qi output",
        dep_names.len()
    );

    Ok(process_dependency_specs(
        dep_names, name, installed, provided, upgradable,
    ))
}

/// What: Resolve dependencies for an official package using pacman -Si.
///
/// Inputs:
/// - `name`: Package name.
/// - `repo`: Repository name (for logging).
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records or an error string.
///
/// Details:
/// - Uses pacman -Si to get dependency information for official packages.
fn resolve_official_package_deps(
    name: &str,
    repo: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Result<Vec<DependencyInfo>, String> {
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
            format!("pacman -Si failed: {e}")
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(
            "pacman -Si {} failed with status {:?}: {}",
            name,
            output.status.code(),
            stderr
        );
        return Err(format!("pacman -Si failed for {name}: {stderr}"));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    tracing::debug!("pacman -Si {} output ({} bytes)", name, text.len());

    let dep_names = parse_pacman_si_deps(&text);
    tracing::debug!(
        "Parsed {} dependency names from pacman -Si output",
        dep_names.len()
    );

    Ok(process_dependency_specs(
        dep_names, name, installed, provided, upgradable,
    ))
}

/// What: Try to resolve dependencies using an AUR helper (paru or yay).
///
/// Inputs:
/// - `helper`: Helper command name ("paru" or "yay").
/// - `name`: Package name.
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns Some(Vec<DependencyInfo>) if successful, None otherwise.
///
/// Details:
/// - Executes helper -Si command and parses the output for dependencies.
fn try_helper_resolution(
    helper: &str,
    name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Option<Vec<DependencyInfo>> {
    tracing::debug!("Trying {} -Si {} for dependency resolution", helper, name);
    let output = Command::new(helper)
        .args(["-Si", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::debug!(
            "{} -Si {} failed (will try other methods): {}",
            helper,
            name,
            stderr.trim()
        );
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    tracing::debug!("{} -Si {} output ({} bytes)", helper, name, text.len());
    let dep_names = parse_pacman_si_deps(&text);

    if dep_names.is_empty() {
        return None;
    }

    tracing::info!(
        "Using {} to resolve runtime dependencies for {} (will fetch .SRCINFO for build-time deps)",
        helper,
        name
    );

    let deps = process_dependency_specs(dep_names, name, installed, provided, upgradable);
    Some(deps)
}

/// What: Enhance dependency list with .SRCINFO data.
///
/// Inputs:
/// - `name`: Package name.
/// - `deps`: Existing dependency list to enhance.
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns the enhanced dependency list.
///
/// Details:
/// - Fetches and parses .SRCINFO to add missing depends entries.
fn enhance_with_srcinfo(
    name: &str,
    mut deps: Vec<DependencyInfo>,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Vec<DependencyInfo> {
    let srcinfo_text = match fetch_srcinfo(name, Some(10)) {
        Ok(text) => text,
        Err(e) => {
            tracing::warn!(
                "Could not fetch .SRCINFO for {}: {} (build-time dependencies will be missing)",
                name,
                e
            );
            return deps;
        }
    };

    tracing::debug!("Successfully fetched .SRCINFO for {}", name);
    let (srcinfo_depends, srcinfo_makedepends, srcinfo_checkdepends, srcinfo_optdepends) =
        parse_srcinfo_deps(&srcinfo_text);

    tracing::debug!(
        "Parsed .SRCINFO: {} depends, {} makedepends, {} checkdepends, {} optdepends",
        srcinfo_depends.len(),
        srcinfo_makedepends.len(),
        srcinfo_checkdepends.len(),
        srcinfo_optdepends.len()
    );

    let existing_dep_names: HashSet<String> = deps.iter().map(|d| d.name.clone()).collect();

    deps.extend(
        srcinfo_depends
            .into_iter()
            .filter_map(|dep_spec| {
                process_dependency_spec(&dep_spec, name, installed, provided, upgradable)
            })
            .filter(|dep_info| !existing_dep_names.contains(&dep_info.name)),
    );

    tracing::info!(
        "Enhanced dependency list with .SRCINFO data: total {} dependencies",
        deps.len()
    );
    deps
}

/// What: Fallback to cached PKGBUILD for dependency resolution.
///
/// Inputs:
/// - `name`: Package name.
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records if `PKGBUILD` is found, empty vector otherwise.
///
/// Details:
/// - Attempts to use cached PKGBUILD when .SRCINFO is unavailable (offline fallback).
fn fallback_to_pkgbuild(
    name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Vec<DependencyInfo> {
    let pkgbuild_text = match get_pkgbuild_from_cache(name) {
        Some(text) => text,
        None => {
            tracing::debug!(
                "No cached PKGBUILD available for {} (offline, no dependencies resolved)",
                name
            );
            return Vec::new();
        }
    };

    tracing::info!(
        "Using cached PKGBUILD for {} to resolve dependencies (offline fallback)",
        name
    );
    let (pkgbuild_depends, _, _, _) = parse_pkgbuild_deps(&pkgbuild_text);

    let deps = process_dependency_specs(pkgbuild_depends, name, installed, provided, upgradable);
    tracing::info!(
        "Resolved {} dependencies from cached PKGBUILD for {}",
        deps.len(),
        name
    );
    deps
}

/// What: Resolve dependencies for an AUR package.
///
/// Inputs:
/// - `name`: Package name.
/// - `installed`: Set of locally installed packages.
/// - `provided`: Set of package names provided by installed packages.
/// - `upgradable`: Set of packages flagged for upgrades.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records.
///
/// Details:
/// - Tries paru/yay first, then falls back to .SRCINFO and cached PKGBUILD.
fn resolve_aur_package_deps(
    name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Vec<DependencyInfo> {
    tracing::debug!(
        "Attempting to resolve AUR package: {} (will skip if not found)",
        name
    );

    let mut deps = Vec::new();
    let mut used_helper = false;

    // Try paru first
    if is_command_available("paru")
        && let Some(helper_deps) =
            try_helper_resolution("paru", name, installed, provided, upgradable)
    {
        deps = helper_deps;
        used_helper = true;
    }

    // Try yay if paru didn't work
    if !used_helper
        && is_command_available("yay")
        && let Some(helper_deps) =
            try_helper_resolution("yay", name, installed, provided, upgradable)
    {
        deps = helper_deps;
        used_helper = true;
    }

    if !used_helper {
        tracing::debug!(
            "Skipping AUR API for {} - paru/yay failed or not available (likely not a real package)",
            name
        );
    }

    // Always try to enhance with .SRCINFO
    deps = enhance_with_srcinfo(name, deps, installed, provided, upgradable);

    // Fallback to PKGBUILD if no dependencies were found
    if !used_helper && deps.is_empty() {
        deps = fallback_to_pkgbuild(name, installed, provided, upgradable);
    }

    deps
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
pub(super) fn resolve_package_deps(
    name: &str,
    source: &Source,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Result<Vec<DependencyInfo>, String> {
    let deps = match source {
        Source::Official { repo, .. } => {
            if repo == "local" {
                resolve_local_package_deps(name, installed, provided, upgradable)?
            } else {
                resolve_official_package_deps(name, repo, installed, provided, upgradable)?
            }
        }
        Source::Aur => resolve_aur_package_deps(name, installed, provided, upgradable),
    };

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
pub(super) fn fetch_package_conflicts(name: &str, source: &Source) -> Vec<String> {
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
            if let Ok(srcinfo_text) = fetch_srcinfo(name, Some(10)) {
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
            // If PATH is missing or empty, use a default system PATH
            let base_path = original
                .as_ref()
                .filter(|p| !p.is_empty())
                .map(String::as_str)
                .unwrap_or("/usr/bin:/bin:/usr/local/bin");
            let mut new_path = dir.display().to_string();
            new_path.push(':');
            new_path.push_str(base_path);
            unsafe {
                std::env::set_var("PATH", &new_path);
            }
            Self { original }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            if let Some(ref orig) = self.original {
                // Only restore if the original PATH was valid (not empty)
                if orig.is_empty() {
                    // If original was empty, restore to a default system PATH
                    unsafe {
                        std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin");
                    }
                } else {
                    unsafe {
                        std::env::set_var("PATH", orig);
                    }
                }
            } else {
                // If PATH was missing, set a default system PATH
                unsafe {
                    std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin");
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
        // Ensure PATH is in a clean state before modifying it
        if std::env::var("PATH").is_err() {
            unsafe { std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin") };
        }
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
        // Ensure PATH is in a clean state before modifying it
        if std::env::var("PATH").is_err() {
            unsafe { std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin") };
        }
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
