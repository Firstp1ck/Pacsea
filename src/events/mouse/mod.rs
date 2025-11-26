//! Mouse event handling for Pacsea's TUI.
//!
//! This module delegates mouse event handling to specialized submodules:
//! - `modals`: Modal interactions (Help, `VirusTotalSetup`, `Preflight`, News)
//! - `details`: Details pane interactions (URL, PKGBUILD buttons, scroll)
//! - `menus`: Menu interactions (sort, options, config, panels, import/export)
//! - `filters`: Filter toggle interactions
//! - `panes`: Pane interactions (Results, Recent, Install/Remove/Downgrade, PKGBUILD viewer)

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem, QueryInput};

mod details;
mod filters;
pub mod menu_options;
pub mod menus;
mod modals;
mod panes;

#[cfg(test)]
mod tests;

/// Handle a single mouse event and update the [`AppState`].
///
/// Returns `true` to request application exit (never used here), `false` otherwise.
///
/// Behavior summary:
/// - Clickable URL in the details pane with Ctrl+Shift+LeftClick (opens via `xdg-open`).
/// - Clickable "Show/Hide PKGBUILD" action in the details content.
/// - Clickable "Copy PKGBUILD" button in the PKGBUILD title (copies to clipboard).
/// - Clickable Sort button and filter toggles in the Results title.
/// - Click-to-select in Results; mouse wheel scroll moves selection in Results/Recent/Install.
/// - Mouse wheel scroll within the PKGBUILD viewer scrolls the content.
///
/// What: Handle a single mouse event and update application state and UI accordingly.
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `app`: Mutable application state (rects, focus, lists, details)
/// - `details_tx`: Channel to request package details when selection changes
/// - `preview_tx`: Channel to request preview details for Recent pane interactions
/// - `_add_tx`: Channel for adding items (used by Import button handler)
/// - `pkgb_tx`: Channel to request PKGBUILD content for the current selection
/// - `query_tx`: Channel to send search queries (for fuzzy toggle)
///
/// Output:
/// - `true` to request application exit (never used here); otherwise `false`.
///
/// Details:
/// - Modal-first: When Help or News is open, clicks/scroll are handled within modal bounds
///   (close on outside click), consuming the event.
/// - Details area: Ctrl+Shift+LeftClick opens URL; PKGBUILD toggle and copy button respond to clicks;
///   while text selection is enabled, clicks inside details are ignored by the app.
/// - Title bar: Sort/options/panels/config buttons toggle menus; filter toggles apply filters.
/// - Results: Click selects; scroll wheel moves selection and triggers details fetch.
/// - Recent/Install/Remove/Downgrade panes: Scroll moves selection; click focuses/sets selection.
/// - Import/Export buttons: Import opens a system file picker to enqueue names; Export writes the
///   current Install list to a timestamped file and shows a toast.
#[allow(clippy::too_many_arguments)]
pub fn handle_mouse_event(
    m: MouseEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    _add_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
) -> bool {
    // Ensure mouse capture is enabled (important after external terminal processes)
    crate::util::ensure_mouse_capture();
    if !app.mouse_capture_enabled {
        app.mouse_capture_enabled = true;
    }

    let mx = m.column;
    let my = m.row;
    let is_left_down = matches!(m.kind, MouseEventKind::Down(MouseButton::Left));
    let ctrl = m.modifiers.contains(KeyModifiers::CONTROL);
    let shift = m.modifiers.contains(KeyModifiers::SHIFT);

    // Track last mouse position for dynamic capture toggling
    app.last_mouse_pos = Some((mx, my));

    // Modal-first handling: modals intercept mouse events
    if let Some(handled) = modals::handle_modal_mouse(m, mx, my, is_left_down, app) {
        return handled;
    }

    // While any modal is open, prevent main window interaction by consuming mouse events
    if !matches!(app.modal, crate::state::Modal::None) {
        return false;
    }

    // Details pane interactions (URL, PKGBUILD buttons, scroll)
    if let Some(handled) = details::handle_details_mouse(
        m,
        mx,
        my,
        is_left_down,
        ctrl,
        shift,
        app,
        pkgb_tx,
        comments_tx,
    ) {
        return handled;
    }

    // Menu interactions (sort, options, config, panels, import/export)
    if is_left_down && let Some(handled) = menus::handle_menus_mouse(mx, my, app, details_tx) {
        return handled;
    }

    // Filter toggle interactions
    if is_left_down && let Some(handled) = filters::handle_filters_mouse(mx, my, app) {
        return handled;
    }

    // Fuzzy search indicator toggle
    if is_left_down
        && let Some((x, y, w, h)) = app.fuzzy_indicator_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.fuzzy_search_enabled = !app.fuzzy_search_enabled;
        crate::theme::save_fuzzy_search(app.fuzzy_search_enabled);
        // Re-trigger search with current query using new mode
        crate::logic::send_query(app, query_tx);
        return false;
    }

    // Pane interactions (Results, Recent, Install/Remove/Downgrade, PKGBUILD viewer)
    if let Some(handled) = panes::handle_panes_mouse(
        m,
        mx,
        my,
        is_left_down,
        app,
        details_tx,
        preview_tx,
        comments_tx,
    ) {
        return handled;
    }

    false
}

// Re-export for use in keyboard handlers
pub use menus::handle_updates_button;
