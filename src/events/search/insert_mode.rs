use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::logic::move_sel_cached;
use crate::state::{AppState, PackageItem, QueryInput};

use super::super::utils::matches_any;
use super::helpers::{handle_shift_keybinds, navigate_pane};
use super::preflight_helpers::open_preflight_modal;
use crate::events::utils::{char_count, refresh_install_details};
use crate::logic::send_query;

/// What: Handle character input in insert mode.
///
/// Inputs:
/// - `ch`: Character to add.
/// - `app`: Mutable application state.
/// - `query_tx`: Channel to send debounced search queries.
///
/// Output: None (modifies app state in place).
///
/// Details:
/// - Handles both News mode and normal search mode.
/// - Updates input, caret position, and triggers search queries.
fn handle_character_input(
    ch: char,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
) {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        app.news_search_input.push(ch);
        app.last_input_change = std::time::Instant::now();
        app.last_saved_value = None;
        let caret = char_count(&app.news_search_input);
        app.news_search_caret = caret;
        app.news_search_select_anchor = None;
        app.refresh_news_results();
    } else {
        app.input.push(ch);
        app.last_input_change = std::time::Instant::now();
        app.last_saved_value = None;
        app.search_caret = char_count(&app.input);
        app.search_select_anchor = None;
        send_query(app, query_tx);
    }
}

/// What: Handle backspace in insert mode.
///
/// Inputs:
/// - `app`: Mutable application state.
/// - `query_tx`: Channel to send debounced search queries.
///
/// Output: None (modifies app state in place).
///
/// Details:
/// - Handles both News mode and normal search mode.
/// - Removes last character and updates caret position.
fn handle_backspace(app: &mut AppState, query_tx: &mpsc::UnboundedSender<QueryInput>) {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        app.news_search_input.pop();
        app.last_input_change = std::time::Instant::now();
        app.last_saved_value = None;
        let caret = char_count(&app.news_search_input);
        app.news_search_caret = caret;
        app.news_search_select_anchor = None;
        app.refresh_news_results();
    } else {
        app.input.pop();
        app.last_input_change = std::time::Instant::now();
        app.last_saved_value = None;
        app.search_caret = char_count(&app.input);
        app.search_select_anchor = None;
        send_query(app, query_tx);
    }
}

/// What: Handle navigation keys (up, down, page up, page down).
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Mutable application state.
/// - `details_tx`: Channel to request details.
/// - `comments_tx`: Channel to request comments.
///
/// Output: `true` if the key was handled, `false` otherwise.
///
/// Details:
/// - Handles both News mode and normal search mode.
/// - Moves selection and updates details/comments.
fn handle_navigation_keys(
    ke: &KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
) -> bool {
    let is_news = matches!(app.app_mode, crate::state::types::AppMode::News);
    let km = &app.keymap;

    if matches_any(ke, &km.search_move_up) {
        if is_news {
            crate::events::utils::move_news_selection(app, -1);
        } else {
            move_sel_cached(app, -1, details_tx, comments_tx);
        }
        return true;
    }
    if matches_any(ke, &km.search_move_down) {
        if is_news {
            crate::events::utils::move_news_selection(app, 1);
        } else {
            move_sel_cached(app, 1, details_tx, comments_tx);
        }
        return true;
    }
    if matches_any(ke, &km.search_page_up) {
        if is_news {
            crate::events::utils::move_news_selection(app, -10);
        } else {
            move_sel_cached(app, -10, details_tx, comments_tx);
        }
        return true;
    }
    if matches_any(ke, &km.search_page_down) {
        if is_news {
            crate::events::utils::move_news_selection(app, 10);
        } else {
            move_sel_cached(app, 10, details_tx, comments_tx);
        }
        return true;
    }
    false
}

