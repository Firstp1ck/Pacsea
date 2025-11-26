use tokio::sync::mpsc;

use crate::state::{AppState, PackageDetails, PackageItem, SearchResults, Source};

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
        use std::collections::HashSet;
        let explicit = crate::index::explicit_names();
        if app.input.trim().is_empty() {
            // For empty query, reconstruct full installed list (official + AUR fallbacks)
            let mut items: Vec<PackageItem> = crate::index::all_official()
                .into_iter()
                .filter(|p| explicit.contains(&p.name))
                .collect();
            let official_names: HashSet<String> = items.iter().map(|p| p.name.clone()).collect();
            for name in explicit {
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
                        out_of_date: None,
                        orphaned: false,
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
            let _ = details_req_tx.send(item);
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
    details: &PackageDetails,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    let details_clone = details.clone();
    if app.details_focus.as_deref() == Some(details.name.as_str()) {
        app.details = details_clone.clone();
    }
    app.details_cache
        .insert(details_clone.name.clone(), details_clone.clone());
    app.cache_dirty = true;
    if let Some(pos) = app.results.iter().position(|p| p.name == details.name) {
        app.results[pos].description = details_clone.description;
        if !details_clone.version.is_empty() && app.results[pos].version != details_clone.version {
            app.results[pos].version = details_clone.version;
        }
        if details_clone.popularity.is_some() {
            app.results[pos].popularity = details_clone.popularity;
        }
        if let crate::state::Source::Official { repo, arch } = &mut app.results[pos].source {
            if repo.is_empty() && !details_clone.repository.is_empty() {
                *repo = details_clone.repository;
            }
            if arch.is_empty() && !details_clone.architecture.is_empty() {
                *arch = details_clone.architecture;
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
        let _ = details_req_tx.send(item);
    }
    if !app.results.is_empty() && app.selected >= app.results.len() {
        app.selected = app.results.len() - 1;
        app.list_state.select(Some(app.selected));
    }
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
    /// What: Verify that `handle_search_results` ignores results with mismatched query ID.
    ///
    /// Inputs:
    /// - `AppState` with `latest_query_id` = 1
    /// - `SearchResults` with `id` = 2
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
    /// What: Verify that `handle_search_results` updates results when query ID matches.
    ///
    /// Inputs:
    /// - `AppState` with `latest_query_id` = 1
    /// - `SearchResults` with `id` = 1 and new items
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
    /// What: Verify that `handle_details_update` updates cache and current details.
    ///
    /// Inputs:
    /// - `AppState` with `details_focus` set
    /// - `PackageDetails` for focused package
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

        handle_details_update(&mut app, &details, &tick_tx);

        // Cache should be updated
        assert!(app.details_cache.contains_key("test-package"));
        // Current details should be updated if focused
        assert_eq!(app.details.name, "test-package");
        // Cache dirty flag should be set
        assert!(app.cache_dirty);
    }

    #[test]
    /// What: Verify that `handle_preview` loads details from cache when available.
    ///
    /// Inputs:
    /// - `AppState` with cached details
    /// - `PackageItem` to preview
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
            .insert("test-package".to_string(), cached_details);

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
}
