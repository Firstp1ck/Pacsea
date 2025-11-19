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

/// What: Context struct grouping all Preflight modal state for key handling.
///
/// Details:
/// - Reduces function argument count and cognitive complexity.
/// - Contains all mutable references needed by key handlers.
/// - Note: `app` is passed separately to avoid borrow checker issues.
struct PreflightKeyContext<'a> {
    tab: &'a mut crate::state::PreflightTab,
    items: &'a [PackageItem],
    action: &'a crate::state::PreflightAction,
    dependency_info: &'a mut Vec<crate::state::modal::DependencyInfo>,
    dep_selected: &'a mut usize,
    dep_tree_expanded: &'a mut HashSet<String>,
    deps_error: &'a mut Option<String>,
    file_info: &'a mut Vec<crate::state::modal::PackageFileInfo>,
    file_selected: &'a mut usize,
    file_tree_expanded: &'a mut HashSet<String>,
    files_error: &'a mut Option<String>,
    service_info: &'a mut Vec<crate::state::modal::ServiceImpact>,
    service_selected: &'a mut usize,
    services_loaded: &'a mut bool,
    services_error: &'a mut Option<String>,
    sandbox_info: &'a mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_selected: &'a mut usize,
    sandbox_tree_expanded: &'a mut HashSet<String>,
    selected_optdepends:
        &'a mut std::collections::HashMap<String, std::collections::HashSet<String>>,
}

/// What: Handle Enter or Space key for Deps tab tree expansion/collapse.
///
/// Inputs:
/// - `ctx`: Context struct containing all necessary state references
///
/// Output:
/// - `true` if handled, `false` otherwise.
///
/// Details:
/// - Toggles expansion/collapse of dependency trees for selected package.
fn handle_deps_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
    if ctx.dependency_info.is_empty() {
        return false;
    }

    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> = HashMap::new();
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
    false
}

/// What: Handle Enter or Space key for Files tab tree expansion/collapse.
///
/// Inputs:
/// - `ctx`: Context struct containing all necessary state references
///
/// Output:
/// - `true` if handled, `false` otherwise.
///
/// Details:
/// - Toggles expansion/collapse of file trees for selected package.
fn handle_files_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
    let display_items = build_file_display_items(ctx.items, ctx.file_info, ctx.file_tree_expanded);
    if let Some((is_header, pkg_name)) = display_items.get(ctx.file_selected)
        && *is_header
    {
        if ctx.file_tree_expanded.contains(pkg_name) {
            ctx.file_tree_expanded.remove(pkg_name);
        } else {
            ctx.file_tree_expanded.insert(pkg_name.clone());
        }
    }
    false
}

/// What: Handle Enter or Space key for Sandbox tab tree expansion/collapse and optdepends selection.
///
/// Inputs:
/// - `ctx`: Context struct containing all necessary state references
///
/// Output:
/// - `true` if handled, `false` otherwise.
///
/// Details:
/// - Toggles expansion/collapse of sandbox dependency trees for selected package.
/// - Toggles optional dependency selection when on an optdepends entry.
fn handle_sandbox_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
    if ctx.items.is_empty() {
        return false;
    }

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
    false
}

/// What: Handle Enter or Space key for Services tab restart decision toggling.
///
/// Inputs:
/// - `ctx`: Context struct containing all necessary state references
///
/// Output:
/// - `true` if handled, `false` otherwise.
///
/// Details:
/// - Toggles restart decision for the selected service.
fn handle_services_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
    if ctx.service_info.is_empty() {
        return false;
    }

    let service_selected = ctx
        .service_selected
        .min(ctx.service_info.len().saturating_sub(1));
    if let Some(service) = ctx.service_info.get_mut(service_selected) {
        service.restart_decision = match service.restart_decision {
            ServiceRestartDecision::Restart => ServiceRestartDecision::Defer,
            ServiceRestartDecision::Defer => ServiceRestartDecision::Restart,
        };
    }
    false
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
    let mut ctx = ctx;
    match *ctx.tab {
        crate::state::PreflightTab::Deps => handle_deps_tab(&mut ctx),
        crate::state::PreflightTab::Files => handle_files_tab(&mut ctx),
        crate::state::PreflightTab::Sandbox => handle_sandbox_tab(&mut ctx),
        crate::state::PreflightTab::Services => handle_services_tab(&mut ctx),
        _ => true, // Default: close modal
    }
}

