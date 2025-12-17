use crossterm::event::KeyEvent;

use super::super::utils::matches_any;
use super::normal_mode::{handle_export, handle_menu_toggles};
use crate::state::AppState;

/// What: Handle Shift+char keybinds (menus, import, export, updates, status) that work across all panes and modes.
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if a Shift+char keybind was handled, `false` otherwise
///
/// Details:
/// - Handles menu toggles (Shift+C, Shift+O, Shift+P), import (Shift+I), export (Shift+E),
///   updates (Shift+U), and status (Shift+S).
/// - Works in insert mode, normal mode, and all panes (Search, Recent, Install).
pub fn handle_shift_keybinds(ke: &KeyEvent, app: &mut AppState) -> bool {
    // Handle menu toggles
    if handle_menu_toggles(ke, app) {
        return true;
    }

    // Handle import (Shift+I)
    if matches_any(ke, &app.keymap.search_normal_import) {
        if !app.installed_only_mode {
            app.modal = crate::state::Modal::ImportHelp;
        }
        return true;
    }

    // Handle export (Shift+E)
    if matches_any(ke, &app.keymap.search_normal_export) {
        handle_export(app);
        return true;
    }

    // Handle updates (Shift+U)
    if matches_any(ke, &app.keymap.search_normal_updates) {
        // In News mode, open News modal; otherwise open Updates modal
        if matches!(app.app_mode, crate::state::types::AppMode::News) {
            crate::events::mouse::handle_news_button(app);
        } else {
            crate::events::mouse::handle_updates_button(app);
        }
        return true;
    }

    // Handle status (Shift+S)
    if matches_any(ke, &app.keymap.search_normal_open_status) {
        crate::util::open_url("https://status.archlinux.org");
        return true;
    }

    false
}

/// What: Handle pane navigation from Search pane to adjacent panes.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `direction`: Direction to navigate ("right" for Install, "left" for Recent)
/// - `details_tx`: Channel to request details for the focused item
/// - `preview_tx`: Channel to request preview details when moving focus
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - In installed-only mode, right navigation targets Downgrade first.
/// - Otherwise, right navigation targets Install list.
/// - Left navigation always targets Recent pane.
pub fn navigate_pane(
    app: &mut AppState,
    direction: &str,
    details_tx: &tokio::sync::mpsc::UnboundedSender<crate::state::PackageItem>,
    preview_tx: &tokio::sync::mpsc::UnboundedSender<crate::state::PackageItem>,
) {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        match direction {
            "right" => {
                app.focus = crate::state::Focus::Install; // bookmarks pane
            }
            "left" => {
                app.focus = crate::state::Focus::Recent; // history pane
            }
            _ => {}
        }
        return;
    }
    match direction {
        "right" => {
            if app.installed_only_mode {
                // Target Downgrade first in installed-only mode
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                    app.downgrade_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::super::utils::refresh_downgrade_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::super::utils::refresh_install_details(app, details_tx);
            }
        }
        "left" => {
            if app.history_state.selected().is_none() && !app.recent.is_empty() {
                app.history_state.select(Some(0));
            }
            app.focus = crate::state::Focus::Recent;
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        _ => {}
    }
}
