//! Pane mouse event handling (Results, Recent, Install/Remove/Downgrade, PKGBUILD viewer).

use crossterm::event::{MouseEvent, MouseEventKind};
use tokio::sync::mpsc;

use crate::events::utils::refresh_install_details;
use crate::logic::move_sel_cached;
use crate::state::{AppState, PackageItem};

/// What: Check if mouse coordinates are within a rectangle.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `rect`: Optional rectangle tuple (x, y, width, height)
///
/// Output:
/// - `true` if mouse is within the rectangle, `false` otherwise
///
/// Details:
/// - Returns `false` if `rect` is `None`.
/// - Checks bounds: `mx >= x && mx < x + w && my >= y && my < y + h`.
const fn is_in_rect(mx: u16, my: u16, rect: Option<(u16, u16, u16, u16)>) -> bool {
    let Some((x, y, w, h)) = rect else {
        return false;
    };
    mx >= x && mx < x + w && my >= y && my < y + h
}

/// What: Handle Results pane mouse interactions.
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `is_left_down`: Whether left button is pressed
/// - `app`: Mutable application state
/// - `details_tx`: Channel for details requests
///
/// Output:
/// - `true` if event was handled, `false` otherwise
///
/// Details:
/// - Left click selects item at clicked row.
/// - Scroll wheel moves selection and triggers details fetch.
fn handle_results_pane(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
) -> bool {
    if !is_in_rect(mx, my, app.results_rect) {
        return false;
    }

    let (_, y, _, _) = app
        .results_rect
        .expect("results_rect should be Some when is_in_rect returns true");
    // Results list has a top border; adjust so first row maps to index 0.
    let row = my.saturating_sub(y.saturating_add(1)) as usize;

    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        let offset = app.news_list_state.offset();
        let idx = offset + row;
        if idx < app.news_results.len() && is_left_down {
            app.news_selected = idx;
            app.news_list_state.select(Some(idx));
            crate::events::utils::update_news_url(app);
        }
        match m.kind {
            MouseEventKind::ScrollUp => {
                crate::events::utils::move_news_selection(app, -1);
                true
            }
            MouseEventKind::ScrollDown => {
                crate::events::utils::move_news_selection(app, 1);
                true
            }
            _ => is_left_down,
        }
    } else {
        if is_left_down {
            let offset = app.list_state.offset();
            let idx = offset + row;
            if idx < app.results.len() {
                app.selected = idx;
                app.list_state.select(Some(idx));
            }
        }

        match m.kind {
            MouseEventKind::ScrollUp => {
                move_sel_cached(app, -1, details_tx, comments_tx);
                true
            }
            MouseEventKind::ScrollDown => {
                move_sel_cached(app, 1, details_tx, comments_tx);
                true
            }
            _ => is_left_down,
        }
    }
}

/// What: Handle Recent pane mouse interactions.
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
/// - `preview_tx`: Channel for preview requests
///
/// Output:
/// - `true` if event was handled, `false` otherwise
///
/// Details:
/// - Scroll wheel moves selection and triggers preview fetch.
fn handle_recent_pane(
    m: MouseEvent,
    mx: u16,
    my: u16,
    app: &mut AppState,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if !is_in_rect(mx, my, app.recent_rect) {
        return false;
    }

    let inds = crate::ui::helpers::filtered_recent_indices(app);
    if inds.is_empty() {
        return false;
    }

    match m.kind {
        MouseEventKind::ScrollUp => {
            if let Some(sel) = app.history_state.selected() {
                let new = sel.saturating_sub(1);
                app.history_state.select(Some(new));
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                true
            } else {
                false
            }
        }
        MouseEventKind::ScrollDown => {
            let sel = app.history_state.selected().unwrap_or(0);
            let max = inds.len().saturating_sub(1);
            let new = std::cmp::min(sel.saturating_add(1), max);
            app.history_state.select(Some(new));
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
            true
        }
        _ => false,
    }
}

