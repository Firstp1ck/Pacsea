//! Key event handling for Preflight modal.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};

use crate::state::modal::ServiceRestartDecision;
use crate::state::{AppState, PackageItem};

use super::display::{
    build_file_display_items, compute_display_items_len, compute_file_display_items_len,
    compute_sandbox_display_items_len,
};
use super::modal::{close_preflight_modal, switch_preflight_tab};

/// What: Context struct grouping parameters for Enter/Space key handling.
///
/// Details:
/// - Reduces function argument count to avoid clippy warnings.
struct EnterOrSpaceContext<'a> {
    tab: &'a crate::state::PreflightTab,
    items: &'a [PackageItem],
    dependency_info: &'a [crate::state::modal::DependencyInfo],
    dep_selected: usize,
    dep_tree_expanded: &'a mut HashSet<String>,
    file_info: &'a [crate::state::modal::PackageFileInfo],
    file_selected: usize,
    file_tree_expanded: &'a mut HashSet<String>,
    sandbox_info: &'a [crate::logic::sandbox::SandboxInfo],
    sandbox_selected: usize,
    sandbox_tree_expanded: &'a mut HashSet<String>,
    selected_optdepends:
        &'a mut std::collections::HashMap<String, std::collections::HashSet<String>>,
    service_info: &'a mut [crate::state::modal::ServiceImpact],
    service_selected: usize,
}

