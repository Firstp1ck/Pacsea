//! Modal mouse event handling (Help, VirusTotalSetup, Preflight, News).

use crate::state::AppState;
use crate::state::modal::ServiceRestartDecision;
use crate::state::types::PackageItem;
use crossterm::event::{MouseEvent, MouseEventKind};
use std::collections::{HashMap, HashSet};

/// Handle mouse events for modals.
///
/// What: Process mouse interactions within modal dialogs (Help, VirusTotalSetup, Preflight, News).
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `is_left_down`: Whether the left mouse button is pressed
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `Some(bool)` if the event was handled (consumed by a modal), `None` if not handled.
///   The boolean value indicates whether the application should exit (always `false` here).
///
/// Details:
/// - Help modal: Supports scrolling within content area and closes on outside click.
/// - VirusTotalSetup modal: Opens URL when clicking the link area; consumes all other events.
/// - Preflight modal: Handles tab clicks, package group header toggles, service restart decisions,
///   and scroll navigation for Deps/Files/Services tabs.
/// - News modal: Handles item selection, URL opening, and scroll navigation; closes on outside click.
pub(super) fn handle_modal_mouse(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    match &mut app.modal {
        crate::state::Modal::Help => handle_help_modal(m, mx, my, is_left_down, app),
        crate::state::Modal::VirusTotalSetup { .. } => {
            handle_virustotal_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::Preflight { .. } => {
            handle_preflight_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::News { .. } => handle_news_modal(m, mx, my, is_left_down, app),
        _ => None,
    }
}

/// Handle mouse events for the Help modal.
///
/// What: Process mouse interactions within the Help modal dialog.
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
/// - Supports scrolling within content area.
/// - Closes modal on outside click.
/// - Consumes all mouse events while Help is open.
fn handle_help_modal(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    // Scroll within Help content area
    if let Some((x, y, w, h)) = app.help_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                app.help_scroll = app.help_scroll.saturating_sub(1);
                return Some(false);
            }
            MouseEventKind::ScrollDown => {
                app.help_scroll = app.help_scroll.saturating_add(1);
                return Some(false);
            }
            _ => {}
        }
    }
    // Clicking outside closes the Help modal
    if is_left_down {
        if let Some((x, y, w, h)) = app.help_rect {
            // Outer rect includes borders around inner help rect
            let outer_x = x.saturating_sub(1);
            let outer_y = y.saturating_sub(1);
            let outer_w = w.saturating_add(2);
            let outer_h = h.saturating_add(2);
            if mx < outer_x || mx >= outer_x + outer_w || my < outer_y || my >= outer_y + outer_h {
                app.modal = crate::state::Modal::None;
            }
        } else {
            // Fallback: close on any click if no rect is known
            app.modal = crate::state::Modal::None;
        }
        return Some(false);
    }
    // Consume remaining mouse events while Help is open
    Some(false)
}

/// Handle mouse events for the VirusTotalSetup modal.
///
/// What: Process mouse interactions within the VirusTotalSetup modal dialog.
///
/// Inputs:
/// - `_m`: Mouse event including position, button, and modifiers (unused but kept for signature consistency)
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `is_left_down`: Whether the left mouse button is pressed
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `Some(false)` if the event was handled, `None` if not handled.
///
/// Details:
/// - Opens URL when clicking the link area.
/// - Consumes all mouse events while VirusTotal setup modal is open.
fn handle_virustotal_modal(
    _m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    if is_left_down
        && let Some((x, y, w, h)) = app.vt_url_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let url = "https://www.virustotal.com/gui/my-apikey";
        std::thread::spawn(move || {
            let _ = std::process::Command::new("xdg-open")
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        });
    }
    // Consume all mouse events while VirusTotal setup modal is open
    Some(false)
}

