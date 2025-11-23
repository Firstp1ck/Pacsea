//! Preflight modal mouse event handling.

use crate::state::AppState;
use crossterm::event::MouseEvent;

use super::preflight_helpers::{load_cached_dependencies, load_cached_files, load_cached_services};
use super::preflight_tabs::{
    handle_deps_tab_click, handle_deps_tab_scroll, handle_files_tab_click, handle_files_tab_scroll,
    handle_services_tab_click, handle_services_tab_scroll, handle_summary_tab_scroll,
};

/// Handle mouse events for the Preflight modal.
///
/// What: Process mouse interactions within the Preflight modal dialog.
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `is_left_down`: Whether the left mouse button is pressed
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `Some(false)` if the event was handled, `None` if not handled.
///
/// Details:
/// - Handles tab clicks and tab switching.
/// - Handles package group header toggles in Deps/Files tabs.
/// - Handles service restart decisions in Services tab.
/// - Handles scroll navigation for Deps/Files/Services tabs.
pub(super) fn handle_preflight_modal(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    // Extract tab value first to avoid borrow conflicts
    let current_tab = if let crate::state::Modal::Preflight { tab, .. } = &app.modal {
        *tab
    } else {
        return None;
    };

    // Handle tab clicks
    if is_left_down {
        if handle_preflight_tab_click(mx, my, app) {
            return Some(false);
        }

        // Handle tab-specific clicks
        match current_tab {
            crate::state::PreflightTab::Deps => {
                if handle_deps_tab_click(mx, my, app) {
                    return Some(false);
                }
            }
            crate::state::PreflightTab::Files => {
                if handle_files_tab_click(mx, my, app) {
                    return Some(false);
                }
            }
            crate::state::PreflightTab::Services => {
                if handle_services_tab_click(mx, my, app) {
                    return Some(false);
                }
            }
            _ => {}
        }
    }

    // Handle tab-specific scrolls
    match current_tab {
        crate::state::PreflightTab::Summary => {
            if handle_summary_tab_scroll(m, app) {
                return Some(false);
            }
        }
        crate::state::PreflightTab::Deps => {
            if handle_deps_tab_scroll(m, app) {
                return Some(false);
            }
        }
        crate::state::PreflightTab::Files => {
            if handle_files_tab_scroll(m, app) {
                return Some(false);
            }
        }
        crate::state::PreflightTab::Services => {
            if handle_services_tab_scroll(m, app) {
                return Some(false);
            }
        }
        crate::state::PreflightTab::Sandbox => {}
    }

    // Consume all mouse events while Preflight modal is open
    Some(false)
}

/// Handle tab click events in the Preflight modal.
///
/// What: Detect clicks on tab buttons and switch to the clicked tab, loading cached data if available.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `true` if a tab was clicked and handled, `false` otherwise.
///
/// Details:
/// - Switches to the clicked tab (Summary, Deps, Files, Services, Sandbox).
/// - Loads cached dependencies when switching to Deps tab.
/// - Loads cached files when switching to Files tab.
/// - Loads cached services when switching to Services tab.
fn handle_preflight_tab_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        tab,
        items,
        action,
        dependency_info,
        dep_selected,
        file_info,
        file_selected,
        service_info,
        service_selected,
        services_loaded,
        ..
    } = &mut app.modal
    {
        for (i, tab_rect_opt) in app.preflight_tab_rects.iter().enumerate() {
            if let Some((x, y, w, h)) = tab_rect_opt
                && mx >= *x
                && mx < x + w
                && my >= *y
                && my < y + h
            {
                // Clicked on tab i - switch to that tab
                let new_tab = match i {
                    0 => crate::state::PreflightTab::Summary,
                    1 => crate::state::PreflightTab::Deps,
                    2 => crate::state::PreflightTab::Files,
                    3 => crate::state::PreflightTab::Services,
                    4 => crate::state::PreflightTab::Sandbox,
                    _ => continue,
                };
                let old_tab = *tab;
                *tab = new_tab;
                tracing::info!(
                    "[Preflight] Mouse tab click: Switching from {:?} to {:?}, items={}, dependency_info.len()={}, file_info.len()={}",
                    old_tab,
                    new_tab,
                    items.len(),
                    dependency_info.len(),
                    file_info.len()
                );

                // Check for cached dependencies when switching to Deps tab
                if *tab == crate::state::PreflightTab::Deps
                    && dependency_info.is_empty()
                    && matches!(*action, crate::state::PreflightAction::Install)
                {
                    tracing::debug!(
                        "[Preflight] Mouse tab click: Deps tab - loading cache, cache.len()={}",
                        app.install_list_deps.len()
                    );
                    let cached_deps = load_cached_dependencies(items, &app.install_list_deps);
                    tracing::info!(
                        "[Preflight] Mouse tab click: Deps tab - loaded {} cached deps",
                        cached_deps.len()
                    );
                    if !cached_deps.is_empty() {
                        *dependency_info = cached_deps;
                        *dep_selected = 0;
                    }
                    // If no cached deps, user can press 'r' to resolve
                }
                // Check for cached files when switching to Files tab
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    tracing::debug!(
                        "[Preflight] Mouse tab click: Files tab - loading cache, cache.len()={}",
                        app.install_list_files.len()
                    );
                    let cached_files = load_cached_files(items, &app.install_list_files);
                    tracing::info!(
                        "[Preflight] Mouse tab click: Files tab - loaded {} cached files",
                        cached_files.len()
                    );
                    if !cached_files.is_empty() {
                        *file_info = cached_files;
                        *file_selected = 0;
                    }
                    // If no cached files, user can press 'r' to resolve
                }
                // Check for cached services when switching to Services tab
                if *tab == crate::state::PreflightTab::Services
                    && service_info.is_empty()
                    && let Some(cached_services) = load_cached_services(
                        items,
                        *action,
                        app.services_resolving,
                        &app.services_cache_path,
                        &app.install_list_services,
                    )
                {
                    *service_info = cached_services;
                    *service_selected = 0;
                    *services_loaded = true;
                    // If no cached services, user can press 'r' to resolve
                }
                return true;
            }
        }
    }
    false
}
