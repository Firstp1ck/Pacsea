use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::logic::{move_sel_cached, send_query};
use crate::state::{AppState, PackageItem, QueryInput};

use super::utils::{byte_index_for_char, char_count, refresh_install_details};

/// Handle key events while the Search pane is focused.
///
/// Supports insert mode (default) and a Vim-like normal mode with selection.
/// Returns `true` to exit the app, `false` to continue.
pub fn handle_search_key(
    ke: KeyEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    let km = &app.keymap;
    let chord = (ke.code, ke.modifiers);
    let matches_any =
        |list: &Vec<crate::theme::KeyChord>| list.iter().any(|c| (c.code, c.mods) == chord);

    // Toggle Normal mode (configurable)
    if matches_any(&km.search_normal_toggle) {
        app.search_normal_mode = !app.search_normal_mode;
        return false;
    }

    // Normal mode: Vim-like navigation without editing input
    if app.search_normal_mode {
        match (ke.code, ke.modifiers) {
            (c, m)
                if matches_any(&km.search_normal_insert) && (c, m) == (ke.code, ke.modifiers) =>
            {
                // return to insert mode
                app.search_normal_mode = false;
                app.search_select_anchor = None;
            }
            // Selection with configured left/right (default: h/l)
            (c, m)
                if matches_any(&km.search_normal_select_left)
                    && (c, m) == (ke.code, ke.modifiers) =>
            {
                // Begin selection if not started
                if app.search_select_anchor.is_none() {
                    app.search_select_anchor = Some(app.search_caret);
                }
                let cc = char_count(&app.input);
                let cur = app.search_caret as isize - 1;
                let new_ci = if cur < 0 { 0 } else { cur as usize };
                app.search_caret = new_ci.min(cc);
            }
            (c, m)
                if matches_any(&km.search_normal_select_right)
                    && (c, m) == (ke.code, ke.modifiers) =>
            {
                if app.search_select_anchor.is_none() {
                    app.search_select_anchor = Some(app.search_caret);
                }
                let cc = char_count(&app.input);
                let cur = app.search_caret + 1;
                app.search_caret = cur.min(cc);
            }
            // Delete selected range (default: d)
            (c, m)
                if matches_any(&km.search_normal_delete) && (c, m) == (ke.code, ke.modifiers) =>
            {
                if let Some(anchor) = app.search_select_anchor.take() {
                    let a = anchor.min(app.search_caret);
                    let b = anchor.max(app.search_caret);
                    if a != b {
                        let bs = byte_index_for_char(&app.input, a);
                        let be = byte_index_for_char(&app.input, b);
                        let mut new_input = String::with_capacity(app.input.len());
                        new_input.push_str(&app.input[..bs]);
                        new_input.push_str(&app.input[be..]);
                        app.input = new_input;
                        app.search_caret = a;
                        app.last_input_change = std::time::Instant::now();
                        app.last_saved_value = None;
                        send_query(app, query_tx);
                    }
                }
            }
            (KeyCode::Char('j'), _) => move_sel_cached(app, 1, details_tx),
            (KeyCode::Char('k'), _) => move_sel_cached(app, -1, details_tx),
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => move_sel_cached(app, 10, details_tx),
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => move_sel_cached(app, -10, details_tx),
            (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
                if app.installed_only_mode
                    && let Some(item) = app.results.get(app.selected).cloned()
                {
                    crate::logic::add_to_downgrade_list(app, item);
                    // Do not change focus; only update details to reflect the new selection
                    super::utils::refresh_downgrade_details(app, details_tx);
                }
            }
            (KeyCode::Char(' '), _) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    if app.installed_only_mode {
                        crate::logic::add_to_remove_list(app, item);
                        super::utils::refresh_remove_details(app, details_tx);
                    } else {
                        let _ = add_tx.send(item);
                    }
                }
            }
            (KeyCode::Char('\n') | KeyCode::Enter, _) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    app.modal = crate::state::Modal::ConfirmInstall { items: vec![item] };
                }
            }
            (c, m) if matches_any(&km.pane_next) && (c, m) == (ke.code, ke.modifiers) => {
                // Desired cycle: Recent -> Search -> Downgrade -> Remove -> Recent
                if app.installed_only_mode {
                    // From Search move to Downgrade first
                    app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                    if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                        app.downgrade_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    super::utils::refresh_downgrade_details(app, details_tx);
                } else {
                    if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                        app.install_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    refresh_install_details(app, details_tx);
                }
            }
            (KeyCode::Right, _) => {
                // Search -> Install (adjacent)
                if app.installed_only_mode {
                    // Target Downgrade first in installed-only mode
                    app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                    if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                        app.downgrade_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    super::utils::refresh_downgrade_details(app, details_tx);
                } else {
                    if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                        app.install_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    refresh_install_details(app, details_tx);
                }
            }
            (KeyCode::Left, _) => {
                if app.history_state.selected().is_none() && !app.recent.is_empty() {
                    app.history_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Recent;
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
            }
            _ => {}
        }
        return false;
    }

    // Insert mode (default for Search)
    match (ke.code, ke.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
        (c, m) if matches_any(&km.pane_next) && (c, m) == (ke.code, ke.modifiers) => {
            // Desired cycle: Recent -> Search -> Downgrade -> Remove -> Recent
            if app.installed_only_mode {
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                    app.downgrade_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::utils::refresh_downgrade_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                refresh_install_details(app, details_tx);
            }
        }
        // previous pane removed
        (KeyCode::Right, _) => {
            // Search -> Install (adjacent)
            if app.installed_only_mode {
                // Always target Downgrade first from Search
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                    app.downgrade_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::utils::refresh_downgrade_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                refresh_install_details(app, details_tx);
            }
        }
        (KeyCode::Left, _) => {
            // Search -> Recent (adjacent)
            if app.history_state.selected().is_none() && !app.recent.is_empty() {
                app.history_state.select(Some(0));
            }
            app.focus = crate::state::Focus::Recent;
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
            if app.installed_only_mode
                && let Some(item) = app.results.get(app.selected).cloned()
            {
                crate::logic::add_to_downgrade_list(app, item);
                // Do not change focus; only update details to reflect the new selection
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        (KeyCode::Char(' '), _) => {
            if let Some(item) = app.results.get(app.selected).cloned() {
                if app.installed_only_mode {
                    crate::logic::add_to_remove_list(app, item);
                    super::utils::refresh_remove_details(app, details_tx);
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
                // Confirm single install
                app.modal = crate::state::Modal::ConfirmInstall { items: vec![item] };
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
        (KeyCode::Up, _) => move_sel_cached(app, -1, details_tx),
        (KeyCode::Down, _) => move_sel_cached(app, 1, details_tx),
        (KeyCode::PageUp, _) => move_sel_cached(app, -10, details_tx),
        (KeyCode::PageDown, _) => move_sel_cached(app, 10, details_tx),
        _ => {}
    }
    false
}
