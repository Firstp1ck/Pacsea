//! Simple modal mouse event handling (Help, `VirusTotalSetup`, News).

use crate::state::AppState;
use crossterm::event::{MouseEvent, MouseEventKind};

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
pub(super) fn handle_news_modal(
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

/// Handle mouse events for Updates modal.
///
/// What: Process mouse interactions within the Updates modal (scroll, close).
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
/// - Handles scroll navigation.
/// - Closes modal on outside click.
pub(super) fn handle_updates_modal(
    m: MouseEvent,
    _mx: u16,
    _my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    if let crate::state::Modal::Updates {
        ref mut scroll,
        ref entries,
    } = app.modal
    {
        // Handle scroll within modal
        match m.kind {
            crossterm::event::MouseEventKind::ScrollUp => {
                *scroll = scroll.saturating_sub(1);
                return Some(false);
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                // Calculate max scroll based on content height
                // Each entry is 1 line, plus header (1 line), blank (1 line), footer (1 line), blank (1 line) = 4 lines
                let content_lines = u16::try_from(entries.len())
                    .unwrap_or(u16::MAX)
                    .saturating_add(4);
                // Estimate visible lines (modal height minus borders and title/footer)
                let max_scroll = content_lines.saturating_sub(10);
                if *scroll < max_scroll {
                    *scroll = scroll.saturating_add(1);
                }
                return Some(false);
            }
            _ => {}
        }

        // Left click outside modal closes it
        if is_left_down && let Some((x, y, w, h)) = app.updates_modal_rect {
            let mx = m.column;
            let my = m.row;
            if mx < x || mx >= x + w || my < y || my >= y + h {
                app.modal = crate::state::Modal::None;
                return Some(false);
            }
        }

        // Consume all mouse events while Updates modal is open
        return Some(false);
    }
    None
}