/// Handle mouse events for the News modal.
///
/// What: Process mouse interactions within the News modal dialog.
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
/// - Handles item selection and URL opening.
/// - Handles scroll navigation.
/// - Closes modal on outside click.
fn handle_news_modal(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    if let crate::state::Modal::News { items, selected } = &mut app.modal {
        // Left click: select/open or close on outside
        if is_left_down {
            if let Some((x, y, w, h)) = app.news_list_rect
                && mx >= x
                && mx < x + w
                && my >= y
                && my < y + h
            {
                let row = my.saturating_sub(y) as usize;
                // Only open if clicking on an actual news item line (not empty space)
                if row < items.len() {
                    *selected = row;
                    if let Some(it) = items.get(*selected) {
                        crate::util::open_url(&it.url);
                    }
                }
            } else if let Some((x, y, w, h)) = app.news_rect
                && (mx < x || mx >= x + w || my < y || my >= y + h)
            {
                // Click outside closes the modal
                app.modal = crate::state::Modal::None;
            }
            return Some(false);
        }
        // Scroll within modal: move selection
        match m.kind {
            MouseEventKind::ScrollUp => {
                if *selected > 0 {
                    *selected -= 1;
                }
                return Some(false);
            }
            MouseEventKind::ScrollDown => {
                if *selected + 1 < items.len() {
                    *selected += 1;
                }
                return Some(false);
            }
            _ => {}
        }
        // If modal is open and event wasn't handled above, consume it
        return Some(false);
    }
    None
}

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
fn handle_preflight_modal(
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
        _ => {}
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
                *tab = new_tab;

                // Check for cached dependencies when switching to Deps tab
                if *tab == crate::state::PreflightTab::Deps
                    && dependency_info.is_empty()
                    && matches!(*action, crate::state::PreflightAction::Install)
                {
                    let cached_deps = load_cached_dependencies(items, &app.install_list_deps);
                    if !cached_deps.is_empty() {
                        *dependency_info = cached_deps;
                        *dep_selected = 0;
                    }
                    // If no cached deps, user can press 'r' to resolve
                }
                // Check for cached files when switching to Files tab
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    let cached_files = load_cached_files(items, &app.install_list_files);
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
                        action,
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

/// Handle click events in the Deps tab of the Preflight modal.
///
/// What: Process clicks on package group headers to toggle expansion state.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
///
/// Details:
/// - Builds display items list to find which package header was clicked.
/// - Calculates clicked index accounting for scroll position.
/// - Toggles package expansion state when clicking on a header.
fn handle_deps_tab_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        items,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        ..
    } = &mut app.modal
    {
        if dependency_info.is_empty() {
            return false;
        }

        let Some((content_x, content_y, content_w, content_h)) = app.preflight_content_rect else {
            return false;
        };

        if mx < content_x
            || mx >= content_x + content_w
            || my < content_y
            || my >= content_y + content_h
        {
            return false;
        }

        // Calculate which row was clicked relative to content area
        let clicked_row = (my - content_y) as usize;

        // Build display items list to find which package header was clicked
        let display_items = build_deps_display_items(items, dependency_info, dep_tree_expanded);

        // Calculate offset for summary line before the list
        // Deps tab has: summary line (1) + empty line (1) = 2 lines
        let list_start_offset = 2;

        // Only process clicks that are on or after the list starts
        if clicked_row < list_start_offset {
            return false;
        }

        let list_clicked_row = clicked_row - list_start_offset;

        // Calculate scroll position to find actual index
        let available_height = content_h.saturating_sub(list_start_offset as u16) as usize;
        let start_idx = (*dep_selected)
            .saturating_sub(available_height / 2)
            .min(display_items.len().saturating_sub(available_height));

        let actual_idx = start_idx + list_clicked_row;
        if let Some((is_header, pkg_name)) = display_items.get(actual_idx)
            && *is_header
            && actual_idx < display_items.len()
        {
            // Toggle this package's expanded state
            if dep_tree_expanded.contains(pkg_name) {
                dep_tree_expanded.remove(pkg_name);
            } else {
                dep_tree_expanded.insert(pkg_name.clone());
            }
            return true;
        }
    }
    false
}

