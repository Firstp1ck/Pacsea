use crate::state::AppState;

/// What: Load cached dependencies filtered by item names.
///
/// Inputs:
/// - `app`: Application state reference
/// - `item_names`: Set of package names to filter dependencies by
///
/// Output:
/// - Vector of filtered dependency info, or empty if cache unavailable
///
/// Details:
/// - Filters cached dependencies to only those required by current items
/// - Returns empty vector if cache is resolving or empty
fn load_cached_dependencies(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
) -> Vec<crate::state::modal::DependencyInfo> {
    if app.deps_resolving || app.install_list_deps.is_empty() {
        tracing::debug!("[Install] No cached dependencies available");
        return Vec::new();
    }

    let filtered: Vec<crate::state::modal::DependencyInfo> = app
        .install_list_deps
        .iter()
        .filter(|dep| {
            dep.required_by
                .iter()
                .any(|req_by| item_names.contains(req_by))
        })
        .cloned()
        .collect();

    if filtered.is_empty() {
        tracing::debug!("[Install] Cached dependencies exist but none match current items");
        Vec::new()
    } else {
        tracing::debug!(
            "[Install] Using {} cached dependencies (filtered from {} total)",
            filtered.len(),
            app.install_list_deps.len()
        );
        filtered
    }
}

/// What: Load cached file information.
///
/// Inputs:
/// - `app`: Application state reference
/// - `items`: Package items for logging
///
/// Output:
/// - Vector of cached file info, or empty if cache unavailable
///
/// Details:
/// - Returns cached files if available and not currently resolving
fn load_cached_files(
    app: &AppState,
    items: &[crate::state::types::PackageItem],
) -> Vec<crate::state::modal::PackageFileInfo> {
    tracing::debug!(
        "[Install] Checking file cache: resolving={}, install_list_files={}, items={:?}",
        app.files_resolving,
        app.install_list_files.len(),
        items.iter().map(|i| &i.name).collect::<Vec<_>>()
    );

    if app.files_resolving || app.install_list_files.is_empty() {
        tracing::debug!(
            "[Install] No cached files available (resolving={}, empty={})",
            app.files_resolving,
            app.install_list_files.is_empty()
        );
        return Vec::new();
    }

    tracing::debug!(
        "[Install] Using {} cached file entries: {:?}",
        app.install_list_files.len(),
        app.install_list_files
            .iter()
            .map(|f| &f.name)
            .collect::<Vec<_>>()
    );
    for file_info in &app.install_list_files {
        tracing::debug!(
            "[Install] Cached file entry: Package '{}' - total={}, new={}, changed={}, removed={}, config={}",
            file_info.name,
            file_info.total_count,
            file_info.new_count,
            file_info.changed_count,
            file_info.removed_count,
            file_info.config_count
        );
    }
    app.install_list_files.clone()
}

/// What: Load cached services and check if services are loaded.
///
/// Inputs:
/// - `app`: Application state reference
///
/// Output:
/// - Tuple of (cached services, whether services are loaded)
///
/// Details:
/// - Checks both in-memory cache and cache file to determine loaded state
fn load_cached_services(app: &AppState) -> (Vec<crate::state::modal::ServiceImpact>, bool) {
    let cached_services = if !app.services_resolving && !app.install_list_services.is_empty() {
        tracing::debug!(
            "[Install] Using {} cached services",
            app.install_list_services.len()
        );
        app.install_list_services.clone()
    } else {
        tracing::debug!("[Install] No cached services available");
        Vec::new()
    };

    let services_cache_loaded = if app.install_list.is_empty() {
        false
    } else {
        let signature = crate::app::services_cache::compute_signature(&app.install_list);
        let loaded =
            crate::app::services_cache::load_cache(&app.services_cache_path, &signature).is_some();
        tracing::debug!(
            "[Install] Services cache check: {} (signature match: {})",
            if loaded { "found" } else { "not found" },
            signature.len()
        );
        loaded
    };

    let services_loaded = services_cache_loaded || !cached_services.is_empty();
    (cached_services, services_loaded)
}

/// What: Restore user restart decisions from pending service plan.
///
/// Inputs:
/// - `app`: Application state reference
/// - `services`: Mutable vector of service impacts to update
///
/// Output:
/// - No return value; modifies services in place
///
/// Details:
/// - Applies saved restart decisions from `pending_service_plan` to services
fn restore_service_decisions(app: &AppState, services: &mut [crate::state::modal::ServiceImpact]) {
    if app.pending_service_plan.is_empty() || services.is_empty() {
        return;
    }

    let decision_map: std::collections::HashMap<
        String,
        crate::state::modal::ServiceRestartDecision,
    > = app
        .pending_service_plan
        .iter()
        .map(|s| (s.unit_name.clone(), s.restart_decision))
        .collect();

    for service in services.iter_mut() {
        if let Some(&saved_decision) = decision_map.get(&service.unit_name) {
            service.restart_decision = saved_decision;
        }
    }
}

