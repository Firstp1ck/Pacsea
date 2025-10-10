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
                crate::ui_helpers::trigger_recent_preview(app, preview_tx);
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
    let chord = (ke.code, ke.modifiers);
    let matches_any =
        |list: &Vec<crate::theme::KeyChord>| list.iter().any(|c| (c.code, c.mods) == chord);

    match ke.code {
        KeyCode::Char('j') => {
            // vim down
            let inds = crate::ui_helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let max = inds.len().saturating_sub(1);
            let new = std::cmp::min(sel + 1, max);
            app.history_state.select(Some(new));
            crate::ui_helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Char('k') => {
            // vim up
            let inds = crate::ui_helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let new = sel.saturating_sub(1);
            app.history_state.select(Some(new));
            crate::ui_helpers::trigger_recent_preview(app, preview_tx);
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
        KeyCode::Left => { /* no-op: already at leftmost pane */ }
        KeyCode::Right => {
            // Recent -> Search (adjacent)
            app.focus = crate::state::Focus::Search;
            refresh_selected_details(app, details_tx);
        }
        KeyCode::Delete if ke.modifiers.contains(KeyModifiers::SHIFT) => {
            app.recent.clear();
            app.history_state.select(None);
            app.recent_dirty = true;
        }
        // Single delete in Recent via configured keys (default: d or Delete)
        code if matches_any(&km.recent_remove) && code == ke.code => {
            let inds = crate::ui_helpers::filtered_recent_indices(app);
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
                        crate::ui_helpers::trigger_recent_preview(app, preview_tx);
                    }
                }
            }
        }
        KeyCode::Down => {
            let inds = crate::ui_helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let max = inds.len().saturating_sub(1);
            let new = std::cmp::min(sel + 1, max);
            app.history_state.select(Some(new));
            crate::ui_helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Up => {
            let inds = crate::ui_helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            let sel = app.history_state.selected().unwrap_or(0);
            let new = sel.saturating_sub(1);
            app.history_state.select(Some(new));
            crate::ui_helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Char(' ') => {
            let inds = crate::ui_helpers::filtered_recent_indices(app);
            if inds.is_empty() {
                return false;
            }
            if let Some(vsel) = app.history_state.selected() {
                let i = inds.get(vsel).copied().unwrap_or(0);
                if let Some(q) = app.recent.get(i).cloned() {
                    let tx = add_tx.clone();
                    tokio::spawn(async move {
                        if let Some(item) = crate::ui_helpers::fetch_first_match_for_query(q).await
                        {
                            let _ = tx.send(item);
                        }
                    });
                }
            }
        }
        KeyCode::Enter => {
            let inds = crate::ui_helpers::filtered_recent_indices(app);
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
