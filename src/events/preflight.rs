//! Preflight modal event handling.

use crossterm::event::{KeyCode, KeyEvent};
use std::collections::{HashMap, HashSet};

use crate::state::modal::ServiceRestartDecision;
use crate::state::{AppState, PackageItem};

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
pub(crate) fn compute_display_items_len(
    items: &[PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    dep_tree_expanded: &std::collections::HashSet<String>,
) -> usize {
    // Group dependencies by the packages that require them (same as UI code)
    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> = HashMap::new();
    for dep in dependency_info.iter() {
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    // Count display items: 1 header + unique deps per package (only if expanded)
    let mut count = 0;
    for pkg_name in items.iter().map(|p| &p.name) {
        if let Some(pkg_deps) = grouped.get(pkg_name) {
            count += 1; // Header
            // Count unique dependencies only if package is expanded
            if dep_tree_expanded.contains(pkg_name) {
                let mut seen_deps = HashSet::new();
                for dep in pkg_deps.iter() {
                    if seen_deps.insert(dep.name.as_str()) {
                        count += 1;
                    }
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
pub(crate) fn compute_sandbox_display_items_len(
    items: &[PackageItem],
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_tree_expanded: &std::collections::HashSet<String>,
) -> usize {
    let mut count = 0;
    for item in items.iter() {
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
/// - `file_info`: Resolved file change metadata grouped per package
/// - `file_tree_expanded`: Set of package names currently expanded in the Files tab
///
/// Output:
/// - Total list length combining headers plus visible file entries.
///
/// Details:
/// - Skips packages with no file changes.
/// - Adds one row per package header and additional rows for each file when expanded.
pub(crate) fn compute_file_display_items_len(
    file_info: &[crate::state::modal::PackageFileInfo],
    file_tree_expanded: &HashSet<String>,
) -> usize {
    let mut count = 0;
    for pkg_info in file_info.iter() {
        if pkg_info.files.is_empty() {
            continue;
        }
        count += 1; // Package header
        if file_tree_expanded.contains(&pkg_info.name) {
            count += pkg_info.files.len();
        }
    }
    count
}

/// What: Build the flattened `(is_header, label)` list shown by the Files tab renderer.
///
/// Inputs:
/// - `file_info`: Resolved file change metadata grouped by package
/// - `file_tree_expanded`: Set of package names that should expand to show individual files
///
/// Output:
/// - Vector of `(bool, String)` pairs distinguishing headers (`true`) from file rows (`false`).
///
/// Details:
/// - Omits packages with zero file changes completely.
/// - Uses empty strings for file rows because UI draws file details from separate collections.
pub(crate) fn build_file_display_items(
    file_info: &[crate::state::modal::PackageFileInfo],
    file_tree_expanded: &HashSet<String>,
) -> Vec<(bool, String)> {
    let mut display_items: Vec<(bool, String)> = Vec::new();
    for pkg_info in file_info.iter() {
        if pkg_info.files.is_empty() {
            continue;
        }
        display_items.push((true, pkg_info.name.clone()));
        if file_tree_expanded.contains(&pkg_info.name) {
            display_items.extend(pkg_info.files.iter().map(|_| (false, String::new())));
        }
    }
    display_items
}

/// What: Handle key events while the Preflight modal is active (install/remove workflows).
///
/// Inputs:
/// - `ke`: Key event received from crossterm while Preflight is focused
/// - `app`: Mutable application state containing the Preflight modal data
///
/// Output:
/// - Always returns `false` so the outer event loop continues processing.
///
/// Details:
/// - Supports tab switching, tree expansion, dependency/file navigation, scans, dry-run toggles, and
///   command execution across install/remove flows.
/// - Mutates `app.modal` (and related cached fields) to close the modal, open nested dialogs, or
///   keep it updated with resolved dependency/file data.
/// - Returns `false` so callers continue processing, matching existing event-loop expectations.
pub(crate) fn handle_preflight_key(ke: KeyEvent, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        tab,
        items,
        action,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        deps_error,
        file_info,
        file_selected,
        file_tree_expanded,
        files_error,
        service_info,
        service_selected,
        services_loaded,
        services_error,
        sandbox_info,
        sandbox_selected,
        sandbox_tree_expanded,
        sandbox_loaded: _,
        sandbox_error: _,
        selected_optdepends,
        cascade_mode,
        ..
    } = &mut app.modal
    {
        match ke.code {
            KeyCode::Esc => {
                app.previous_modal = None; // Clear previous modal when closing Preflight
                app.remove_preflight_summary.clear();
                app.modal = crate::state::Modal::None;
            }
            KeyCode::Enter => {
                // In Deps tab, Enter toggles expand/collapse; in Files tab, Enter toggles expand/collapse; otherwise closes modal
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    // Find which package header is selected
                    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                        HashMap::new();
                    for dep in dependency_info.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    let mut display_items: Vec<(bool, String)> = Vec::new();
                    for pkg_name in items.iter().map(|p| &p.name) {
                        if grouped.contains_key(pkg_name) {
                            display_items.push((true, pkg_name.clone()));
                            if dep_tree_expanded.contains(pkg_name) {
                                // Add placeholder entries for dependencies (we just need to count them)
                                let mut seen_deps = HashSet::new();
                                if let Some(pkg_deps) = grouped.get(pkg_name) {
                                    for dep in pkg_deps.iter() {
                                        if seen_deps.insert(dep.name.as_str()) {
                                            display_items.push((false, String::new()));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some((is_header, pkg_name)) = display_items.get(*dep_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if dep_tree_expanded.contains(pkg_name) {
                            dep_tree_expanded.remove(pkg_name);
                        } else {
                            dep_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    let display_items = build_file_display_items(file_info, file_tree_expanded);

                    if let Some((is_header, pkg_name)) = display_items.get(*file_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if file_tree_expanded.contains(pkg_name) {
                            file_tree_expanded.remove(pkg_name);
                        } else {
                            file_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Sandbox && !items.is_empty() {
                    // Build display items list: (is_header, package_name, Option<(dep_type, dep_name)>)
                    type SandboxDisplayItem = (bool, String, Option<(&'static str, String)>);
                    let mut display_items: Vec<SandboxDisplayItem> = Vec::new();
                    for item in items.iter() {
                        let is_aur = matches!(item.source, crate::state::Source::Aur);
                        display_items.push((true, item.name.clone(), None));
                        if is_aur
                            && sandbox_tree_expanded.contains(&item.name)
                            && let Some(info) =
                                sandbox_info.iter().find(|s| s.package_name == item.name)
                        {
                            // Add dependencies with their types
                            for dep in &info.depends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("depends", dep.name.clone())),
                                ));
                            }
                            for dep in &info.makedepends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("makedepends", dep.name.clone())),
                                ));
                            }
                            for dep in &info.checkdepends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("checkdepends", dep.name.clone())),
                                ));
                            }
                            for dep in &info.optdepends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("optdepends", dep.name.clone())),
                                ));
                            }
                        }
                    }

                    if let Some((is_header, pkg_name, dep_opt)) =
                        display_items.get(*sandbox_selected)
                    {
                        if *is_header {
                            // Toggle this package's expanded state (only for AUR packages)
                            let item = items.iter().find(|p| p.name == *pkg_name).unwrap();
                            if matches!(item.source, crate::state::Source::Aur) {
                                if sandbox_tree_expanded.contains(pkg_name) {
                                    sandbox_tree_expanded.remove(pkg_name);
                                } else {
                                    sandbox_tree_expanded.insert(pkg_name.clone());
                                }
                            }
                        } else if let Some((dep_type, dep_name)) = dep_opt {
                            // This is a dependency item
                            if *dep_type == "optdepends" {
                                // Toggle optional dependency selection
                                let selected_set = selected_optdepends
                                    .entry(pkg_name.clone())
                                    .or_insert_with(std::collections::HashSet::new);
                                // Extract package name from dependency spec (may include version or description)
                                let pkg_name_from_dep =
                                    crate::logic::sandbox::extract_package_name(dep_name);
                                if selected_set.contains(dep_name)
                                    || selected_set.contains(&pkg_name_from_dep)
                                {
                                    // Remove both possible formats
                                    selected_set.remove(dep_name);
                                    selected_set.remove(&pkg_name_from_dep);
                                } else {
                                    // Add the dependency spec as-is (preserves version requirements)
                                    selected_set.insert(dep_name.clone());
                                }
                            }
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Services && !service_info.is_empty() {
                    if *service_selected >= service_info.len() {
                        *service_selected = service_info.len().saturating_sub(1);
                    }
                    if let Some(service) = service_info.get_mut(*service_selected) {
                        service.restart_decision = match service.restart_decision {
                            ServiceRestartDecision::Restart => ServiceRestartDecision::Defer,
                            ServiceRestartDecision::Defer => ServiceRestartDecision::Restart,
                        };
                    }
                } else {
                    // Close modal on Enter when not in Deps/Files/Sandbox tab or no data
                    // Save current service restart decisions before closing
                    if !service_info.is_empty() {
                        app.pending_service_plan = service_info.clone();
                    }
                    app.previous_modal = None;
                    app.remove_preflight_summary.clear();
                    app.modal = crate::state::Modal::None;
                }
            }
            KeyCode::Left => {
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Summary,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Services,
                };
                // Check for cached dependencies when switching to Deps tab
                // Don't resolve automatically - use cache or show placeholder
                if *tab == crate::state::PreflightTab::Deps && dependency_info.is_empty() {
                    match *action {
                        crate::state::PreflightAction::Install => {
                            // Try to use cached dependencies from app state
                            let item_names: std::collections::HashSet<String> =
                                items.iter().map(|i| i.name.clone()).collect();
                            let cached_deps: Vec<crate::state::modal::DependencyInfo> = app
                                .install_list_deps
                                .iter()
                                .filter(|dep| {
                                    dep.required_by
                                        .iter()
                                        .any(|req_by| item_names.contains(req_by))
                                })
                                .cloned()
                                .collect();
                            if !cached_deps.is_empty() {
                                *dependency_info = cached_deps;
                                *dep_selected = 0;
                            }
                            // If no cached deps, user can press 'r' to resolve
                            app.remove_preflight_summary.clear();
                        }
                        crate::state::PreflightAction::Remove => {
                            // For remove action, reverse deps are computed on-demand
                            // User can press 'r' to resolve if needed
                        }
                    }
                }
                // Check for cached files when switching to Files tab
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    // Try to use cached files from app state
                    let item_names: std::collections::HashSet<String> =
                        items.iter().map(|i| i.name.clone()).collect();
                    let cached_files: Vec<crate::state::modal::PackageFileInfo> = app
                        .install_list_files
                        .iter()
                        .filter(|file_info| item_names.contains(&file_info.name))
                        .cloned()
                        .collect();
                    if !cached_files.is_empty() {
                        *file_info = cached_files;
                        *file_selected = 0;
                    }
                    // If no cached files, user can press 'r' to resolve
                }
                // Services tab resolution happens in render function for better responsiveness
            }
            KeyCode::Right => {
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                };
                // Check for cached dependencies when switching to Deps tab
                // Don't resolve automatically - use cache or show placeholder
                if *tab == crate::state::PreflightTab::Deps && dependency_info.is_empty() {
                    match *action {
                        crate::state::PreflightAction::Install => {
                            // Try to use cached dependencies from app state
                            let item_names: std::collections::HashSet<String> =
                                items.iter().map(|i| i.name.clone()).collect();
                            let cached_deps: Vec<crate::state::modal::DependencyInfo> = app
                                .install_list_deps
                                .iter()
                                .filter(|dep| {
                                    dep.required_by
                                        .iter()
                                        .any(|req_by| item_names.contains(req_by))
                                })
                                .cloned()
                                .collect();
                            if !cached_deps.is_empty() {
                                tracing::debug!(
                                    "[Preflight] Using {} cached dependencies when switching to Deps tab",
                                    cached_deps.len()
                                );
                                *dependency_info = cached_deps;
                                *dep_selected = 0;
                            } else {
                                tracing::debug!("[Preflight] No cached dependencies available, user can press 'r' to resolve");
                            }
                            // If no cached deps, user can press 'r' to resolve
                            app.remove_preflight_summary.clear();
                        }
                        crate::state::PreflightAction::Remove => {
                            // For remove action, reverse deps are computed on-demand
                            // User can press 'r' to resolve if needed
                        }
                    }
                }
                // Check for cached files when switching to Files tab
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    // Try to use cached files from app state
                    let item_names: std::collections::HashSet<String> =
                        items.iter().map(|i| i.name.clone()).collect();
                    let cached_files: Vec<crate::state::modal::PackageFileInfo> = app
                        .install_list_files
                        .iter()
                        .filter(|file_info| item_names.contains(&file_info.name))
                        .cloned()
                        .collect();
                    if !cached_files.is_empty() {
                        *file_info = cached_files;
                        *file_selected = 0;
                    }
                    // If no cached files, user can press 'r' to resolve
                }
                // Check for cached services when switching to Services tab
                if *tab == crate::state::PreflightTab::Services && service_info.is_empty() {
                    // Try to use cached services from app state (for install actions)
                    if matches!(*action, crate::state::PreflightAction::Install) && !app.services_resolving {
                        // Check if cache file exists with matching signature
                        let cache_exists = if !items.is_empty() {
                            let signature = crate::app::services_cache::compute_signature(items);
                            crate::app::services_cache::load_cache(&app.services_cache_path, &signature)
                                .is_some()
                        } else {
                            false
                        };
                        if cache_exists && !app.install_list_services.is_empty() {
                            *service_info = app.install_list_services.clone();
                            *service_selected = 0;
                            *services_loaded = true;
                        }
                    }
                    // If no cached services, user can press 'r' to resolve
                }
            }
            KeyCode::Tab => {
                // Cycle forward through tabs (same as Right)
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                };
                // Check for cached dependencies when switching to Deps tab
                // Don't resolve automatically - use cache or show placeholder
                if *tab == crate::state::PreflightTab::Deps && dependency_info.is_empty() {
                    match *action {
                        crate::state::PreflightAction::Install => {
                            // Try to use cached dependencies from app state
                            let item_names: std::collections::HashSet<String> =
                                items.iter().map(|i| i.name.clone()).collect();
                            let cached_deps: Vec<crate::state::modal::DependencyInfo> = app
                                .install_list_deps
                                .iter()
                                .filter(|dep| {
                                    dep.required_by
                                        .iter()
                                        .any(|req_by| item_names.contains(req_by))
                                })
                                .cloned()
                                .collect();
                            if !cached_deps.is_empty() {
                                *dependency_info = cached_deps;
                                *dep_selected = 0;
                            }
                            // If no cached deps, user can press 'r' to resolve
                            app.remove_preflight_summary.clear();
                        }
                        crate::state::PreflightAction::Remove => {
                            // For remove action, reverse deps are computed on-demand
                            // User can press 'r' to resolve if needed
                        }
                    }
                }
                // Check for cached files when switching to Files tab
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    // Try to use cached files from app state
                    let item_names: std::collections::HashSet<String> =
                        items.iter().map(|i| i.name.clone()).collect();
                    let cached_files: Vec<crate::state::modal::PackageFileInfo> = app
                        .install_list_files
                        .iter()
                        .filter(|file_info| item_names.contains(&file_info.name))
                        .cloned()
                        .collect();
                    if !cached_files.is_empty() {
                        *file_info = cached_files;
                        *file_selected = 0;
                    }
                    // If no cached files, user can press 'r' to resolve
                }
                // Check for cached services when switching to Services tab
                if *tab == crate::state::PreflightTab::Services && service_info.is_empty() {
                    // Try to use cached services from app state (for install actions)
                    if matches!(*action, crate::state::PreflightAction::Install) && !app.services_resolving {
                        // Check if cache file exists with matching signature
                        let cache_exists = if !items.is_empty() {
                            let signature = crate::app::services_cache::compute_signature(items);
                            crate::app::services_cache::load_cache(&app.services_cache_path, &signature)
                                .is_some()
                        } else {
                            false
                        };
                        if cache_exists && !app.install_list_services.is_empty() {
                            *service_info = app.install_list_services.clone();
                            *service_selected = 0;
                            *services_loaded = true;
                        }
                    }
                    // If no cached services, user can press 'r' to resolve
                }
            }
            KeyCode::Up => {
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    if *dep_selected > 0 {
                        *dep_selected -= 1;
                    }
                } else if *tab == crate::state::PreflightTab::Files
                    && !file_info.is_empty()
                    && *file_selected > 0
                {
                    *file_selected -= 1;
                } else if *tab == crate::state::PreflightTab::Services
                    && !service_info.is_empty()
                    && *service_selected > 0
                {
                    *service_selected -= 1;
                } else if *tab == crate::state::PreflightTab::Sandbox
                    && !items.is_empty()
                    && *sandbox_selected > 0
                {
                    *sandbox_selected -= 1;
                }
            }
            KeyCode::Down => {
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    let display_len =
                        compute_display_items_len(items, dependency_info, dep_tree_expanded);
                    if *dep_selected < display_len.saturating_sub(1) {
                        *dep_selected += 1;
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    let display_len = compute_file_display_items_len(file_info, file_tree_expanded);
                    if *file_selected < display_len.saturating_sub(1) {
                        *file_selected += 1;
                    }
                } else if *tab == crate::state::PreflightTab::Services && !service_info.is_empty() {
                    let max_index = service_info.len().saturating_sub(1);
                    if *service_selected < max_index {
                        *service_selected += 1;
                    }
                } else if *tab == crate::state::PreflightTab::Sandbox && !items.is_empty() {
                    let display_len = compute_sandbox_display_items_len(
                        items,
                        sandbox_info,
                        sandbox_tree_expanded,
                    );
                    if *sandbox_selected < display_len.saturating_sub(1) {
                        *sandbox_selected += 1;
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Toggle expand/collapse for selected package group (Space key)
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    // Find which package header is selected
                    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                        HashMap::new();
                    for dep in dependency_info.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    let mut display_items: Vec<(bool, String)> = Vec::new();
                    for pkg_name in items.iter().map(|p| &p.name) {
                        if grouped.contains_key(pkg_name) {
                            display_items.push((true, pkg_name.clone()));
                            if dep_tree_expanded.contains(pkg_name) {
                                // Add placeholder entries for dependencies (we just need to count them)
                                let mut seen_deps = HashSet::new();
                                if let Some(pkg_deps) = grouped.get(pkg_name) {
                                    for dep in pkg_deps.iter() {
                                        if seen_deps.insert(dep.name.as_str()) {
                                            display_items.push((false, String::new()));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some((is_header, pkg_name)) = display_items.get(*dep_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if dep_tree_expanded.contains(pkg_name) {
                            dep_tree_expanded.remove(pkg_name);
                        } else {
                            dep_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    let display_items = build_file_display_items(file_info, file_tree_expanded);

                    if let Some((is_header, pkg_name)) = display_items.get(*file_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if file_tree_expanded.contains(pkg_name) {
                            file_tree_expanded.remove(pkg_name);
                        } else {
                            file_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Sandbox && !items.is_empty() {
                    // Build display items list: (is_header, package_name, Option<(dep_type, dep_name)>)
                    type SandboxDisplayItem = (bool, String, Option<(&'static str, String)>);
                    let mut display_items: Vec<SandboxDisplayItem> = Vec::new();
                    for item in items.iter() {
                        let is_aur = matches!(item.source, crate::state::Source::Aur);
                        display_items.push((true, item.name.clone(), None));
                        if is_aur
                            && sandbox_tree_expanded.contains(&item.name)
                            && let Some(info) =
                                sandbox_info.iter().find(|s| s.package_name == item.name)
                        {
                            // Add dependencies with their types
                            for dep in &info.depends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("depends", dep.name.clone())),
                                ));
                            }
                            for dep in &info.makedepends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("makedepends", dep.name.clone())),
                                ));
                            }
                            for dep in &info.checkdepends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("checkdepends", dep.name.clone())),
                                ));
                            }
                            for dep in &info.optdepends {
                                display_items.push((
                                    false,
                                    item.name.clone(),
                                    Some(("optdepends", dep.name.clone())),
                                ));
                            }
                        }
                    }

                    if let Some((is_header, pkg_name, dep_opt)) =
                        display_items.get(*sandbox_selected)
                    {
                        if *is_header {
                            // Toggle this package's expanded state (only for AUR packages)
                            let item = items.iter().find(|p| p.name == *pkg_name).unwrap();
                            if matches!(item.source, crate::state::Source::Aur) {
                                if sandbox_tree_expanded.contains(pkg_name) {
                                    sandbox_tree_expanded.remove(pkg_name);
                                } else {
                                    sandbox_tree_expanded.insert(pkg_name.clone());
                                }
                            }
                        } else if let Some((dep_type, dep_name)) = dep_opt {
                            // This is a dependency item
                            if *dep_type == "optdepends" {
                                // Toggle optional dependency selection
                                let selected_set = selected_optdepends
                                    .entry(pkg_name.clone())
                                    .or_insert_with(std::collections::HashSet::new);
                                // Extract package name from dependency spec (may include version or description)
                                let pkg_name_from_dep =
                                    crate::logic::sandbox::extract_package_name(dep_name);
                                if selected_set.contains(dep_name)
                                    || selected_set.contains(&pkg_name_from_dep)
                                {
                                    // Remove both possible formats
                                    selected_set.remove(dep_name);
                                    selected_set.remove(&pkg_name_from_dep);
                                } else {
                                    // Add the dependency spec as-is (preserves version requirements)
                                    selected_set.insert(dep_name.clone());
                                }
                            }
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Services && !service_info.is_empty() {
                    if *service_selected >= service_info.len() {
                        *service_selected = service_info.len().saturating_sub(1);
                    }
                    if let Some(service) = service_info.get_mut(*service_selected) {
                        service.restart_decision = match service.restart_decision {
                            ServiceRestartDecision::Restart => ServiceRestartDecision::Defer,
                            ServiceRestartDecision::Defer => ServiceRestartDecision::Restart,
                        };
                    }
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Retry resolution for current tab or toggle service restart decision
                if *tab == crate::state::PreflightTab::Services && !service_info.is_empty() {
                    // Toggle restart decision for selected service (only if no error)
                    if *service_selected >= service_info.len() {
                        *service_selected = service_info.len().saturating_sub(1);
                    }
                    if let Some(service) = service_info.get_mut(*service_selected) {
                        service.restart_decision = ServiceRestartDecision::Restart;
                    }
                } else if *tab == crate::state::PreflightTab::Deps
                    && matches!(*action, crate::state::PreflightAction::Install)
                {
                    // Retry dependency resolution
                    *deps_error = None;
                    *dependency_info = crate::logic::deps::resolve_dependencies(items);
                    *dep_selected = 0;
                } else if *tab == crate::state::PreflightTab::Files {
                    // Retry file resolution
                    *files_error = None;
                    *file_info = crate::logic::files::resolve_file_changes(items, *action);
                    *file_selected = 0;
                } else if *tab == crate::state::PreflightTab::Services {
                    // Retry service resolution
                    *services_error = None;
                    *services_loaded = false;
                    *service_info = crate::logic::services::resolve_service_impacts(items, *action);
                    *service_selected = 0;
                    *services_loaded = true;
                }
            }
            KeyCode::Char('D') => {
                if *tab == crate::state::PreflightTab::Services && !service_info.is_empty() {
                    if *service_selected >= service_info.len() {
                        *service_selected = service_info.len().saturating_sub(1);
                    }
                    if let Some(service) = service_info.get_mut(*service_selected) {
                        service.restart_decision = ServiceRestartDecision::Defer;
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // Expand/collapse all package groups
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                        HashMap::new();
                    for dep in dependency_info.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    let all_expanded = items.iter().all(|p| dep_tree_expanded.contains(&p.name));
                    if all_expanded {
                        // Collapse all
                        dep_tree_expanded.clear();
                    } else {
                        // Expand all
                        for pkg_name in items.iter().map(|p| &p.name) {
                            if grouped.contains_key(pkg_name) {
                                dep_tree_expanded.insert(pkg_name.clone());
                            }
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    // Expand/collapse all packages in Files tab
                    let all_expanded = file_info
                        .iter()
                        .filter(|p| !p.files.is_empty())
                        .all(|p| file_tree_expanded.contains(&p.name));
                    if all_expanded {
                        // Collapse all
                        file_tree_expanded.clear();
                    } else {
                        // Expand all
                        for pkg_info in file_info.iter() {
                            if !pkg_info.files.is_empty() {
                                file_tree_expanded.insert(pkg_info.name.clone());
                            }
                        }
                    }
                }
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                // File database sync (Files tab only)
                if *tab == crate::state::PreflightTab::Files {
                    // Use the new ensure_file_db_synced function with force=true
                    // This will attempt to sync regardless of timestamp
                    let sync_result = crate::logic::files::ensure_file_db_synced(true, 7);
                    match sync_result {
                        Ok(synced) => {
                            if synced {
                                app.toast_message = Some(
                                    "File database sync completed. Files tab will refresh."
                                        .to_string(),
                                );
                            } else {
                                app.toast_message =
                                    Some("File database is already fresh.".to_string());
                            }
                            app.toast_expires_at =
                                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                            // Clear file_info to trigger re-resolution after sync completes
                            *file_info = Vec::new();
                            *file_selected = 0;
                        }
                        Err(e) => {
                            // Sync failed (likely requires root), launch terminal with sudo
                            let sync_cmd = "sudo pacman -Fy".to_string();
                            let cmds = vec![sync_cmd];
                            std::thread::spawn(move || {
                                crate::install::spawn_shell_commands_in_terminal(&cmds);
                            });
                            app.toast_message = Some(format!(
                                "File database sync started in terminal (requires root). Error: {}",
                                e
                            ));
                            app.toast_expires_at =
                                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                            // Clear file_info to trigger re-resolution after sync completes
                            *file_info = Vec::new();
                            *file_selected = 0;
                        }
                    }
                    return false;
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                // Build AUR package name list to scan
                let mut names: Vec<String> = Vec::new();
                for it in items.iter() {
                    if matches!(it.source, crate::state::Source::Aur) {
                        names.push(it.name.clone());
                    }
                }
                if names.is_empty() {
                    app.modal = crate::state::Modal::Alert {
                        message: "No AUR packages selected to scan.\nAdd AUR packages to scan, then press 's'.".into(),
                    };
                } else {
                    app.pending_install_names = Some(names);
                    // Open Scan Configuration modal initialized from settings.conf
                    let prefs = crate::theme::settings();
                    // Store current Preflight modal state before opening ScanConfig
                    app.previous_modal = Some(app.modal.clone());
                    app.modal = crate::state::Modal::ScanConfig {
                        do_clamav: prefs.scan_do_clamav,
                        do_trivy: prefs.scan_do_trivy,
                        do_semgrep: prefs.scan_do_semgrep,
                        do_shellcheck: prefs.scan_do_shellcheck,
                        do_virustotal: prefs.scan_do_virustotal,
                        do_custom: prefs.scan_do_custom,
                        do_sleuth: prefs.scan_do_sleuth,
                        cursor: 0,
                    };
                }
            }
            KeyCode::Char('d') => {
                // toggle dry-run globally pre-apply
                app.dry_run = !app.dry_run;
                app.toast_message = Some(format!(
                    "Dry-run: {}",
                    if app.dry_run { "ON" } else { "OFF" }
                ));
            }
            KeyCode::Char('m') => {
                if matches!(*action, crate::state::PreflightAction::Remove) {
                    let next_mode = cascade_mode.next();
                    *cascade_mode = next_mode;
                    app.remove_cascade_mode = next_mode;
                    app.toast_message = Some(format!(
                        "Cascade mode set to {} ({})",
                        next_mode.flag(),
                        next_mode.description()
                    ));
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
                }
            }
            KeyCode::Char('p') => {
                let mut close_modal = false;
                let mut new_summary: Option<Vec<crate::state::modal::ReverseRootSummary>> = None;
                let mut blocked_dep_count: Option<usize> = None;
                let mut removal_names: Option<Vec<String>> = None;
                let mut removal_mode: Option<crate::state::modal::CascadeMode> = None;
                let mut install_targets: Option<Vec<PackageItem>> = None;

                match *action {
                    crate::state::PreflightAction::Install => {
                        install_targets = Some(items.clone());
                    }
                    crate::state::PreflightAction::Remove => {
                        if dependency_info.is_empty() {
                            let report = crate::logic::deps::resolve_reverse_dependencies(items);
                            new_summary = Some(report.summaries);
                            *dependency_info = report.dependencies;
                        }

                        if dependency_info.is_empty() || cascade_mode.allows_dependents() {
                            removal_names = Some(items.iter().map(|p| p.name.clone()).collect());
                            removal_mode = Some(*cascade_mode);
                        } else {
                            blocked_dep_count = Some(dependency_info.len());
                        }
                    }
                }

                if let Some(summary) = new_summary {
                    app.remove_preflight_summary = summary;
                }

                if !service_info.is_empty() {
                    app.pending_service_plan = service_info.clone();
                } else {
                    app.pending_service_plan.clear();
                }

                if let Some(mut packages) = install_targets {
                    // Add selected optional dependencies as additional packages to install
                    for (_pkg_name, optdeps) in selected_optdepends.iter() {
                        for optdep in optdeps {
                            // Extract package name from dependency spec (may include version or description)
                            let optdep_pkg_name =
                                crate::logic::sandbox::extract_package_name(optdep);
                            // Check if this optional dependency is not already in the install list
                            if !packages.iter().any(|p| p.name == optdep_pkg_name) {
                                // Create a PackageItem for the optional dependency
                                // We don't know the source, so we'll let pacman/paru figure it out
                                packages.push(PackageItem {
                                    name: optdep_pkg_name,
                                    version: String::new(),
                                    description: String::new(),
                                    source: crate::state::Source::Official {
                                        repo: String::new(),
                                        arch: String::new(),
                                    },
                                    popularity: None,
                                });
                            }
                        }
                    }
                    crate::install::spawn_install_all(&packages, app.dry_run);
                    close_modal = true;
                } else if let Some(names) = removal_names {
                    let mode = removal_mode.unwrap_or(*cascade_mode);
                    crate::install::spawn_remove_all(&names, app.dry_run, mode);
                    close_modal = true;
                } else if let Some(count) = blocked_dep_count {
                    let root_list: Vec<String> = app
                        .remove_preflight_summary
                        .iter()
                        .filter(|summary| summary.total_dependents > 0)
                        .map(|summary| summary.package.clone())
                        .collect();
                    let subject = if root_list.is_empty() {
                        "the selected packages".to_string()
                    } else {
                        root_list.join(", ")
                    };
                    app.toast_message = Some(format!(
                        "Removal blocked: {count} dependent package(s) rely on {subject}. Enable cascade removal to proceed."
                    ));
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(6));
                }

                if close_modal {
                    app.previous_modal = None;
                    app.modal = crate::state::Modal::None;
                }
            }
            KeyCode::Char('c') => {
                // Snapshot placeholder
                app.toast_message = Some("Snapshot (placeholder)".to_string());
            }
            KeyCode::Char('q') => {
                // Save current service restart decisions before closing
                if !service_info.is_empty() {
                    app.pending_service_plan = service_info.clone();
                }
                app.previous_modal = None; // Clear previous modal when closing Preflight
                app.remove_preflight_summary.clear();
                app.modal = crate::state::Modal::None;
            }
            KeyCode::Char('?') => {
                // Show Deps tab help when on Deps tab, otherwise show general Preflight help
                let help_message = if *tab == crate::state::PreflightTab::Deps {
                    "Dependencies Tab Help\n\n\
                        This tab shows all dependencies required for the selected packages.\n\n\
                        Status Indicators:\n\
                        • ✓ (green) - Already installed\n\
                        • + (yellow) - Needs to be installed\n\
                        • ↑ (yellow) - Needs upgrade\n\
                        • ⚠ (red) - Conflict detected\n\
                        • ? (red) - Missing/unavailable\n\n\
                        Repository Badges:\n\
                        • [core] - Core repository (fundamental system packages)\n\
                        • [extra] - Extra repository\n\
                        • [AUR] - Arch User Repository\n\n\
                        Markers:\n\
                        • [CORE] (red) - Package from core repository\n\
                        • [SYSTEM] (yellow) - Critical system package\n\n\
                        Navigation:\n\
                        • Up/Down - Navigate dependency list\n\
                        • Left/Right/Tab - Switch tabs\n\
                        • Enter/Space - Expand/collapse package group\n\
                        • a - Expand/collapse all package groups\n\
                        • r - Retry dependency resolution (if error occurred)\n\
                        • ? - Show this help\n\
                        • q/Esc - Close preflight\n\n\
                        Dependencies are automatically resolved when you navigate to this tab.\n\
                        For AUR packages, dependencies are fetched from the AUR API."
                        .to_string()
                } else {
                    "Preflight Help\n\n\
                        Navigation:\n\
                        • Left/Right/Tab - Switch between tabs\n\
                        • Up/Down - Navigate lists (Deps, Files, Services tabs)\n\
                        • ? - Show help for current tab\n\
                        • q/Esc/Enter - Close preflight (Enter also toggles expansion in Deps/Files tabs)\n\n\
                        Tab-Specific Actions:\n\
                        • Deps tab:\n\
                          - Enter/Space - Expand/collapse package group\n\
                          - a - Expand/collapse all package groups\n\
                          - r - Retry dependency resolution (if error occurred)\n\
                        • Files tab:\n\
                          - Enter/Space - Expand/collapse package file list\n\
                          - a - Expand/collapse all package file lists\n\
                          - f - Sync pacman file database (pacman -Fy)\n\
                          - r - Retry file resolution (if error occurred)\n\
                        • Services tab:\n\
                          - Space - Toggle restart/defer for selected service\n\
                          - R - Mark selected service to restart\n\
                          - Shift+D - Mark selected service to defer\n\
                          - r - Retry service resolution (if error occurred)\n\n\
                        Global Actions:\n\
                        • s - Scan AUR packages (if AUR packages selected)\n\
                        • d - Toggle dry-run mode\n\
                        • m - Toggle cascade removal mode (Remove action only)\n\
                        • p - Proceed with installation/removal\n\
                        • c - Create snapshot (placeholder)\n\n\
                        Tabs:\n\
                        • Summary - Overview of packages, versions, sizes, and risk score\n\
                        • Deps - Dependency information and reverse dependencies\n\
                        • Files - File changes preview and pacnew/pacsave prediction\n\
                        • Services - Systemd service impact and restart planning\n\
                        • Sandbox - AUR build checks (placeholder)"
                        .to_string()
                };
                // Store current Preflight modal state before opening Alert
                app.previous_modal = Some(app.modal.clone());
                app.modal = crate::state::Modal::Alert {
                    message: help_message,
                };
            }
            _ => {}
        }
        return false;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::{
        CascadeMode, DependencyInfo, DependencySource, DependencyStatus, FileChange,
        FileChangeType, PackageFileInfo, ServiceImpact, ServiceRestartDecision,
    };
    use crate::state::{Modal, PackageItem, PreflightAction, PreflightTab, Source};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::collections::HashSet;

    /// What: Construct a minimal `PackageItem` fixture used by preflight tests.
    ///
    /// Inputs:
    /// - `name`: Package identifier to embed in the resulting fixture.
    ///
    /// Output:
    /// - `PackageItem` populated with deterministic metadata for assertions.
    ///
    /// Details:
    /// - Provides consistent version/description/source values so each test can focus on modal behaviour.
    fn pkg(name: &str) -> PackageItem {
        PackageItem {
            name: name.into(),
            version: "1.0.0".into(),
            description: "pkg".into(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        }
    }

    /// What: Build a `DependencyInfo` fixture describing a package edge for dependency tests.
    ///
    /// Inputs:
    /// - `name`: Dependency package name to populate the struct.
    /// - `required_by`: Slice of package names that declare the dependency.
    ///
    /// Output:
    /// - `DependencyInfo` instance configured for deterministic assertions.
    ///
    /// Details:
    /// - Sets predictable version/status/source fields so tests can concentrate on tree expansion logic.
    fn dep(name: &str, required_by: &[&str]) -> DependencyInfo {
        DependencyInfo {
            name: name.into(),
            version: ">=1".into(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "extra".into(),
            },
            required_by: required_by.iter().map(|s| (*s).into()).collect(),
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        }
    }

    /// What: Create a `PackageFileInfo` fixture with a configurable number of synthetic files.
    ///
    /// Inputs:
    /// - `name`: Package identifier associated with the file changes.
    /// - `file_count`: Number of file entries to generate.
    ///
    /// Output:
    /// - `PackageFileInfo` containing `file_count` new file records under `/tmp`.
    ///
    /// Details:
    /// - Each generated file is marked as a new change, allowing tests to validate expansion counts easily.
    fn file_info(name: &str, file_count: usize) -> PackageFileInfo {
        let mut files = Vec::new();
        for idx in 0..file_count {
            files.push(FileChange {
                path: format!("/tmp/{name}_{idx}"),
                change_type: FileChangeType::New,
                package: name.into(),
                is_config: false,
                predicted_pacnew: false,
                predicted_pacsave: false,
            });
        }
        PackageFileInfo {
            name: name.into(),
            files,
            total_count: file_count,
            new_count: file_count,
            changed_count: 0,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        }
    }

    /// What: Construct a `ServiceImpact` fixture representing a single unit for Services tab tests.
    ///
    /// Inputs:
    /// - `unit`: Fully-qualified systemd unit identifier to populate the struct.
    /// - `decision`: Initial restart preference that the test expects to mutate.
    ///
    /// Output:
    /// - `ServiceImpact` configured with deterministic metadata for assertions.
    ///
    /// Details:
    /// - Marks the unit as active and needing restart so event handlers may flip the decision.
    fn svc(unit: &str, decision: ServiceRestartDecision) -> ServiceImpact {
        ServiceImpact {
            unit_name: unit.into(),
            providers: vec!["target".into()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: decision,
        }
    }

    #[test]
    /// What: Ensure dependency display length counts unique entries when groups are expanded.
    ///
    /// Inputs:
    /// - Dependency list with duplicates and an expanded set containing the first package.
    ///
    /// Output:
    /// - Computed length includes headers plus unique dependencies, yielding four rows.
    ///
    /// Details:
    /// - Demonstrates deduplication of repeated dependency records across packages.
    fn deps_display_len_counts_unique_expanded_dependencies() {
        let items = vec![pkg("app"), pkg("tool")];
        let deps = vec![
            dep("libfoo", &["app"]),
            dep("libbar", &["app", "tool"]),
            dep("libbar", &["app"]),
        ];
        let mut expanded = HashSet::new();
        expanded.insert("app".to_string());
        let len = compute_display_items_len(&items, &deps, &expanded);
        assert_eq!(len, 4);
    }

    #[test]
    /// What: Verify the collapsed dependency view only counts package headers.
    ///
    /// Inputs:
    /// - Dependency list with multiple packages but an empty expanded set.
    ///
    /// Output:
    /// - Display length equals the number of packages (two).
    ///
    /// Details:
    /// - Confirms collapsed state omits dependency children entirely.
    fn deps_display_len_collapsed_counts_only_headers() {
        let items = vec![pkg("app"), pkg("tool")];
        let deps = vec![dep("libfoo", &["app"]), dep("libbar", &["tool"])];
        let expanded = HashSet::new();
        let len = compute_display_items_len(&items, &deps, &expanded);
        assert_eq!(len, 2);
    }

    #[test]
    /// What: Confirm file display counts add child rows only for expanded entries.
    ///
    /// Inputs:
    /// - File info for a package with two files and another with zero files.
    ///
    /// Output:
    /// - Collapsed count returns one; expanded count increases to three.
    ///
    /// Details:
    /// - Exercises the branch that toggles between header-only and expanded file listings.
    fn file_display_len_respects_expansion_state() {
        let info = vec![file_info("pkg", 2), file_info("empty", 0)];
        let mut expanded = HashSet::new();
        let collapsed = compute_file_display_items_len(&info, &expanded);
        assert_eq!(collapsed, 1);
        expanded.insert("pkg".to_string());
        let expanded_len = compute_file_display_items_len(&info, &expanded);
        assert_eq!(expanded_len, 3);
    }

    #[test]
    /// What: Ensure file display item builder yields headers plus placeholder rows when expanded.
    ///
    /// Inputs:
    /// - File info with two entries for a single package and varying expansion sets.
    ///
    /// Output:
    /// - Collapsed result contains only the header; expanded result includes header plus two child slots.
    ///
    /// Details:
    /// - Helps guarantee alignment between item construction and length calculations.
    fn build_file_items_match_expansion() {
        let info = vec![file_info("pkg", 2)];
        let collapsed = build_file_display_items(&info, &HashSet::new());
        assert_eq!(collapsed, vec![(true, "pkg".into())]);
        let mut expanded = HashSet::new();
        expanded.insert("pkg".to_string());
        let expanded_items = build_file_display_items(&info, &expanded);
        assert_eq!(
            expanded_items,
            vec![
                (true, "pkg".into()),
                (false, String::new()),
                (false, String::new())
            ]
        );
    }

    /// What: Prepare an `AppState` with a seeded Preflight modal tailored for keyboard interaction tests.
    ///
    /// Inputs:
    /// - `tab`: Initial tab to display inside the Preflight modal.
    /// - `dependency_info`: Pre-resolved dependency list to seed the modal state.
    /// - `dep_selected`: Initial selection index for the dependency list.
    /// - `dep_tree_expanded`: Set of package names that should start expanded.
    ///
    /// Output:
    /// - `AppState` instance whose `modal` field is pre-populated with consistent fixtures.
    ///
    /// Details:
    /// - Reduces duplication across tests that exercise navigation/expansion logic within the Preflight modal.
    fn setup_preflight_app(
        tab: PreflightTab,
        dependency_info: Vec<DependencyInfo>,
        dep_selected: usize,
        dep_tree_expanded: HashSet<String>,
    ) -> AppState {
        let mut app = AppState::default();
        let items = vec![pkg("target")];
        app.modal = Modal::Preflight {
            items,
            action: PreflightAction::Install,
            tab,
            summary: None,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            dependency_info,
            dep_selected,
            dep_tree_expanded,
            deps_error: None,
            file_info: Vec::new(),
            file_selected: 0,
            file_tree_expanded: HashSet::new(),
            files_error: None,
            service_info: Vec::new(),
            service_selected: 0,
            services_loaded: false,
            services_error: None,
            sandbox_info: Vec::new(),
            sandbox_selected: 0,
            sandbox_tree_expanded: std::collections::HashSet::new(),
            sandbox_loaded: false,
            sandbox_error: None,
            selected_optdepends: std::collections::HashMap::new(),
            cascade_mode: CascadeMode::Basic,
        };
        app
    }

    /// What: Build an `AppState` seeded with Services tab data for restart decision tests.
    ///
    /// Inputs:
    /// - `tab`: Initial tab to display inside the Preflight modal (expected to be Services).
    /// - `service_info`: Collection of service impacts to expose through the modal.
    /// - `service_selected`: Index that should be focused when the test begins.
    ///
    /// Output:
    /// - `AppState` populated with deterministic fixtures for Services tab interactions.
    ///
    /// Details:
    /// - Marks services as already loaded so handlers operate directly on the provided data.
    fn setup_preflight_app_with_services(
        tab: PreflightTab,
        service_info: Vec<ServiceImpact>,
        service_selected: usize,
    ) -> AppState {
        let mut app = AppState::default();
        let items = vec![pkg("target")];
        app.modal = Modal::Preflight {
            items,
            action: PreflightAction::Install,
            tab,
            summary: None,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            dependency_info: Vec::new(),
            dep_selected: 0,
            dep_tree_expanded: HashSet::new(),
            deps_error: None,
            file_info: Vec::new(),
            file_selected: 0,
            file_tree_expanded: HashSet::new(),
            files_error: None,
            service_info,
            service_selected,
            services_loaded: true,
            services_error: None,
            sandbox_info: Vec::new(),
            sandbox_selected: 0,
            sandbox_tree_expanded: std::collections::HashSet::new(),
            sandbox_loaded: false,
            sandbox_error: None,
            selected_optdepends: std::collections::HashMap::new(),
            cascade_mode: CascadeMode::Basic,
        };
        app
    }

    #[test]
    /// What: Verify `Enter` toggles dependency expansion state within the preflight modal.
    ///
    /// Inputs:
    /// - Preflight modal focused on dependencies with an initial collapsed state.
    ///
    /// Output:
    /// - First `Enter` expands the target group; second `Enter` collapses it.
    ///
    /// Details:
    /// - Uses synthetic key events to mimic user interaction without rendering.
    fn handle_enter_toggles_dependency_group() {
        let deps = vec![dep("libfoo", &["target"])];
        let mut app = setup_preflight_app(PreflightTab::Deps, deps, 0, HashSet::new());
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        handle_preflight_key(enter, &mut app);
        if let Modal::Preflight {
            dep_tree_expanded, ..
        } = &app.modal
        {
            assert!(dep_tree_expanded.contains("target"));
        } else {
            panic!("expected Preflight modal");
        }
        let enter_again = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        handle_preflight_key(enter_again, &mut app);
        if let Modal::Preflight {
            dep_tree_expanded, ..
        } = &app.modal
        {
            assert!(!dep_tree_expanded.contains("target"));
        } else {
            panic!("expected Preflight modal");
        }
    }

    #[test]
    /// What: Ensure navigation does not move past the last visible dependency row when expanded.
    ///
    /// Inputs:
    /// - Preflight modal with expanded dependencies and repeated `Down` key events.
    ///
    /// Output:
    /// - Selection stops at the final row instead of wrapping or overshooting.
    ///
    /// Details:
    /// - Exercises selection bounds checking for keyboard navigation.
    fn handle_down_stops_at_last_visible_dependency_row() {
        let deps = vec![dep("libfoo", &["target"]), dep("libbar", &["target"])];
        let mut expanded = HashSet::new();
        expanded.insert("target".to_string());
        let mut app = setup_preflight_app(PreflightTab::Deps, deps, 0, expanded);
        handle_preflight_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
            &mut app,
        );
        handle_preflight_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
            &mut app,
        );
        handle_preflight_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
            &mut app,
        );
        if let Modal::Preflight { dep_selected, .. } = &app.modal {
            assert_eq!(*dep_selected, 2);
        } else {
            panic!("expected Preflight modal");
        }
    }

    #[test]
    /// What: Confirm spacebar toggles the service restart decision within the Services tab.
    ///
    /// Inputs:
    /// - Preflight modal focused on Services with a single restart-ready unit selected.
    ///
    /// Output:
    /// - First space press defers the restart; second space press restores the restart decision.
    ///
    /// Details:
    /// - Exercises the branch that flips `ServiceRestartDecision` without mutating selection.
    fn handle_space_toggles_service_restart_decision() {
        let services = vec![svc("nginx.service", ServiceRestartDecision::Restart)];
        let mut app = setup_preflight_app_with_services(PreflightTab::Services, services, 0);
        handle_preflight_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
            &mut app,
        );
        if let Modal::Preflight { service_info, .. } = &app.modal {
            assert_eq!(
                service_info[0].restart_decision,
                ServiceRestartDecision::Defer
            );
        } else {
            panic!("expected Preflight modal");
        }
        handle_preflight_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
            &mut app,
        );
        if let Modal::Preflight { service_info, .. } = &app.modal {
            assert_eq!(
                service_info[0].restart_decision,
                ServiceRestartDecision::Restart
            );
        } else {
            panic!("expected Preflight modal");
        }
    }

    #[test]
    /// What: Ensure dedicated shortcuts force service restart decisions regardless of current state.
    ///
    /// Inputs:
    /// - Services tab with one unit initially set to defer, then the `r` and `Shift+D` bindings.
    ///
    /// Output:
    /// - `r` enforces restart, `Shift+D` enforces defer on the focused service.
    ///
    /// Details:
    /// - Verifies that direct commands override any prior toggled state for the selected row.
    fn handle_service_restart_shortcuts_force_decisions() {
        let services = vec![svc("postgresql.service", ServiceRestartDecision::Defer)];
        let mut app = setup_preflight_app_with_services(PreflightTab::Services, services, 0);
        handle_preflight_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty()),
            &mut app,
        );
        if let Modal::Preflight { service_info, .. } = &app.modal {
            assert_eq!(
                service_info[0].restart_decision,
                ServiceRestartDecision::Restart
            );
        } else {
            panic!("expected Preflight modal");
        }
        handle_preflight_key(
            KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT),
            &mut app,
        );
        if let Modal::Preflight { service_info, .. } = &app.modal {
            assert_eq!(
                service_info[0].restart_decision,
                ServiceRestartDecision::Defer
            );
        } else {
            panic!("expected Preflight modal");
        }
    }
}
