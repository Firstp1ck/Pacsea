//! Modal management functions for Preflight modal.

use crate::state::{AppState, PackageItem};

/// Parameters for handling deps tab switch.
struct DepsTabParams<'a> {
    dependency_info: &'a mut Vec<crate::state::modal::DependencyInfo>,
    dep_selected: &'a mut usize,
    install_list_deps: &'a [crate::state::modal::DependencyInfo],
    preflight_deps_resolving: bool,
    preflight_deps_items: &'a mut Option<Vec<PackageItem>>,
    remove_preflight_summary_cleared: &'a mut bool,
}

/// Parameters for handling services tab switch.
struct ServicesTabParams<'a> {
    service_info: &'a mut Vec<crate::state::modal::ServiceImpact>,
    service_selected: &'a mut usize,
    services_loaded: &'a mut bool,
    install_list_services: &'a [crate::state::modal::ServiceImpact],
    services_cache_path: &'a std::path::PathBuf,
    services_resolving: bool,
}

/// Parameters for handling sandbox tab switch.
struct SandboxTabParams<'a> {
    sandbox_info: &'a mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_selected: &'a mut usize,
    sandbox_loaded: &'a mut bool,
    install_list_sandbox: &'a [crate::logic::sandbox::SandboxInfo],
    preflight_sandbox_items: &'a mut Option<Vec<PackageItem>>,
    preflight_sandbox_resolving: &'a mut bool,
}

/// What: Close the Preflight modal and clean up all related state.
///
/// Inputs:
/// - `app`: Mutable application state containing the Preflight modal
/// - `service_info`: Service info to save before closing
///
/// Output:
/// - None (mutates app state directly).
///
/// Details:
/// - Cancels in-flight operations, clears queues, and saves service restart decisions.
pub(super) fn close_preflight_modal(
    app: &mut AppState,
    service_info: &[crate::state::modal::ServiceImpact],
) {
    if service_info.is_empty() {
        // No services to plan
    } else {
        app.pending_service_plan = service_info.to_vec();
    }
    app.previous_modal = None;
    app.remove_preflight_summary.clear();
    app.preflight_cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    app.preflight_summary_items = None;
    app.preflight_deps_items = None;
    app.preflight_files_items = None;
    app.preflight_services_items = None;
    app.preflight_sandbox_items = None;
    app.modal = crate::state::Modal::None;
}

/// What: Handle switching to Deps tab and load cached dependencies.
///
/// Inputs:
/// - `items`: Packages in the transaction
/// - `action`: Install or remove action
/// - `params`: Parameters struct containing mutable state and cache references
///
/// Output:
/// - Returns true if background resolution was triggered, false otherwise.
///
/// Details:
/// - Loads cached dependencies if available, otherwise triggers background resolution.
fn handle_deps_tab_switch(
    items: &[PackageItem],
    action: &crate::state::PreflightAction,
    params: &mut DepsTabParams<'_>,
) -> bool {
    tracing::debug!(
        "[Preflight] switch_preflight_tab: Deps tab - dependency_info.len()={}, cache.len()={}, resolving={}",
        params.dependency_info.len(),
        params.install_list_deps.len(),
        params.preflight_deps_resolving
    );

    if params.dependency_info.is_empty() {
        match action {
            crate::state::PreflightAction::Install => {
                let item_names: std::collections::HashSet<String> =
                    items.iter().map(|i| i.name.clone()).collect();
                let cached_deps: Vec<crate::state::modal::DependencyInfo> = params
                    .install_list_deps
                    .iter()
                    .filter(|dep| {
                        dep.required_by
                            .iter()
                            .any(|req_by| item_names.contains(req_by))
                    })
                    .cloned()
                    .collect();
                tracing::info!(
                    "[Preflight] switch_preflight_tab: Deps - Found {} cached deps (filtered from {} total), items={:?}",
                    cached_deps.len(),
                    params.install_list_deps.len(),
                    item_names
                );
                if cached_deps.is_empty() {
                    tracing::debug!(
                        "[Preflight] Triggering background dependency resolution for {} packages",
                        items.len()
                    );
                    *params.preflight_deps_items = Some(items.to_vec());
                    *params.remove_preflight_summary_cleared = true;
                    return true;
                }
                *params.dependency_info = cached_deps;
                *params.dep_selected = 0;
                tracing::info!(
                    "[Preflight] switch_preflight_tab: Deps - Loaded {} deps into modal, dep_selected=0",
                    params.dependency_info.len()
                );
                *params.remove_preflight_summary_cleared = true;
            }
            crate::state::PreflightAction::Remove => {
                // For remove action, reverse deps are computed on-demand
            }
        }
    } else {
        tracing::debug!(
            "[Preflight] switch_preflight_tab: Deps tab - dependency_info not empty ({} entries), skipping cache load",
            params.dependency_info.len()
        );
    }
    false
}