/// What: Load cached sandbox info and check if sandbox is loaded.
///
/// Inputs:
/// - `app`: Application state reference
/// - `items`: Package items for cache signature computation
///
/// Output:
/// - Tuple of (cached sandbox info, whether sandbox is loaded)
///
/// Details:
/// - Checks both in-memory cache and cache file to determine loaded state
fn load_cached_sandbox(
    app: &AppState,
    items: &[crate::state::types::PackageItem],
) -> (Vec<crate::logic::sandbox::SandboxInfo>, bool) {
    tracing::debug!(
        "[Install] Checking sandbox cache: resolving={}, install_list_sandbox={}, items={:?}",
        app.sandbox_resolving,
        app.install_list_sandbox.len(),
        items.iter().map(|i| &i.name).collect::<Vec<_>>()
    );

    let cached_sandbox = if !app.sandbox_resolving && !app.install_list_sandbox.is_empty() {
        tracing::debug!(
            "[Install] Using {} cached sandbox entries: {:?}",
            app.install_list_sandbox.len(),
            app.install_list_sandbox
                .iter()
                .map(|s| &s.package_name)
                .collect::<Vec<_>>()
        );
        app.install_list_sandbox.clone()
    } else {
        tracing::debug!(
            "[Install] No cached sandbox available (resolving={}, empty={})",
            app.sandbox_resolving,
            app.install_list_sandbox.is_empty()
        );
        Vec::new()
    };

    let sandbox_cache_loaded = if items.is_empty() {
        false
    } else {
        let signature = crate::app::sandbox_cache::compute_signature(items);
        let cache_result =
            crate::app::sandbox_cache::load_cache(&app.sandbox_cache_path, &signature);
        tracing::debug!(
            "[Install] Sandbox cache file check: signature={:?}, found={}, cached_entries={}",
            signature,
            cache_result.is_some(),
            cache_result.as_ref().map_or(0, Vec::len)
        );
        if let Some(ref cached) = cache_result {
            tracing::debug!(
                "[Install] Cached sandbox packages: {:?}",
                cached.iter().map(|s| &s.package_name).collect::<Vec<_>>()
            );
        }
        cache_result.is_some()
    };

    let sandbox_loaded = sandbox_cache_loaded || !cached_sandbox.is_empty();
    tracing::debug!(
        "[Install] Final sandbox state: cached_sandbox={}, sandbox_cache_loaded={}, sandbox_loaded={}",
        cached_sandbox.len(),
        sandbox_cache_loaded,
        sandbox_loaded
    );
    (cached_sandbox, sandbox_loaded)
}

/// What: Create minimal summary data for immediate display.
///
/// Inputs:
/// - `app`: Application state reference
/// - `items`: Package items to create summary for
///
/// Output:
/// - Tuple of (summary data, header chips)
///
/// Details:
/// - Creates minimal summary without blocking pacman calls
/// - Full summary computed asynchronously after modal opens
fn create_minimal_summary(
    app: &AppState,
    items: &[crate::state::types::PackageItem],
) -> (
    crate::state::modal::PreflightSummaryData,
    crate::state::modal::PreflightHeaderChips,
) {
    let aur_count = items
        .iter()
        .filter(|p| matches!(p.source, crate::state::Source::Aur))
        .count();

    tracing::debug!(
        "[Install] Creating minimal summary for {} packages ({} AUR)",
        items.len(),
        aur_count
    );

    let has_aur = aur_count > 0;
    let risk_score = if has_aur { 2 } else { 0 };
    let risk_level = if has_aur {
        crate::state::modal::RiskLevel::Medium
    } else {
        crate::state::modal::RiskLevel::Low
    };
    let aur_warning = if has_aur {
        vec![crate::i18n::t(
            app,
            "app.modals.preflight.summary.aur_packages_included",
        )]
    } else {
        vec![]
    };
    let aur_note = if has_aur {
        vec![crate::i18n::t(
            app,
            "app.modals.preflight.summary.aur_packages_present",
        )]
    } else {
        vec![]
    };

    let minimal_summary = crate::state::modal::PreflightSummaryData {
        packages: items
            .iter()
            .map(|item| crate::state::modal::PreflightPackageSummary {
                name: item.name.clone(),
                source: item.source.clone(),
                installed_version: None,
                target_version: item.version.clone(),
                is_downgrade: false,
                is_major_bump: false,
                download_bytes: None,
                install_delta_bytes: None,
                notes: vec![],
            })
            .collect(),
        package_count: items.len(),
        aur_count,
        download_bytes: 0,
        install_delta_bytes: 0,
        risk_score,
        risk_level,
        risk_reasons: aur_warning.clone(),
        major_bump_packages: vec![],
        core_system_updates: vec![],
        pacnew_candidates: 0,
        pacsave_candidates: 0,
        config_warning_packages: vec![],
        service_restart_units: vec![],
        summary_warnings: aur_warning,
        summary_notes: aur_note,
    };

    let minimal_header = crate::state::modal::PreflightHeaderChips {
        package_count: items.len(),
        download_bytes: 0,
        install_delta_bytes: 0,
        aur_count,
        risk_score,
        risk_level,
    };

    (minimal_summary, minimal_header)
}

