//! Modal management functions for Preflight modal.

use crate::state::{AppState, PackageItem};

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
pub(crate) fn close_preflight_modal(
    app: &mut AppState,
    service_info: &[crate::state::modal::ServiceImpact],
) {
    if !service_info.is_empty() {
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
pub(crate) fn switch_preflight_tab(
    new_tab: crate::state::PreflightTab,
    app: &mut AppState,
    items: &[PackageItem],
    action: &crate::state::PreflightAction,
) {
    if let crate::state::Modal::Preflight {
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
        // Check for cached dependencies when switching to Deps tab
        if new_tab == crate::state::PreflightTab::Deps && dependency_info.is_empty() {
            match action {
                crate::state::PreflightAction::Install => {
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
                    } else {
                        tracing::debug!(
                            "[Preflight] Triggering background dependency resolution for {} packages",
                            items.len()
                        );
                        app.preflight_deps_items = Some(items.to_vec());
                        app.preflight_deps_resolving = true;
                    }
                    app.remove_preflight_summary.clear();
                }
                crate::state::PreflightAction::Remove => {
                    // For remove action, reverse deps are computed on-demand
                }
            }
        }

        // Check for cached files when switching to Files tab
        if new_tab == crate::state::PreflightTab::Files && file_info.is_empty() {
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
            } else {
                tracing::debug!(
                    "[Preflight] Triggering background file resolution for {} packages",
                    items.len()
                );
                app.preflight_files_items = Some(items.to_vec());
                app.preflight_files_resolving = true;
            }
        }

        // Check for cached services when switching to Services tab
        if new_tab == crate::state::PreflightTab::Services
            && service_info.is_empty()
            && matches!(action, crate::state::PreflightAction::Install)
            && !app.services_resolving
        {
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

        // Check for cached sandbox when switching to Sandbox tab
        if new_tab == crate::state::PreflightTab::Sandbox
            && sandbox_info.is_empty()
            && !*sandbox_loaded
        {
            match action {
                crate::state::PreflightAction::Install => {
                    let item_names: std::collections::HashSet<String> =
                        items.iter().map(|i| i.name.clone()).collect();
                    let cached_sandbox: Vec<crate::logic::sandbox::SandboxInfo> = app
                        .install_list_sandbox
                        .iter()
                        .filter(|s| item_names.contains(&s.package_name))
                        .cloned()
                        .collect();
                    if !cached_sandbox.is_empty() {
                        *sandbox_info = cached_sandbox;
                        *sandbox_selected = 0;
                        *sandbox_loaded = true;
                    } else {
                        let aur_items: Vec<_> = items
                            .iter()
                            .filter(|p| matches!(p.source, crate::state::Source::Aur))
                            .cloned()
                            .collect();
                        if !aur_items.is_empty() {
                            tracing::debug!(
                                "[Preflight] Triggering background sandbox resolution for {} AUR packages",
                                aur_items.len()
                            );
                            app.preflight_sandbox_items = Some(aur_items);
                            app.preflight_sandbox_resolving = true;
                        } else {
                            *sandbox_loaded = true;
                        }
                    }
                }
                crate::state::PreflightAction::Remove => {
                    *sandbox_loaded = true;
                }
            }
        }
    }
}
