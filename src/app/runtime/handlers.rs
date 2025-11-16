use tokio::sync::mpsc;

use crate::logic::add_to_install_list;
use crate::state::*;

/// What: Handle search results update event.
///
/// Inputs:
/// - `app`: Application state
/// - `new_results`: New search results
/// - `details_req_tx`: Channel sender for detail requests
/// - `index_notify_tx`: Channel sender for index update notifications
///
/// Details:
/// - Filters results based on installed-only mode if enabled
/// - Updates selection to preserve previously selected item
/// - Triggers detail fetch and ring prefetch for selected item
/// - Requests index enrichment for official packages near selection
pub fn handle_search_results(
    app: &mut AppState,
    new_results: SearchResults,
    details_req_tx: &mpsc::UnboundedSender<PackageItem>,
    index_notify_tx: &mpsc::UnboundedSender<()>,
) {
    if new_results.id != app.latest_query_id {
        return;
    }
    let prev_selected_name = app.results.get(app.selected).map(|p| p.name.clone());
    // Respect installed-only mode: keep results restricted to explicit installs
    let mut incoming = new_results.items;
    if app.installed_only_mode {
        let explicit = crate::index::explicit_names();
        if app.input.trim().is_empty() {
            // For empty query, reconstruct full installed list (official + AUR fallbacks)
            let mut items: Vec<PackageItem> = crate::index::all_official()
                .into_iter()
                .filter(|p| explicit.contains(&p.name))
                .collect();
            use std::collections::HashSet;
            let official_names: HashSet<String> = items.iter().map(|p| p.name.clone()).collect();
            for name in explicit.into_iter() {
                if !official_names.contains(&name) {
                    let is_eos = name.to_lowercase().contains("eos-");
                    let src = if is_eos {
                        Source::Official {
                            repo: "EOS".to_string(),
                            arch: String::new(),
                        }
                    } else {
                        Source::Aur
                    };
                    items.push(PackageItem {
                        name: name.clone(),
                        version: String::new(),
                        description: String::new(),
                        source: src,
                        popularity: None,
                    });
                }
            }
            incoming = items;
        } else {
            // For non-empty query, just intersect results with explicit installed set
            incoming.retain(|p| explicit.contains(&p.name));
        }
    }
    app.all_results = incoming;
    crate::logic::apply_filters_and_sort_preserve_selection(app);
    let new_sel = prev_selected_name
        .and_then(|name| app.results.iter().position(|p| p.name == name))
        .unwrap_or(0);
    app.selected = new_sel.min(app.results.len().saturating_sub(1));
    app.list_state.select(if app.results.is_empty() {
        None
    } else {
        Some(app.selected)
    });
    if let Some(item) = app.results.get(app.selected).cloned() {
        app.details_focus = Some(item.name.clone());
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_req_tx.send(item.clone());
        }
    }
    crate::logic::set_allowed_ring(app, 30);
    if app.need_ring_prefetch {
        /* defer */
    } else {
        crate::logic::ring_prefetch_from_selected(app, details_req_tx);
    }
    let len_u = app.results.len();
    let mut enrich_names: Vec<String> = Vec::new();
    if let Some(sel) = app.results.get(app.selected)
        && matches!(sel.source, Source::Official { .. })
    {
        enrich_names.push(sel.name.clone());
    }
    let max_radius: usize = 30;
    let mut step: usize = 1;
    while step <= max_radius {
        if let Some(i) = app.selected.checked_sub(step)
            && let Some(it) = app.results.get(i)
            && matches!(it.source, Source::Official { .. })
        {
            enrich_names.push(it.name.clone());
        }
        let below = app.selected + step;
        if below < len_u
            && let Some(it) = app.results.get(below)
            && matches!(it.source, Source::Official { .. })
        {
            enrich_names.push(it.name.clone());
        }
        step += 1;
    }
    if !enrich_names.is_empty() {
        crate::index::request_enrich_for(
            app.official_index_path.clone(),
            index_notify_tx.clone(),
            enrich_names,
        );
    }
}

