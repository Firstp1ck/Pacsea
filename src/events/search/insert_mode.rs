use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::logic::move_sel_cached;
use crate::state::{AppState, PackageItem, QueryInput};

use super::super::utils::matches_any;
use super::helpers::navigate_pane;
use super::preflight_helpers::open_preflight_modal;
use crate::events::utils::{char_count, refresh_install_details};
use crate::logic::send_query;

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
) -> bool {
    let km = &app.keymap;

    match (ke.code, ke.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
        (c, m) if matches_any(&ke, &km.pane_next) && (c, m) == (ke.code, ke.modifiers) => {
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
            app.input.pop();
            app.last_input_change = std::time::Instant::now();
            app.last_saved_value = None;
            // Move caret to end and clear selection in insert mode
            app.search_caret = char_count(&app.input);
            app.search_select_anchor = None;
            send_query(app, query_tx);
        }
        (KeyCode::Char('\n') | KeyCode::Enter, _) => {
            if let Some(item) = app.results.get(app.selected).cloned() {
                open_preflight_modal(app, vec![item], false);
            }
        }
        (KeyCode::Char(ch), _) => {
            app.input.push(ch);
            app.last_input_change = std::time::Instant::now();
            app.last_saved_value = None;
            app.search_caret = char_count(&app.input);
            app.search_select_anchor = None;
            send_query(app, query_tx);
        }
        _ if matches_any(&ke, &km.search_move_up) => {
            move_sel_cached(app, -1, details_tx);
        }
        _ if matches_any(&ke, &km.search_move_down) => {
            move_sel_cached(app, 1, details_tx);
        }
        _ if matches_any(&ke, &km.search_page_up) => {
            move_sel_cached(app, -10, details_tx);
        }
        _ if matches_any(&ke, &km.search_page_down) => {
            move_sel_cached(app, 10, details_tx);
        }
        _ => {}
    }
    false
}
