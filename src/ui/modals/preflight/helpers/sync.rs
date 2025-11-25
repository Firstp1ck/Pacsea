use crate::state::modal::{
    DependencyInfo, PackageFileInfo, PreflightAction, PreflightTab, ServiceImpact,
};
use crate::state::{AppState, PackageItem};

/// What: Synchronize dependency information from app cache to preflight modal.
///
/// Inputs:
/// - `app`: Application state containing cached dependencies
/// - `items`: Packages currently in preflight review
/// - `action`: Whether this is an install or remove action
/// - `tab`: Current active tab
/// - `dependency_info`: Mutable reference to dependency info in modal
/// - `dep_selected`: Mutable reference to selected dependency index
///
/// Output:
/// - Updates `dependency_info` and `dep_selected` if cache has new data
///
/// Details:
/// - Only syncs when action is Install and tab is Deps
/// - Filters dependencies to only those required by current items
/// - Handles background resolution state checking
pub fn sync_dependencies(
    app: &AppState,
    items: &[PackageItem],
    action: PreflightAction,
    tab: PreflightTab,
    dependency_info: &mut Vec<DependencyInfo>,
    dep_selected: &mut usize,
) {
    if !matches!(action, PreflightAction::Install) {
        return;
    }

    // Sync dependencies when:
    // 1. On Deps tab (to show dependency list)
    // 2. On Summary tab (to show conflicts)
    // 3. Or when dependency_info is empty (first load)
    let should_sync = dependency_info.is_empty()
        || matches!(tab, PreflightTab::Deps)
        || matches!(tab, PreflightTab::Summary);

    if !should_sync {
        return;
    }

    if !app.install_list_deps.is_empty() {
        // Get set of current package names for filtering
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();

        // Filter to only show dependencies required by current items
        let filtered: Vec<DependencyInfo> = app
            .install_list_deps
            .iter()
            .filter(|dep| {
                // Show dependency if any current item requires it
                dep.required_by
                    .iter()
                    .any(|req_by| item_names.contains(req_by))
            })
            .cloned()
            .collect();

        tracing::debug!(
            "[UI] Deps sync: tab={:?}, cache={}, filtered={}, items={:?}, resolving={}, current={}",
            tab,
            app.install_list_deps.len(),
            filtered.len(),
            item_names,
            app.deps_resolving,
            dependency_info.len()
        );

        // Always update when cache has data and we're on a tab that needs it
        // This ensures conflicts are shown in Summary tab and dependencies in Deps tab
        // Only reset selection if dependencies were empty (first load) - don't reset on every render
        let was_empty = dependency_info.is_empty();
        let old_len = dependency_info.len();
        *dependency_info = filtered;
        let new_len = dependency_info.len();

        if old_len != new_len {
            tracing::info!(
                "[UI] Deps sync: Updated dependency_info from {} to {} entries (was_empty={})",
                old_len,
                new_len,
                was_empty
            );
        }

        // Only reset selection if this is the first load (was empty), not on every render
        if was_empty {
            *dep_selected = 0;
        }
    } else if dependency_info.is_empty() {
        // Check if background resolution is in progress
        if app.preflight_deps_resolving || app.deps_resolving {
            // Background resolution in progress - UI will show loading state
            tracing::debug!(
                "[UI] Deps tab: background resolution in progress, items={:?}",
                items.iter().map(|i| &i.name).collect::<Vec<_>>()
            );
        } else {
            // Cache is empty and no resolution in progress - trigger background resolution
            // This will be handled by the event handler when switching to Deps tab
            tracing::debug!(
                "[UI] Deps tab: cache is empty, will auto-resolve, items={:?}",
                items.iter().map(|i| &i.name).collect::<Vec<_>>()
            );
        }
    }
}

/// What: Filter cached files to match current items.
///
/// Inputs:
/// - `app`: Application state with cached files
/// - `item_names`: Set of current package names
///
/// Output:
/// - Vector of matching cached files.
fn filter_cached_files(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
) -> Vec<PackageFileInfo> {
    app.install_list_files
        .iter()
        .filter(|file_info| item_names.contains(&file_info.name))
        .cloned()
        .collect()
}