/// What: Handle package details update event.
///
/// Inputs:
/// - `app`: Application state
/// - `details`: New package details
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates details cache and current details if focused
/// - Updates result list entry with new information
pub fn handle_details_update(
    app: &mut AppState,
    details: PackageDetails,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    if app.details_focus.as_deref() == Some(details.name.as_str()) {
        app.details = details.clone();
    }
    app.details_cache
        .insert(details.name.clone(), details.clone());
    app.cache_dirty = true;
    if let Some(pos) = app.results.iter().position(|p| p.name == details.name) {
        app.results[pos].description = details.description.clone();
        if !details.version.is_empty() && app.results[pos].version != details.version {
            app.results[pos].version = details.version.clone();
        }
        if details.popularity.is_some() {
            app.results[pos].popularity = details.popularity;
        }
        if let crate::state::Source::Official { repo, arch } = &mut app.results[pos].source {
            if repo.is_empty() && !details.repository.is_empty() {
                *repo = details.repository.clone();
            }
            if arch.is_empty() && !details.architecture.is_empty() {
                *arch = details.architecture.clone();
            }
        }
    }
    let _ = tick_tx.send(());
}

/// What: Handle preview item event.
///
/// Inputs:
/// - `app`: Application state
/// - `item`: Package item to preview
/// - `details_req_tx`: Channel sender for detail requests
///
/// Details:
/// - Loads details for previewed item (from cache or network)
/// - Adjusts selection if needed
pub fn handle_preview(
    app: &mut AppState,
    item: PackageItem,
    details_req_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    if let Some(cached) = app.details_cache.get(&item.name).cloned() {
        app.details = cached;
    } else {
        let _ = details_req_tx.send(item.clone());
    }
    if !app.results.is_empty() && app.selected >= app.results.len() {
        app.selected = app.results.len() - 1;
        app.list_state.select(Some(app.selected));
    }
}

/// What: Handle add to install list event (single item).
///
/// Inputs:
/// - `app`: Application state
/// - `item`: Package item to add
/// - `deps_req_tx`: Channel sender for dependency resolution requests
/// - `files_req_tx`: Channel sender for file resolution requests
/// - `services_req_tx`: Channel sender for service resolution requests
/// - `sandbox_req_tx`: Channel sender for sandbox resolution requests
///
/// Details:
/// - Adds item to install list
/// - Triggers background resolution for dependencies, files, services, and sandbox
pub fn handle_add_to_install_list(
    app: &mut AppState,
    item: PackageItem,
    deps_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    files_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    services_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    add_to_install_list(app, item);
    // Trigger background dependency resolution for updated install list
    if !app.install_list.is_empty() {
        app.deps_resolving = true;
        let _ = deps_req_tx.send(app.install_list.clone());
        // Trigger background file resolution for updated install list
        app.files_resolving = true;
        let _ = files_req_tx.send(app.install_list.clone());
        // Trigger background service resolution for updated install list
        app.services_resolving = true;
        let _ = services_req_tx.send(app.install_list.clone());
        // Trigger background sandbox resolution for updated install list
        app.sandbox_resolving = true;
        let _ = sandbox_req_tx.send(app.install_list.clone());
    }
}

/// What: Handle dependency resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `deps`: Dependency resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached dependencies
/// - Syncs dependencies to preflight modal if open
/// - Respects cancellation flag
pub fn handle_dependency_result(
    app: &mut AppState,
    deps: Vec<crate::state::modal::DependencyInfo>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_deps_resolving;
    app.deps_resolving = false; // CRITICAL: Always reset this flag when we receive ANY result
    app.preflight_deps_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached dependencies
        tracing::info!(
            stage = "dependencies",
            result_count = deps.len(),
            "[Runtime] Dependency resolution worker completed"
        );
        app.install_list_deps = deps.clone();
        // Sync dependencies to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items,
            dependency_info,
            ..
        } = &mut app.modal
        {
            // Filter dependencies to only those required by current modal items
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let filtered_deps: Vec<_> = deps
                .iter()
                .filter(|dep| {
                    dep.required_by
                        .iter()
                        .any(|req_by| item_names.contains(req_by))
                })
                .cloned()
                .collect();
            if !filtered_deps.is_empty() {
                tracing::debug!(
                    "[Runtime] Synced {} dependencies to preflight modal (was_preflight={})",
                    filtered_deps.len(),
                    was_preflight
                );
                *dependency_info = filtered_deps;
            }
        }
        if was_preflight {
            app.preflight_deps_items = None;
        }
        app.deps_cache_dirty = true; // Mark cache as dirty for persistence
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring dependency result (preflight cancelled)");
        app.preflight_deps_items = None;
    }
    let _ = tick_tx.send(());
}