/// What: Handle key events in Insert mode for the Search pane.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send debounced search queries
/// - `details_tx`: Channel to request details for the focused item
/// - `add_tx`: Channel to add items to the Install/Remove lists
/// - `preview_tx`: Channel to request preview details when moving focus
///
/// Output:
/// - `true` to request application exit (e.g., Ctrl+C); `false` to continue processing
///
/// Details:
/// - Handles typing, backspace, navigation, space to add items, and Enter to open preflight.
/// - Typing updates the input, caret position, and triggers debounced search queries.
pub fn handle_insert_mode(
    ke: KeyEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
) -> bool {
    // Handle Shift+char keybinds (menus, import, export, updates, status) that work in all modes
    if handle_shift_keybinds(&ke, app) {
        return false;
    }

    match (ke.code, ke.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
        (c, m)
            if {
                let km = &app.keymap;
                matches_any(&ke, &km.pane_next) && (c, m) == (ke.code, ke.modifiers)
            } =>
        {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                app.focus = crate::state::Focus::Install;
            } else {
                // Desired cycle: Recent -> Search -> Downgrade -> Remove -> Recent
                if app.installed_only_mode {
                    app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                    if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                        app.downgrade_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    crate::events::utils::refresh_downgrade_details(app, details_tx);
                } else {
                    if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                        app.install_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    refresh_install_details(app, details_tx);
                }
            }
        }
        (KeyCode::Right, _) => {
            navigate_pane(app, "right", details_tx, preview_tx);
        }
        (KeyCode::Left, _) => {
            navigate_pane(app, "left", details_tx, preview_tx);
        }
        (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
            if app.installed_only_mode
                && let Some(item) = app.results.get(app.selected).cloned()
            {
                crate::logic::add_to_downgrade_list(app, item);
                // Do not change focus; only update details to reflect the new selection
                crate::events::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        (KeyCode::Char(' '), _) => {
            if let Some(item) = app.results.get(app.selected).cloned() {
                if app.installed_only_mode {
                    crate::logic::add_to_remove_list(app, item);
                    crate::events::utils::refresh_remove_details(app, details_tx);
                } else {
                    let _ = add_tx.send(item);
                }
            }
        }
        (KeyCode::Backspace, _) => {
            handle_backspace(app, query_tx);
        }
        // Handle Enter - but NOT if it's actually Ctrl+M (which some terminals send as Enter)
        (KeyCode::Char('\n' | '\r') | KeyCode::Enter, m) => {
            // Don't open preflight if Ctrl is held (might be Ctrl+M interpreted as Enter)
            if m.contains(KeyModifiers::CONTROL) {
                tracing::debug!(
                    "[InsertMode] Enter with Ctrl detected, ignoring (likely Ctrl+M interpreted as Enter)"
                );
                return false;
            }
            if let Some(item) = app.results.get(app.selected).cloned() {
                tracing::debug!(
                    "[InsertMode] Enter pressed, opening preflight for package: {}",
                    item.name
                );
                open_preflight_modal(app, vec![item], false);
            }
        }
        // Only handle character input if no modifiers are present (to allow global keybinds with modifiers)
        (KeyCode::Char(ch), m) if m.is_empty() => {
            handle_character_input(ch, app, query_tx);
        }
        _ => {
            if handle_navigation_keys(&ke, app, details_tx, comments_tx) {
                // Navigation handled
            } else {
                let km = &app.keymap;
                if matches_any(&ke, &km.search_insert_clear) {
                    // Clear entire search input
                    if matches!(app.app_mode, crate::state::types::AppMode::News) {
                        if !app.news_search_input.is_empty() {
                            app.news_search_input.clear();
                            app.news_search_caret = 0;
                            app.news_search_select_anchor = None;
                            app.last_input_change = std::time::Instant::now();
                            app.last_saved_value = None;
                            app.refresh_news_results();
                        }
                    } else if !app.input.is_empty() {
                        app.input.clear();
                        app.search_caret = 0;
                        app.search_select_anchor = None;
                        app.last_input_change = std::time::Instant::now();
                        app.last_saved_value = None;
                        send_query(app, query_tx);
                    }
                }
            }
        }
    }
    false
}
