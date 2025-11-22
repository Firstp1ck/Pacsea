//! Helper functions for Preflight modal mouse event handling.

use crate::state::types::PackageItem;
use std::collections::{HashMap, HashSet};

/// Build display items list for the Deps tab.
///
/// What: Creates a list of display items (headers and dependencies) for the dependency tree view.
///
/// Inputs:
/// - `items`: Packages selected for install/remove shown in the modal
/// - `dependency_info`: Flattened dependency metadata resolved for those packages
/// - `dep_tree_expanded`: Set of package names currently expanded in the UI tree
///
/// Output:
/// - Vector of `(bool, String)` pairs distinguishing headers (`true`) from dependency rows (`false`).
///
/// Details:
/// - Groups dependencies by the packages that require them.
/// - Only includes dependencies when their parent package is expanded.
/// - Always includes all packages, even if they have no dependencies.
/// - Deduplicates dependencies by name within each package group.
pub(super) fn build_deps_display_items(
    items: &[PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    dep_tree_expanded: &HashSet<String>,
) -> Vec<(bool, String)> {
    // Build display items list to find which package header was clicked
    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> = HashMap::new();
    for dep in dependency_info {
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    let mut display_items: Vec<(bool, String)> = Vec::new();
    // Always show ALL packages, even if they have no dependencies
    // This ensures packages that failed to resolve dependencies (e.g., due to conflicts) are still visible
    for pkg_name in items.iter().map(|p| &p.name) {
        display_items.push((true, pkg_name.clone()));
        if dep_tree_expanded.contains(pkg_name) {
            let mut seen_deps = HashSet::new();
            if let Some(pkg_deps) = grouped.get(pkg_name) {
                for dep in pkg_deps {
                    if seen_deps.insert(dep.name.as_str()) {
                        display_items.push((false, String::new()));
                    }
                }
            }
        }
    }
    display_items
}

/// Load cached dependencies from app state.
///
/// What: Retrieves cached dependency information for the given packages from the install list.
///
/// Inputs:
/// - `items`: Packages to find cached dependencies for
/// - `install_list_deps`: Cached dependency information from app state
///
/// Output:
/// - Vector of cached dependency information, filtered to only include dependencies required by the given packages.
///
/// Details:
/// - Filters cached dependencies to only those required by packages in `items`.
/// - Returns empty vector if no matching cached dependencies are found.
pub(super) fn load_cached_dependencies(
    items: &[PackageItem],
    install_list_deps: &[crate::state::modal::DependencyInfo],
) -> Vec<crate::state::modal::DependencyInfo> {
    let item_names: HashSet<String> = items.iter().map(|i| i.name.clone()).collect();
    install_list_deps
        .iter()
        .filter(|dep| {
            dep.required_by
                .iter()
                .any(|req_by| item_names.contains(req_by))
        })
        .cloned()
        .collect()
}

/// Load cached files from app state.
///
/// What: Retrieves cached file information for the given packages from the install list.
///
/// Inputs:
/// - `items`: Packages to find cached files for
/// - `install_list_files`: Cached file information from app state
///
/// Output:
/// - Vector of cached file information, filtered to only include files for the given packages.
///
/// Details:
/// - Filters cached files to only those belonging to packages in `items`.
/// - Returns empty vector if no matching cached files are found.
pub(super) fn load_cached_files(
    items: &[PackageItem],
    install_list_files: &[crate::state::modal::PackageFileInfo],
) -> Vec<crate::state::modal::PackageFileInfo> {
    let item_names: HashSet<String> = items.iter().map(|i| i.name.clone()).collect();
    install_list_files
        .iter()
        .filter(|file_info| item_names.contains(&file_info.name))
        .cloned()
        .collect()
}

/// Load cached services from app state.
///
/// What: Retrieves cached service information for the given packages from the install list.
///
/// Inputs:
/// - `items`: Packages to find cached services for
/// - `action`: Preflight action (Install or Remove)
/// - `services_resolving`: Whether services are currently being resolved
/// - `services_cache_path`: Path to the services cache file
/// - `install_list_services`: Cached service information from app state
///
/// Output:
/// - `Some(Vec<ServiceImpact>)` if cached services are available, `None` otherwise.
///
/// Details:
/// - Only loads cache for Install actions.
/// - Checks if cache file exists with matching signature.
/// - Returns `None` if services are currently being resolved, cache doesn't exist, or cached services are empty.
pub(super) fn load_cached_services(
    items: &[PackageItem],
    action: &crate::state::PreflightAction,
    services_resolving: bool,
    services_cache_path: &std::path::PathBuf,
    install_list_services: &[crate::state::modal::ServiceImpact],
) -> Option<Vec<crate::state::modal::ServiceImpact>> {
    // Try to use cached services from app state (for install actions)
    if !matches!(*action, crate::state::PreflightAction::Install) || services_resolving {
        return None;
    }

    // Check if cache file exists with matching signature
    let cache_exists = if items.is_empty() {
        false
    } else {
        let signature = crate::app::services_cache::compute_signature(items);
        crate::app::services_cache::load_cache(services_cache_path, &signature).is_some()
    };

    if cache_exists && !install_list_services.is_empty() {
        Some(install_list_services.to_vec())
    } else {
        None
    }
}