/// What: Handle file resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `files`: File resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached files
/// - Syncs files to preflight modal if open
/// - Respects cancellation flag
pub fn handle_file_result(
    app: &mut AppState,
    files: Vec<crate::state::modal::PackageFileInfo>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_files_resolving;
    app.files_resolving = false;
    app.preflight_files_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached files
        tracing::info!(
            stage = "files",
            result_count = files.len(),
            "[Runtime] File resolution worker completed"
        );
        app.install_list_files = files.clone();
        // Sync files to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items, file_info, ..
        } = &mut app.modal
        {
            // Filter files to only those for current modal items
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let filtered_files: Vec<_> = files
                .iter()
                .filter(|file_info| item_names.contains(&file_info.name))
                .cloned()
                .collect();
            if !filtered_files.is_empty() {
                tracing::debug!(
                    "[Runtime] Synced {} file infos to preflight modal (was_preflight={})",
                    filtered_files.len(),
                    was_preflight
                );
                *file_info = filtered_files;
            }
        }
        if was_preflight {
            app.preflight_files_items = None;
        }
        app.files_cache_dirty = true; // Mark cache as dirty for persistence
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring file result (preflight cancelled)");
        app.preflight_files_items = None;
    }
    let _ = tick_tx.send(());
}

/// What: Handle service resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `services`: Service resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached services
/// - Syncs services to preflight modal if open
/// - Respects cancellation flag
pub fn handle_service_result(
    app: &mut AppState,
    services: Vec<crate::state::modal::ServiceImpact>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_services_resolving;
    app.services_resolving = false;
    app.preflight_services_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached services
        tracing::info!(
            stage = "services",
            result_count = services.len(),
            "[Runtime] Service resolution worker completed"
        );
        app.install_list_services = services;
        // Sync services to preflight modal if it's open
        if was_preflight {
            if let crate::state::Modal::Preflight {
                service_info,
                services_loaded,
                ..
            } = &mut app.modal
            {
                *service_info = app.install_list_services.clone();
                *services_loaded = true;
                tracing::debug!(
                    "[Runtime] Synced {} services to preflight modal",
                    service_info.len()
                );
            }
            app.preflight_services_items = None;
        }
        app.services_cache_dirty = true; // Mark cache as dirty for persistence
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring service result (preflight cancelled)");
        app.preflight_services_items = None;
    }
    let _ = tick_tx.send(());
}

