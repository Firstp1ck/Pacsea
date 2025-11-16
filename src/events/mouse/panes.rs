//! Pane mouse event handling (Results, Recent, Install/Remove/Downgrade, PKGBUILD viewer).

use crossterm::event::{MouseEvent, MouseEventKind};
use tokio::sync::mpsc;

use crate::events::utils::refresh_install_details;
use crate::logic::move_sel_cached;
use crate::state::{AppState, PackageItem};

/// Handle mouse events for panes (Results, Recent, Install/Remove/Downgrade, PKGBUILD viewer).
///
/// What: Process mouse interactions within list panes for selection, scrolling, and focus changes.
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `is_left_down`: Whether the left mouse button is pressed
/// - `app`: Mutable application state containing pane state and UI rectangles
/// - `details_tx`: Channel to request package details when selection changes
/// - `preview_tx`: Channel to request preview details for Recent pane interactions
///
/// Output:
/// - `Some(bool)` if the event was handled (consumed by a pane), `None` if not handled.
///   The boolean value indicates whether the application should exit (always `false` here).
///
/// Details:
/// - Results pane: Left click selects item; scroll wheel moves selection and triggers details fetch.
/// - Recent pane: Scroll wheel moves selection and triggers preview fetch.
/// - Install/Remove panes: Left click focuses pane and selects item; scroll wheel moves selection.
/// - Downgrade pane: Left click focuses pane and selects item; scroll wheel moves selection.
/// - PKGBUILD viewer: Scroll wheel scrolls the PKGBUILD content.
pub(super) fn handle_panes_mouse(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    // Results: click to select
    if is_left_down
        && let Some((x, y, w, h)) = app.results_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let row = my.saturating_sub(y) as usize; // row in viewport
        let offset = app.list_state.offset();
        let idx = offset + row;
        if idx < app.results.len() {
            app.selected = idx;
            app.list_state.select(Some(idx));
        }
    }

    // Results: scroll with mouse wheel to move selection
    if let Some((x, y, w, h)) = app.results_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                move_sel_cached(app, -1, details_tx);
            }
            MouseEventKind::ScrollDown => {
                move_sel_cached(app, 1, details_tx);
            }
            _ => {}
        }
    }

    // Recent pane: scroll with mouse wheel to change selection
    if let Some((x, y, w, h)) = app.recent_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let inds = crate::ui::helpers::filtered_recent_indices(app);
        if !inds.is_empty() {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    let sel = app.history_state.selected().unwrap_or(0);
                    let new = sel.saturating_sub(1);
                    app.history_state.select(Some(new));
                    crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                }
                MouseEventKind::ScrollDown => {
                    let sel = app.history_state.selected().unwrap_or(0);
                    let max = inds.len().saturating_sub(1);
                    let new = std::cmp::min(sel.saturating_add(1), max);
                    app.history_state.select(Some(new));
                    crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                }
                _ => {}
            }
        }
    }

    // Right panes: click to focus/select rows and scroll to change selection
    // Click inside Remove/Install area (right subpane or full right pane)
    if is_left_down
        && let Some((x, y, w, h)) = app.install_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.focus = crate::state::Focus::Install;
        if app.installed_only_mode {
            app.right_pane_focus = crate::state::RightPaneFocus::Remove;
            let row = my.saturating_sub(y) as usize;
            let max = app.remove_list.len().saturating_sub(1);
            if !app.remove_list.is_empty() {
                let idx = std::cmp::min(row, max);
                app.remove_state.select(Some(idx));
                crate::events::utils::refresh_remove_details(app, details_tx);
            }
        } else {
            app.right_pane_focus = crate::state::RightPaneFocus::Install;
            let row = my.saturating_sub(y) as usize;
            let inds = crate::ui::helpers::filtered_install_indices(app);
            if !inds.is_empty() {
                let max = inds.len().saturating_sub(1);
                let vis_idx = std::cmp::min(row, max);
                app.install_state.select(Some(vis_idx));
                crate::events::utils::refresh_install_details(app, details_tx);
            }
        }
        return Some(false);
    }

    // Click inside Downgrade subpane (left half in installed-only mode)
    if app.installed_only_mode
        && is_left_down
        && let Some((x, y, w, h)) = app.downgrade_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.focus = crate::state::Focus::Install;
        app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
        let row = my.saturating_sub(y) as usize;
        let max = app.downgrade_list.len().saturating_sub(1);
        if !app.downgrade_list.is_empty() {
            let idx = std::cmp::min(row, max);
            app.downgrade_state.select(Some(idx));
            crate::events::utils::refresh_downgrade_details(app, details_tx);
        }
        return Some(false);
    }

    // Scroll inside Remove/Install area
    // Right panes: scroll with mouse wheel to change selection
    // Remove (or Install in normal mode)
    if let Some((x, y, w, h)) = app.install_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        if app.installed_only_mode {
            let len = app.remove_list.len();
            if len > 0 {
                match m.kind {
                    MouseEventKind::ScrollUp => {
                        if let Some(sel) = app.remove_state.selected() {
                            let new = sel.saturating_sub(1);
                            app.remove_state.select(Some(new));
                            crate::events::utils::refresh_remove_details(app, details_tx);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        let sel = app.remove_state.selected().unwrap_or(0);
                        let max = len.saturating_sub(1);
                        let new = std::cmp::min(sel.saturating_add(1), max);
                        app.remove_state.select(Some(new));
                        crate::events::utils::refresh_remove_details(app, details_tx);
                    }
                    _ => {}
                }
            }
        } else {
            let inds = crate::ui::helpers::filtered_install_indices(app);
            if !inds.is_empty() {
                match m.kind {
                    MouseEventKind::ScrollUp => {
                        if let Some(sel) = app.install_state.selected() {
                            let new = sel.saturating_sub(1);
                            app.install_state.select(Some(new));
                            refresh_install_details(app, details_tx);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        let sel = app.install_state.selected().unwrap_or(0);
                        let max = inds.len().saturating_sub(1);
                        let new = std::cmp::min(sel.saturating_add(1), max);
                        app.install_state.select(Some(new));
                        refresh_install_details(app, details_tx);
                    }
                    _ => {}
                }
            }
        }
    }

    // Downgrade subpane scroll
    if app.installed_only_mode
        && let Some((x, y, w, h)) = app.downgrade_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let len = app.downgrade_list.len();
        if len > 0 {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if let Some(sel) = app.downgrade_state.selected() {
                        let new = sel.saturating_sub(1);
                        app.downgrade_state.select(Some(new));
                        crate::events::utils::refresh_downgrade_details(app, details_tx);
                    }
                }
                MouseEventKind::ScrollDown => {
                    let sel = app.downgrade_state.selected().unwrap_or(0);
                    let max = len.saturating_sub(1);
                    let new = std::cmp::min(sel.saturating_add(1), max);
                    app.downgrade_state.select(Some(new));
                    crate::events::utils::refresh_downgrade_details(app, details_tx);
                }
                _ => {}
            }
        }
    }

    // Scroll support inside PKGBUILD viewer using mouse wheel
    if let Some((x, y, w, h)) = app.pkgb_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                app.pkgb_scroll = app.pkgb_scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                app.pkgb_scroll = app.pkgb_scroll.saturating_add(1);
            }
            _ => {}
        }
    }

    None
}