/// What: Handle Install/Remove pane click interactions.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
/// - `details_tx`: Channel for details requests
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
///
/// Details:
/// - Focuses Install pane and selects item at clicked row.
/// - Handles both Remove (installed-only mode) and Install (normal mode).
fn handle_install_click(
    mx: u16,
    my: u16,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    if !is_in_rect(mx, my, app.install_rect) {
        return None;
    }

    app.focus = crate::state::Focus::Install;
    let (_, y, _, _) = app
        .install_rect
        .expect("install_rect should be Some when is_in_rect returns true");
    let row = my.saturating_sub(y) as usize;

    if app.installed_only_mode {
        app.right_pane_focus = crate::state::RightPaneFocus::Remove;
        let max = app.remove_list.len().saturating_sub(1);
        if !app.remove_list.is_empty() {
            let idx = std::cmp::min(row, max);
            app.remove_state.select(Some(idx));
            crate::events::utils::refresh_remove_details(app, details_tx);
        }
    } else {
        app.right_pane_focus = crate::state::RightPaneFocus::Install;
        let inds = crate::ui::helpers::filtered_install_indices(app);
        if !inds.is_empty() {
            let max = inds.len().saturating_sub(1);
            let vis_idx = std::cmp::min(row, max);
            app.install_state.select(Some(vis_idx));
            crate::events::utils::refresh_install_details(app, details_tx);
        }
    }
    Some(false)
}

/// What: Handle Install/Remove pane scroll interactions.
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
/// - `details_tx`: Channel for details requests
///
/// Output:
/// - `true` if event was handled, `false` otherwise
///
/// Details:
/// - Scroll wheel moves selection in Remove (installed-only mode) or Install (normal mode).
fn handle_install_scroll(
    m: MouseEvent,
    mx: u16,
    my: u16,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if !is_in_rect(mx, my, app.install_rect) {
        return false;
    }

    if app.installed_only_mode {
        let len = app.remove_list.len();
        if len > 0 {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if let Some(sel) = app.remove_state.selected() {
                        let new = sel.saturating_sub(1);
                        app.remove_state.select(Some(new));
                        crate::events::utils::refresh_remove_details(app, details_tx);
                        true
                    } else {
                        false
                    }
                }
                MouseEventKind::ScrollDown => {
                    let sel = app.remove_state.selected().unwrap_or(0);
                    let max = len.saturating_sub(1);
                    let new = std::cmp::min(sel.saturating_add(1), max);
                    app.remove_state.select(Some(new));
                    crate::events::utils::refresh_remove_details(app, details_tx);
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    } else {
        let inds = crate::ui::helpers::filtered_install_indices(app);
        if inds.is_empty() {
            false
        } else {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if let Some(sel) = app.install_state.selected() {
                        let new = sel.saturating_sub(1);
                        app.install_state.select(Some(new));
                        refresh_install_details(app, details_tx);
                        true
                    } else {
                        false
                    }
                }
                MouseEventKind::ScrollDown => {
                    let sel = app.install_state.selected().unwrap_or(0);
                    let max = inds.len().saturating_sub(1);
                    let new = std::cmp::min(sel.saturating_add(1), max);
                    app.install_state.select(Some(new));
                    refresh_install_details(app, details_tx);
                    true
                }
                _ => false,
            }
        }
    }
}

/// What: Handle Downgrade pane click interactions.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
/// - `details_tx`: Channel for details requests
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
///
/// Details:
/// - Only active in installed-only mode.
/// - Focuses Install pane, sets Downgrade focus, and selects item at clicked row.
fn handle_downgrade_click(
    mx: u16,
    my: u16,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    if !app.installed_only_mode || !is_in_rect(mx, my, app.downgrade_rect) {
        return None;
    }

    app.focus = crate::state::Focus::Install;
    app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
    let (_, y, _, _) = app
        .downgrade_rect
        .expect("downgrade_rect should be Some when is_in_rect returns true");
    let row = my.saturating_sub(y) as usize;
    let max = app.downgrade_list.len().saturating_sub(1);
    if !app.downgrade_list.is_empty() {
        let idx = std::cmp::min(row, max);
        app.downgrade_state.select(Some(idx));
        crate::events::utils::refresh_downgrade_details(app, details_tx);
    }
    Some(false)
}

