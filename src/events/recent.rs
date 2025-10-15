use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::logic::send_query;
use crate::state::{AppState, PackageItem, QueryInput};

use super::utils::{char_count, find_in_recent, refresh_selected_details};

pub fn handle_recent_key(
    ke: KeyEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    // Allow exiting with Ctrl+C while in Recent pane
    if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }

    // Pane-search mode first
    if app.pane_find.is_some() {
        match ke.code {
            KeyCode::Enter => {
                find_in_recent(app, true);
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
            }
            KeyCode::Esc => {
                app.pane_find = None;
            }
            KeyCode::Backspace => {
                if let Some(buf) = &mut app.pane_find {
                    buf.pop();
                }
            }
            KeyCode::Char(ch) => {
                if let Some(buf) = &mut app.pane_find {
                    buf.push(ch);
                }
            }
            _ => {}
        }
        return false;
    }

    let km = &app.keymap;
    // Match helper that treats Shift+<char> from config as equivalent to uppercase char without Shift from terminal
    let matches_any = |list: &Vec<crate::theme::KeyChord>| {
        list.iter().any(|c| {
            // Exact match first
            if (c.code, c.mods) == (ke.code, ke.modifiers) {
                return true;
            }
            // Equivalence: config Shift+char vs event uppercase char (no Shift)
            match (c.code, ke.code) {
                (
                    crossterm::event::KeyCode::Char(cfg_ch),
                    crossterm::event::KeyCode::Char(ev_ch),
                ) => {
                    let cfg_has_shift = c.mods.contains(crossterm::event::KeyModifiers::SHIFT);
                    let ev_has_no_shift =
                        !ke.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
                    cfg_has_shift && ev_has_no_shift && ev_ch == cfg_ch.to_ascii_uppercase()
                }
                _ => false,
            }
        })
    };

    match ke.code {
        KeyCode::Char('j') => {
            // vim down
            let inds = crate::ui::helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let max = inds.len().saturating_sub(1);
            let new = std::cmp::min(sel + 1, max);
            app.history_state.select(Some(new));
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Char('k') => {
            // vim up
            let inds = crate::ui::helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let new = sel.saturating_sub(1);
            app.history_state.select(Some(new));
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Char('/') => {
            app.pane_find = Some(String::new());
        }
        KeyCode::Esc => {
            app.focus = crate::state::Focus::Search;
            refresh_selected_details(app, details_tx);
        }
        code if matches_any(&km.pane_next) && code == ke.code => {
            // Recent -> Search (cycle)
            app.focus = crate::state::Focus::Search;
            refresh_selected_details(app, details_tx);
        }
        KeyCode::Left => {
            // Wrap-around: Recent (leftmost) -> Install (rightmost)
            if app.installed_only_mode {
                // In installed-only mode, land on the Remove subpane when wrapping
                app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                if app.remove_state.selected().is_none() && !app.remove_list.is_empty() {
                    app.remove_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::utils::refresh_remove_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::utils::refresh_install_details(app, details_tx);
            }
        }
        KeyCode::Right => {
            // Recent -> Search (adjacent)
            app.focus = crate::state::Focus::Search;
            refresh_selected_details(app, details_tx);
        }
        // Configurable clear-all for Recent (default: Shift+Del)
        code if matches_any(&km.recent_clear) && code == ke.code => {
            app.recent.clear();
            app.history_state.select(None);
            app.recent_dirty = true;
        }
        // Single delete in Recent via configured keys (default: d or Delete)
        code if matches_any(&km.recent_remove) && code == ke.code => {
            let inds = crate::ui::helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            if let Some(vsel) = app.history_state.selected() {
                let i = inds.get(vsel).copied().unwrap_or(0);
                if i < app.recent.len() {
                    app.recent.remove(i);
                    app.recent_dirty = true;
                    let vis_len = inds.len().saturating_sub(1); // now one less visible
                    if vis_len == 0 {
                        app.history_state.select(None);
                    } else {
                        let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                        app.history_state.select(Some(new_sel));
                        crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                    }
                }
            }
        }
        KeyCode::Down => {
            let inds = crate::ui::helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let max = inds.len().saturating_sub(1);
            let new = std::cmp::min(sel + 1, max);
            app.history_state.select(Some(new));
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Up => {
            let inds = crate::ui::helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let new = sel.saturating_sub(1);
            app.history_state.select(Some(new));
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Char(' ') => {
            let inds = crate::ui::helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            if let Some(vsel) = app.history_state.selected() {
                let i = inds.get(vsel).copied().unwrap_or(0);
                if let Some(q) = app.recent.get(i).cloned() {
                    let tx = add_tx.clone();
                    tokio::spawn(async move {
                        if let Some(item) = crate::ui::helpers::fetch_first_match_for_query(q).await
                        {
                            let _ = tx.send(item);
                        }
                    });
                }
            }
        }
        KeyCode::Enter => {
            let inds = crate::ui::helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            if let Some(vsel) = app.history_state.selected() {
                let i = inds.get(vsel).copied().unwrap_or(0);
                if let Some(q) = app.recent.get(i).cloned() {
                    app.input = q;
                    app.focus = crate::state::Focus::Search;
                    app.last_input_change = std::time::Instant::now();
                    app.last_saved_value = None;
                    // Position caret at end and clear selection
                    app.search_caret = char_count(&app.input);
                    app.search_select_anchor = None;
                    send_query(app, query_tx);
                }
            }
        }
        _ => {}
    }
    false
}
