//! Action key handlers for Preflight modal.

use std::collections::HashMap;

use crate::state::AppState;
use crate::state::modal::ServiceRestartDecision;

use super::context::{EnterOrSpaceContext, PreflightKeyContext};
use super::tab_handlers::handle_enter_or_space;
use crate::events::preflight::modal::close_preflight_modal;

/// What: Handle Esc key - close the Preflight modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if modal should close, `false` otherwise.
pub(crate) fn handle_esc_key(app: &mut AppState) -> bool {
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
pub(crate) fn handle_enter_key(app: &mut AppState) -> bool {
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

/// What: Handle Space key - toggle expand/collapse.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
pub(crate) fn handle_space_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
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
pub(crate) fn handle_shift_r_key(app: &mut AppState) -> bool {
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
pub(crate) fn handle_r_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
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
pub(crate) fn handle_d_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
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
pub(crate) fn handle_a_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
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
