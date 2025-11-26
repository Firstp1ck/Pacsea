use crate::state::{AppState, PackageItem};

/// What: Filter cached dependencies for current items.
///
/// Inputs:
/// - `app`: Application state with cached dependencies.
/// - `item_names`: Set of current package names.
///
/// Output:
/// - Vector of matching cached dependencies.
fn filter_cached_dependencies(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
) -> Vec<crate::state::modal::DependencyInfo> {
    app.install_list_deps
        .iter()
        .filter(|dep| {
            dep.required_by
                .iter()
                .any(|req_by| item_names.contains(req_by))
        })
        .cloned()
        .collect()
}

/// What: Filter cached files for current items.
///
/// Inputs:
/// - `app`: Application state with cached files.
/// - `item_names`: Set of current package names.
///
/// Output:
/// - Vector of matching cached files.
fn filter_cached_files(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
) -> Vec<crate::state::modal::PackageFileInfo> {
    app.install_list_files
        .iter()
        .filter(|file_info| item_names.contains(&file_info.name))
        .cloned()
        .collect()
}

/// What: Trigger background resolution for preflight data.
///
/// Inputs:
/// - `app`: Application state.
/// - `items`: Packages to resolve.
/// - `dependency_info`: Current dependency info (empty triggers resolution).
/// - `cached_files`: Current cached files (empty triggers resolution).
///
/// Output:
/// - Updates app state with resolution flags and items.
fn trigger_background_resolution(
    app: &mut AppState,
    items: &[PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    cached_files: &[crate::state::modal::PackageFileInfo],
) {
    if dependency_info.is_empty() {
        app.preflight_deps_items = Some((
            items.to_vec(),
            crate::state::modal::PreflightAction::Install,
        ));
        app.preflight_deps_resolving = true;
    }
    if cached_files.is_empty() {
        app.preflight_files_items = Some(items.to_vec());
        app.preflight_files_resolving = true;
    }
    app.preflight_services_items = Some(items.to_vec());
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
}

/// What: Create preflight modal with cached data.
///
/// Inputs:
/// - `app`: Application state.
/// - `items`: Packages under review.
/// - `summary`: Preflight summary data.
/// - `header`: Header chips data.
/// - `dependency_info`: Dependency information.
/// - `cached_files`: Cached file information.
///
/// Output:
/// - Creates and sets the Preflight modal in app state.
fn create_preflight_modal_with_cache(
    app: &mut AppState,
    items: Vec<PackageItem>,
    summary: crate::state::modal::PreflightSummaryData,
    header: crate::state::modal::PreflightHeaderChips,
    dependency_info: Vec<crate::state::modal::DependencyInfo>,
    cached_files: Vec<crate::state::modal::PackageFileInfo>,
) {
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
        cached_reverse_deps_report: None,
    };
}

/// What: Create preflight modal for insert mode (background computation).
///
/// Inputs:
/// - `app`: Application state.
/// - `items`: Packages under review.
///
/// Output:
/// - Creates and sets the Preflight modal with background computation queued.
fn create_preflight_modal_insert_mode(app: &mut AppState, items: Vec<PackageItem>) {
    let items_clone = items.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    app.preflight_summary_items = Some((items_clone, crate::state::PreflightAction::Install));
    app.preflight_summary_resolving = true;
    app.pending_service_plan.clear();
    app.modal = crate::state::Modal::Preflight {
        items,
        action: crate::state::PreflightAction::Install,
        tab: crate::state::PreflightTab::Summary,
        summary: None,
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
        cached_reverse_deps_report: None,
    };
}

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
        // Check if this is a batch update scenario requiring confirmation
        let has_versions = items.iter().any(|item| {
            matches!(item.source, crate::state::Source::Official { .. }) && !item.version.is_empty()
        });
        let reinstall_any = items.iter().any(|item| {
            matches!(item.source, crate::state::Source::Official { .. })
                && crate::index::is_installed(&item.name)
        });

        if has_versions && reinstall_any {
            // Show confirmation modal for batch updates
            app.modal = crate::state::Modal::ConfirmBatchUpdate {
                items,
                dry_run: app.dry_run,
            };
            return;
        }

        crate::install::spawn_install_all(&items, app.dry_run);
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.installing_skipped"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        return;
    }

    if use_cache {
        let crate::logic::preflight::PreflightSummaryOutcome {
            summary,
            header,
            reverse_deps_report: _,
        } = crate::logic::preflight::compute_preflight_summary(
            &items,
            crate::state::PreflightAction::Install,
        );
        app.pending_service_plan.clear();

        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_deps = filter_cached_dependencies(app, &item_names);
        let cached_files = filter_cached_files(app, &item_names);

        let dependency_info = if cached_deps.is_empty() {
            tracing::debug!(
                "[Preflight] Cache empty, will trigger background dependency resolution for {} packages",
                items.len()
            );
            Vec::new()
        } else {
            cached_deps
        };

        trigger_background_resolution(app, &items, &dependency_info, &cached_files);
        create_preflight_modal_with_cache(
            app,
            items,
            summary,
            header,
            dependency_info,
            cached_files,
        );
    } else {
        create_preflight_modal_insert_mode(app, items);
    }
    app.toast_message = Some(if use_cache {
        crate::i18n::t(app, "app.toasts.preflight_opened")
    } else {
        "Preflight opened".to_string()
    });
    app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
}