/// What: Log detailed cache lookup information for debugging.
///
/// Inputs:
/// - `item_names`: Set of current package names
/// - `app`: Application state with cached files
/// - `cached_files`: Filtered cached files
///
/// Output:
/// - None (logs to tracing).
fn log_cache_lookup(
    item_names: &std::collections::HashSet<String>,
    app: &AppState,
    cached_files: &[PackageFileInfo],
) {
    if item_names.is_empty() {
        return;
    }
    let cache_package_names: Vec<String> = app
        .install_list_files
        .iter()
        .map(|f| f.name.clone())
        .collect();
    let missing_from_cache: Vec<String> = item_names
        .iter()
        .filter(|item_name| !cache_package_names.contains(item_name))
        .cloned()
        .collect();
    if !missing_from_cache.is_empty() {
        tracing::debug!(
            "[UI] sync_files: Cache lookup - items={:?}, cache_has={:?}, missing_from_cache={:?}, matched={:?}",
            item_names.iter().collect::<Vec<_>>(),
            cache_package_names,
            missing_from_cache,
            cached_files.iter().map(|f| &f.name).collect::<Vec<_>>()
        );
    }
}

/// What: Build complete file info list including empty entries for missing packages.
///
/// Inputs:
/// - `items`: Current packages
/// - `cached_files_map`: Map of cached files by package name
///
/// Output:
/// - Complete file info list with all items represented.
fn build_complete_file_info(
    items: &[PackageItem],
    cached_files_map: &std::collections::HashMap<String, PackageFileInfo>,
) -> Vec<PackageFileInfo> {
    items
        .iter()
        .map(|item| {
            cached_files_map
                .get(&item.name)
                .cloned()
                .unwrap_or_else(|| {
                    tracing::debug!(
                        "[UI] sync_files: Creating empty entry for '{}' (not found in cache map)",
                        item.name
                    );
                    PackageFileInfo {
                        name: item.name.clone(),
                        files: Vec::new(),
                        total_count: 0,
                        new_count: 0,
                        changed_count: 0,
                        removed_count: 0,
                        config_count: 0,
                        pacnew_candidates: 0,
                        pacsave_candidates: 0,
                    }
                })
        })
        .collect()
}

/// What: Check if file info needs to be updated.
///
/// Inputs:
/// - `file_info`: Current file info in modal
/// - `items`: Current packages
/// - `complete_file_info`: Complete file info to compare
///
/// Output:
/// - `true` if update is needed, `false` otherwise.
fn should_update_file_info(
    file_info: &[PackageFileInfo],
    items: &[PackageItem],
    complete_file_info: &[PackageFileInfo],
) -> bool {
    file_info.is_empty()
        || file_info.len() != items.len()
        || complete_file_info.iter().any(|new_info| {
            !file_info.iter().any(|old_info| {
                old_info.name == new_info.name && old_info.total_count == new_info.total_count
            })
        })
}

