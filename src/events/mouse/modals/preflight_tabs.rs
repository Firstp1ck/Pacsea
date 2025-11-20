//! Preflight modal tab-specific mouse event handling.

use crate::state::AppState;
use crate::state::modal::ServiceRestartDecision;
use crossterm::event::{MouseEvent, MouseEventKind};
use std::collections::{HashMap, HashSet};

use super::preflight_helpers::build_deps_display_items;

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
pub(super) fn handle_deps_tab_click(mx: u16, my: u16, app: &mut AppState) -> bool {
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
pub(super) fn handle_files_tab_click(mx: u16, my: u16, app: &mut AppState) -> bool {
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
        let display_items = crate::events::preflight::build_file_display_items(
            items,
            file_info,
            file_tree_expanded,
        );

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
pub(super) fn handle_services_tab_click(mx: u16, my: u16, app: &mut AppState) -> bool {
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
pub(super) fn handle_deps_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
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
pub(super) fn handle_files_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
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
pub(super) fn handle_summary_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
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
pub(super) fn handle_services_tab_scroll(m: MouseEvent, app: &mut AppState) -> bool {
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