/// What: Trigger background resolution for missing data.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Package items to resolve
/// - `dependency_info`: Current dependency info (empty triggers resolution)
/// - `cached_files`: Current file info (empty triggers resolution)
/// - `services_loaded`: Whether services are already loaded
/// - `cached_sandbox`: Current sandbox info (empty triggers resolution)
///
/// Output:
/// - No return value; sets resolution flags in app state
///
/// Details:
/// - Triggers background resolution for dependencies, files, services, and sandbox
/// - Only triggers if cache is empty or not loaded
fn trigger_background_resolution(
    app: &mut AppState,
    items: &[crate::state::types::PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    cached_files: &[crate::state::modal::PackageFileInfo],
    services_loaded: bool,
    cached_sandbox: &[crate::logic::sandbox::SandboxInfo],
) {
    if dependency_info.is_empty() {
        if app.deps_resolving {
            tracing::debug!(
                "[Preflight] NOT setting preflight_deps_resolving (global deps_resolving already in progress, will reuse result)"
            );
        } else {
            tracing::debug!(
                "[Preflight] Setting preflight_deps_resolving=true for {} items (cache empty)",
                items.len()
            );
            app.preflight_deps_items = Some((
                items.to_vec(),
                crate::state::modal::PreflightAction::Install,
            ));
            app.preflight_deps_resolving = true;
        }
    } else {
        tracing::debug!(
            "[Preflight] NOT setting preflight_deps_resolving (cache has {} deps)",
            dependency_info.len()
        );
    }
    if cached_files.is_empty() {
        if app.files_resolving {
            tracing::debug!(
                "[Preflight] NOT setting preflight_files_resolving (global files_resolving already in progress, will reuse result)"
            );
        } else {
            tracing::debug!(
                "[Preflight] Setting preflight_files_resolving=true for {} items (cache empty)",
                items.len()
            );
            app.preflight_files_items = Some(items.to_vec());
            app.preflight_files_resolving = true;
        }
    } else {
        tracing::debug!(
            "[Preflight] NOT setting preflight_files_resolving (cache has {} files)",
            cached_files.len()
        );
    }
    if services_loaded {
        tracing::debug!("[Preflight] NOT setting preflight_services_resolving (already loaded)");
    } else if app.services_resolving {
        tracing::debug!(
            "[Preflight] NOT setting preflight_services_resolving (global services_resolving already in progress, will reuse result)"
        );
    } else {
        tracing::debug!(
            "[Preflight] Setting preflight_services_resolving=true for {} items (not loaded)",
            items.len()
        );
        app.preflight_services_items = Some(items.to_vec());
        app.preflight_services_resolving = true;
    }
    if cached_sandbox.is_empty() {
        let aur_items: Vec<_> = items
            .iter()
            .filter(|p| matches!(p.source, crate::state::Source::Aur))
            .cloned()
            .collect();
        if aur_items.is_empty() {
            tracing::debug!("[Preflight] NOT setting preflight_sandbox_resolving (no AUR items)");
        } else if app.sandbox_resolving {
            tracing::debug!(
                "[Preflight] NOT setting preflight_sandbox_resolving (global sandbox_resolving already in progress, will reuse result)"
            );
        } else {
            tracing::debug!(
                "[Preflight] Setting preflight_sandbox_resolving=true for {} AUR items (cache empty)",
                aur_items.len()
            );
            app.preflight_sandbox_items = Some(aur_items);
            app.preflight_sandbox_resolving = true;
        }
    } else {
        tracing::debug!(
            "[Preflight] NOT setting preflight_sandbox_resolving (cache has {} sandbox entries)",
            cached_sandbox.len()
        );
    }
}

/// What: Open Preflight modal for install action with cached data.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - No return value; sets app.modal to Preflight with Install action
///
/// Details:
/// - Loads cached dependencies, files, services, and sandbox info
/// - Creates minimal summary for immediate display
/// - Triggers background resolution for missing data
pub fn open_preflight_install_modal(app: &mut AppState) {
    tracing::info!(
        "[Install] Opening preflight modal for {} packages",
        app.install_list.len()
    );
    let start_time = std::time::Instant::now();
    let item_count = app.install_list.len();
    let items = app.install_list.clone();

    let cache_start = std::time::Instant::now();
    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();

    let cached_deps = load_cached_dependencies(app, &item_names);
    let cached_files = load_cached_files(app, &items);
    let (cached_services, services_loaded) = load_cached_services(app);
    let (cached_sandbox, sandbox_loaded) = load_cached_sandbox(app, &items);

    tracing::debug!("[Install] Cache loading took {:?}", cache_start.elapsed());

    let mut final_services = cached_services;
    restore_service_decisions(app, &mut final_services);

    let summary_start = std::time::Instant::now();
    let (minimal_summary, minimal_header) = create_minimal_summary(app, &items);
    tracing::debug!(
        "[Install] Minimal summary creation took {:?}",
        summary_start.elapsed()
    );

    let modal_set_start = std::time::Instant::now();
    let dependency_info = if cached_deps.is_empty() {
        tracing::debug!(
            "[Preflight] Cache empty, will trigger background dependency resolution for {} packages",
            items.len()
        );
        Vec::new()
    } else {
        cached_deps
    };

    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    app.preflight_summary_items = Some((items.clone(), crate::state::PreflightAction::Install));
    // Don't set preflight_summary_resolving=true here - let the tick handler trigger it
    // This prevents the tick handler from being blocked by the flag already being set
    tracing::debug!(
        "[Preflight] Queued summary computation for {} items",
        items.len()
    );

    trigger_background_resolution(
        app,
        &items,
        &dependency_info,
        &cached_files,
        services_loaded,
        &cached_sandbox,
    );

    app.modal = crate::state::Modal::Preflight {
        items,
        action: crate::state::PreflightAction::Install,
        tab: crate::state::PreflightTab::Summary,
        summary: Some(Box::new(minimal_summary)),
        summary_scroll: 0,
        header_chips: minimal_header,
        dependency_info,
        dep_selected: 0,
        dep_tree_expanded: std::collections::HashSet::new(),
        deps_error: None,
        file_info: cached_files,
        file_selected: 0,
        file_tree_expanded: std::collections::HashSet::new(),
        files_error: None,
        service_info: final_services,
        service_selected: 0,
        services_loaded,
        services_error: None,
        sandbox_info: cached_sandbox,
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: app.remove_cascade_mode,
        cached_reverse_deps_report: None,
    };
    tracing::debug!(
        "[Install] Modal state set in {:?}",
        modal_set_start.elapsed()
    );
    tracing::info!(
        "[Install] Preflight modal opened successfully in {:?} ({} packages)",
        start_time.elapsed(),
        item_count
    );
    app.remove_preflight_summary.clear();
}

/// What: Open Preflight modal for remove action.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - No return value; sets app.modal to Preflight with Remove action
///
/// Details:
/// - Opens modal immediately without blocking. Reverse dependency resolution can be triggered
///   manually via the Dependencies tab or will be computed when user tries to proceed.
///   This avoids blocking the UI when opening the modal.
pub fn open_preflight_remove_modal(app: &mut AppState) {
    let items = app.remove_list.clone();
    let item_count = items.len();

    // Reset cancellation flag when opening modal
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    // Queue summary computation in background - modal will render with None initially
    app.preflight_summary_items = Some((items.clone(), crate::state::PreflightAction::Remove));
    // Don't set preflight_summary_resolving=true here - let the tick handler trigger it
    // This prevents the tick handler from being blocked by the flag already being set
    tracing::debug!(
        "[Preflight] Queued summary computation for {} items (remove)",
        items.len()
    );
    app.pending_service_plan.clear();

    // Open modal immediately with empty dependency_info to avoid blocking UI
    // Dependency info can be resolved later via Dependencies tab or when proceeding
    app.modal = crate::state::Modal::Preflight {
        items,
        action: crate::state::PreflightAction::Remove,
        tab: crate::state::PreflightTab::Summary,
        summary: None, // Will be populated when background computation completes
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips {
            package_count: item_count,
            download_bytes: 0,
            install_delta_bytes: 0,
            aur_count: 0,
            risk_score: 0,
            risk_level: crate::state::modal::RiskLevel::Low,
        },
        dependency_info: Vec::new(), // Will be populated when user switches to Deps tab or proceeds
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
    app.remove_preflight_summary = Vec::new(); // Will be populated when dependencies are resolved
    app.toast_message = Some(crate::i18n::t(app, "app.toasts.preflight_remove_list"));
}