/// What: Apply file info update to modal.
///
/// Inputs:
/// - `file_info`: Mutable reference to file info in modal
/// - `file_selected`: Mutable reference to selected index
/// - `complete_file_info`: Complete file info to apply
/// - `cached_files`: Cached files for logging
///
/// Output:
/// - Updates modal state.
fn apply_file_info_update(
    file_info: &mut Vec<PackageFileInfo>,
    file_selected: &mut usize,
    complete_file_info: Vec<PackageFileInfo>,
    cached_files: &[PackageFileInfo],
) {
    let old_file_info_len = file_info.len();
    let missing_count = complete_file_info
        .iter()
        .filter(|f| f.total_count == 0)
        .count();
    tracing::info!(
        "[UI] sync_files: Found {} cached file entries, {} missing (creating empty entries), modal had {}, updating...",
        cached_files.len(),
        missing_count,
        old_file_info_len
    );
    for file_info_entry in &complete_file_info {
        if file_info_entry.total_count > 0 {
            tracing::info!(
                "[UI] sync_files: Package '{}' - total={}, new={}, changed={}, removed={}, config={}",
                file_info_entry.name,
                file_info_entry.total_count,
                file_info_entry.new_count,
                file_info_entry.changed_count,
                file_info_entry.removed_count,
                file_info_entry.config_count
            );
        } else {
            tracing::debug!(
                "[UI] sync_files: Package '{}' - no file info yet (empty entry)",
                file_info_entry.name
            );
        }
    }
    tracing::debug!(
        "[UI] sync_files: Syncing {} file infos ({} with data, {} empty) to Preflight modal",
        complete_file_info.len(),
        cached_files.len(),
        missing_count
    );
    *file_info = complete_file_info;
    if *file_selected >= file_info.len() {
        *file_selected = 0;
    }
    tracing::info!(
        "[UI] sync_files: Successfully synced, modal now has {} file entries (was {})",
        file_info.len(),
        old_file_info_len
    );
}

/// What: Synchronize file information from app cache to preflight modal.
///
/// Inputs:
/// - `app`: Application state containing cached file info
/// - `items`: Packages currently in preflight review
/// - `tab`: Current active tab
/// - `file_info`: Mutable reference to file info in modal
/// - `file_selected`: Mutable reference to selected file index
///
/// Output:
/// - Updates `file_info` and `file_selected` if cache has new data
///
/// Details:
/// - Only syncs when tab is Files
/// - Filters files to only those belonging to current items
/// - Handles background resolution state checking
pub fn sync_files(
    app: &AppState,
    items: &[PackageItem],
    tab: PreflightTab,
    file_info: &mut Vec<PackageFileInfo>,
    file_selected: &mut usize,
) {
    if !matches!(tab, PreflightTab::Files) {
        return;
    }

    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();
    let cached_files = filter_cached_files(app, &item_names);
    log_cache_lookup(&item_names, app, &cached_files);

    let cached_files_map: std::collections::HashMap<String, PackageFileInfo> = cached_files
        .iter()
        .map(|f| (f.name.clone(), f.clone()))
        .collect();
    let complete_file_info = build_complete_file_info(items, &cached_files_map);

    tracing::debug!(
        "[UI] sync_files: items={}, cache_size={}, cached_matching={}, modal_size={}, complete={}, resolving={}/{}",
        items.len(),
        app.install_list_files.len(),
        cached_files.len(),
        file_info.len(),
        complete_file_info.len(),
        app.preflight_files_resolving,
        app.files_resolving
    );

    if should_update_file_info(file_info, items, &complete_file_info) {
        apply_file_info_update(file_info, file_selected, complete_file_info, &cached_files);
    } else {
        if app.preflight_files_resolving || app.files_resolving {
            tracing::debug!(
                "[UI] sync_files: Background resolution in progress, items={:?}",
                items.iter().map(|i| &i.name).collect::<Vec<_>>()
            );
        } else if cached_files.is_empty() {
            tracing::debug!(
                "[UI] sync_files: No cached files available and not resolving, items={:?}",
                items.iter().map(|i| &i.name).collect::<Vec<_>>()
            );
        } else {
            tracing::debug!(
                "[UI] sync_files: File info already synced (modal has {} entries, cache has {} matching), no update needed",
                file_info.len(),
                cached_files.len()
            );
        }
        tracing::debug!(
            "[UI] sync_files: No update needed, file_info already in sync (modal={}, items={}, complete={})",
            file_info.len(),
            items.len(),
            complete_file_info.len()
        );
    }
}