/// What: Handle Esc key - close the Preflight modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if modal should close, `false` otherwise.
fn handle_esc_key(app: &mut AppState) -> bool {
    let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
        service_info.clone()
    } else {
        Vec::new()
    };
    close_preflight_modal(app, &service_info);
    true
}

/// What: Handle Enter key - execute Enter/Space action.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if modal should close, `false` otherwise.
fn handle_enter_key(app: &mut AppState) -> bool {
    let should_close = if let crate::state::Modal::Preflight {
        tab,
        items,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        file_info,
        file_selected,
        file_tree_expanded,
        sandbox_info,
        sandbox_selected,
        sandbox_tree_expanded,
        selected_optdepends,
        service_info,
        service_selected,
        ..
    } = &mut app.modal
    {
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
        })
    } else {
        false
    };

    if should_close {
        let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
            service_info.clone()
        } else {
            Vec::new()
        };
        close_preflight_modal(app, &service_info);
        return true;
    }
    false
}

/// What: Handle Left/Right/Tab keys - switch tabs.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `direction`: Direction to switch (true for right/tab, false for left)
///
/// Output:
/// - Always returns `false`.
fn handle_tab_switch(app: &mut AppState, direction: bool) -> bool {
    let (new_tab, items, action) =
        if let crate::state::Modal::Preflight {
            tab, items, action, ..
        } = &app.modal
        {
            let current_tab = *tab;
            let next_tab = if direction {
                match current_tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                }
            } else {
                match current_tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Summary,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Services,
                }
            };
            (next_tab, items.clone(), *action)
        } else {
            return false;
        };

    if let crate::state::Modal::Preflight { tab, .. } = &mut app.modal {
        *tab = new_tab;
    }

    switch_preflight_tab(new_tab, app, &items, &action);
    false
}

/// What: Handle Up key - move selection up in current tab.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
fn handle_up_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Deps && !ctx.items.is_empty() {
        if *ctx.dep_selected > 0 {
            *ctx.dep_selected -= 1;
            tracing::debug!(
                "[Preflight] Deps Up: dep_selected={}, items={}",
                *ctx.dep_selected,
                ctx.items.len()
            );
        } else {
            tracing::debug!(
                "[Preflight] Deps Up: already at top (dep_selected=0), items={}",
                ctx.items.len()
            );
        }
    } else if *ctx.tab == crate::state::PreflightTab::Files
        && !ctx.file_info.is_empty()
        && *ctx.file_selected > 0
    {
        *ctx.file_selected -= 1;
    } else if *ctx.tab == crate::state::PreflightTab::Services
        && !ctx.service_info.is_empty()
        && *ctx.service_selected > 0
    {
        *ctx.service_selected -= 1;
    } else if *ctx.tab == crate::state::PreflightTab::Sandbox
        && !ctx.items.is_empty()
        && *ctx.sandbox_selected > 0
    {
        *ctx.sandbox_selected -= 1;
    }
    false
}

/// What: Handle Down key - move selection down in current tab.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
fn handle_down_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Deps && !ctx.items.is_empty() {
        let display_len =
            compute_display_items_len(ctx.items, ctx.dependency_info, ctx.dep_tree_expanded);
        tracing::debug!(
            "[Preflight] Deps Down: dep_selected={}, display_len={}, items={}, deps={}, expanded_count={}",
            *ctx.dep_selected,
            display_len,
            ctx.items.len(),
            ctx.dependency_info.len(),
            ctx.dep_tree_expanded.len()
        );
        if *ctx.dep_selected < display_len.saturating_sub(1) {
            *ctx.dep_selected += 1;
            tracing::debug!(
                "[Preflight] Deps Down: moved to dep_selected={}",
                *ctx.dep_selected
            );
        } else {
            tracing::debug!(
                "[Preflight] Deps Down: already at bottom (dep_selected={}, display_len={})",
                *ctx.dep_selected,
                display_len
            );
        }
    } else if *ctx.tab == crate::state::PreflightTab::Files {
        let display_len =
            compute_file_display_items_len(ctx.items, ctx.file_info, ctx.file_tree_expanded);
        if *ctx.file_selected < display_len.saturating_sub(1) {
            *ctx.file_selected += 1;
        }
    } else if *ctx.tab == crate::state::PreflightTab::Services && !ctx.service_info.is_empty() {
        let max_index = ctx.service_info.len().saturating_sub(1);
        if *ctx.service_selected < max_index {
            *ctx.service_selected += 1;
        }
    } else if *ctx.tab == crate::state::PreflightTab::Sandbox && !ctx.items.is_empty() {
        let display_len = compute_sandbox_display_items_len(
            ctx.items,
            ctx.sandbox_info,
            ctx.sandbox_tree_expanded,
        );
        if *ctx.sandbox_selected < display_len.saturating_sub(1) {
            *ctx.sandbox_selected += 1;
        }
    }
    false
}

