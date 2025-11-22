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

use crate::state::modal::{DependencyInfo, DependencyStatus};
use crate::state::types::{PackageItem, Source};
use parse::parse_dep_spec;
use query::get_upgradable_packages;
use resolve::{batch_fetch_official_deps, fetch_package_conflicts, resolve_package_deps};
use source::{determine_dependency_source, is_system_package};
use status::determine_status;
use std::collections::{HashMap, HashSet};
use utils::dependency_priority;

pub use query::{get_installed_packages, get_provided_packages, is_package_installed_or_provided};
pub use reverse::resolve_reverse_dependencies;
pub use status::{get_installed_version, version_satisfies};

/// What: Check and process conflicts for a package.
///
/// Inputs:
/// - `item`: Package item to check conflicts for.
/// - `root_names`: Set of root package names in the install list.
/// - `installed`: Set of installed package names.
/// - `provided`: Map of provided packages.
/// - `deps`: Mutable reference to the dependency map to update.
///
/// Output:
/// - Updates the `deps` map with conflict entries.
///
/// Details:
/// - Checks conflicts against installed packages and packages in the install list.
/// - Creates conflict entries for both the conflicting package and the current package if needed.
fn process_conflicts(
    item: &PackageItem,
    root_names: &HashSet<String>,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    deps: &mut HashMap<String, DependencyInfo>,
) {
    let conflicts = fetch_package_conflicts(&item.name, &item.source);
    if conflicts.is_empty() {
        return;
    }

    tracing::debug!("Package {} conflicts with: {:?}", item.name, conflicts);

    for conflict_name in conflicts {
        // Skip self-conflicts (package conflicting with itself)
        if conflict_name.eq_ignore_ascii_case(&item.name) {
            tracing::debug!(
                "Skipping self-conflict: {} conflicts with itself",
                item.name
            );
            continue;
        }

        // Check if conflict is installed or provided by any installed package
        let is_installed = crate::logic::deps::query::is_package_installed_or_provided(
            &conflict_name,
            installed,
            provided,
        );

        // Check if conflict is in the install list
        let is_in_install_list = root_names.contains(&conflict_name);

        if !is_installed && !is_in_install_list {
            continue;
        }

        let reason = if is_installed && is_in_install_list {
            format!("conflicts with {conflict_name} (installed and in install list)")
        } else if is_installed {
            format!("conflicts with installed package {conflict_name}")
        } else {
            format!("conflicts with package {conflict_name} in install list")
        };

        // Add or update conflict entry for the conflicting package
        let entry = deps.entry(conflict_name.clone()).or_insert_with(|| {
            // Determine source for conflicting package
            let (source, is_core) =
                crate::logic::deps::source::determine_dependency_source(&conflict_name, installed);
            let is_system =
                is_core || crate::logic::deps::source::is_system_package(&conflict_name);

            DependencyInfo {
                name: conflict_name.clone(),
                version: String::new(),
                status: DependencyStatus::Conflict {
                    reason: reason.clone(),
                },
                source,
                required_by: vec![item.name.clone()],
                depends_on: Vec::new(),
                is_core,
                is_system,
            }
        });

        // Update status to Conflict if not already
        if !matches!(entry.status, DependencyStatus::Conflict { .. }) {
            entry.status = DependencyStatus::Conflict { reason };
        }

        // Add to required_by if not present
        if !entry.required_by.contains(&item.name) {
            entry.required_by.push(item.name.clone());
        }

        // If the conflict is with another package in the install list, also create
        // a conflict entry for the current package being checked, so it shows up
        // in the UI as having a conflict
        if is_in_install_list {
            let reverse_reason = format!("conflicts with package {conflict_name} in install list");
            let current_entry = deps.entry(item.name.clone()).or_insert_with(|| {
                // Determine source for current package
                let (dep_source, is_core) =
                    crate::logic::deps::source::determine_dependency_source(&item.name, installed);
                let is_system =
                    is_core || crate::logic::deps::source::is_system_package(&item.name);

                DependencyInfo {
                    name: item.name.clone(),
                    version: String::new(),
                    status: DependencyStatus::Conflict {
                        reason: reverse_reason.clone(),
                    },
                    source: dep_source,
                    required_by: vec![conflict_name.clone()],
                    depends_on: Vec::new(),
                    is_core,
                    is_system,
                }
            });

            // Update status to Conflict if not already
            if !matches!(current_entry.status, DependencyStatus::Conflict { .. }) {
                current_entry.status = DependencyStatus::Conflict {
                    reason: reverse_reason,
                };
            }

            // Add to required_by if not present
            if !current_entry.required_by.contains(&conflict_name) {
                current_entry.required_by.push(conflict_name.clone());
            }
        }
    }
}