/// What: Synchronize service impact information from app cache to preflight modal.
///
/// Inputs:
/// - `app`: Application state containing cached service info
/// - `items`: Packages currently in preflight review
/// - `action`: Whether this is an install or remove action
/// - `service_info`: Mutable reference to service info in modal
/// - `service_selected`: Mutable reference to selected service index
/// - `services_loaded`: Mutable reference to loaded flag
///
/// Output:
/// - Updates `service_info`, `service_selected`, and `services_loaded` if cache has new data
///
/// Details:
/// - Syncs for both Install and Remove actions when no resolution is in progress
/// - Filters services to only those provided by current items
/// - Handles cache file checking for empty results
/// - Syncs resolved service impacts from background resolution
pub fn sync_services(
    app: &AppState,
    items: &[PackageItem],
    _action: PreflightAction,
    service_info: &mut Vec<ServiceImpact>,
    service_selected: &mut usize,
    services_loaded: &mut bool,
) {
    if app.services_resolving || app.preflight_services_resolving {
        return;
    }

    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();
    let cached_services: Vec<_> = app
        .install_list_services
        .iter()
        .filter(|s| s.providers.iter().any(|p| item_names.contains(p)))
        .cloned()
        .collect();

    // Sync results from background resolution if available
    if !cached_services.is_empty() {
        let needs_update = service_info.is_empty()
            || cached_services.len() != service_info.len()
            || cached_services.iter().any(|cached| {
                !service_info
                    .iter()
                    .any(|existing| existing.unit_name == cached.unit_name)
            });
        if needs_update {
            tracing::debug!(
                "[UI] Syncing {} services from background resolution to Preflight modal",
                cached_services.len()
            );
            *service_info = cached_services;
            *services_loaded = true;
        }
    } else if service_info.is_empty() && !*services_loaded {
        // Check if cache file exists with matching signature (even if empty)
        let cache_check_start = std::time::Instant::now();
        let cache_exists = if items.is_empty() {
            false
        } else {
            let signature = crate::app::services_cache::compute_signature(items);
            let result =
                crate::app::services_cache::load_cache(&app.services_cache_path, &signature)
                    .is_some();
            let cache_duration = cache_check_start.elapsed();
            if cache_duration.as_millis() > 10 {
                tracing::warn!(
                    "[UI] Services cache check took {:?} (slow!)",
                    cache_duration
                );
            }
            result
        };

        if cache_exists {
            // Cache exists but is empty - this is valid, means no services found
            tracing::debug!("[UI] Using cached service impacts (empty - no services found)");
            *services_loaded = true;
        }
    }

    if !service_info.is_empty() && *service_selected >= service_info.len() {
        *service_selected = service_info.len().saturating_sub(1);
    }
}

/// What: Check if sandbox resolution is in progress.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - Returns true if sandbox resolution is in progress
const fn is_sandbox_resolving(app: &AppState) -> bool {
    app.preflight_sandbox_resolving || app.sandbox_resolving
}

/// What: Check if cached sandbox data needs to be updated.
///
/// Inputs:
/// - `sandbox_info`: Current sandbox info in modal
/// - `cached_sandbox`: Cached sandbox data from app state
///
/// Output:
/// - Returns true if update is needed
fn needs_sandbox_update(
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    cached_sandbox: &[crate::logic::sandbox::SandboxInfo],
) -> bool {
    sandbox_info.is_empty()
        || cached_sandbox.len() != sandbox_info.len()
        || cached_sandbox.iter().any(|cached| {
            !sandbox_info
                .iter()
                .any(|existing| existing.package_name == cached.package_name)
        })
}

/// What: Log detailed dependency information for sandbox entries.
///
/// Inputs:
/// - `cached_sandbox`: Cached sandbox data to log
fn log_sandbox_dependencies(cached_sandbox: &[crate::logic::sandbox::SandboxInfo]) {
    tracing::info!(
        "[UI] sync_sandbox: Found {} cached sandbox entries matching current items",
        cached_sandbox.len()
    );
    for cached in cached_sandbox {
        let total_deps = cached.depends.len()
            + cached.makedepends.len()
            + cached.checkdepends.len()
            + cached.optdepends.len();
        let installed_deps = cached.depends.iter().filter(|d| d.is_installed).count()
            + cached.makedepends.iter().filter(|d| d.is_installed).count()
            + cached
                .checkdepends
                .iter()
                .filter(|d| d.is_installed)
                .count()
            + cached.optdepends.iter().filter(|d| d.is_installed).count();
        tracing::info!(
            "[UI] sync_sandbox: Package '{}' - total_deps={}, installed_deps={}, depends={}, makedepends={}, checkdepends={}, optdepends={}",
            cached.package_name,
            total_deps,
            installed_deps,
            cached.depends.len(),
            cached.makedepends.len(),
            cached.checkdepends.len(),
            cached.optdepends.len()
        );
    }
}