/// What: Handle switching to Files tab and load cached file information.
///
/// Inputs:
/// - `items`: Packages in the transaction
/// - `file_info`: Mutable reference to file info vector
/// - `file_selected`: Mutable reference to selected index
/// - `install_list_files`: Reference to cached files
/// - `preflight_files_items`: Mutable reference to items for resolution
/// - `preflight_files_resolving`: Mutable reference to resolving flag
///
/// Output:
/// - None (mutates state directly).
///
/// Details:
/// - Loads cached file information if available, otherwise triggers background resolution.
fn handle_files_tab_switch(
    items: &[PackageItem],
    file_info: &mut Vec<crate::state::modal::PackageFileInfo>,
    file_selected: &mut usize,
    install_list_files: &[crate::state::modal::PackageFileInfo],
    preflight_files_items: &mut Option<Vec<PackageItem>>,
    preflight_files_resolving: &mut bool,
) {
    tracing::debug!(
        "[Preflight] switch_preflight_tab: Files tab - file_info.len()={}, cache.len()={}, resolving={}",
        file_info.len(),
        install_list_files.len(),
        *preflight_files_resolving
    );

    if file_info.is_empty() {
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_files: Vec<crate::state::modal::PackageFileInfo> = install_list_files
            .iter()
            .filter(|file_info| item_names.contains(&file_info.name))
            .cloned()
            .collect();
        tracing::info!(
            "[Preflight] switch_preflight_tab: Files - Found {} cached files (filtered from {} total), items={:?}",
            cached_files.len(),
            install_list_files.len(),
            item_names
        );
        if cached_files.is_empty() {
            tracing::debug!(
                "[Preflight] Triggering background file resolution for {} packages",
                items.len()
            );
            *preflight_files_items = Some(items.to_vec());
            *preflight_files_resolving = true;
        } else {
            *file_info = cached_files;
            *file_selected = 0;
            tracing::info!(
                "[Preflight] switch_preflight_tab: Files - Loaded {} files into modal, file_selected=0",
                file_info.len()
            );
        }
    } else {
        tracing::debug!(
            "[Preflight] switch_preflight_tab: Files tab - file_info not empty ({} entries), skipping cache load",
            file_info.len()
        );
    }
}

/// What: Handle switching to Services tab and load cached service information.
///
/// Inputs:
/// - `items`: Packages in the transaction
/// - `action`: Install or remove action
/// - `params`: Parameters struct containing mutable state and cache references
///
/// Output:
/// - None (mutates state directly).
///
/// Details:
/// - Loads cached service information if available and conditions are met.
fn handle_services_tab_switch(
    items: &[PackageItem],
    action: &crate::state::PreflightAction,
    params: &mut ServicesTabParams<'_>,
) {
    if params.service_info.is_empty()
        && matches!(action, crate::state::PreflightAction::Install)
        && !params.services_resolving
    {
        let cache_exists = if items.is_empty() {
            false
        } else {
            let signature = crate::app::services_cache::compute_signature(items);
            crate::app::services_cache::load_cache(params.services_cache_path, &signature).is_some()
        };
        if cache_exists {
            if params.install_list_services.is_empty() {
                // Skip if services list is empty
            } else {
                *params.service_info = params.install_list_services.to_vec();
                *params.service_selected = 0;
                *params.services_loaded = true;
            }
        }
    }
}

/// What: Handle switching to Sandbox tab and load cached sandbox information.
///
/// Inputs:
/// - `items`: Packages in the transaction
/// - `action`: Install or remove action
/// - `params`: Parameters struct containing mutable state and cache references
///
/// Output:
/// - None (mutates state directly).
///
/// Details:
/// - Loads cached sandbox information if available, otherwise triggers background resolution.
fn handle_sandbox_tab_switch(
    items: &[PackageItem],
    action: &crate::state::PreflightAction,
    params: &mut SandboxTabParams<'_>,
) {
    if params.sandbox_info.is_empty() && !*params.sandbox_loaded {
        match action {
            crate::state::PreflightAction::Install => {
                let item_names: std::collections::HashSet<String> =
                    items.iter().map(|i| i.name.clone()).collect();
                let cached_sandbox: Vec<crate::logic::sandbox::SandboxInfo> = params
                    .install_list_sandbox
                    .iter()
                    .filter(|s| item_names.contains(&s.package_name))
                    .cloned()
                    .collect();
                if cached_sandbox.is_empty() {
                    let aur_items: Vec<_> = items
                        .iter()
                        .filter(|p| matches!(p.source, crate::state::Source::Aur))
                        .cloned()
                        .collect();
                    if aur_items.is_empty() {
                        *params.sandbox_loaded = true;
                    } else {
                        tracing::debug!(
                            "[Preflight] Triggering background sandbox resolution for {} AUR packages",
                            aur_items.len()
                        );
                        *params.preflight_sandbox_items = Some(aur_items);
                        *params.preflight_sandbox_resolving = true;
                    }
                } else {
                    *params.sandbox_info = cached_sandbox;
                    *params.sandbox_selected = 0;
                    *params.sandbox_loaded = true;
                }
            }
            crate::state::PreflightAction::Remove => {
                *params.sandbox_loaded = true;
            }
        }
    }
}

