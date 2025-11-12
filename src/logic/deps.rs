//! Dependency resolution and analysis for preflight checks.

mod aur;
mod parse;
mod query;
mod resolve;
mod reverse;
mod source;
mod srcinfo;
mod status;
mod utils;

use crate::state::modal::DependencyInfo;
use crate::state::types::{PackageItem, Source};
use query::get_upgradable_packages;
use resolve::resolve_package_deps;
use source::determine_dependency_source;
use status::determine_status;
use std::collections::{HashMap, HashSet};
use utils::dependency_priority;

pub use query::get_installed_packages;
pub use reverse::resolve_reverse_dependencies;
pub use status::{get_installed_version, version_satisfies};

/// What: Determine the source type for a dependency package name.
///
/// Inputs:
/// - `name`: Package name to check.
/// - `installed`: Set of installed packages.
///
/// Output:
/// - Returns `Source` enum indicating whether the package is official or AUR.
///
/// Details:
/// - Checks if package is installed and queries its repository.
/// - Defaults to Official if repository cannot be determined.
fn determine_dep_source(name: &str, installed: &HashSet<String>) -> Source {
    let (source, _) = determine_dependency_source(name, installed);
    match source {
        crate::state::modal::DependencySource::Official { repo } => Source::Official {
            repo,
            arch: "x86_64".to_string(), // Default arch, could be improved
        },
        crate::state::modal::DependencySource::Aur => Source::Aur,
        crate::state::modal::DependencySource::Local => Source::Official {
            repo: "local".to_string(),
            arch: "x86_64".to_string(),
        },
    }
}

/// What: Resolve dependencies for the requested install set while consolidating duplicates.
///
/// Inputs:
/// - `items`: Ordered slice of packages that should be analysed for dependency coverage.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records summarising dependency status and provenance.
///
/// Details:
/// - Resolves ONLY direct dependencies (non-recursive) for each package in the list.
/// - Merges duplicates by name, retaining the most severe status across all requesters.
/// - Populates `depends_on` and `required_by` relationships to reflect dependency relationships.
pub fn resolve_dependencies(items: &[PackageItem]) -> Vec<DependencyInfo> {
    tracing::info!(
        "Starting dependency resolution for {} package(s)",
        items.len()
    );
    let start_time = std::time::Instant::now();
    // Only warn if called from UI thread (not from background workers)
    // Background workers use spawn_blocking which is fine and expected
    let backtrace = std::backtrace::Backtrace::force_capture();
    let backtrace_str = format!("{:?}", backtrace);
    // Only warn if NOT in a blocking task (i.e., called from UI thread/event handlers)
    if !backtrace_str.contains("blocking::task") && !backtrace_str.contains("spawn_blocking") {
        tracing::warn!(
            "[Deps] resolve_dependencies called synchronously from UI thread! This will block! Backtrace:\n{}",
            backtrace_str
        );
    }

    if items.is_empty() {
        tracing::warn!("No packages provided for dependency resolution");
        return Vec::new();
    }

    let mut deps: HashMap<String, DependencyInfo> = HashMap::new();

    // Get installed packages set
    tracing::info!("Fetching list of installed packages...");
    let installed = get_installed_packages();
    tracing::info!("Found {} installed packages", installed.len());

    // Get list of upgradable packages to detect if dependencies need upgrades
    let upgradable = get_upgradable_packages();
    tracing::info!("Found {} upgradable packages", upgradable.len());

    // Initialize set of root packages (for tracking)
    let root_names: HashSet<String> = items.iter().map(|i| i.name.clone()).collect();

    // Resolve ONLY direct dependencies (non-recursive)
    // This is faster and avoids resolving transitive dependencies which can be slow and error-prone
    for item in items {
        let name = item.name.clone();
        let source = item.source.clone();

        tracing::debug!(
            "Resolving direct dependencies for {} (source: {:?})",
            name,
            source
        );

        match resolve_package_deps(&name, &source, &installed, &upgradable) {
            Ok(mut resolved_deps) => {
                tracing::debug!("  Found {} dependencies for {}", resolved_deps.len(), name);

                for dep in resolved_deps.drain(..) {
                    let dep_name = dep.name.clone();

                    // Check if dependency already exists and get its current state
                    let existing_dep = deps.get(&dep_name).cloned();
                    let needs_required_by_update = existing_dep
                        .as_ref()
                        .map(|e| !e.required_by.contains(&name))
                        .unwrap_or(true);

                    // Update or create dependency entry
                    {
                        let entry =
                            deps.entry(dep_name.clone())
                                .or_insert_with(|| DependencyInfo {
                                    name: dep_name.clone(),
                                    version: dep.version.clone(),
                                    status: dep.status.clone(),
                                    source: dep.source.clone(),
                                    required_by: vec![name.clone()],
                                    depends_on: Vec::new(),
                                    is_core: dep.is_core,
                                    is_system: dep.is_system,
                                });

                        // Update required_by (add the parent if not already present)
                        if needs_required_by_update {
                            entry.required_by.push(name.clone());
                        }

                        // Merge status (keep worst)
                        let existing_priority = dependency_priority(&entry.status);
                        let new_priority = dependency_priority(&dep.status);
                        if new_priority < existing_priority {
                            entry.status = dep.status.clone();
                        }

                        // Merge version requirements (keep more restrictive)
                        if !dep.version.is_empty() && dep.version != entry.version {
                            if entry.version.is_empty() {
                                entry.version = dep.version.clone();
                            } else {
                                // Check which version requirement is more restrictive
                                let existing_status = determine_status(
                                    &entry.name,
                                    &entry.version,
                                    &installed,
                                    &upgradable,
                                );
                                let new_status = determine_status(
                                    &entry.name,
                                    &dep.version,
                                    &installed,
                                    &upgradable,
                                );
                                let existing_req_priority = dependency_priority(&existing_status);
                                let new_req_priority = dependency_priority(&new_status);

                                if new_req_priority < existing_req_priority {
                                    entry.version = dep.version.clone();
                                    entry.status = new_status;
                                }
                            }
                        }
                    } // Drop entry borrow here

                    // DON'T recursively resolve dependencies - only show direct dependencies
                    // This prevents resolving transitive dependencies which can be slow and error-prone
                }
            }
            Err(e) => {
                tracing::warn!("  Failed to resolve dependencies for {}: {}", name, e);
            }
        }
    }

    let mut result: Vec<DependencyInfo> = deps.into_values().collect();
    tracing::info!("Total unique dependencies found: {}", result.len());

    // Sort dependencies: conflicts first, then missing, then to-install, then installed
    result.sort_by(|a, b| {
        let priority_a = dependency_priority(&a.status);
        let priority_b = dependency_priority(&b.status);
        priority_a
            .cmp(&priority_b)
            .then_with(|| a.name.cmp(&b.name))
    });

    tracing::info!(
        "Dependency resolution complete. Returning {} dependencies",
        result.len()
    );
    let elapsed = start_time.elapsed();
    if elapsed.as_secs() > 1 {
        tracing::warn!(
            "[Deps] Dependency resolution took {:?} (very slow!)",
            elapsed
        );
    } else {
        tracing::info!("[Deps] Dependency resolution took {:?}", elapsed);
    }
    result
}