/// What: Sync cached sandbox data to modal if update is needed.
///
/// Inputs:
/// - `sandbox_info`: Mutable reference to sandbox info in modal
/// - `sandbox_loaded`: Mutable reference to loaded flag
/// - `cached_sandbox`: Cached sandbox data from app state
///
/// Output:
/// - Updates `sandbox_info` and `sandbox_loaded` if needed
fn sync_cached_sandbox(
    sandbox_info: &mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_loaded: &mut bool,
    cached_sandbox: Vec<crate::logic::sandbox::SandboxInfo>,
) {
    if !needs_sandbox_update(sandbox_info, &cached_sandbox) {
        tracing::debug!("[UI] sync_sandbox: No update needed, sandbox_info already in sync");
        return;
    }

    log_sandbox_dependencies(&cached_sandbox);
    tracing::info!(
        "[UI] sync_sandbox: Syncing {} sandbox info entries from background resolution to Preflight modal (was_empty={}, len_diff={})",
        cached_sandbox.len(),
        sandbox_info.is_empty(),
        cached_sandbox.len() != sandbox_info.len()
    );
    *sandbox_info = cached_sandbox;
    *sandbox_loaded = true;
    tracing::debug!(
        "[UI] sync_sandbox: Successfully synced sandbox info, loaded={}",
        *sandbox_loaded
    );
}

/// What: Check if sandbox cache file exists.
///
/// Inputs:
/// - `app`: Application state
/// - `items`: Packages to check cache for
///
/// Output:
/// - Returns true if cache file exists
fn check_sandbox_cache(app: &AppState, items: &[PackageItem]) -> bool {
    let cache_start = std::time::Instant::now();
    let signature = crate::app::sandbox_cache::compute_signature(items);
    let cache_exists =
        crate::app::sandbox_cache::load_cache(&app.sandbox_cache_path, &signature).is_some();
    let cache_duration = cache_start.elapsed();
    if cache_duration.as_millis() > 10 {
        tracing::warn!("[UI] Sandbox cache check took {:?} (slow!)", cache_duration);
    }
    cache_exists
}

/// What: Handle empty sandbox info case by checking cache or resolution state.
///
/// Inputs:
/// - `app`: Application state
/// - `items`: Packages currently in preflight review
/// - `aur_items`: AUR packages only
/// - `sandbox_loaded`: Mutable reference to loaded flag
///
/// Output:
/// - Updates `sandbox_loaded` if appropriate
fn handle_empty_sandbox_info(
    app: &AppState,
    items: &[PackageItem],
    aur_items: &[&PackageItem],
    sandbox_loaded: &mut bool,
) {
    tracing::debug!(
        "[UI] sync_sandbox: Empty sandbox_info, checking cache for items={:?}",
        items.iter().map(|i| &i.name).collect::<Vec<_>>()
    );

    let cache_exists = check_sandbox_cache(app, items);
    if cache_exists {
        if !is_sandbox_resolving(app) {
            tracing::info!(
                "[UI] sync_sandbox: Using cached sandbox info (empty - no sandbox info found)"
            );
            *sandbox_loaded = true;
        }
        return;
    }

    if aur_items.is_empty() {
        tracing::debug!("[UI] sync_sandbox: No AUR packages, marking as loaded");
        *sandbox_loaded = true;
        return;
    }

    if is_sandbox_resolving(app) {
        tracing::info!(
            "[UI] sync_sandbox: Background resolution in progress, items={:?}, aur_items={:?}",
            items.iter().map(|i| &i.name).collect::<Vec<_>>(),
            aur_items.iter().map(|i| &i.name).collect::<Vec<_>>()
        );
    } else {
        tracing::debug!(
            "[UI] sync_sandbox: No cache and no resolution in progress, will trigger on tab switch"
        );
    }
}

