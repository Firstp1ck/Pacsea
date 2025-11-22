use crate::state::{AppState, PackageItem};

/// What: Open preflight modal with cached dependencies and files, or trigger background resolution.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Packages to open preflight for
/// - `use_cache`: Whether to use cached dependencies/files or trigger background resolution
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - If `use_cache` is true, checks cache and uses cached data if available, otherwise triggers background resolution.
/// - If `use_cache` is false, always triggers background resolution (used in insert mode).
/// - Sets up all preflight resolution flags and initializes the modal state.
pub fn open_preflight_modal(app: &mut AppState, items: Vec<PackageItem>, use_cache: bool) {
    if crate::theme::settings().skip_preflight {
        // Direct install of single item
        crate::install::spawn_install_all(&items, app.dry_run);
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.installing_skipped"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        return;
    }

    if use_cache {
        // Normal mode: use cache or trigger background resolution
        let crate::logic::preflight::PreflightSummaryOutcome { summary, header } =
            crate::logic::preflight::compute_preflight_summary(
                &items,
                crate::state::PreflightAction::Install,
            );
        app.pending_service_plan.clear();

        // Check cache and auto-resolve dependencies if needed (opening on Deps tab)
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
        let cached_files: Vec<crate::state::modal::PackageFileInfo> = app
            .install_list_files
            .iter()
            .filter(|file_info| item_names.contains(&file_info.name))
            .cloned()
            .collect();

        // Use cached dependencies, or trigger background resolution if cache is empty
        let dependency_info = if cached_deps.is_empty() {
            tracing::debug!(
                "[Preflight] Cache empty, will trigger background dependency resolution for {} packages",
                items.len()
            );
            // Trigger background resolution - results will be synced when they arrive
            Vec::new()
        } else {
            cached_deps
        };

        // Trigger background resolution for all stages in parallel if cache is empty
        if dependency_info.is_empty() {
            app.preflight_deps_items = Some(items.clone());
            app.preflight_deps_resolving = true;
        }
        if cached_files.is_empty() {
            app.preflight_files_items = Some(items.clone());
            app.preflight_files_resolving = true;
        }
        // Services resolution (always trigger for install actions)
        app.preflight_services_items = Some(items.clone());
        app.preflight_services_resolving = true;
        let aur_items: Vec<_> = items
            .iter()
            .filter(|p| matches!(p.source, crate::state::Source::Aur))
            .cloned()
            .collect();
        if !aur_items.is_empty() {
            app.preflight_sandbox_items = Some(aur_items);
            app.preflight_sandbox_resolving = true;
        }

        app.modal = crate::state::Modal::Preflight {
            items,
            action: crate::state::PreflightAction::Install,
            tab: crate::state::PreflightTab::Deps,
            summary: Some(Box::new(summary)),
            summary_scroll: 0,
            header_chips: header,
            dependency_info,
            dep_selected: 0,
            dep_tree_expanded: std::collections::HashSet::new(),
            deps_error: None,
            file_info: cached_files,
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
            cascade_mode: app.remove_cascade_mode,
        };
    } else {
        // Insert mode: always use background computation
        let items_clone = items.clone();
        // Reset cancellation flag when opening modal
        app.preflight_cancelled
            .store(false, std::sync::atomic::Ordering::Relaxed);
        // Queue summary computation in background - modal will render with None initially
        app.preflight_summary_items = Some((items_clone, crate::state::PreflightAction::Install));
        app.preflight_summary_resolving = true;
        app.pending_service_plan.clear();
        app.modal = crate::state::Modal::Preflight {
            items,
            action: crate::state::PreflightAction::Install,
            tab: crate::state::PreflightTab::Summary,
            summary: None, // Will be populated when background computation completes
            summary_scroll: 0,
            header_chips: crate::state::modal::PreflightHeaderChips {
                package_count: 1,
                download_bytes: 0,
                install_delta_bytes: 0,
                aur_count: 0,
                risk_score: 0,
                risk_level: crate::state::modal::RiskLevel::Low,
            },
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
            cascade_mode: app.remove_cascade_mode,
        };
    }
    app.toast_message = Some(if use_cache {
        crate::i18n::t(app, "app.toasts.preflight_opened")
    } else {
        "Preflight opened".to_string()
    });
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
}
