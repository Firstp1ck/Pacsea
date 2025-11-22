//! Preflight resolution status utilities.
//!
//! This module provides functions for checking preflight resolution status of packages.

use crate::state::AppState;

/// What: Check if a package is currently being processed by any preflight resolver.
///
/// Inputs:
/// - `app`: Application state containing preflight resolution queues and flags.
/// - `package_name`: Name of the package to check.
///
/// Output:
/// - `true` if the package is in any preflight resolution queue and the corresponding resolver is active.
///
/// Details:
/// - Checks if the package name appears in any of the preflight queues (summary, deps, files, services, sandbox)
///   and if the corresponding resolving flag is set to true.
/// - Also checks install list resolution (when preflight modal is not open) by checking if the package
///   is in `app.install_list` and any resolver is active.
#[must_use]
pub fn is_package_loading_preflight(app: &AppState, package_name: &str) -> bool {
    // Check summary resolution (preflight-specific)
    if app.preflight_summary_resolving
        && let Some((ref items, _)) = app.preflight_summary_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check dependency resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_deps_resolving
        && let Some(ref items) = app.preflight_deps_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Only show indicator if deps are actually resolving AND package is in install list
    if app.deps_resolving
        && !app.preflight_deps_resolving
        && app.install_list.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check file resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_files_resolving
        && let Some(ref items) = app.preflight_files_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Only show indicator if files are actually resolving AND preflight is not resolving
    if app.files_resolving
        && !app.preflight_files_resolving
        && app.install_list.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check service resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_services_resolving
        && let Some(ref items) = app.preflight_services_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Only show indicator if services are actually resolving AND preflight is not resolving
    if app.services_resolving
        && !app.preflight_services_resolving
        && app.install_list.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check sandbox resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_sandbox_resolving
        && let Some(ref items) = app.preflight_sandbox_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Note: sandbox only applies to AUR packages
    // Only show indicator if sandbox is actually resolving AND preflight is not resolving
    if app.sandbox_resolving
        && !app.preflight_sandbox_resolving
        && app
            .install_list
            .iter()
            .any(|p| p.name == package_name && matches!(p.source, crate::state::Source::Aur))
    {
        return true;
    }

    false
}