/// What: Handle Enter or Space key for tree expansion/collapse in various tabs.
///
/// Inputs:
/// - `ctx`: Context struct containing all necessary state references
///
/// Output:
/// - `true` if the key was handled (should close modal), `false` otherwise.
///
/// Details:
/// - Handles expansion/collapse logic for Deps, Files, and Sandbox tabs.
/// - Handles service restart decision toggling in Services tab.
fn handle_enter_or_space(ctx: EnterOrSpaceContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Deps && !ctx.dependency_info.is_empty() {
        let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
            HashMap::new();
        for dep in ctx.dependency_info.iter() {
            for req_by in &dep.required_by {
                grouped.entry(req_by.clone()).or_default().push(dep);
            }
        }

        let mut display_items: Vec<(bool, String)> = Vec::new();
        for pkg_name in ctx.items.iter().map(|p| &p.name) {
            display_items.push((true, pkg_name.clone()));
            if ctx.dep_tree_expanded.contains(pkg_name)
                && let Some(pkg_deps) = grouped.get(pkg_name)
            {
                let mut seen_deps = HashSet::new();
                for dep in pkg_deps.iter() {
                    if seen_deps.insert(dep.name.as_str()) {
                        display_items.push((false, String::new()));
                    }
                }
            }
        }

        if let Some((is_header, pkg_name)) = display_items.get(ctx.dep_selected)
            && *is_header
        {
            if ctx.dep_tree_expanded.contains(pkg_name) {
                ctx.dep_tree_expanded.remove(pkg_name);
            } else {
                ctx.dep_tree_expanded.insert(pkg_name.clone());
            }
        }
        return false;
    }

    if *ctx.tab == crate::state::PreflightTab::Files {
        let display_items =
            build_file_display_items(ctx.items, ctx.file_info, ctx.file_tree_expanded);
        if let Some((is_header, pkg_name)) = display_items.get(ctx.file_selected)
            && *is_header
        {
            if ctx.file_tree_expanded.contains(pkg_name) {
                ctx.file_tree_expanded.remove(pkg_name);
            } else {
                ctx.file_tree_expanded.insert(pkg_name.clone());
            }
        }
        return false;
    }

    if *ctx.tab == crate::state::PreflightTab::Sandbox && !ctx.items.is_empty() {
        type SandboxDisplayItem = (bool, String, Option<(&'static str, String)>);
        let mut display_items: Vec<SandboxDisplayItem> = Vec::new();
        for item in ctx.items.iter() {
            let is_aur = matches!(item.source, crate::state::Source::Aur);
            display_items.push((true, item.name.clone(), None));
            if is_aur
                && ctx.sandbox_tree_expanded.contains(&item.name)
                && let Some(info) = ctx
                    .sandbox_info
                    .iter()
                    .find(|s| s.package_name == item.name)
            {
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

        if let Some((is_header, pkg_name, dep_opt)) = display_items.get(ctx.sandbox_selected) {
            if *is_header {
                let item = ctx.items.iter().find(|p| p.name == *pkg_name).unwrap();
                if matches!(item.source, crate::state::Source::Aur) {
                    if ctx.sandbox_tree_expanded.contains(pkg_name) {
                        ctx.sandbox_tree_expanded.remove(pkg_name);
                    } else {
                        ctx.sandbox_tree_expanded.insert(pkg_name.clone());
                    }
                }
            } else if let Some((dep_type, dep_name)) = dep_opt
                && *dep_type == "optdepends"
            {
                let selected_set = ctx.selected_optdepends.entry(pkg_name.clone()).or_default();
                let pkg_name_from_dep = crate::logic::sandbox::extract_package_name(dep_name);
                if selected_set.contains(dep_name) || selected_set.contains(&pkg_name_from_dep) {
                    selected_set.remove(dep_name);
                    selected_set.remove(&pkg_name_from_dep);
                } else {
                    selected_set.insert(dep_name.clone());
                }
            }
        }
        return false;
    }

    if *ctx.tab == crate::state::PreflightTab::Services && !ctx.service_info.is_empty() {
        let service_selected = ctx
            .service_selected
            .min(ctx.service_info.len().saturating_sub(1));
        if let Some(service) = ctx.service_info.get_mut(service_selected) {
            service.restart_decision = match service.restart_decision {
                ServiceRestartDecision::Restart => ServiceRestartDecision::Defer,
                ServiceRestartDecision::Defer => ServiceRestartDecision::Restart,
            };
        }
        return false;
    }

    // Default: close modal
    true
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
        summary,
        summary_scroll: _,
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
        sandbox_loaded,
        sandbox_error,
        selected_optdepends,
        cascade_mode,
        ..
    } = &mut app.modal
    {
        match ke.code {
            KeyCode::Esc => {
                let service_info_clone = service_info.clone();
                close_preflight_modal(app, &service_info_clone);
                return false;
            }
            KeyCode::Enter => {
                let should_close = handle_enter_or_space(EnterOrSpaceContext {
                    tab,
                    items,
                    dependency_info,
                    dep_selected: *dep_selected,
                    dep_tree_expanded,
                    file_info,
                    file_selected: *file_selected,
                    file_tree_expanded,
                    sandbox_info,
                    sandbox_selected: *sandbox_selected,
                    sandbox_tree_expanded,
                    selected_optdepends,
                    service_info,
                    service_selected: *service_selected,
                });
                if should_close {
                    let service_info_clone = service_info.clone();
                    close_preflight_modal(app, &service_info_clone);
                    return false;
                }
            }
            KeyCode::Left => {
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Summary,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Services,
                };
                let new_tab = *tab;
                let items_clone = items.clone();
                let action_clone = *action;
                // Temporarily release the borrow to call switch_preflight_tab
                let _ = service_info;
                let _ = dependency_info;
                let _ = file_info;
                let _ = sandbox_info;
                switch_preflight_tab(new_tab, app, &items_clone, &action_clone);
            }
            KeyCode::Right => {
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                };
                let new_tab = *tab;
                let items_clone = items.clone();
                let action_clone = *action;
                // Temporarily release the borrow to call switch_preflight_tab
                let _ = service_info;
                let _ = dependency_info;
                let _ = file_info;
                let _ = sandbox_info;
                switch_preflight_tab(new_tab, app, &items_clone, &action_clone);
            }
            KeyCode::Tab => {
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                };
                let new_tab = *tab;
                let items_clone = items.clone();
                let action_clone = *action;
                // Temporarily release the borrow to call switch_preflight_tab
                let _ = service_info;
                let _ = dependency_info;
                let _ = file_info;
                let _ = sandbox_info;
                switch_preflight_tab(new_tab, app, &items_clone, &action_clone);
            }
            KeyCode::Up => {
                if *tab == crate::state::PreflightTab::Deps && !items.is_empty() {
                    if *dep_selected > 0 {
                        *dep_selected -= 1;
                        tracing::debug!(
                            "[Preflight] Deps Up: dep_selected={}, items={}",
                            *dep_selected,
                            items.len()
                        );
                    } else {
                        tracing::debug!(
                            "[Preflight] Deps Up: already at top (dep_selected=0), items={}",
                            items.len()
                        );
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
                if *tab == crate::state::PreflightTab::Deps && !items.is_empty() {
                    let display_len =
                        compute_display_items_len(items, dependency_info, dep_tree_expanded);
                    tracing::debug!(
                        "[Preflight] Deps Down: dep_selected={}, display_len={}, items={}, deps={}, expanded_count={}",
                        *dep_selected,
                        display_len,
                        items.len(),
                        dependency_info.len(),
                        dep_tree_expanded.len()
                    );
                    if *dep_selected < display_len.saturating_sub(1) {
                        *dep_selected += 1;
                        tracing::debug!(
                            "[Preflight] Deps Down: moved to dep_selected={}",
                            *dep_selected
                        );
                    } else {
                        tracing::debug!(
                            "[Preflight] Deps Down: already at bottom (dep_selected={}, display_len={})",
                            *dep_selected,
                            display_len
                        );
                    }
                } else if *tab == crate::state::PreflightTab::Files {
                    let display_len =
                        compute_file_display_items_len(items, file_info, file_tree_expanded);
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
                handle_enter_or_space(EnterOrSpaceContext {
                    tab,
                    items,
                    dependency_info,
                    dep_selected: *dep_selected,
                    dep_tree_expanded,
                    file_info,
                    file_selected: *file_selected,
                    file_tree_expanded,
                    sandbox_info,
                    sandbox_selected: *sandbox_selected,
                    sandbox_tree_expanded,
                    selected_optdepends,
                    service_info,
                    service_selected: *service_selected,
                });
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Shift+R: Re-run all analyses (clear cache and re-queue all stages)
                if ke.modifiers.contains(KeyModifiers::SHIFT) {
                    tracing::info!("Shift+R pressed: Re-running all preflight analyses");

                    // Clear all cached data in the modal
                    *dependency_info = Vec::new();
                    *deps_error = None;
                    *file_info = Vec::new();
                    *files_error = None;
                    *service_info = Vec::new();
                    *services_error = None;
                    *services_loaded = false;
                    *sandbox_info = Vec::new();
                    *sandbox_error = None;
                    *sandbox_loaded = false;
                    *summary = None;

                    // Reset selection indices
                    *dep_selected = 0;
                    *file_selected = 0;
                    *service_selected = 0;
                    *sandbox_selected = 0;

                    // Clear expanded trees
                    dep_tree_expanded.clear();
                    file_tree_expanded.clear();
                    sandbox_tree_expanded.clear();

                    // Reset cancellation flag
                    app.preflight_cancelled
                        .store(false, std::sync::atomic::Ordering::Relaxed);

                    // Queue all stages for background resolution (same as opening modal)
                    app.preflight_summary_items = Some((items.clone(), *action));
                    app.preflight_summary_resolving = true;

                    if matches!(*action, crate::state::PreflightAction::Install) {
                        app.preflight_deps_items = Some(items.clone());
                        app.preflight_deps_resolving = true;

                        app.preflight_files_items = Some(items.clone());
                        app.preflight_files_resolving = true;

                        app.preflight_services_items = Some(items.clone());
                        app.preflight_services_resolving = true;

                        // Only queue sandbox for AUR packages
                        let aur_items: Vec<_> = items
                            .iter()
                            .filter(|p| matches!(p.source, crate::state::Source::Aur))
                            .cloned()
                            .collect();
                        if !aur_items.is_empty() {
                            app.preflight_sandbox_items = Some(aur_items);
                            app.preflight_sandbox_resolving = true;
                        } else {
                            app.preflight_sandbox_items = None;
                            app.preflight_sandbox_resolving = false;
                            *sandbox_loaded = true; // No AUR packages, mark as loaded
                        }
                    }

                    app.toast_message = Some("Re-running all preflight analyses...".to_string());
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                } else {
                    // Regular 'r': Retry resolution for current tab or toggle service restart decision
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
                        *service_info =
                            crate::logic::services::resolve_service_impacts(items, *action);
                        *service_selected = 0;
                        *services_loaded = true;
                    }
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
                        // Expand all packages (even if they have no dependencies)
                        for pkg_name in items.iter().map(|p| &p.name) {
                            dep_tree_expanded.insert(pkg_name.clone());
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
                let toast_key = if app.dry_run {
                    "app.toasts.dry_run_enabled"
                } else {
                    "app.toasts.dry_run_disabled"
                };
                app.toast_message = Some(crate::i18n::t(app, toast_key));
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
                    let service_info_clone = service_info.clone();
                    close_preflight_modal(app, &service_info_clone);
                    return false;
                }
            }
            KeyCode::Char('c') => {
                // Snapshot placeholder
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.snapshot_placeholder"));
            }
            KeyCode::Char('q') => {
                let service_info_clone = service_info.clone();
                close_preflight_modal(app, &service_info_clone);
                return false;
            }
            KeyCode::Char('?') => {
                // Show Deps tab help when on Deps tab, otherwise show general Preflight help
                let help_message = if *tab == crate::state::PreflightTab::Deps {
                    crate::i18n::t(app, "app.modals.preflight.help.deps_tab")
                } else {
                    crate::i18n::t(app, "app.modals.preflight.help.general")
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
