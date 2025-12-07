//! Modal management functions for Preflight modal.

use crate::state::{AppState, PackageItem};

/// Parameters for handling deps tab switch.
struct DepsTabParams<'a> {
    /// Dependency information list.
    dependency_info: &'a mut Vec<crate::state::modal::DependencyInfo>,
    /// Currently selected dependency index.
    dep_selected: &'a mut usize,
    /// Dependency info from install list.
    install_list_deps: &'a [crate::state::modal::DependencyInfo],
    /// Whether dependency resolution is in progress.
    preflight_deps_resolving: bool,
    /// Pending dependency resolution request (packages, action).
    preflight_deps_items: &'a mut Option<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Flag indicating if preflight summary was cleared for remove action.
    remove_preflight_summary_cleared: &'a mut bool,
    /// Cached reverse dependency report.
    cached_reverse_deps_report: &'a mut Option<crate::logic::deps::ReverseDependencyReport>,
}

/// Parameters for handling services tab switch.
struct ServicesTabParams<'a> {
    /// Service impact information list.
    service_info: &'a mut Vec<crate::state::modal::ServiceImpact>,
    /// Currently selected service index.
    service_selected: &'a mut usize,
    /// Whether service information has been loaded.
    services_loaded: &'a mut bool,
    /// Service info from install list.
    install_list_services: &'a [crate::state::modal::ServiceImpact],
    /// Path to services cache file.
    services_cache_path: &'a std::path::PathBuf,
    /// Whether service resolution is in progress.
    services_resolving: bool,
    /// Pending service analysis request (packages).
    preflight_services_items: &'a mut Option<Vec<PackageItem>>,
    /// Whether preflight service resolution is in progress.
    preflight_services_resolving: &'a mut bool,
}

