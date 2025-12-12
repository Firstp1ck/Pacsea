//! Simple modal mouse event handling (Help, `VirusTotalSetup`, News).

use crate::state::AppState;
use crossterm::event::{MouseEvent, MouseEventKind};

/// What: Calculate scroll offset to keep the selected item in the middle of the viewport.
///
/// Inputs:
/// - `selected`: Currently selected item index
/// - `total_items`: Total number of items in the list
/// - `visible_height`: Height of the visible content area (in lines)
///
/// Output:
/// - Scroll offset (lines) that centers the selected item
///
/// Details:
/// - Calculates scroll so selected item is in the middle of visible area
/// - Ensures scroll doesn't go negative or past the end
fn calculate_news_scroll_for_selection(
    selected: usize,
    total_items: usize,
    visible_height: u16,
) -> u16 {
    if total_items == 0 || visible_height == 0 {
        return 0;
    }

    let selected_line = u16::try_from(selected).unwrap_or(u16::MAX);
    let total_lines = u16::try_from(total_items).unwrap_or(u16::MAX);

    // Calculate middle position: we want selected item to be at visible_height / 2
    let middle_offset = visible_height / 2;

    // Calculate desired scroll to center the selection
    let desired_scroll = selected_line.saturating_sub(middle_offset);

    // Calculate maximum scroll (when last item is at the bottom)
    let max_scroll = total_lines.saturating_sub(visible_height);

    // Clamp scroll to valid range
    desired_scroll.min(max_scroll)
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
/// - `false` if the event was handled
///
/// Details:
/// - Supports scrolling within content area.
/// - Closes modal on outside click.
/// - Consumes all mouse events while Help is open.
pub(super) fn handle_help_modal(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> bool {
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
                return false;
            }
            MouseEventKind::ScrollDown => {
                app.help_scroll = app.help_scroll.saturating_add(1);
                return false;
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
        return false;
    }
    // Consume remaining mouse events while Help is open
    false
}

/// Handle mouse events for the `VirusTotalSetup` modal.
///
/// What: Process mouse interactions within the `VirusTotalSetup` modal dialog.
///
/// Inputs:
/// - `_m`: Mouse event including position, button, and modifiers (unused but kept for signature consistency)
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `is_left_down`: Whether the left mouse button is pressed
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `false` if the event was handled
///
/// Details:
/// - Opens URL when clicking the link area.
/// - Consumes all mouse events while `VirusTotal` setup modal is open.
pub(super) fn handle_virustotal_modal(
    _m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &AppState,
) -> bool {
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
    false
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
pub(super) fn handle_announcement_modal(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    if let crate::state::Modal::Announcement { scroll, .. } = &mut app.modal {
        // Left click: check for URL click first, then close on outside
        if is_left_down {
            // Check if click matches any URL position
            for (url_x, url_y, url_width, url) in &app.announcement_urls {
                if mx >= *url_x && mx < url_x.saturating_add(*url_width) && my == *url_y {
                    crate::util::open_url(url);
                    return Some(false);
                }
            }

            // Click outside closes the modal (dismiss temporarily)
            if let Some((x, y, w, h)) = app.announcement_rect
                && (mx < x || mx >= x + w || my < y || my >= y + h)
            {
                // Click outside closes the modal (dismiss temporarily)
                app.modal = crate::state::Modal::None;
            }
            return Some(false);
        }
        // Scroll within modal: scroll content
        match m.kind {
            MouseEventKind::ScrollUp => {
                if *scroll > 0 {
                    *scroll -= 1;
                }
                return Some(false);
            }
            MouseEventKind::ScrollDown => {
                *scroll = scroll.saturating_add(1);
                return Some(false);
            }
            _ => {}
        }
        // If modal is open and event wasn't handled above, consume it
        return Some(false);
    }
    None
}

/// What: Handle mouse events in the News modal.
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `is_left_down`: Whether left mouse button is pressed
/// - `app`: Mutable application state
///
/// Output:
/// - `Some(true)` if application should exit
/// - `Some(false)` if event was handled
/// - `None` if event was not handled
///
/// Details:
/// - Handles left clicks to select/open news items or close modal on outside click
/// - Handles scrolling in the news list
pub(super) fn handle_news_modal(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    if let crate::state::Modal::News {
        items,
        selected,
        scroll,
    } = &mut app.modal
    {
        // Left click: select/open or close on outside
        if is_left_down {
            if let Some((x, y, w, h)) = app.news_list_rect
                && mx >= x
                && mx < x + w
                && my >= y
                && my < y + h
            {
                // Calculate clicked row accounting for scroll offset
                let relative_y = my.saturating_sub(y);
                let clicked_row = (relative_y as usize).saturating_add(*scroll as usize);
                // Only open if clicking on an actual news item line (not empty space)
                if clicked_row < items.len() {
                    *selected = clicked_row;
                    // Update scroll to keep selection centered
                    *scroll = calculate_news_scroll_for_selection(*selected, items.len(), h);
                    if let Some(it) = items.get(*selected)
                        && let Some(url) = &it.url
                    {
                        crate::util::open_url(url);
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
                    // Update scroll to keep selection centered
                    if let Some((_, _, _, visible_h)) = app.news_list_rect {
                        *scroll =
                            calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
                    }
                }
                return Some(false);
            }
            MouseEventKind::ScrollDown => {
                if *selected + 1 < items.len() {
                    *selected += 1;
                    // Update scroll to keep selection centered
                    if let Some((_, _, _, visible_h)) = app.news_list_rect {
                        *scroll =
                            calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
                    }
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

/// Handle mouse events for Updates modal.
///
/// What: Process mouse interactions within the Updates modal (scroll, selection, close).
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `is_left_down`: Whether left button is pressed
/// - `app`: Mutable application state
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
///
/// Details:
/// - Handles row selection on click.
/// - Handles scroll navigation (updates selection to match scroll position).
/// - Closes modal on outside click.
pub(super) fn handle_updates_modal(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    if let crate::state::Modal::Updates {
        ref mut scroll,
        ref entries,
        ref mut selected,
    } = app.modal
    {
        // Left click: select row or close on outside
        if is_left_down {
            if let Some((x, y, w, h)) = app.updates_modal_content_rect
                && mx >= x
                && mx < x + w
                && my >= y
                && my < y + h
            {
                // Account for header (2 lines) when calculating clicked row
                const HEADER_LINES: u16 = 2;
                let relative_y = my.saturating_sub(y);
                // Only process clicks in the content area (below header)
                if relative_y >= HEADER_LINES {
                    // Calculate which row was clicked (accounting for header and scroll offset)
                    let clicked_row = (relative_y.saturating_sub(HEADER_LINES) as usize)
                        .saturating_add(*scroll as usize);
                    // Only update selection if clicking on an actual entry
                    if clicked_row < entries.len() {
                        *selected = clicked_row;
                        // Auto-scroll to keep selected item visible
                        update_scroll_for_selection(scroll, *selected);
                    }
                }
            } else if let Some((x, y, w, h)) = app.updates_modal_rect {
                // Click outside modal closes it
                if mx < x || mx >= x + w || my < y || my >= y + h {
                    app.modal = crate::state::Modal::None;
                    return Some(false); // Return immediately after closing
                }
            }
            return Some(false);
        }

        // Handle scroll within modal: update selection to match scroll position
        match m.kind {
            crossterm::event::MouseEventKind::ScrollUp => {
                if *selected > 0 {
                    *selected -= 1;
                }
                // Auto-scroll to keep selected item visible
                update_scroll_for_selection(scroll, *selected);
                return Some(false);
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                if *selected + 1 < entries.len() {
                    *selected += 1;
                }
                // Auto-scroll to keep selected item visible
                update_scroll_for_selection(scroll, *selected);
                return Some(false);
            }
            _ => {}
        }

        // Consume all mouse events while Updates modal is open
        return Some(false);
    }
    None
}

/// What: Update scroll offset to keep the selected item visible.
///
/// Inputs:
/// - `scroll`: Mutable scroll offset
/// - `selected`: Selected index
///
/// Output:
/// - Updates scroll to ensure selected item is visible
///
/// Details:
/// - Estimates visible lines based on modal height
/// - Adjusts scroll so selected item is within visible range
fn update_scroll_for_selection(scroll: &mut u16, selected: usize) {
    // Estimate visible content lines (modal height minus header/footer/borders)
    // Header: 2 lines, borders: 2 lines, footer: 0 lines = ~4 lines overhead
    // Assume ~20 visible content lines as a reasonable default
    const VISIBLE_LINES: u16 = 20;

    let selected_line = u16::try_from(selected).unwrap_or(u16::MAX);

    // If selected item is above visible area, scroll up
    if selected_line < *scroll {
        *scroll = selected_line;
    }
    // If selected item is below visible area, scroll down
    else if selected_line >= *scroll + VISIBLE_LINES {
        *scroll = selected_line.saturating_sub(VISIBLE_LINES.saturating_sub(1));
    }
}