/// What: Handle Space key - toggle expand/collapse.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
fn handle_space_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    handle_enter_or_space(EnterOrSpaceContext {
        tab: ctx.tab,
        items: ctx.items,
        dependency_info: ctx.dependency_info,
        dep_selected: *ctx.dep_selected,
        dep_tree_expanded: ctx.dep_tree_expanded,
        file_info: ctx.file_info,
        file_selected: *ctx.file_selected,
        file_tree_expanded: ctx.file_tree_expanded,
        sandbox_info: ctx.sandbox_info,
        sandbox_selected: *ctx.sandbox_selected,
        sandbox_tree_expanded: ctx.sandbox_tree_expanded,
        selected_optdepends: ctx.selected_optdepends,
        service_info: ctx.service_info,
        service_selected: *ctx.service_selected,
    });
    false
}

/// What: Handle Shift+R key - re-run all analyses.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_shift_r_key(app: &mut AppState) -> bool {
    tracing::info!("Shift+R pressed: Re-running all preflight analyses");

    let (items, action) = if let crate::state::Modal::Preflight { items, action, .. } = &app.modal {
        (items.clone(), *action)
    } else {
        return false;
    };

    // Clear all cached data in the modal
    if let crate::state::Modal::Preflight {
        dependency_info,
        deps_error,
        file_info,
        files_error,
        service_info,
        services_error,
        services_loaded,
        sandbox_info,
        sandbox_error,
        sandbox_loaded,
        summary,
        dep_selected,
        file_selected,
        service_selected,
        sandbox_selected,
        dep_tree_expanded,
        file_tree_expanded,
        sandbox_tree_expanded,
        ..
    } = &mut app.modal
    {
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

        *dep_selected = 0;
        *file_selected = 0;
        *service_selected = 0;
        *sandbox_selected = 0;

        dep_tree_expanded.clear();
        file_tree_expanded.clear();
        sandbox_tree_expanded.clear();
    }

    // Reset cancellation flag
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);

    // Queue all stages for background resolution (same as opening modal)
    app.preflight_summary_items = Some((items.clone(), action));
    app.preflight_summary_resolving = true;

    if matches!(action, crate::state::PreflightAction::Install) {
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
            if let crate::state::Modal::Preflight { sandbox_loaded, .. } = &mut app.modal {
                *sandbox_loaded = true;
            }
        }
    }

    app.toast_message = Some("Re-running all preflight analyses...".to_string());
    app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    false
}

/// What: Handle regular R key - retry resolution for current tab.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
fn handle_r_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Services && !ctx.service_info.is_empty() {
        // Toggle restart decision for selected service (only if no error)
        if *ctx.service_selected >= ctx.service_info.len() {
            *ctx.service_selected = ctx.service_info.len().saturating_sub(1);
        }
        if let Some(service) = ctx.service_info.get_mut(*ctx.service_selected) {
            service.restart_decision = ServiceRestartDecision::Restart;
        }
    } else if *ctx.tab == crate::state::PreflightTab::Deps
        && matches!(*ctx.action, crate::state::PreflightAction::Install)
    {
        // Retry dependency resolution
        *ctx.deps_error = None;
        *ctx.dependency_info = crate::logic::deps::resolve_dependencies(ctx.items);
        *ctx.dep_selected = 0;
    } else if *ctx.tab == crate::state::PreflightTab::Files {
        // Retry file resolution
        *ctx.files_error = None;
        *ctx.file_info = crate::logic::files::resolve_file_changes(ctx.items, *ctx.action);
        *ctx.file_selected = 0;
    } else if *ctx.tab == crate::state::PreflightTab::Services {
        // Retry service resolution
        *ctx.services_error = None;
        *ctx.services_loaded = false;
        *ctx.service_info = crate::logic::services::resolve_service_impacts(ctx.items, *ctx.action);
        *ctx.service_selected = 0;
        *ctx.services_loaded = true;
    }
    false
}