/// Parameters for handling sandbox tab switch.
struct SandboxTabParams<'a> {
    /// Sandbox analysis information list.
    sandbox_info: &'a mut Vec<crate::logic::sandbox::SandboxInfo>,
    /// Currently selected sandbox item index.
    sandbox_selected: &'a mut usize,
    /// Whether sandbox information has been loaded.
    sandbox_loaded: &'a mut bool,
    /// Sandbox info from install list.
    install_list_sandbox: &'a [crate::logic::sandbox::SandboxInfo],
    /// Pending sandbox analysis request (packages).
    preflight_sandbox_items: &'a mut Option<Vec<PackageItem>>,
    /// Whether preflight sandbox resolution is in progress.
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
    action: crate::state::PreflightAction,
    params: &mut DepsTabParams<'_>,
) -> bool {
    tracing::debug!(
        "[Preflight] switch_preflight_tab: Deps tab - dependency_info.len()={}, cache.len()={}, resolving={}",
        params.dependency_info.len(),
        params.install_list_deps.len(),
        params.preflight_deps_resolving
    );

    tracing::info!(
        "[Preflight] handle_deps_tab_switch: dependency_info.len()={}, action={:?}, items={}",
        params.dependency_info.len(),
        action,
        items.len()
    );

    if params.dependency_info.is_empty() {
        tracing::info!(
            "[Preflight] handle_deps_tab_switch: dependency_info is empty, will trigger resolution"
        );
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
                    *params.preflight_deps_items =
                        Some((items.to_vec(), crate::state::PreflightAction::Install));
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
                // Check if we have a cached reverse dependency report from summary computation
                if let Some(report) = params.cached_reverse_deps_report.as_ref() {
                    tracing::info!(
                        "[Preflight] Using cached reverse dependency report ({} deps) from summary computation",
                        report.dependencies.len()
                    );
                    params.dependency_info.clone_from(&report.dependencies);
                    *params.dep_selected = 0;
                    // Clear the cache after using it to free memory
                    *params.cached_reverse_deps_report = None;
                    *params.remove_preflight_summary_cleared = true;
                    return false;
                }
                // No cached report available, trigger background resolution
                tracing::debug!(
                    "[Preflight] No cached report available, triggering background reverse dependency resolution for {} packages",
                    items.len()
                );
                *params.preflight_deps_items =
                    Some((items.to_vec(), crate::state::PreflightAction::Remove));
                *params.remove_preflight_summary_cleared = true;
                return true;
            }
            crate::state::PreflightAction::Downgrade => {
                // For downgrade, we don't need to resolve dependencies
                // Downgrade tool handles its own logic
                tracing::debug!("[Preflight] Downgrade action: skipping dependency resolution");
                *params.dependency_info = Vec::new();
                *params.dep_selected = 0;
                *params.remove_preflight_summary_cleared = true;
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
/// - `action`: Install or remove action
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
/// - Loads cached file information if available for Install actions, otherwise triggers background resolution.
/// - For Remove actions, always triggers fresh resolution since cached files contain Install-specific data (New/Changed) that is incorrect for Remove (should be Removed).
fn handle_files_tab_switch(
    items: &[PackageItem],
    action: crate::state::PreflightAction,
    file_info: &mut Vec<crate::state::modal::PackageFileInfo>,
    file_selected: &mut usize,
    install_list_files: &[crate::state::modal::PackageFileInfo],
    preflight_files_items: &mut Option<Vec<PackageItem>>,
    preflight_files_resolving: &mut bool,
) {
    tracing::debug!(
        "[Preflight] switch_preflight_tab: Files tab - file_info.len()={}, cache.len()={}, resolving={}, action={:?}",
        file_info.len(),
        install_list_files.len(),
        *preflight_files_resolving,
        action
    );

    if file_info.is_empty() {
        match action {
            crate::state::PreflightAction::Install => {
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
            }
            crate::state::PreflightAction::Remove => {
                // For Remove actions, always trigger fresh resolution since cached files
                // contain Install-specific data (New/Changed) that is incorrect for Remove (should be Removed)
                tracing::debug!(
                    "[Preflight] Triggering background file resolution for {} packages (action=Remove) - cache not used due to action mismatch",
                    items.len()
                );
                *preflight_files_items = Some(items.to_vec());
                *preflight_files_resolving = true;
            }
            crate::state::PreflightAction::Downgrade => {
                // For Downgrade actions, always trigger fresh resolution
                tracing::debug!(
                    "[Preflight] Triggering background file resolution for {} packages (action=Downgrade)",
                    items.len()
                );
                *preflight_files_items = Some(items.to_vec());
                *preflight_files_resolving = true;
            }
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
/// - For Install actions: Loads cached service information if available (from `install_list_services`).
/// - For Remove actions: Always triggers fresh resolution since cached services contain Install-specific
///   `needs_restart` values that differ from Remove actions.
fn handle_services_tab_switch(
    items: &[PackageItem],
    action: crate::state::PreflightAction,
    params: &mut ServicesTabParams<'_>,
) {
    if params.service_info.is_empty() && !params.services_resolving {
        match action {
            crate::state::PreflightAction::Install => {
                // For Install actions, check cache and use if available
                let cache_exists = if items.is_empty() {
                    false
                } else {
                    let signature = crate::app::services_cache::compute_signature(items);
                    crate::app::services_cache::load_cache(params.services_cache_path, &signature)
                        .is_some()
                };
                if cache_exists {
                    if !params.install_list_services.is_empty() {
                        *params.service_info = params.install_list_services.to_vec();
                        *params.service_selected = 0;
                    }
                    // Cache exists (empty or not) - mark as loaded
                    tracing::debug!(
                        "[Preflight] Services cache exists for Install action, marking as loaded ({} services)",
                        params.service_info.len()
                    );
                    *params.services_loaded = true;
                } else {
                    // No cache exists - trigger background resolution
                    tracing::debug!(
                        "[Preflight] Triggering background service resolution for {} packages (action=Install)",
                        items.len()
                    );
                    *params.preflight_services_items = Some(items.to_vec());
                    *params.preflight_services_resolving = true;
                }
            }
            crate::state::PreflightAction::Remove => {
                // For Remove actions, always trigger fresh resolution since cached services
                // contain Install-specific needs_restart values that are incorrect for Remove
                tracing::debug!(
                    "[Preflight] Triggering background service resolution for {} packages (action=Remove) - cache not used due to action mismatch",
                    items.len()
                );
                *params.preflight_services_items = Some(items.to_vec());
                *params.preflight_services_resolving = true;
            }
            crate::state::PreflightAction::Downgrade => {
                // For Downgrade actions, always trigger fresh resolution
                tracing::debug!(
                    "[Preflight] Triggering background service resolution for {} packages (action=Downgrade)",
                    items.len()
                );
                *params.preflight_services_items = Some(items.to_vec());
                *params.preflight_services_resolving = true;
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
    action: crate::state::PreflightAction,
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
            crate::state::PreflightAction::Remove | crate::state::PreflightAction::Downgrade => {
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
    action: crate::state::PreflightAction,
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
    let mut preflight_deps_items: Option<(Vec<PackageItem>, crate::state::PreflightAction)> = None;
    let mut preflight_files_items = None;
    let mut preflight_services_items = None;
    let mut preflight_sandbox_items = None;
    let mut preflight_files_resolving = false;
    let mut preflight_services_resolving = false;
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
        cached_reverse_deps_report,
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
                    cached_reverse_deps_report,
                };
                // handle_deps_tab_switch sets preflight_deps_items with the correct action
                let _should_trigger = handle_deps_tab_switch(items, action, &mut deps_params);
            }
            crate::state::PreflightTab::Files => {
                handle_files_tab_switch(
                    items,
                    action,
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
                    preflight_services_items: &mut preflight_services_items,
                    preflight_services_resolving: &mut preflight_services_resolving,
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
    if let Some((items, action)) = preflight_deps_items {
        tracing::info!(
            "[Preflight] switch_preflight_tab: Setting preflight_deps_items with {} items, action={:?}, setting preflight_deps_resolving=true",
            items.len(),
            action
        );
        app.preflight_deps_items = Some((items, action));
        app.preflight_deps_resolving = true;
    }
    if remove_preflight_summary_cleared {
        app.remove_preflight_summary.clear();
    }
    if let Some(items) = preflight_files_items {
        app.preflight_files_items = Some(items);
        app.preflight_files_resolving = preflight_files_resolving;
    }
    if let Some(items) = preflight_services_items {
        app.preflight_services_items = Some(items);
        app.preflight_services_resolving = preflight_services_resolving;
    }
    if let Some(items) = preflight_sandbox_items {
        app.preflight_sandbox_items = Some(items);
        app.preflight_sandbox_resolving = preflight_sandbox_resolving;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::{
        PreflightAction, PreflightTab, ServiceImpact, ServiceRestartDecision,
    };

    /// What: Test that Install actions can use cached services.
    ///
    /// Inputs:
    /// - `action`: `PreflightAction::Install`
    /// - Cached services in `install_list_services`
    /// - Cache file exists with matching signature
    ///
    /// Output:
    /// - `service_info` is populated from cache
    /// - `services_loaded` is set to true
    ///
    /// Details:
    /// - Verifies that Install actions correctly load cached services.
    #[test]
    fn test_handle_services_tab_switch_install_uses_cache() {
        let mut app = AppState::default();
        let items = vec![PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];

        // Set up cached services
        app.install_list_services = vec![ServiceImpact {
            unit_name: "test.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        }];

        // Create a temporary cache file
        let temp_dir = std::env::temp_dir();
        let cache_path = temp_dir.join("test_services_cache.json");
        let signature = crate::app::services_cache::compute_signature(&items);
        crate::app::services_cache::save_cache(&cache_path, &signature, &app.install_list_services);
        app.services_cache_path = cache_path.clone();

        // Open preflight modal with Install action
        app.modal = crate::state::Modal::Preflight {
            items: items.clone(),
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            summary: None,
            summary_scroll: 0,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            dependency_info: Vec::new(),
            dep_selected: 0,
            dep_tree_expanded: std::collections::HashSet::new(),
            deps_error: None,
            file_info: Vec::new(),
            file_selected: 0,
            file_tree_expanded: std::collections::HashSet::new(),
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
            cascade_mode: crate::state::modal::CascadeMode::Basic,
            cached_reverse_deps_report: None,
        };

        // Switch to Services tab
        switch_preflight_tab(
            PreflightTab::Services,
            &mut app,
            &items,
            PreflightAction::Install,
        );

        // Verify services were loaded from cache
        if let crate::state::Modal::Preflight {
            service_info,
            services_loaded,
            ..
        } = &app.modal
        {
            assert!(*services_loaded, "Services should be marked as loaded");
            assert!(
                !service_info.is_empty(),
                "Services should be loaded from cache"
            );
            assert_eq!(service_info.len(), 1);
            assert_eq!(service_info[0].unit_name, "test.service");
            assert!(
                service_info[0].needs_restart,
                "Install action should have needs_restart=true"
            );
        } else {
            panic!("Expected Preflight modal");
        }

        // Clean up
        let _ = std::fs::remove_file(&cache_path);
    }

    /// What: Test that Remove actions always trigger resolution (don't use Install cache).
    ///
    /// Inputs:
    /// - `action`: `PreflightAction::Remove`
    /// - Cached services in `install_list_services` (from Install action)
    /// - Cache file exists with matching signature
    ///
    /// Output:
    /// - `preflight_services_items` is set (triggering resolution)
    /// - `preflight_services_resolving` is set to true
    /// - `service_info` remains empty (not loaded from Install cache)
    ///
    /// Details:
    /// - Verifies that Remove actions don't reuse Install-cached services,
    ///   ensuring correct `needs_restart` values.
    #[test]
    fn test_handle_services_tab_switch_remove_ignores_install_cache() {
        let mut app = AppState::default();
        let items = vec![PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];

        // Set up cached services from Install action (with needs_restart=true)
        app.install_list_services = vec![ServiceImpact {
            unit_name: "test.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true, // This is incorrect for Remove actions
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        }];

        // Create a temporary cache file (from Install action)
        let temp_dir = std::env::temp_dir();
        let cache_path = temp_dir.join("test_services_cache_remove.json");
        let signature = crate::app::services_cache::compute_signature(&items);
        crate::app::services_cache::save_cache(&cache_path, &signature, &app.install_list_services);
        app.services_cache_path = cache_path.clone();

        // Open preflight modal with Remove action
        app.modal = crate::state::Modal::Preflight {
            items: items.clone(),
            action: PreflightAction::Remove,
            tab: PreflightTab::Summary,
            summary: None,
            summary_scroll: 0,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            dependency_info: Vec::new(),
            dep_selected: 0,
            dep_tree_expanded: std::collections::HashSet::new(),
            deps_error: None,
            file_info: Vec::new(),
            file_selected: 0,
            file_tree_expanded: std::collections::HashSet::new(),
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
            cascade_mode: crate::state::modal::CascadeMode::Basic,
            cached_reverse_deps_report: None,
        };

        // Switch to Services tab
        switch_preflight_tab(
            PreflightTab::Services,
            &mut app,
            &items,
            PreflightAction::Remove,
        );

        // Verify that resolution was triggered instead of using cache
        assert!(
            app.preflight_services_items.is_some(),
            "Remove action should trigger resolution, not use Install cache"
        );
        assert!(
            app.preflight_services_resolving,
            "Services should be marked as resolving for Remove action"
        );

        // Verify service_info was NOT loaded from Install cache
        if let crate::state::Modal::Preflight {
            service_info,
            services_loaded,
            ..
        } = &app.modal
        {
            assert!(
                service_info.is_empty(),
                "Service info should be empty (not loaded from Install cache)"
            );
            assert!(
                !*services_loaded,
                "Services should not be marked as loaded (resolution in progress)"
            );
        } else {
            panic!("Expected Preflight modal");
        }

        // Clean up
        let _ = std::fs::remove_file(&cache_path);
    }
}