/// What: Handle sandbox resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `sandbox_info`: Sandbox resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached sandbox info
/// - Syncs sandbox info to preflight modal if open
/// - Handles empty results and errors gracefully
/// - Respects cancellation flag
pub fn handle_sandbox_result(
    app: &mut AppState,
    sandbox_info: Vec<crate::logic::sandbox::SandboxInfo>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_sandbox_resolving;
    app.sandbox_resolving = false;
    app.preflight_sandbox_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached sandbox info
        tracing::info!(
            stage = "sandbox",
            result_count = sandbox_info.len(),
            "[Runtime] Sandbox resolution worker completed"
        );
        app.install_list_sandbox = sandbox_info.clone();
        // Sync sandbox info to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items,
            sandbox_info: modal_sandbox,
            sandbox_loaded,
            sandbox_error,
            ..
        } = &mut app.modal
        {
            // Filter sandbox info to only those for current modal items (AUR only)
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let aur_items: Vec<_> = items
                .iter()
                .filter(|p| matches!(p.source, crate::state::Source::Aur))
                .collect();
            let filtered_sandbox: Vec<_> = sandbox_info
                .iter()
                .filter(|sb| item_names.contains(&sb.package_name))
                .cloned()
                .collect();
            // Always sync sandbox info if we have matching entries, even if dependency lists are empty
            // (empty lists mean all dependencies are already installed, which is still useful info)
            if !filtered_sandbox.is_empty() {
                tracing::debug!(
                    "[Runtime] Synced {} sandbox infos to preflight modal (was_preflight={})",
                    filtered_sandbox.len(),
                    was_preflight
                );
                *modal_sandbox = filtered_sandbox;
                *sandbox_loaded = true;
                *sandbox_error = None; // Clear any previous errors
            } else {
                // Check if we have AUR packages but no sandbox info
                if aur_items.is_empty() {
                    // No AUR packages, mark as loaded
                    *sandbox_loaded = true;
                    *sandbox_error = None;
                } else if !sandbox_info.is_empty() {
                    // We have sandbox info but it doesn't match current items
                    // This could happen if items changed between resolution start and completion
                    // Try to sync anyway - maybe some packages match
                    let partial_match: Vec<_> = sandbox_info
                        .iter()
                        .filter(|sb| item_names.contains(&sb.package_name))
                        .cloned()
                        .collect();
                    if !partial_match.is_empty() {
                        tracing::debug!(
                            "[Runtime] Partial sandbox sync: {} of {} packages matched",
                            partial_match.len(),
                            item_names.len()
                        );
                        *modal_sandbox = partial_match;
                        *sandbox_loaded = true;
                        *sandbox_error = None;
                    } else {
                        tracing::warn!(
                            "[Runtime] Sandbox info exists but doesn't match modal items. Modal items: {:?}, Sandbox packages: {:?}",
                            item_names,
                            sandbox_info
                                .iter()
                                .map(|s| &s.package_name)
                                .collect::<Vec<_>>()
                        );
                        // Still mark as loaded to prevent infinite loading state
                        *sandbox_loaded = true;
                        *sandbox_error = None;
                    }
                } else {
                    // sandbox_info is empty but we have AUR packages - resolution likely failed
                    // This could happen if AUR is down or network issues
                    tracing::warn!(
                        "[Runtime] Sandbox resolution returned empty results for {} AUR packages (AUR may be down or network issues)",
                        aur_items.len()
                    );
                    *sandbox_loaded = true; // Mark as loaded so UI can show error/empty state
                    *sandbox_error = Some(format!(
                        "Failed to fetch sandbox information for {} AUR package(s). AUR may be temporarily unavailable.",
                        aur_items.len()
                    ));
                }
            }
        }
        if was_preflight {
            app.preflight_sandbox_items = None;
        }
        app.sandbox_cache_dirty = true; // Mark cache as dirty for persistence
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring sandbox result (preflight cancelled)");
        app.preflight_sandbox_items = None;
    }
    let _ = tick_tx.send(());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Provide a baseline `AppState` for handler tests.
    ///
    /// Inputs: None
    /// Output: Fresh `AppState` with default values
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Verify that handle_search_results ignores results with mismatched query ID.
    ///
    /// Inputs:
    /// - App state with latest_query_id = 1
    /// - SearchResults with id = 2
    ///
    /// Output:
    /// - Results are ignored, app state unchanged
    ///
    /// Details:
    /// - Tests that stale results are properly filtered
    fn handle_search_results_ignores_stale_results() {
        let mut app = new_app();
        app.latest_query_id = 1;
        app.results = vec![PackageItem {
            name: "old-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Old".to_string(),
            source: Source::Aur,
            popularity: None,
        }];

        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (index_tx, _index_rx) = mpsc::unbounded_channel();

        let stale_results = SearchResults {
            id: 2, // Different from app.latest_query_id
            items: vec![PackageItem {
                name: "new-package".to_string(),
                version: "2.0.0".to_string(),
                description: "New".to_string(),
                source: Source::Aur,
                popularity: None,
            }],
        };

        handle_search_results(&mut app, stale_results, &details_tx, &index_tx);

        // Results should not be updated
        assert_eq!(app.results.len(), 1);
        assert_eq!(app.results[0].name, "old-package");
    }

    #[test]
    /// What: Verify that handle_search_results updates results when query ID matches.
    ///
    /// Inputs:
    /// - App state with latest_query_id = 1
    /// - SearchResults with id = 1 and new items
    ///
    /// Output:
    /// - Results are updated with new items
    /// - Selection is preserved or adjusted
    ///
    /// Details:
    /// - Tests that valid results are properly processed
    fn handle_search_results_updates_when_id_matches() {
        let mut app = new_app();
        app.latest_query_id = 1;
        app.results = vec![PackageItem {
            name: "old-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Old".to_string(),
            source: Source::Aur,
            popularity: None,
        }];

        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (index_tx, _index_rx) = mpsc::unbounded_channel();

        let new_results = SearchResults {
            id: 1, // Matches app.latest_query_id
            items: vec![PackageItem {
                name: "new-package".to_string(),
                version: "2.0.0".to_string(),
                description: "New".to_string(),
                source: Source::Aur,
                popularity: None,
            }],
        };

        handle_search_results(&mut app, new_results, &details_tx, &index_tx);

        // Results should be updated
        assert_eq!(app.results.len(), 1);
        assert_eq!(app.results[0].name, "new-package");
    }

    #[test]
    /// What: Verify that handle_details_update updates cache and current details.
    ///
    /// Inputs:
    /// - App state with details_focus set
    /// - PackageDetails for focused package
    ///
    /// Output:
    /// - Details cache is updated
    /// - Current details are updated if focused
    ///
    /// Details:
    /// - Tests that details are properly cached and displayed
    fn handle_details_update_updates_cache_and_details() {
        let mut app = new_app();
        app.details_focus = Some("test-package".to_string());
        app.details_cache = std::collections::HashMap::new();

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let details = PackageDetails {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Test package".to_string(),
            repository: String::new(),
            architecture: String::new(),
            url: String::new(),
            licenses: Vec::new(),
            groups: Vec::new(),
            provides: Vec::new(),
            depends: Vec::new(),
            opt_depends: Vec::new(),
            required_by: Vec::new(),
            optional_for: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            download_size: None,
            install_size: None,
            owner: String::new(),
            build_date: String::new(),
            popularity: None,
        };

        handle_details_update(&mut app, details.clone(), &tick_tx);

        // Cache should be updated
        assert!(app.details_cache.contains_key("test-package"));
        // Current details should be updated if focused
        assert_eq!(app.details.name, "test-package");
        // Cache dirty flag should be set
        assert!(app.cache_dirty);
    }

    #[test]
    /// What: Verify that handle_preview loads details from cache when available.
    ///
    /// Inputs:
    /// - App state with cached details
    /// - PackageItem to preview
    ///
    /// Output:
    /// - Details are loaded from cache
    /// - No network request is made
    ///
    /// Details:
    /// - Tests that cached details are used when available
    fn handle_preview_uses_cache_when_available() {
        let mut app = new_app();
        let cached_details = PackageDetails {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Cached".to_string(),
            repository: String::new(),
            architecture: String::new(),
            url: String::new(),
            licenses: Vec::new(),
            groups: Vec::new(),
            provides: Vec::new(),
            depends: Vec::new(),
            opt_depends: Vec::new(),
            required_by: Vec::new(),
            optional_for: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            download_size: None,
            install_size: None,
            owner: String::new(),
            build_date: String::new(),
            popularity: None,
        };
        app.details_cache
            .insert("test-package".to_string(), cached_details.clone());

        let (details_tx, mut details_rx) = mpsc::unbounded_channel();

        let item = PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: Source::Aur,
            popularity: None,
        };

        handle_preview(&mut app, item, &details_tx);

        // Details should be loaded from cache
        assert_eq!(app.details.name, "test-package");
        // No request should be sent (channel should be empty)
        assert!(details_rx.try_recv().is_err());
    }

    #[test]
    /// What: Verify that handle_add_to_install_list adds item and triggers resolutions.
    ///
    /// Inputs:
    /// - App state with empty install list
    /// - PackageItem to add
    /// - Channel senders
    ///
    /// Output:
    /// - Item is added to install list
    /// - Resolution flags are set
    /// - Requests are sent to resolution channels
    ///
    /// Details:
    /// - Tests that adding items triggers background resolution
    fn handle_add_to_install_list_adds_and_triggers_resolution() {
        let mut app = new_app();
        app.install_list.clear();

        let (deps_tx, mut deps_rx) = mpsc::unbounded_channel();
        let (files_tx, mut files_rx) = mpsc::unbounded_channel();
        let (services_tx, mut services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, mut sandbox_rx) = mpsc::unbounded_channel();

        let item = PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: Source::Aur,
            popularity: None,
        };

        handle_add_to_install_list(
            &mut app,
            item,
            &deps_tx,
            &files_tx,
            &services_tx,
            &sandbox_tx,
        );

        // Item should be added
        assert_eq!(app.install_list.len(), 1);
        assert_eq!(app.install_list[0].name, "test-package");
        // Flags should be set
        assert!(app.deps_resolving);
        assert!(app.files_resolving);
        assert!(app.services_resolving);
        assert!(app.sandbox_resolving);
        // Requests should be sent
        assert!(deps_rx.try_recv().is_ok());
        assert!(files_rx.try_recv().is_ok());
        assert!(services_rx.try_recv().is_ok());
        assert!(sandbox_rx.try_recv().is_ok());
    }

    #[test]
    /// What: Verify that handle_dependency_result updates cache and respects cancellation.
    ///
    /// Inputs:
    /// - App state
    /// - Dependency resolution results
    /// - Cancellation flag not set
    ///
    /// Output:
    /// - Dependencies are cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that dependency results are properly processed
    fn handle_dependency_result_updates_cache() {
        let mut app = new_app();
        app.deps_resolving = true;
        app.preflight_deps_resolving = false;
        app.preflight_cancelled
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let deps = vec![crate::state::modal::DependencyInfo {
            name: "dep-package".to_string(),
            version: "1.0.0".to_string(),
            status: crate::state::modal::DependencyStatus::ToInstall,
            source: crate::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        }];

        handle_dependency_result(&mut app, deps.clone(), &tick_tx);

        // Dependencies should be cached
        assert_eq!(app.install_list_deps.len(), 1);
        // Flags should be reset
        assert!(!app.deps_resolving);
        assert!(!app.preflight_deps_resolving);
        // Cache dirty flag should be set
        assert!(app.deps_cache_dirty);
    }

    #[test]
    /// What: Verify that handle_dependency_result ignores results when cancelled.
    ///
    /// Inputs:
    /// - App state with cancellation flag set
    /// - Dependency resolution results
    ///
    /// Output:
    /// - Results are ignored
    /// - Flags are still reset
    ///
    /// Details:
    /// - Tests that cancellation is properly respected
    fn handle_dependency_result_respects_cancellation() {
        let mut app = new_app();
        app.preflight_deps_resolving = true;
        app.preflight_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        app.install_list_deps = vec![]; // Empty before

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let deps = vec![crate::state::modal::DependencyInfo {
            name: "dep-package".to_string(),
            version: "1.0.0".to_string(),
            status: crate::state::modal::DependencyStatus::ToInstall,
            source: crate::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        }];

        handle_dependency_result(&mut app, deps, &tick_tx);

        // Dependencies should not be updated when cancelled
        assert_eq!(app.install_list_deps.len(), 0);
        // Flags should still be reset
        assert!(!app.deps_resolving);
        assert!(!app.preflight_deps_resolving);
    }

    #[test]
    /// What: Verify that handle_file_result updates cache correctly.
    ///
    /// Inputs:
    /// - App state
    /// - File resolution results
    ///
    /// Output:
    /// - Files are cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that file results are properly processed
    fn handle_file_result_updates_cache() {
        let mut app = new_app();
        app.files_resolving = true;

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let files = vec![crate::state::modal::PackageFileInfo {
            name: "test-package".to_string(),
            files: vec![],
            total_count: 0,
            new_count: 0,
            changed_count: 0,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        }];

        handle_file_result(&mut app, files.clone(), &tick_tx);

        // Files should be cached
        assert_eq!(app.install_list_files.len(), 1);
        // Flags should be reset
        assert!(!app.files_resolving);
        assert!(!app.preflight_files_resolving);
        // Cache dirty flag should be set
        assert!(app.files_cache_dirty);
    }

    #[test]
    /// What: Verify that handle_service_result updates cache correctly.
    ///
    /// Inputs:
    /// - App state
    /// - Service resolution results
    ///
    /// Output:
    /// - Services are cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that service results are properly processed
    fn handle_service_result_updates_cache() {
        let mut app = new_app();
        app.services_resolving = true;
        app.preflight_services_resolving = true;

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let services = vec![crate::state::modal::ServiceImpact {
            unit_name: "test.service".to_string(),
            providers: vec!["test-package".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate::state::modal::ServiceRestartDecision::Defer,
        }];

        handle_service_result(&mut app, services.clone(), &tick_tx);

        // Services should be cached
        assert_eq!(app.install_list_services.len(), 1);
        // Flags should be reset
        assert!(!app.services_resolving);
        assert!(!app.preflight_services_resolving);
        // Cache dirty flag should be set
        assert!(app.services_cache_dirty);
    }

    #[test]
    /// What: Verify that handle_sandbox_result updates cache correctly.
    ///
    /// Inputs:
    /// - App state
    /// - Sandbox resolution results
    ///
    /// Output:
    /// - Sandbox info is cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that sandbox results are properly processed
    fn handle_sandbox_result_updates_cache() {
        let mut app = new_app();
        app.sandbox_resolving = true;

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let sandbox_info = vec![crate::logic::sandbox::SandboxInfo {
            package_name: "test-package".to_string(),
            depends: vec![],
            makedepends: vec![],
            checkdepends: vec![],
            optdepends: vec![],
        }];

        handle_sandbox_result(&mut app, sandbox_info.clone(), &tick_tx);

        // Sandbox info should be cached
        assert_eq!(app.install_list_sandbox.len(), 1);
        // Flags should be reset
        assert!(!app.sandbox_resolving);
        assert!(!app.preflight_sandbox_resolving);
        // Cache dirty flag should be set
        assert!(app.sandbox_cache_dirty);
    }
}