/// Handle click events in the Files tab of the Preflight modal.
///
/// What: Process clicks on package group headers to toggle expansion state.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
///
/// Details:
/// - Uses existing helper to build display items list.
/// - Calculates clicked index accounting for scroll position and sync timestamp offset.
/// - Toggles package expansion state when clicking on a header.
fn handle_files_tab_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        items,
        file_info,
        file_selected,
        file_tree_expanded,
        ..
    } = &mut app.modal
    {
        let Some((content_x, content_y, content_w, content_h)) = app.preflight_content_rect else {
            return false;
        };

        if mx < content_x
            || mx >= content_x + content_w
            || my < content_y
            || my >= content_y + content_h
        {
            return false;
        }

        // Calculate which row was clicked relative to content area
        let clicked_row = (my - content_y) as usize;

        // Build display items list to find which package header was clicked
        // Always show all packages, even if they have no files
        let display_items =
            crate::events::preflight::build_file_display_items(items, file_info, file_tree_expanded);

        // Calculate offset for summary lines before the list
        // Files tab has: summary line (1) + empty line (1) + optional sync timestamp (0-2) + empty line (0-1)
        // Minimum offset is 2 lines (summary + empty)
        let sync_timestamp_lines = if crate::logic::files::get_file_db_sync_info().is_some() {
            2 // timestamp line + empty line
        } else {
            0
        };
        let list_start_offset = 2 + sync_timestamp_lines; // summary + empty + sync timestamp lines

        // Only process clicks that are on or after the list starts
        if clicked_row < list_start_offset {
            return false;
        }

        let list_clicked_row = clicked_row - list_start_offset;

        // Calculate scroll position to find actual index
        let total_items = display_items.len();
        let file_selected_clamped = (*file_selected).min(total_items.saturating_sub(1));
        let available_height = content_h.saturating_sub(list_start_offset as u16) as usize;
        let (start_idx, _end_idx) = if total_items <= available_height {
            (0, total_items)
        } else {
            let start = file_selected_clamped
                .saturating_sub(available_height / 2)
                .min(total_items.saturating_sub(available_height));
            let end = (start + available_height).min(total_items);
            (start, end)
        };

        let actual_idx = start_idx + list_clicked_row;
        if let Some((is_header, pkg_name)) = display_items.get(actual_idx)
            && *is_header
            && actual_idx < display_items.len()
        {
            // Toggle this package's expanded state
            if file_tree_expanded.contains(pkg_name) {
                file_tree_expanded.remove(pkg_name);
            } else {
                file_tree_expanded.insert(pkg_name.clone());
            }
            return true;
        }
    }
    false
}

/// Handle click events in the Services tab of the Preflight modal.
///
/// What: Process clicks on service items to select them or toggle restart decision.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
///
/// Details:
/// - Calculates clicked index accounting for scroll position.
/// - Selects service when clicking on a different item.
/// - Toggles restart decision when clicking on the currently selected item.
fn handle_services_tab_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        service_info,
        service_selected,
        ..
    } = &mut app.modal
    {
        if service_info.is_empty() {
            return false;
        }

        let Some((content_x, content_y, content_w, content_h)) = app.preflight_content_rect else {
            return false;
        };

        if mx < content_x
            || mx >= content_x + content_w
            || my < content_y
            || my >= content_y + content_h
        {
            return false;
        }

        let list_start_offset = 2;
        let clicked_row_offset = (my - content_y) as usize;
        if clicked_row_offset < list_start_offset {
            return false;
        }

        let list_clicked_row = clicked_row_offset - list_start_offset;
        let available_height = content_h.saturating_sub(list_start_offset as u16) as usize;
        let total_items = service_info.len();
        if total_items == 0 {
            return false;
        }

        let selected_clamped = (*service_selected).min(total_items.saturating_sub(1));
        let start_idx = if total_items <= available_height {
            0
        } else {
            selected_clamped
                .saturating_sub(available_height / 2)
                .min(total_items.saturating_sub(available_height))
        };
        let end_idx = (start_idx + available_height).min(total_items);
        let actual_idx = start_idx + list_clicked_row;
        if actual_idx >= end_idx {
            return false;
        }

        let actual_idx = actual_idx.min(total_items.saturating_sub(1));
        if actual_idx == *service_selected {
            if let Some(service) = service_info.get_mut(actual_idx) {
                service.restart_decision = match service.restart_decision {
                    ServiceRestartDecision::Restart => ServiceRestartDecision::Defer,
                    ServiceRestartDecision::Defer => ServiceRestartDecision::Restart,
                };
            }
        } else {
            *service_selected = actual_idx;
        }
        return true;
    }
    false
}