/// What: Switch to a new Preflight tab and load cached data if available.
///
/// Inputs:
/// - `new_tab`: Target tab to switch to
/// - `app`: Mutable application state
/// - `items`: Packages in the transaction
/// - `action`: Install or remove action
///
/// Output:
/// - None (mutates app.modal directly).
///
/// Details:
/// - Handles cache loading and background resolution for each tab type.
pub(super) fn switch_preflight_tab(
    new_tab: crate::state::PreflightTab,
    app: &mut AppState,
    items: &[PackageItem],
    action: &crate::state::PreflightAction,
) {
    tracing::info!(
        "[Preflight] switch_preflight_tab: Switching to {:?}, items={}, action={:?}",
        new_tab,
        items.len(),
        action
    );

    // Extract needed app fields before borrowing modal
    let install_list_deps: &[crate::state::modal::DependencyInfo] = &app.install_list_deps;
    let install_list_files: &[crate::state::modal::PackageFileInfo] = &app.install_list_files;
    let install_list_services: &[crate::state::modal::ServiceImpact] = &app.install_list_services;
    let install_list_sandbox: &[crate::logic::sandbox::SandboxInfo] = &app.install_list_sandbox;
    let services_cache_path = app.services_cache_path.clone();
    let services_resolving = app.services_resolving;
    let preflight_deps_resolving_value = app.preflight_deps_resolving;

    // Prepare mutable state that will be updated
    let mut preflight_deps_items = None;
    let mut preflight_files_items = None;
    let mut preflight_sandbox_items = None;
    let mut preflight_files_resolving = false;
    let mut preflight_sandbox_resolving = false;
    let mut remove_preflight_summary_cleared = false;

    if let crate::state::Modal::Preflight {
        tab,
        dependency_info,
        dep_selected,
        file_info,
        file_selected,
        service_info,
        service_selected,
        services_loaded,
        sandbox_info,
        sandbox_selected,
        sandbox_loaded,
        ..
    } = &mut app.modal
    {
        let old_tab = *tab;
        *tab = new_tab;
        tracing::debug!(
            "[Preflight] switch_preflight_tab: Tab field updated from {:?} to {:?}",
            old_tab,
            new_tab
        );

        match new_tab {
            crate::state::PreflightTab::Deps => {
                let mut deps_params = DepsTabParams {
                    dependency_info,
                    dep_selected,
                    install_list_deps,
                    preflight_deps_resolving: preflight_deps_resolving_value,
                    preflight_deps_items: &mut preflight_deps_items,
                    remove_preflight_summary_cleared: &mut remove_preflight_summary_cleared,
                };
                let should_trigger = handle_deps_tab_switch(items, action, &mut deps_params);
                if should_trigger {
                    preflight_deps_items = Some(items.to_vec());
                }
            }
            crate::state::PreflightTab::Files => {
                handle_files_tab_switch(
                    items,
                    file_info,
                    file_selected,
                    install_list_files,
                    &mut preflight_files_items,
                    &mut preflight_files_resolving,
                );
            }
            crate::state::PreflightTab::Services => {
                let mut services_params = ServicesTabParams {
                    service_info,
                    service_selected,
                    services_loaded,
                    install_list_services,
                    services_cache_path: &services_cache_path,
                    services_resolving,
                };
                handle_services_tab_switch(items, action, &mut services_params);
            }
            crate::state::PreflightTab::Sandbox => {
                let mut sandbox_params = SandboxTabParams {
                    sandbox_info,
                    sandbox_selected,
                    sandbox_loaded,
                    install_list_sandbox,
                    preflight_sandbox_items: &mut preflight_sandbox_items,
                    preflight_sandbox_resolving: &mut preflight_sandbox_resolving,
                };
                handle_sandbox_tab_switch(items, action, &mut sandbox_params);
            }
            crate::state::PreflightTab::Summary => {}
        }
    }

    // Apply mutations after modal borrow is released
    if let Some(items) = preflight_deps_items {
        app.preflight_deps_items = Some(items);
        app.preflight_deps_resolving = true;
    }
    if remove_preflight_summary_cleared {
        app.remove_preflight_summary.clear();
    }
    if let Some(items) = preflight_files_items {
        app.preflight_files_items = Some(items);
        app.preflight_files_resolving = preflight_files_resolving;
    }
    if let Some(items) = preflight_sandbox_items {
        app.preflight_sandbox_items = Some(items);
        app.preflight_sandbox_resolving = preflight_sandbox_resolving;
    }
}
