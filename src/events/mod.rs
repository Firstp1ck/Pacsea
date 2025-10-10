//! Event handling layer for Pacsea's TUI (modularized).
//!
//! This module re-exports `handle_event` and delegates pane-specific logic
//! and mouse handling to submodules to keep files small and maintainable.

use crossterm::event::{Event as CEvent, KeyCode, KeyEventKind};
use tokio::sync::mpsc;

use crate::state::{AppState, Focus, PackageItem, QueryInput};
use crate::theme::reload_theme;

mod install;
mod mouse;
mod recent;
mod search;
mod utils;

// re-export intentionally omitted; handled internally

/// Dispatch a single terminal event and mutate the [`AppState`].
///
/// Returns `true` to signal the application should exit; otherwise `false`.
pub fn handle_event(
    ev: CEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if let CEvent::Key(ke) = ev {
        if ke.kind != KeyEventKind::Press {
            return false;
        }

        // Modal handling
        match &app.modal {
            crate::state::Modal::Alert { .. } => {
                match ke.code {
                    KeyCode::Enter | KeyCode::Esc => app.modal = crate::state::Modal::None,
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::ConfirmInstall { items } => {
                match ke.code {
                    KeyCode::Esc => {
                        app.modal = crate::state::Modal::None;
                    }
                    KeyCode::Enter => {
                        let list = items.clone();
                        app.modal = crate::state::Modal::None;
                        if list.len() <= 1 {
                            if let Some(it) = list.first() {
                                crate::install::spawn_install(it, None, app.dry_run);
                            }
                        } else {
                            crate::install::spawn_install_all(&list, app.dry_run);
                        }
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::Help => {
                match ke.code {
                    KeyCode::Esc | KeyCode::Enter => app.modal = crate::state::Modal::None,
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::None => {}
        }

        // Global keymap shortcuts (regardless of focus)
        // First: allow ESC to close the PKGBUILD viewer if it is open
        if ke.code == KeyCode::Esc && app.pkgb_visible {
            app.pkgb_visible = false;
            app.pkgb_text = None;
            return false;
        }

        let km = &app.keymap;
        let chord = (ke.code, ke.modifiers);
        let matches_any =
            |list: &Vec<crate::theme::KeyChord>| list.iter().any(|c| (c.code, c.mods) == chord);
        if matches_any(&km.help_overlay) {
            app.modal = crate::state::Modal::Help;
            return false;
        }
        if matches_any(&km.reload_theme) {
            match reload_theme() {
                Ok(()) => {}
                Err(msg) => {
                    app.modal = crate::state::Modal::Alert { message: msg };
                }
            }
            return false;
        }
        if matches_any(&km.exit) {
            return true;
        }
        // Toggle PKGBUILD viewer globally
        if matches_any(&km.show_pkgbuild) {
            if app.pkgb_visible {
                app.pkgb_visible = false;
                app.pkgb_text = None;
            } else {
                app.pkgb_visible = true;
                app.pkgb_text = None;
                if let Some(item) = app.results.get(app.selected).cloned() {
                    let _ = pkgb_tx.send(item);
                }
            }
            return false;
        }

        // Global: Shift+Tab cycles sort mode and opens the dropdown for visual feedback
        if ke.code == KeyCode::BackTab {
            // Cycle through sort modes in fixed order
            app.sort_mode = match app.sort_mode {
                crate::state::SortMode::RepoThenName => {
                    crate::state::SortMode::AurPopularityThenOfficial
                }
                crate::state::SortMode::AurPopularityThenOfficial => {
                    crate::state::SortMode::BestMatches
                }
                crate::state::SortMode::BestMatches => crate::state::SortMode::RepoThenName,
            };
            // Persist preference and apply immediately
            crate::theme::save_sort_mode(app.sort_mode);
            crate::logic::sort_results_preserve_selection(app);
            // Jump selection to top and refresh details
            if !app.results.is_empty() {
                app.selected = 0;
                app.list_state.select(Some(0));
                utils::refresh_selected_details(app, details_tx);
            } else {
                app.list_state.select(None);
            }
            // Show the dropdown so the user sees the current option with a check mark
            app.sort_menu_open = true;
            app.sort_menu_auto_close_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
            return false;
        }

        // Recent pane focused
        if matches!(app.focus, Focus::Recent) {
            let should_exit =
                recent::handle_recent_key(ke, app, query_tx, details_tx, preview_tx, add_tx);
            return should_exit;
        }

        // Install pane focused
        if matches!(app.focus, Focus::Install) {
            let should_exit = install::handle_install_key(ke, app, details_tx, preview_tx, add_tx);
            return should_exit;
        }

        // Search pane focused (delegated)
        if matches!(app.focus, Focus::Search) {
            let should_exit =
                search::handle_search_key(ke, app, query_tx, details_tx, add_tx, preview_tx);
            return should_exit;
        }

        // Fallback: not handled
        return false;
    }

    // Mouse handling delegated
    if let CEvent::Mouse(m) = ev {
        return mouse::handle_mouse_event(m, app, details_tx, preview_tx, add_tx, pkgb_tx);
    }
    false
}