/// What: Process batched dependencies for an official package.
///
/// Inputs:
/// - `name`: Package name.
/// - `dep_names`: Vector of dependency specification strings.
/// - `installed`: Set of installed package names.
/// - `provided`: Map of provided packages.
/// - `upgradable`: Set of upgradable package names.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records.
///
/// Details:
/// - Parses dependency specifications and filters out self-references and .so files.
fn process_batched_dependencies(
    name: &str,
    dep_names: Vec<String>,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Vec<DependencyInfo> {
    let mut deps = Vec::new();
    for dep_spec in dep_names {
        let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
        if pkg_name == name {
            continue;
        }
        if pkg_name.ends_with(".so") || pkg_name.contains(".so.") || pkg_name.contains(".so=") {
            continue;
        }
        let status = determine_status(&pkg_name, &version_req, installed, provided, upgradable);
        let (dep_source, is_core) = determine_dependency_source(&pkg_name, installed);
        let is_system = is_core || is_system_package(&pkg_name);
        deps.push(DependencyInfo {
            name: pkg_name,
            version: version_req,
            status,
            source: dep_source,
            required_by: vec![name.to_string()],
            depends_on: Vec::new(),
            is_core,
            is_system,
        });
    }
    deps
}

/// What: Merge a dependency into the dependency map.
///
/// Inputs:
/// - `dep`: Dependency to merge.
/// - `parent_name`: Name of the package that requires this dependency.
/// - `installed`: Set of installed package names.
/// - `provided`: Map of provided packages.
/// - `upgradable`: Set of upgradable package names.
/// - `deps`: Mutable reference to the dependency map to update.
///
/// Output:
/// - Updates the `deps` map with the merged dependency.
///
/// Details:
/// - Merges status (keeps worst), version requirements (keeps more restrictive), and `required_by` lists.
fn merge_dependency(
    dep: DependencyInfo,
    parent_name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
    deps: &mut HashMap<String, DependencyInfo>,
) {
    let dep_name = dep.name.clone();

    // Check if dependency already exists and get its current state
    let existing_dep = deps.get(&dep_name).cloned();
    let needs_required_by_update = existing_dep
        .as_ref()
        .map(|e| !e.required_by.contains(&parent_name.to_string()))
        .unwrap_or(true);

    // Update or create dependency entry
    let entry = deps
        .entry(dep_name.clone())
        .or_insert_with(|| DependencyInfo {
            name: dep_name.clone(),
            version: dep.version.clone(),
            status: dep.status.clone(),
            source: dep.source.clone(),
            required_by: vec![parent_name.to_string()],
            depends_on: Vec::new(),
            is_core: dep.is_core,
            is_system: dep.is_system,
        });

    // Update required_by (add the parent if not already present)
    if needs_required_by_update {
        entry.required_by.push(parent_name.to_string());
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
            let existing_status =
                determine_status(&entry.name, &entry.version, installed, provided, upgradable);
            let new_status =
                determine_status(&entry.name, &dep.version, installed, provided, upgradable);
            let existing_req_priority = dependency_priority(&existing_status);
            let new_req_priority = dependency_priority(&new_status);

            if new_req_priority < existing_req_priority {
                entry.version = dep.version.clone();
                entry.status = new_status;
            }
        }
    }
}

