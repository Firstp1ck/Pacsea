//! Display computation functions for Preflight modal tabs.

use std::collections::{HashMap, HashSet};

use crate::state::PackageItem;

/// What: Compute how many rows the dependency list will render in the Preflight Deps tab.
///
/// Inputs:
/// - `items`: Packages selected for install/remove shown in the modal
/// - `dependency_info`: Flattened dependency metadata resolved for those packages
/// - `dep_tree_expanded`: Set of package names currently expanded in the UI tree
///
/// Output:
/// - Total number of list rows (headers plus visible dependency entries).
///
/// Details:
/// - Mirrors the UI logic to keep keyboard navigation in sync with rendered rows.
/// - Counts one header per package that has dependencies; only counts individual dependencies when
///   that package appears in `dep_tree_expanded` and deduplicates by dependency name.
pub(super) fn compute_display_items_len(
    items: &[PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    dep_tree_expanded: &HashSet<String>,
) -> usize {
    // Group dependencies by the packages that require them (same as UI code)
    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> = HashMap::new();
    for dep in dependency_info {
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    // Count display items: 1 header per package + unique deps per package (only if expanded)
    // IMPORTANT: Show ALL packages, even if they have no dependencies
    // This matches the UI rendering logic
    let mut count = 0;
    for pkg_name in items.iter().map(|p| &p.name) {
        // Always add header for each package (even if no dependencies)
        count += 1;
        // Count unique dependencies only if package is expanded AND has dependencies
        if dep_tree_expanded.contains(pkg_name)
            && let Some(pkg_deps) = grouped.get(pkg_name)
        {
            let mut seen_deps = HashSet::new();
            for dep in pkg_deps {
                if seen_deps.insert(dep.name.as_str()) {
                    count += 1;
                }
            }
        }
    }

    count
}

/// What: Compute how many rows the Sandbox tab list should expose given expansion state.
///
/// Inputs:
/// - `items`: Packages in the transaction
/// - `sandbox_info`: Resolved sandbox analysis for AUR packages
/// - `sandbox_tree_expanded`: Set of package names currently expanded in the Sandbox tab
///
/// Output:
/// - Total list length combining headers plus visible dependency entries.
///
/// Details:
/// - Adds one row per package header.
/// - Adds additional rows for each dependency when package is expanded (only for AUR packages).
pub(super) fn compute_sandbox_display_items_len(
    items: &[PackageItem],
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_tree_expanded: &HashSet<String>,
) -> usize {
    let mut count = 0;
    for item in items {
        count += 1; // Package header
        // Add dependencies only if expanded and AUR
        if matches!(item.source, crate::state::Source::Aur)
            && sandbox_tree_expanded.contains(&item.name)
            && let Some(info) = sandbox_info.iter().find(|s| s.package_name == item.name)
        {
            count += info.depends.len();
            count += info.makedepends.len();
            count += info.checkdepends.len();
            count += info.optdepends.len();
        }
    }
    count
}

/// What: Compute how many rows the Files tab list should expose given expansion state.
///
/// Inputs:
/// - `items`: All packages under review.
/// - `file_info`: Resolved file change metadata grouped per package
/// - `file_tree_expanded`: Set of package names currently expanded in the Files tab
///
/// Output:
/// - Total list length combining headers plus visible file entries.
///
/// Details:
/// - Always counts ALL packages from items, even if they have no files.
/// - Adds one row per package header and additional rows for each file when expanded.
pub fn compute_file_display_items_len(
    items: &[PackageItem],
    file_info: &[crate::state::modal::PackageFileInfo],
    file_tree_expanded: &HashSet<String>,
) -> usize {
    // Create a map for quick lookup of file info by package name
    let file_info_map: HashMap<String, &crate::state::modal::PackageFileInfo> = file_info
        .iter()
        .map(|info| (info.name.clone(), info))
        .collect();

    let mut count = 0;
    // Always count ALL packages from items, even if they have no file info
    for item in items {
        count += 1; // Package header
        if file_tree_expanded.contains(&item.name) {
            // Count file rows if available
            if let Some(pkg_info) = file_info_map.get(&item.name) {
                count += pkg_info.files.len();
            }
        }
    }
    count
}

/// What: Build the flattened `(is_header, label)` list shown by the Files tab renderer.
///
/// Inputs:
/// - `items`: All packages under review.
/// - `file_info`: Resolved file change metadata grouped by package
/// - `file_tree_expanded`: Set of package names that should expand to show individual files
///
/// Output:
/// - Vector of `(bool, String)` pairs distinguishing headers (`true`) from file rows (`false`).
///
/// Details:
/// - Always shows ALL packages from items, even if they have no files.
/// - This ensures packages that failed to resolve files (e.g., due to conflicts) are still visible.
/// - Uses empty strings for file rows because UI draws file details from separate collections.
pub fn build_file_display_items(
    items: &[PackageItem],
    file_info: &[crate::state::modal::PackageFileInfo],
    file_tree_expanded: &HashSet<String>,
) -> Vec<(bool, String)> {
    // Create a map for quick lookup of file info by package name
    let file_info_map: HashMap<String, &crate::state::modal::PackageFileInfo> = file_info
        .iter()
        .map(|info| (info.name.clone(), info))
        .collect();

    let mut display_items: Vec<(bool, String)> = Vec::new();
    // Always show ALL packages from items, even if they have no file info
    for item in items {
        let pkg_name = &item.name;
        display_items.push((true, pkg_name.clone()));
        if file_tree_expanded.contains(pkg_name) {
            // Show file rows if available
            if let Some(pkg_info) = file_info_map.get(pkg_name) {
                display_items.extend(pkg_info.files.iter().map(|_| (false, String::new())));
            }
        }
    }
    display_items
}
