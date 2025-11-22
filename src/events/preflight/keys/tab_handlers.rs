//! Tab-specific handlers for Enter/Space key actions.

use std::collections::{HashMap, HashSet};

use crate::state::modal::ServiceRestartDecision;

use super::context::EnterOrSpaceContext;
use crate::events::preflight::display::build_file_display_items;

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
pub(super) fn handle_deps_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
    if ctx.dependency_info.is_empty() {
        return false;
    }

    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> = HashMap::new();
    for dep in ctx.dependency_info {
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
            for dep in pkg_deps {
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
pub(super) fn handle_files_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
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
pub(super) fn handle_sandbox_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
    if ctx.items.is_empty() {
        return false;
    }

    type SandboxDisplayItem = (bool, String, Option<(&'static str, String)>);
    let mut display_items: Vec<SandboxDisplayItem> = Vec::new();
    for item in ctx.items {
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
            let item = ctx
                .items
                .iter()
                .find(|p| p.name == *pkg_name)
                .expect("package should exist in items when present in display_items");
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
pub(super) fn handle_services_tab(ctx: &mut EnterOrSpaceContext<'_>) -> bool {
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
pub(super) fn handle_enter_or_space(ctx: EnterOrSpaceContext<'_>) -> bool {
    let mut ctx = ctx;
    match *ctx.tab {
        crate::state::PreflightTab::Deps => handle_deps_tab(&mut ctx),
        crate::state::PreflightTab::Files => handle_files_tab(&mut ctx),
        crate::state::PreflightTab::Sandbox => handle_sandbox_tab(&mut ctx),
        crate::state::PreflightTab::Services => handle_services_tab(&mut ctx),
        _ => true, // Default: close modal
    }
}
