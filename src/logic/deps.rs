//! Dependency resolution and analysis for preflight checks.

mod aur;
mod parse;
mod query;
mod resolve;
mod reverse;
mod source;
mod status;
mod utils;

use crate::state::modal::DependencyInfo;
use crate::state::types::PackageItem;
use query::{get_installed_packages, get_upgradable_packages};
use resolve::resolve_package_deps;
use status::determine_status;
use std::collections::HashSet;
use utils::dependency_priority;

pub use reverse::resolve_reverse_dependencies;

/// What: Resolve dependencies for the requested install set while consolidating duplicates.
///
/// Inputs:
/// - `items`: Ordered slice of packages that should be analysed for dependency coverage.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records summarising dependency status and provenance.
///
/// Details:
/// - Invokes pacman/paru helpers to enumerate direct and transitive dependencies, then merges them
///   by name, retaining the most severe status across all requesters for UI presentation.
pub fn resolve_dependencies(items: &[PackageItem]) -> Vec<DependencyInfo> {
    tracing::info!(
        "Starting dependency resolution for {} package(s)",
        items.len()
    );

    if items.is_empty() {
        tracing::warn!("No packages provided for dependency resolution");
        return Vec::new();
    }

    let mut deps = Vec::new();
    let mut seen = HashSet::new();

    // Get installed packages set
    tracing::info!("Fetching list of installed packages...");
    let installed = get_installed_packages();
    tracing::info!("Found {} installed packages", installed.len());

    // Get list of upgradable packages to detect if dependencies need upgrades
    let upgradable = get_upgradable_packages();
    tracing::info!("Found {} upgradable packages", upgradable.len());

    // For each package, resolve its dependencies
    for (idx, item) in items.iter().enumerate() {
        tracing::info!(
            "[{}/{}] Resolving dependencies for package: {} ({:?})",
            idx + 1,
            items.len(),
            item.name,
            item.source
        );

        match resolve_package_deps(&item.name, &item.source, &installed, &upgradable) {
            Ok(mut resolved) => {
                tracing::info!("  Found {} dependencies for {}", resolved.len(), item.name);
                for dep in resolved.drain(..) {
                    let dep_name = dep.name.clone();
                    if let Some(existing_idx) = deps
                        .iter()
                        .position(|d: &DependencyInfo| d.name == dep_name)
                    {
                        // Merge required_by lists for duplicate dependencies
                        let existing = &mut deps[existing_idx];
                        for req in &dep.required_by {
                            if !existing.required_by.contains(req) {
                                existing.required_by.push(req.clone());
                            }
                        }
                        // Keep the "worst" status when merging (Conflict > Missing > ToUpgrade > ToInstall > Installed)
                        let existing_priority = dependency_priority(&existing.status);
                        let new_priority = dependency_priority(&dep.status);

                        // Update version requirement if needed and re-evaluate status
                        let version_changed = if !dep.version.is_empty()
                            && dep.version != existing.version
                        {
                            if existing.version.is_empty() {
                                // Existing has no version requirement, use the new one
                                existing.version = dep.version.clone();
                                true
                            } else {
                                // Both have version requirements - check which one is more restrictive
                                // by evaluating both and keeping the one that results in worse status
                                let existing_status = determine_status(
                                    &existing.name,
                                    &existing.version,
                                    &installed,
                                    &upgradable,
                                );
                                let new_status = determine_status(
                                    &existing.name,
                                    &dep.version,
                                    &installed,
                                    &upgradable,
                                );
                                let existing_req_priority = dependency_priority(&existing_status);
                                let new_req_priority = dependency_priority(&new_status);

                                if new_req_priority < existing_req_priority {
                                    // New requirement is more restrictive (results in worse status)
                                    existing.version = dep.version.clone();
                                    true
                                } else {
                                    // Existing requirement is more restrictive, keep it
                                    false
                                }
                            }
                        } else {
                            false
                        };

                        // If version requirement changed, we need to re-evaluate the status
                        if version_changed {
                            // Re-evaluate status based on the updated version requirement
                            existing.status = determine_status(
                                &existing.name,
                                &existing.version,
                                &installed,
                                &upgradable,
                            );
                        } else if new_priority < existing_priority {
                            // New status is worse (lower priority number), so update it
                            existing.status = dep.status.clone();
                        }
                        tracing::debug!(
                            "    Merged dependency: {} (now required by: {:?}, status: {:?})",
                            dep_name,
                            existing.required_by,
                            existing.status
                        );
                    } else {
                        seen.insert(dep_name.clone());
                        tracing::debug!("    Added dependency: {} ({:?})", dep_name, dep.status);
                        deps.push(dep);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("  Failed to resolve dependencies for {}: {}", item.name, e);
            }
        }
    }

    tracing::info!("Total unique dependencies found: {}", deps.len());

    // Sort dependencies: conflicts first, then missing, then to-install, then installed
    deps.sort_by(|a, b| {
        let priority_a = dependency_priority(&a.status);
        let priority_b = dependency_priority(&b.status);
        priority_a
            .cmp(&priority_b)
            .then_with(|| a.name.cmp(&b.name))
    });

    tracing::info!(
        "Dependency resolution complete. Returning {} dependencies",
        deps.len()
    );
    deps
}
