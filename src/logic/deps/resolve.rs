//! Core dependency resolution logic for individual packages.

use super::aur::fetch_aur_deps_from_api;
use super::parse::{parse_dep_spec, parse_pacman_si_deps};
use super::source::{determine_dependency_source, is_system_package};
use super::status::determine_status;
use crate::state::modal::DependencyInfo;
use crate::state::types::Source;
use std::collections::HashSet;
use std::process::Command;

/// Resolve dependencies for a single package.
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