/// What: Handle remove action case for sandbox sync.
///
/// Inputs:
/// - `items`: Packages currently in preflight review
/// - `sandbox_loaded`: Mutable reference to loaded flag
///
/// Output:
/// - Updates `sandbox_loaded` if no AUR packages
fn handle_remove_action(items: &[PackageItem], sandbox_loaded: &mut bool) {
    if !items
        .iter()
        .any(|p| matches!(p.source, crate::state::Source::Aur))
    {
        tracing::debug!("[UI] sync_sandbox: Remove action, no AUR packages, marking as loaded");
        *sandbox_loaded = true;
    }
}

/// What: Synchronize sandbox information from app cache to preflight modal.
///
/// Inputs:
/// - `app`: Application state containing cached sandbox info
/// - `items`: Packages currently in preflight review
/// - `action`: Whether this is an install or remove action
/// - `tab`: Current active tab
/// - `sandbox_info`: Mutable reference to sandbox info in modal
/// - `sandbox_selected`: Mutable reference to selected sandbox index
/// - `sandbox_loaded`: Mutable reference to loaded flag
///
/// Output:
/// - Updates `sandbox_info`, `sandbox_selected`, and `sandbox_loaded` if cache has new data
///
/// Details:
/// - Only syncs when action is Install and tab is Sandbox
/// - Filters sandbox info to only AUR packages
/// - Handles cache file checking and background resolution state
/// - Includes comprehensive logging for dependency loading verification
pub fn sync_sandbox(
    app: &AppState,
    items: &[PackageItem],
    action: PreflightAction,
    tab: PreflightTab,
    sandbox_info: &mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_loaded: &mut bool,
) {
    if matches!(action, PreflightAction::Remove) {
        handle_remove_action(items, sandbox_loaded);
        return;
    }

    if !matches!(action, PreflightAction::Install) || !matches!(tab, PreflightTab::Sandbox) {
        return;
    }

    // Show all packages, but only analyze AUR packages
    let aur_items: Vec<_> = items
        .iter()
        .filter(|p| matches!(p.source, crate::state::Source::Aur))
        .collect();

    tracing::debug!(
        "[UI] sync_sandbox: items={}, aur_items={}, cache_size={}, modal_size={}, loaded={}, resolving={}/{}",
        items.len(),
        aur_items.len(),
        app.install_list_sandbox.len(),
        sandbox_info.len(),
        *sandbox_loaded,
        app.preflight_sandbox_resolving,
        app.sandbox_resolving
    );

    // Check if we have cached sandbox info from app state that matches current items
    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();
    let cached_sandbox: Vec<_> = app
        .install_list_sandbox
        .iter()
        .filter(|s| item_names.contains(&s.package_name))
        .cloned()
        .collect();

    // Sync results from background resolution if available
    if !cached_sandbox.is_empty() {
        sync_cached_sandbox(sandbox_info, sandbox_loaded, cached_sandbox);
    }

    // If sandbox_info is empty and we haven't loaded yet, check cache or trigger resolution
    if sandbox_info.is_empty() && !*sandbox_loaded && !is_sandbox_resolving(app) {
        handle_empty_sandbox_info(app, items, &aur_items, sandbox_loaded);
    }

    // Also check if we have sandbox_info already populated (from previous sync or initial load)
    // This ensures we show data even if cached_sandbox is empty but sandbox_info has data
    // But don't mark as loaded if resolution is still in progress
    if !sandbox_info.is_empty() && !*sandbox_loaded && !is_sandbox_resolving(app) {
        tracing::info!(
            "[UI] sync_sandbox: sandbox_info has {} entries, marking as loaded",
            sandbox_info.len()
        );
        *sandbox_loaded = true;
    }
}
