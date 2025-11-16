//! Modal mouse event handling (Help, VirusTotalSetup, Preflight, News).

use crate::state::AppState;
use crate::state::modal::ServiceRestartDecision;
use crossterm::event::{MouseEvent, MouseEventKind};

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
    // Help modal handling
    if let crate::state::Modal::Help = &app.modal {
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
                if mx < outer_x
                    || mx >= outer_x + outer_w
                    || my < outer_y
                    || my >= outer_y + outer_h
                {
                    app.modal = crate::state::Modal::None;
                }
            } else {
                // Fallback: close on any click if no rect is known
                app.modal = crate::state::Modal::None;
            }
            return Some(false);
        }
        // Consume remaining mouse events while Help is open
        return Some(false);
    }

    // VirusTotalSetup modal handling
    if let crate::state::Modal::VirusTotalSetup { .. } = &app.modal {
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
        return Some(false);
    }

    // Preflight modal handling
    if let crate::state::Modal::Preflight {
        tab,
        items,
        action,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        deps_error: _,
        file_info,
        file_selected,
        file_tree_expanded,
        files_error: _,
        service_info,
        service_selected,
        services_loaded,
        services_error: _,
        ..
    } = &mut app.modal
    {
        // Handle tab clicks
        if is_left_down {
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
                        // Try to use cached dependencies from app state
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
                        if !cached_deps.is_empty() {
                            *dependency_info = cached_deps;
                            *dep_selected = 0;
                        }
                        // If no cached deps, user can press 'r' to resolve
                    }
                    // Check for cached files when switching to Files tab
                    if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                        // Try to use cached files from app state
                        let item_names: std::collections::HashSet<String> =
                            items.iter().map(|i| i.name.clone()).collect();
                        let cached_files: Vec<crate::state::modal::PackageFileInfo> = app
                            .install_list_files
                            .iter()
                            .filter(|file_info| item_names.contains(&file_info.name))
                            .cloned()
                            .collect();
                        if !cached_files.is_empty() {
                            *file_info = cached_files;
                            *file_selected = 0;
                        }
                        // If no cached files, user can press 'r' to resolve
                    }
                    // Check for cached services when switching to Services tab
                    if *tab == crate::state::PreflightTab::Services && service_info.is_empty() {
                        // Try to use cached services from app state (for install actions)
                        if matches!(*action, crate::state::PreflightAction::Install)
                            && !app.services_resolving
                        {
                            // Check if cache file exists with matching signature
                            let cache_exists = if !items.is_empty() {
                                let signature =
                                    crate::app::services_cache::compute_signature(items);
                                crate::app::services_cache::load_cache(
                                    &app.services_cache_path,
                                    &signature,
                                )
                                .is_some()
                            } else {
                                false
                            };
                            if cache_exists && !app.install_list_services.is_empty() {
                                *service_info = app.install_list_services.clone();
                                *service_selected = 0;
                                *services_loaded = true;
                            }
                        }
                        // If no cached services, user can press 'r' to resolve
                    }
                    return Some(false);
                }
            }

            // Handle package group header clicks in Deps tab
            if *tab == crate::state::PreflightTab::Deps
                && !dependency_info.is_empty()
                && let Some((content_x, content_y, content_w, content_h)) =
                    app.preflight_content_rect
                && mx >= content_x
                && mx < content_x + content_w
                && my >= content_y
                && my < content_y + content_h
            {
                // Calculate which row was clicked relative to content area
                let clicked_row = (my - content_y) as usize;

                // Build display items list to find which package header was clicked
                use std::collections::{HashMap, HashSet};
                let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                    HashMap::new();
                for dep in dependency_info.iter() {
                    for req_by in &dep.required_by {
                        grouped.entry(req_by.clone()).or_default().push(dep);
                    }
                }

                let mut display_items: Vec<(bool, String)> = Vec::new();
                for pkg_name in items.iter().map(|p| &p.name) {
                    if grouped.contains_key(pkg_name) {
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
                }

                // Calculate offset for summary line before the list
                // Deps tab has: summary line (1) + empty line (1) = 2 lines
                let list_start_offset = 2;

                // Only process clicks that are on or after the list starts
                if clicked_row >= list_start_offset {
                    let list_clicked_row = clicked_row - list_start_offset;

                    // Calculate scroll position to find actual index
                    let available_height =
                        content_h.saturating_sub(list_start_offset as u16) as usize;
                    let start_idx = (*dep_selected)
                        .saturating_sub(available_height / 2)
                        .min(display_items.len().saturating_sub(available_height));

                    let actual_idx = start_idx + list_clicked_row;
                    if actual_idx < display_items.len()
                        && let Some((is_header, pkg_name)) = display_items.get(actual_idx)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if dep_tree_expanded.contains(pkg_name) {
                            dep_tree_expanded.remove(pkg_name);
                        } else {
                            dep_tree_expanded.insert(pkg_name.clone());
                        }
                        return Some(false);
                    }
                }
            }

            // Handle package group header clicks in Files tab
            if *tab == crate::state::PreflightTab::Files
                && !file_info.is_empty()
                && let Some((content_x, content_y, content_w, content_h)) =
                    app.preflight_content_rect
                && mx >= content_x
                && mx < content_x + content_w
                && my >= content_y
                && my < content_y + content_h
            {
                // Calculate which row was clicked relative to content area
                let clicked_row = (my - content_y) as usize;

                // Build display items list to find which package header was clicked
                let display_items = crate::events::preflight::build_file_display_items(
                    file_info,
                    file_tree_expanded,
                );

                // Calculate offset for summary lines before the list
                // Files tab has: summary line (1) + empty line (1) + optional sync timestamp (0-2) + empty line (0-1)
                // Minimum offset is 2 lines (summary + empty)
                let sync_timestamp_lines = if crate::logic::files::get_file_db_sync_info().is_some()
                {
                    2 // timestamp line + empty line
                } else {
                    0
                };
                let list_start_offset = 2 + sync_timestamp_lines; // summary + empty + sync timestamp lines

                // Only process clicks that are on or after the list starts
                if clicked_row >= list_start_offset {
                    let list_clicked_row = clicked_row - list_start_offset;

                    // Calculate scroll position to find actual index
                    let total_items = display_items.len();
                    let file_selected_clamped = (*file_selected).min(total_items.saturating_sub(1));
                    let available_height =
                        content_h.saturating_sub(list_start_offset as u16) as usize;
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
                    if actual_idx < display_items.len()
                        && let Some((is_header, pkg_name)) = display_items.get(actual_idx)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if file_tree_expanded.contains(pkg_name) {
                            file_tree_expanded.remove(pkg_name);
                        } else {
                            file_tree_expanded.insert(pkg_name.clone());
                        }
                        return Some(false);
                    }
                }
            }

            if *tab == crate::state::PreflightTab::Services
                && !service_info.is_empty()
                && let Some((content_x, content_y, content_w, content_h)) =
                    app.preflight_content_rect
                && mx >= content_x
                && mx < content_x + content_w
                && my >= content_y
                && my < content_y + content_h
            {
                let list_start_offset = 2;
                if (my - content_y) as usize >= list_start_offset {
                    let list_clicked_row = (my - content_y) as usize - list_start_offset;
                    let available_height =
                        content_h.saturating_sub(list_start_offset as u16) as usize;
                    let total_items = service_info.len();
                    if total_items > 0 {
                        let selected_clamped =
                            (*service_selected).min(total_items.saturating_sub(1));
                        let start_idx = if total_items <= available_height {
                            0
                        } else {
                            selected_clamped
                                .saturating_sub(available_height / 2)
                                .min(total_items.saturating_sub(available_height))
                        };
                        let end_idx = (start_idx + available_height).min(total_items);
                        let actual_idx = start_idx + list_clicked_row;
                        if actual_idx < end_idx {
                            let actual_idx = actual_idx.min(total_items.saturating_sub(1));
                            if actual_idx == *service_selected {
                                if let Some(service) = service_info.get_mut(actual_idx) {
                                    service.restart_decision = match service.restart_decision {
                                        ServiceRestartDecision::Restart => {
                                            ServiceRestartDecision::Defer
                                        }
                                        ServiceRestartDecision::Defer => {
                                            ServiceRestartDecision::Restart
                                        }
                                    };
                                }
                            } else {
                                *service_selected = actual_idx;
                            }
                            return Some(false);
                        }
                    }
                }
            }
        }

        // Handle mouse scroll for Deps tab
        if *tab == crate::state::PreflightTab::Deps {
            // Compute display_items length (headers + dependencies, accounting for folded groups)
            use std::collections::{HashMap, HashSet};
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
                        return Some(false);
                    }
                    MouseEventKind::ScrollDown => {
                        if *dep_selected < display_len.saturating_sub(1) {
                            *dep_selected += 1;
                        }
                        return Some(false);
                    }
                    _ => {}
                }
            }
        }

        // Handle mouse scroll for Files tab
        if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if *file_selected > 0 {
                        *file_selected -= 1;
                    }
                    return Some(false);
                }
                MouseEventKind::ScrollDown => {
                    let display_len = crate::events::preflight::compute_file_display_items_len(
                        file_info,
                        file_tree_expanded,
                    );
                    if *file_selected < display_len.saturating_sub(1) {
                        *file_selected += 1;
                    }
                    return Some(false);
                }
                _ => {}
            }
        }

        // Handle mouse scroll for Services tab
        if *tab == crate::state::PreflightTab::Services && !service_info.is_empty() {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if *service_selected > 0 {
                        *service_selected -= 1;
                    }
                    return Some(false);
                }
                MouseEventKind::ScrollDown => {
                    if *service_selected + 1 < service_info.len() {
                        *service_selected += 1;
                    }
                    return Some(false);
                }
                _ => {}
            }
        }

        // Consume all mouse events while Preflight modal is open
        return Some(false);
    }

    // News modal handling
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