/// What: Handle Downgrade pane scroll interactions.
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
/// - `details_tx`: Channel for details requests
///
/// Output:
/// - `true` if event was handled, `false` otherwise
///
/// Details:
/// - Only active in installed-only mode.
/// - Scroll wheel moves selection in Downgrade list.
fn handle_downgrade_scroll(
    m: MouseEvent,
    mx: u16,
    my: u16,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if !app.installed_only_mode || !is_in_rect(mx, my, app.downgrade_rect) {
        return false;
    }

    let len = app.downgrade_list.len();
    if len > 0 {
        match m.kind {
            MouseEventKind::ScrollUp => {
                if let Some(sel) = app.downgrade_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.downgrade_state.select(Some(new));
                    crate::events::utils::refresh_downgrade_details(app, details_tx);
                    true
                } else {
                    false
                }
            }
            MouseEventKind::ScrollDown => {
                let sel = app.downgrade_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel.saturating_add(1), max);
                app.downgrade_state.select(Some(new));
                crate::events::utils::refresh_downgrade_details(app, details_tx);
                true
            }
            _ => false,
        }
    } else {
        false
    }
}

/// What: Handle PKGBUILD viewer scroll interactions.
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if event was handled, `false` otherwise
///
/// Details:
/// - Scroll wheel scrolls the PKGBUILD content.
#[allow(clippy::missing_const_for_fn)]
fn handle_pkgbuild_scroll(m: MouseEvent, mx: u16, my: u16, app: &mut AppState) -> bool {
    if !is_in_rect(mx, my, app.pkgb_rect) {
        return false;
    }

    match m.kind {
        MouseEventKind::ScrollUp => {
            app.pkgb_scroll = app.pkgb_scroll.saturating_sub(1);
            true
        }
        MouseEventKind::ScrollDown => {
            app.pkgb_scroll = app.pkgb_scroll.saturating_add(1);
            true
        }
        _ => false,
    }
}

/// What: Handle comments viewer scroll interactions.
///
/// Inputs:
/// - `m`: Mouse event
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if event was handled, `false` otherwise
///
/// Details:
/// - Scroll wheel scrolls the comments content.
#[allow(clippy::missing_const_for_fn)]
fn handle_comments_scroll(m: MouseEvent, mx: u16, my: u16, app: &mut AppState) -> bool {
    if !is_in_rect(mx, my, app.comments_rect) {
        return false;
    }

    match m.kind {
        MouseEventKind::ScrollUp => {
            app.comments_scroll = app.comments_scroll.saturating_sub(1);
            true
        }
        MouseEventKind::ScrollDown => {
            app.comments_scroll = app.comments_scroll.saturating_add(1);
            true
        }
        _ => false,
    }
}

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
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_panes_mouse(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
) -> Option<bool> {
    // Handle clicks first (they return early when handled)
    if is_left_down {
        if let Some(result) = handle_install_click(mx, my, app, details_tx) {
            return Some(result);
        }
        if let Some(result) = handle_downgrade_click(mx, my, app, details_tx) {
            return Some(result);
        }
    }

    // Handle scroll events (execute handlers, they don't return early)
    handle_results_pane(m, mx, my, is_left_down, app, details_tx, comments_tx);
    handle_recent_pane(m, mx, my, app, preview_tx);
    handle_install_scroll(m, mx, my, app, details_tx);
    handle_downgrade_scroll(m, mx, my, app, details_tx);
    handle_pkgbuild_scroll(m, mx, my, app);
    handle_comments_scroll(m, mx, my, app);

    None
}