/// What: Resolve dependencies for a single package.
///
/// Inputs:
/// - `item`: Package item to resolve dependencies for.
/// - `batched_deps_cache`: Optional cache of batched dependencies for official packages.
/// - `installed`: Set of installed package names.
/// - `provided`: Map of provided packages.
/// - `upgradable`: Set of upgradable package names.
///
/// Output:
/// - Returns a result containing a vector of `DependencyInfo` records or an error.
///
/// Details:
/// - Uses batched cache if available for official packages, otherwise calls `resolve_package_deps`.
fn resolve_single_package_deps(
    item: &PackageItem,
    batched_deps_cache: &HashMap<String, Vec<String>>,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Result<Vec<DependencyInfo>, String> {
    let name = &item.name;
    let source = &item.source;

    tracing::debug!(
        "Resolving direct dependencies for {} (source: {:?})",
        name,
        source
    );

    // Check if we have batched results for this official package
    let use_batched = matches!(source, Source::Official { repo, .. } if repo != "local")
        && batched_deps_cache.contains_key(name.as_str());

    if use_batched {
        // Use batched dependency list
        let dep_names = batched_deps_cache
            .get(name.as_str())
            .cloned()
            .unwrap_or_default();
        let deps = process_batched_dependencies(name, dep_names, installed, provided, upgradable);
        Ok(deps)
    } else {
        resolve_package_deps(name, source, installed, provided, upgradable)
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
    let _span = tracing::info_span!(
        "resolve_dependencies",
        stage = "dependencies",
        item_count = items.len()
    )
    .entered();
    let start_time = std::time::Instant::now();
    // Only warn if called from UI thread (not from background workers)
    // Background workers use spawn_blocking which is fine and expected
    let backtrace = std::backtrace::Backtrace::force_capture();
    let backtrace_str = format!("{:?}", backtrace);
    // Only warn if NOT in a blocking task (i.e., called from UI thread/event handlers)
    // Check for various indicators that we're in a blocking thread pool
    let is_blocking_task = backtrace_str.contains("blocking::task")
        || backtrace_str.contains("blocking::pool")
        || backtrace_str.contains("spawn_blocking");
    if !is_blocking_task {
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

    // Get all provided packages (e.g., rustup provides rust)
    // Note: Provides are checked lazily on-demand for performance, not built upfront
    tracing::debug!(
        "Provides will be checked lazily on-demand (not building full set for performance)"
    );
    let provided = get_provided_packages(&installed);

    // Get list of upgradable packages to detect if dependencies need upgrades
    let upgradable = get_upgradable_packages();
    tracing::info!("Found {} upgradable packages", upgradable.len());

    // Initialize set of root packages (for tracking)
    let root_names: HashSet<String> = items.iter().map(|i| i.name.clone()).collect();

    // Check conflicts for packages being installed
    // 1. Check conflicts against installed packages
    // 2. Check conflicts between packages in the install list
    tracing::info!("Checking conflicts for {} package(s)", items.len());
    for item in items.iter() {
        process_conflicts(item, &root_names, &installed, &provided, &mut deps);
    }

    // Note: Reverse conflict checking (checking all installed packages for conflicts with install list)
    // has been removed for performance reasons. Checking 2000+ installed packages would require
    // 2000+ calls to pacman -Si / yay -Si, which is extremely slow.
    //
    // The forward check above is sufficient and fast:
    // - For each package in install list, fetch its conflicts once (1-10 calls total)
    // - Check if those conflict names are in the installed package set (O(1) HashSet lookup)
    // - This catches all conflicts where install list packages conflict with installed packages
    //
    // Conflicts are typically symmetric (if A conflicts with B, then B conflicts with A),
    // so the forward check should catch most cases. If an installed package declares a conflict
    // with a package in the install list, it will be detected when we check the install list
    // package's conflicts against the installed package set.

    // Batch fetch official package dependencies to reduce pacman command overhead
    let official_packages: Vec<&str> = items
        .iter()
        .filter_map(|item| {
            if let Source::Official { repo, .. } = &item.source {
                if *repo != "local" {
                    Some(item.name.as_str())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    let batched_deps_cache = if !official_packages.is_empty() {
        batch_fetch_official_deps(&official_packages)
    } else {
        std::collections::HashMap::new()
    };

    // Resolve ONLY direct dependencies (non-recursive)
    // This is faster and avoids resolving transitive dependencies which can be slow and error-prone
    for item in items {
        match resolve_single_package_deps(
            item,
            &batched_deps_cache,
            &installed,
            &provided,
            &upgradable,
        ) {
            Ok(mut resolved_deps) => {
                tracing::debug!(
                    "  Found {} dependencies for {}",
                    resolved_deps.len(),
                    item.name
                );

                for dep in resolved_deps.drain(..) {
                    merge_dependency(
                        dep,
                        &item.name,
                        &installed,
                        &provided,
                        &upgradable,
                        &mut deps,
                    );

                    // DON'T recursively resolve dependencies - only show direct dependencies
                    // This prevents resolving transitive dependencies which can be slow and error-prone
                }
            }
            Err(e) => {
                tracing::warn!("  Failed to resolve dependencies for {}: {}", item.name, e);
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

    let elapsed = start_time.elapsed();
    let duration_ms = elapsed.as_millis() as u64;
    tracing::info!(
        stage = "dependencies",
        item_count = items.len(),
        result_count = result.len(),
        duration_ms = duration_ms,
        "Dependency resolution complete"
    );
    result
}