/// What: Handle D key - set service restart decision to Defer.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
fn handle_d_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Services && !ctx.service_info.is_empty() {
        if *ctx.service_selected >= ctx.service_info.len() {
            *ctx.service_selected = ctx.service_info.len().saturating_sub(1);
        }
        if let Some(service) = ctx.service_info.get_mut(*ctx.service_selected) {
            service.restart_decision = ServiceRestartDecision::Defer;
        }
    }
    false
}

/// What: Handle A key - expand/collapse all package groups.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
fn handle_a_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Deps && !ctx.dependency_info.is_empty() {
        let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
            HashMap::new();
        for dep in ctx.dependency_info.iter() {
            for req_by in &dep.required_by {
                grouped.entry(req_by.clone()).or_default().push(dep);
            }
        }

        let all_expanded = ctx
            .items
            .iter()
            .all(|p| ctx.dep_tree_expanded.contains(&p.name));
        if all_expanded {
            // Collapse all
            ctx.dep_tree_expanded.clear();
        } else {
            // Expand all packages (even if they have no dependencies)
            for pkg_name in ctx.items.iter().map(|p| &p.name) {
                ctx.dep_tree_expanded.insert(pkg_name.clone());
            }
        }
    } else if *ctx.tab == crate::state::PreflightTab::Files && !ctx.file_info.is_empty() {
        // Expand/collapse all packages in Files tab
        let all_expanded = ctx
            .file_info
            .iter()
            .filter(|p| !p.files.is_empty())
            .all(|p| ctx.file_tree_expanded.contains(&p.name));
        if all_expanded {
            // Collapse all
            ctx.file_tree_expanded.clear();
        } else {
            // Expand all
            for pkg_info in ctx.file_info.iter() {
                if !pkg_info.files.is_empty() {
                    ctx.file_tree_expanded.insert(pkg_info.name.clone());
                }
            }
        }
    }
    false
}

/// What: Handle F key - sync file database.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if handled (should return early), `false` otherwise.
fn handle_f_key(app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight { tab, .. } = &app.modal {
        if *tab != crate::state::PreflightTab::Files {
            return false;
        }
    } else {
        return false;
    }

    // Use the new ensure_file_db_synced function with force=true
    // This will attempt to sync regardless of timestamp
    let sync_result = crate::logic::files::ensure_file_db_synced(true, 7);
    match sync_result {
        Ok(synced) => {
            if synced {
                app.toast_message =
                    Some("File database sync completed. Files tab will refresh.".to_string());
            } else {
                app.toast_message = Some("File database is already fresh.".to_string());
            }
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            // Clear file_info to trigger re-resolution after sync completes
            if let crate::state::Modal::Preflight {
                file_info,
                file_selected,
                ..
            } = &mut app.modal
            {
                *file_info = Vec::new();
                *file_selected = 0;
            }
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
            if let crate::state::Modal::Preflight {
                file_info,
                file_selected,
                ..
            } = &mut app.modal
            {
                *file_info = Vec::new();
                *file_selected = 0;
            }
        }
    }
    true
}

/// What: Handle S key - open scan configuration modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_s_key(app: &mut AppState) -> bool {
    // Build AUR package name list to scan
    let names = if let crate::state::Modal::Preflight { items, .. } = &app.modal {
        items
            .iter()
            .filter(|p| matches!(p.source, crate::state::Source::Aur))
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
    } else {
        return false;
    };

    if names.is_empty() {
        app.modal = crate::state::Modal::Alert {
            message: "No AUR packages selected to scan.\nAdd AUR packages to scan, then press 's'."
                .into(),
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
    false
}

/// What: Handle d key - toggle dry-run mode.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_dry_run_key(app: &mut AppState) -> bool {
    app.dry_run = !app.dry_run;
    let toast_key = if app.dry_run {
        "app.toasts.dry_run_enabled"
    } else {
        "app.toasts.dry_run_disabled"
    };
    app.toast_message = Some(crate::i18n::t(app, toast_key));
    false
}

