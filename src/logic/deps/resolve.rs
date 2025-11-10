//! Core dependency resolution logic for individual packages.

use super::aur::fetch_aur_deps_from_api;
use super::parse::{parse_dep_spec, parse_pacman_si_deps};
use super::source::{determine_dependency_source, is_system_package};
use super::status::determine_status;
use crate::state::modal::DependencyInfo;
use crate::state::types::Source;
use std::collections::HashSet;
use std::process::Command;

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
            // Use pacman -Si to get dependency list (shows all deps, not just ones to download)
            tracing::debug!("Running: pacman -Si {}", name);
            let spec = if repo.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", repo, name)
            };
            let output = Command::new("pacman")
                .args(["-Si", &spec])
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .output()
                .map_err(|e| {
                    tracing::error!("Failed to execute pacman -Si {}: {}", spec, e);
                    format!("pacman -Si failed: {}", e)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!(
                    "pacman -Si {} failed with status {:?}: {}",
                    spec,
                    output.status.code(),
                    stderr
                );
                return Err(format!("pacman -Si failed for {}: {}", spec, stderr));
            }

            let text = String::from_utf8_lossy(&output.stdout);
            tracing::debug!("pacman -Si {} output ({} bytes)", spec, text.len());

            // Parse "Depends On" field from pacman -Si output
            let dep_names = parse_pacman_si_deps(&text);
            tracing::debug!(
                "Parsed {} dependency names from pacman -Si output",
                dep_names.len()
            );

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
        }
        Source::Aur => {
            // For AUR packages, try to use paru/yay to resolve dependencies
            // Fallback: fetch from AUR API if paru/yay is not available
            tracing::debug!(
                "Checking for paru/yay availability for AUR package: {}",
                name
            );

            // Check if paru exists
            let has_paru = Command::new("paru").args(["--version"]).output().is_ok();

            // Check if yay exists
            let has_yay = Command::new("yay").args(["--version"]).output().is_ok();

            // Try paru/yay first, but fall back to API if they fail
            // Use -Si to get all dependencies (similar to pacman -Si)
            let mut used_helper = false;

            if has_paru {
                tracing::debug!("Trying paru -Si {} for dependency resolution", name);
                match Command::new("paru")
                    .args(["-Si", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
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

            // Always fall back to AUR API if helper didn't work or wasn't available
            // This ensures we get dependencies even if paru/yay fails or isn't installed
            if !used_helper {
                if has_paru || has_yay {
                    tracing::info!(
                        "Using AUR API to resolve dependencies for {} (paru/yay -Si failed or not available)",
                        name
                    );
                } else {
                    tracing::info!(
                        "Using AUR API to resolve dependencies for {} (paru/yay not available)",
                        name
                    );
                }
                match fetch_aur_deps_from_api(name, installed, upgradable) {
                    Ok(api_deps) => {
                        tracing::info!("Fetched {} dependencies from AUR API", api_deps.len());
                        deps.extend(api_deps);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch dependencies from AUR API: {}", e);
                    }
                }
            }
        }
    }

    tracing::debug!("Resolved {} dependencies for package {}", deps.len(), name);
    Ok(deps)
}

#[cfg(test)]
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
