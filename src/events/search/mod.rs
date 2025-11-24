//! Search pane event handling module.
//!
//! This module handles all keyboard input events when the Search pane is focused.
//! It supports two modes:
//! - **Insert mode** (default): Direct text input for searching packages
//! - **Normal mode**: Vim-like navigation and editing commands
//!
//! The module is split into submodules for maintainability:
//! - `helpers`: Shared utility functions for key matching and pane navigation
//! - `insert_mode`: Insert mode key event handling
//! - `normal_mode`: Normal mode key event handling
//! - `preflight_helpers`: Preflight modal opening logic

use crossterm::event::KeyEvent;
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem, QueryInput};

mod helpers;
mod insert_mode;
mod normal_mode;
mod preflight_helpers;

// Re-export preflight modal opener for use in other modules
pub use preflight_helpers::open_preflight_modal;

#[cfg(test)]
mod tests;

/// What: Handle key events while the Search pane is focused.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state (input, selection, sort, modes)
/// - `query_tx`: Channel to send debounced search queries
/// - `details_tx`: Channel to request details for the focused item
/// - `add_tx`: Channel to add items to the Install/Remove lists
/// - `preview_tx`: Channel to request preview details when moving focus
///
/// Output:
/// - `true` to request application exit (e.g., Ctrl+C); `false` to continue processing.
///
/// Details:
/// - Insert mode (default): typing edits the query, triggers debounced network/idx search, and
///   moves caret; Backspace edits; Space adds to list (Install by default, Remove in installed-only).
/// - Normal mode: toggled via configured chord; supports selection (h/l), deletion (d), navigation
///   (j/k, Ctrl+U/D), and list add/remove with Space/ Ctrl+Space (downgrade).
/// - Pane navigation: Left/Right and configured `pane_next` cycle focus across panes and subpanes,
///   differing slightly when installed-only mode is active.
/// - PKGBUILD reload is handled via debounced requests scheduled in the selection logic.
pub fn handle_search_key(
    ke: KeyEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    let km = &app.keymap;

    // Toggle fuzzy search mode (works in both insert and normal mode)
    if super::utils::matches_any(&ke, &km.toggle_fuzzy) {
        app.fuzzy_search_enabled = !app.fuzzy_search_enabled;
        crate::theme::save_fuzzy_search(app.fuzzy_search_enabled);
        // Re-trigger search with current query using new mode
        crate::logic::send_query(app, query_tx);
        return false;
    }

    // Toggle Normal mode (configurable)
    if super::utils::matches_any(&ke, &km.search_normal_toggle) {
        app.search_normal_mode = !app.search_normal_mode;
        return false;
    }

    // Normal mode: Vim-like navigation without editing input
    if app.search_normal_mode {
        return normal_mode::handle_normal_mode(ke, app, query_tx, details_tx, add_tx, preview_tx);
    }

    // Insert mode (default for Search)
    insert_mode::handle_insert_mode(ke, app, query_tx, details_tx, add_tx, preview_tx)
}