/// What: Handle m key - cycle cascade mode.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_m_key(app: &mut AppState) -> bool {
    let mut next_mode_opt = None;
    if let crate::state::Modal::Preflight {
        action: crate::state::PreflightAction::Remove,
        cascade_mode,
        ..
    } = &mut app.modal
    {
        let next_mode = cascade_mode.next();
        *cascade_mode = next_mode;
        next_mode_opt = Some(next_mode);
    }

    if let Some(next_mode) = next_mode_opt {
        app.remove_cascade_mode = next_mode;
        app.toast_message = Some(format!(
            "Cascade mode set to {} ({})",
            next_mode.flag(),
            next_mode.description()
        ));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
    }
    false
}

/// What: Handle p key - proceed with install/remove.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if modal should close, `false` otherwise.
fn handle_p_key(app: &mut AppState) -> bool {
    let mut close_modal = false;
    let mut new_summary: Option<Vec<crate::state::modal::ReverseRootSummary>> = None;
    let mut blocked_dep_count: Option<usize> = None;
    let mut removal_names: Option<Vec<String>> = None;
    let mut removal_mode: Option<crate::state::modal::CascadeMode> = None;
    let mut install_targets: Option<Vec<PackageItem>> = None;
    let mut service_info_for_plan: Option<Vec<crate::state::modal::ServiceImpact>> = None;

    // Scope for borrowing app.modal
    {
        if let crate::state::Modal::Preflight {
            action,
            items,
            dependency_info,
            cascade_mode,
            selected_optdepends,
            service_info,
            ..
        } = &mut app.modal
        {
            match action {
                crate::state::PreflightAction::Install => {
                    let mut packages = items.to_vec();
                    // Add selected optional dependencies as additional packages to install
                    for (_pkg_name, optdeps) in selected_optdepends.iter() {
                        for optdep in optdeps {
                            let optdep_pkg_name =
                                crate::logic::sandbox::extract_package_name(optdep);
                            if !packages.iter().any(|p| p.name == optdep_pkg_name) {
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
                    install_targets = Some(packages);
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

            if !service_info.is_empty() {
                service_info_for_plan = Some(service_info.clone());
            }
        }
    }

    if let Some(summary) = new_summary {
        app.remove_preflight_summary = summary;
    }

    if let Some(plan) = service_info_for_plan {
        app.pending_service_plan = plan;
    } else {
        app.pending_service_plan.clear();
    }

    if let Some(packages) = install_targets {
        crate::install::spawn_install_all(&packages, app.dry_run);
        close_modal = true;
    } else if let Some(names) = removal_names {
        let mode = removal_mode.unwrap_or(crate::state::modal::CascadeMode::Basic);
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
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(6));
    }

    if close_modal {
        let service_info_clone =
            if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
                service_info.clone()
            } else {
                Vec::new()
            };
        close_preflight_modal(app, &service_info_clone);
        return true;
    }
    false
}

/// What: Handle c key - snapshot placeholder.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_c_key(app: &mut AppState) -> bool {
    app.toast_message = Some(crate::i18n::t(app, "app.toasts.snapshot_placeholder"));
    false
}

/// What: Handle q key - close modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if modal should close, `false` otherwise.
fn handle_q_key(app: &mut AppState) -> bool {
    let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
        service_info.clone()
    } else {
        Vec::new()
    };
    close_preflight_modal(app, &service_info);
    true
}

/// What: Handle ? key - show help.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_help_key(app: &mut AppState) -> bool {
    let help_message = if let crate::state::Modal::Preflight { tab, .. } = &app.modal {
        if *tab == crate::state::PreflightTab::Deps {
            crate::i18n::t(app, "app.modals.preflight.help.deps_tab")
        } else {
            crate::i18n::t(app, "app.modals.preflight.help.general")
        }
    } else {
        return false;
    };

    app.previous_modal = Some(app.modal.clone());
    app.modal = crate::state::Modal::Alert {
        message: help_message,
    };
    false
}