/// Handle scroll events in the Deps tab of the Preflight modal.
///
/// What: Process mouse scroll to navigate the dependency list.
///
/// Inputs:
/// - `m`: Mouse event including scroll direction
/// - `app`: Mutable application state containing modal state
///
/// Output:
/// - `true` if the scroll was handled, `false` otherwise.
///
/// Details:
/// - Computes display length (headers + dependencies, accounting for folded groups).
/// - Handles scroll up/down to move selection.
fn handle_deps_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        items,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        ..
    } = &mut app.modal
    {
        // Compute display_items length (headers + dependencies, accounting for folded groups)
        let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
            HashMap::new();
        for dep in dependency_info.iter() {
            for req_by in &dep.required_by {
                grouped.entry(req_by.clone()).or_default().push(dep);
            }
        }
        let mut display_len: usize = 0;
        for pkg_name in items.iter().map(|p| &p.name) {
            if let Some(pkg_deps) = grouped.get(pkg_name) {
                display_len += 1; // Header
                // Count dependencies only if expanded
                if dep_tree_expanded.contains(pkg_name) {
                    let mut seen_deps = HashSet::new();
                    for dep in pkg_deps.iter() {
                        if seen_deps.insert(dep.name.as_str()) {
                            display_len += 1;
                        }
                    }
                }
            }
        }

        // Handle mouse scroll to navigate dependency list
        if display_len > 0 {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if *dep_selected > 0 {
                        *dep_selected -= 1;
                    }
                    return true;
                }
                MouseEventKind::ScrollDown => {
                    if *dep_selected < display_len.saturating_sub(1) {
                        *dep_selected += 1;
                    }
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// Handle scroll events in the Files tab of the Preflight modal.
///
/// What: Process mouse scroll to navigate the file list.
///
/// Inputs:
/// - `m`: Mouse event including scroll direction
/// - `app`: Mutable application state containing modal state
///
/// Output:
/// - `true` if the scroll was handled, `false` otherwise.
///
/// Details:
/// - Uses existing helper to compute display length.
/// - Handles scroll up/down to move selection.
fn handle_files_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        items,
        file_info,
        file_selected,
        file_tree_expanded,
        ..
    } = &mut app.modal
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                if *file_selected > 0 {
                    *file_selected -= 1;
                }
                return true;
            }
            MouseEventKind::ScrollDown => {
                let display_len = crate::events::preflight::compute_file_display_items_len(
                    items,
                    file_info,
                    file_tree_expanded,
                );
                if *file_selected < display_len.saturating_sub(1) {
                    *file_selected += 1;
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Handle scroll events in the Services tab of the Preflight modal.
///
/// What: Process mouse scroll to navigate the service list.
///
/// Inputs:
/// - `m`: Mouse event including scroll direction
/// - `app`: Mutable application state containing modal state
///
/// Output:
/// - `true` if the scroll was handled, `false` otherwise.
///
/// Details:
/// - Handles scroll up/down to move selection.
///
/// Handle scroll events in the Summary tab of the Preflight modal.
///
/// What: Process mouse scroll to navigate the package list.
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `true` if the scroll was handled, `false` otherwise.
///
/// Handle scroll events in the Summary tab of the Preflight modal.
///
/// What: Process mouse scroll to scroll the entire Summary tab content.
///
/// Inputs:
/// - `m`: Mouse event including scroll direction
/// - `app`: Mutable application state containing modal state
///
/// Output:
/// - `true` if the scroll was handled, `false` otherwise.
///
/// Details:
/// - Increments/decrements scroll offset for the entire Summary tab content.
fn handle_summary_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight { summary_scroll, .. } = &mut app.modal {
        match m.kind {
            MouseEventKind::ScrollUp => {
                if *summary_scroll > 0 {
                    *summary_scroll = summary_scroll.saturating_sub(1);
                }
                return true;
            }
            MouseEventKind::ScrollDown => {
                *summary_scroll = summary_scroll.saturating_add(1);
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Handle scroll events in the Services tab of the Preflight modal.
///
/// What: Process mouse scroll to navigate the file list.
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `true` if the scroll was handled, `false` otherwise.
///
/// Details:
/// - Handles scroll up/down to move selection.
fn handle_services_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        service_info,
        service_selected,
        ..
    } = &mut app.modal
    {
        if service_info.is_empty() {
            return false;
        }

        match m.kind {
            MouseEventKind::ScrollUp => {
                if *service_selected > 0 {
                    *service_selected -= 1;
                }
                return true;
            }
            MouseEventKind::ScrollDown => {
                if *service_selected + 1 < service_info.len() {
                    *service_selected += 1;
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Build display items list for the Deps tab.
///
/// What: Creates a list of display items (headers and dependencies) for the dependency tree view.
///
/// Inputs:
/// - `items`: Packages selected for install/remove shown in the modal
/// - `dependency_info`: Flattened dependency metadata resolved for those packages
/// - `dep_tree_expanded`: Set of package names currently expanded in the UI tree
///
/// Output:
/// - Vector of `(bool, String)` pairs distinguishing headers (`true`) from dependency rows (`false`).
///
/// Details:
/// - Groups dependencies by the packages that require them.
/// - Only includes dependencies when their parent package is expanded.
/// - Always includes all packages, even if they have no dependencies.
/// - Deduplicates dependencies by name within each package group.
fn build_deps_display_items(
    items: &[PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    dep_tree_expanded: &HashSet<String>,
) -> Vec<(bool, String)> {
    // Build display items list to find which package header was clicked
    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> = HashMap::new();
    for dep in dependency_info.iter() {
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    let mut display_items: Vec<(bool, String)> = Vec::new();
    // Always show ALL packages, even if they have no dependencies
    // This ensures packages that failed to resolve dependencies (e.g., due to conflicts) are still visible
    for pkg_name in items.iter().map(|p| &p.name) {
        display_items.push((true, pkg_name.clone()));
        if dep_tree_expanded.contains(pkg_name) {
            let mut seen_deps = HashSet::new();
            if let Some(pkg_deps) = grouped.get(pkg_name) {
                for dep in pkg_deps.iter() {
                    if seen_deps.insert(dep.name.as_str()) {
                        display_items.push((false, String::new()));
                    }
                }
            }
        }
    }
    display_items
}

/// Load cached dependencies from app state.
///
/// What: Retrieves cached dependency information for the given packages from the install list.
///
/// Inputs:
/// - `items`: Packages to find cached dependencies for
/// - `install_list_deps`: Cached dependency information from app state
///
/// Output:
/// - Vector of cached dependency information, filtered to only include dependencies required by the given packages.
///
/// Details:
/// - Filters cached dependencies to only those required by packages in `items`.
/// - Returns empty vector if no matching cached dependencies are found.
fn load_cached_dependencies(
    items: &[PackageItem],
    install_list_deps: &[crate::state::modal::DependencyInfo],
) -> Vec<crate::state::modal::DependencyInfo> {
    let item_names: HashSet<String> = items.iter().map(|i| i.name.clone()).collect();
    install_list_deps
        .iter()
        .filter(|dep| {
            dep.required_by
                .iter()
                .any(|req_by| item_names.contains(req_by))
        })
        .cloned()
        .collect()
}

/// Load cached files from app state.
///
/// What: Retrieves cached file information for the given packages from the install list.
///
/// Inputs:
/// - `items`: Packages to find cached files for
/// - `install_list_files`: Cached file information from app state
///
/// Output:
/// - Vector of cached file information, filtered to only include files for the given packages.
///
/// Details:
/// - Filters cached files to only those belonging to packages in `items`.
/// - Returns empty vector if no matching cached files are found.
fn load_cached_files(
    items: &[PackageItem],
    install_list_files: &[crate::state::modal::PackageFileInfo],
) -> Vec<crate::state::modal::PackageFileInfo> {
    let item_names: HashSet<String> = items.iter().map(|i| i.name.clone()).collect();
    install_list_files
        .iter()
        .filter(|file_info| item_names.contains(&file_info.name))
        .cloned()
        .collect()
}

/// Load cached services from app state.
///
/// What: Retrieves cached service information for the given packages from the install list.
///
/// Inputs:
/// - `items`: Packages to find cached services for
/// - `action`: Preflight action (Install or Remove)
/// - `services_resolving`: Whether services are currently being resolved
/// - `services_cache_path`: Path to the services cache file
/// - `install_list_services`: Cached service information from app state
///
/// Output:
/// - `Some(Vec<ServiceImpact>)` if cached services are available, `None` otherwise.
///
/// Details:
/// - Only loads cache for Install actions.
/// - Checks if cache file exists with matching signature.
/// - Returns `None` if services are currently being resolved, cache doesn't exist, or cached services are empty.
fn load_cached_services(
    items: &[PackageItem],
    action: &crate::state::PreflightAction,
    services_resolving: bool,
    services_cache_path: &std::path::PathBuf,
    install_list_services: &[crate::state::modal::ServiceImpact],
) -> Option<Vec<crate::state::modal::ServiceImpact>> {
    // Try to use cached services from app state (for install actions)
    if !matches!(*action, crate::state::PreflightAction::Install) || services_resolving {
        return None;
    }

    // Check if cache file exists with matching signature
    let cache_exists = if !items.is_empty() {
        let signature = crate::app::services_cache::compute_signature(items);
        crate::app::services_cache::load_cache(services_cache_path, &signature).is_some()
    } else {
        false
    };

    if cache_exists && !install_list_services.is_empty() {
        Some(install_list_services.to_vec())
    } else {
        None
    }
}