/// What: Handle keys that need access to app fields outside of modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_keys_needing_app(ke: KeyEvent, app: &mut AppState) -> bool {
    match ke.code {
        KeyCode::Esc => handle_esc_key(app),
        KeyCode::Enter => handle_enter_key(app),
        KeyCode::Left => handle_tab_switch(app, false),
        KeyCode::Right => handle_tab_switch(app, true),
        KeyCode::Tab => handle_tab_switch(app, true),
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if ke.modifiers.contains(KeyModifiers::SHIFT) {
                handle_shift_r_key(app)
            } else {
                false // Handled in first block
            }
        }
        KeyCode::Char('f') | KeyCode::Char('F') => handle_f_key(app),
        KeyCode::Char('s') | KeyCode::Char('S') => handle_s_key(app),
        KeyCode::Char('d') => handle_dry_run_key(app),
        KeyCode::Char('m') => handle_m_key(app),
        KeyCode::Char('p') => handle_p_key(app),
        KeyCode::Char('c') => handle_c_key(app),
        KeyCode::Char('q') => handle_q_key(app),
        KeyCode::Char('?') => handle_help_key(app),
        _ => false,
    }
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
    // First, handle keys that only need ctx (no app access required)
    // This avoids borrow checker conflicts
    {
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
            selected_optdepends,
            ..
        } = &mut app.modal
        {
            let mut ctx = PreflightKeyContext {
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
                selected_optdepends,
            };

            match ke.code {
                KeyCode::Up => {
                    handle_up_key(&mut ctx);
                    return false;
                }
                KeyCode::Down => {
                    handle_down_key(&mut ctx);
                    return false;
                }
                KeyCode::Char(' ') => {
                    handle_space_key(&mut ctx);
                    return false;
                }
                KeyCode::Char('D') => {
                    handle_d_key(&mut ctx);
                    return false;
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    handle_a_key(&mut ctx);
                    return false;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if !ke.modifiers.contains(KeyModifiers::SHIFT) {
                        handle_r_key(&mut ctx);
                        return false;
                    }
                    // Shift+R needs app, fall through
                }
                _ => {
                    // Keys that need app access, fall through
                }
            }
        }
        false
    };

    // Now handle keys that need app access
    // The borrow of app.modal has been released, so we can mutably borrow app again
    handle_keys_needing_app(ke, app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PackageItem;
    use crate::state::modal::DependencyInfo;
    use std::collections::{HashMap, HashSet};

    // Helper to create dummy items
    fn make_item(name: &str) -> PackageItem {
        PackageItem {
            name: name.to_string(),
            version: "1.0".to_string(),
            description: "desc".to_string(),
            source: crate::state::Source::Official {
                repo: "core".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        }
    }

    #[test]
    fn test_handle_deps_tab_toggle() {
        // Setup
        let items = vec![make_item("pkg1")];
        let deps = vec![DependencyInfo {
            name: "dep1".to_string(),
            version: "1.0".to_string(),
            status: crate::state::modal::DependencyStatus::ToInstall,
            source: crate::state::modal::DependencySource::Official {
                repo: "core".into(),
            },
            required_by: vec!["pkg1".to_string()],
            depends_on: vec![],
            is_core: false,
            is_system: false,
        }];
        let mut expanded = HashSet::new();
        let mut selected_optdepends = HashMap::new();
        let mut service_info = Vec::new();

        let mut ctx = EnterOrSpaceContext {
            tab: &crate::state::PreflightTab::Deps,
            items: &items,
            dependency_info: &deps,
            dep_selected: 0, // "pkg1" header
            dep_tree_expanded: &mut expanded,
            file_info: &[],
            file_selected: 0,
            file_tree_expanded: &mut HashSet::new(),
            sandbox_info: &[],
            sandbox_selected: 0,
            sandbox_tree_expanded: &mut HashSet::new(),
            selected_optdepends: &mut selected_optdepends,
            service_info: &mut service_info,
            service_selected: 0,
        };

        // Act: Expand pkg1
        handle_deps_tab(&mut ctx);
        assert!(ctx.dep_tree_expanded.contains("pkg1"));

        // Act: Collapse pkg1
        handle_deps_tab(&mut ctx);
        assert!(!ctx.dep_tree_expanded.contains("pkg1"));
    }
}
