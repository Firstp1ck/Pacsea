use crossterm::event::{Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

use crate::{
    logic::{move_sel_cached, send_query},
    state::{AppState, Focus, PackageItem, QueryInput},
};

fn find_in_recent(app: &mut AppState, forward: bool) {
    let Some(pattern) = app.pane_find.clone() else {
        return;
    };
    let inds = crate::ui_helpers::filtered_recent_indices(app);
    if inds.is_empty() {
        return;
    }
    let start = app.history_state.selected().unwrap_or(0);
    let mut vi = start;
    let n = inds.len();
    for _ in 0..n {
        vi = if forward {
            (vi + 1) % n
        } else if vi == 0 { n - 1 } else { vi - 1 };
        let i = inds[vi];
        if let Some(s) = app.recent.get(i)
            && s.to_lowercase().contains(&pattern.to_lowercase()) {
                app.history_state.select(Some(vi));
                break;
            }
    }
}

fn find_in_install(app: &mut AppState, forward: bool) {
    let Some(pattern) = app.pane_find.clone() else {
        return;
    };
    let inds = crate::ui_helpers::filtered_install_indices(app);
    if inds.is_empty() {
        return;
    }
    let start = app.install_state.selected().unwrap_or(0);
    let mut vi = start;
    let n = inds.len();
    for _ in 0..n {
        vi = if forward {
            (vi + 1) % n
        } else if vi == 0 { n - 1 } else { vi - 1 };
        let i = inds[vi];
        if let Some(p) = app.install_list.get(i)
            && (p.name.to_lowercase().contains(&pattern.to_lowercase())
                || p.description
                    .to_lowercase()
                    .contains(&pattern.to_lowercase()))
            {
                app.install_state.select(Some(vi));
                break;
            }
    }
}

pub fn handle_event(
    ev: CEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if let CEvent::Key(ke) = ev {
        if ke.kind != KeyEventKind::Press {
            return false;
        }

        // Alert modal
        if let crate::state::Modal::Alert { .. } = app.modal {
            match ke.code {
                KeyCode::Enter | KeyCode::Esc => {
                    app.modal = crate::state::Modal::None;
                }
                _ => {}
            }
            return false;
        }

        // Recent pane focused
        if matches!(app.focus, Focus::Recent) {
            // Allow exiting with Ctrl+C while in Recent pane
            if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
                return true;
            }
            // If in pane-search mode, only handle find editing/confirm/cancel
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
                    app.focus = Focus::Search;
                    refresh_selected_details(app, details_tx);
                }
                KeyCode::Tab => {
                    // Recent -> Search (cycle)
                    app.focus = Focus::Search;
                    refresh_selected_details(app, details_tx);
                }
                KeyCode::BackTab => {
                    app.focus = Focus::Search;
                    refresh_selected_details(app, details_tx);
                }
                KeyCode::Left => { /* no-op: already at leftmost pane */ }
                KeyCode::Right => {
                    // Recent -> Search (adjacent)
                    app.focus = Focus::Search;
                    refresh_selected_details(app, details_tx);
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
                                let (items, _errors) =
                                    crate::fetch_all_with_errors(q.clone()).await;
                                if items.is_empty() {
                                    return;
                                }
                                if let Some(item) = items
                                    .iter()
                                    .find(|it| it.name.eq_ignore_ascii_case(&q))
                                    .cloned()
                                {
                                    let _ = tx.send(item);
                                } else {
                                    let _ = tx.send(items[0].clone());
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
                            app.focus = Focus::Search;
                            app.last_input_change = std::time::Instant::now();
                            app.last_saved_value = None;
                            send_query(app, query_tx);
                        }
                    }
                }
                _ => {}
            }
            return false;
        }

        // Install pane focused
        if matches!(app.focus, Focus::Install) {
            if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
                return true;
            }
            // Pane-search mode first
            if app.pane_find.is_some() {
                match ke.code {
                    KeyCode::Enter => {
                        find_in_install(app, true);
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
            match ke.code {
                KeyCode::Char('j') => {
                    // vim down
                    let inds = crate::ui_helpers::filtered_install_indices(app);
                    if inds.is_empty() {
                        return false;
                    }
                    let sel = app.install_state.selected().unwrap_or(0);
                    let max = inds.len().saturating_sub(1);
                    let new = std::cmp::min(sel + 1, max);
                    app.install_state.select(Some(new));
                }
                KeyCode::Char('k') => {
                    // vim up
                    let inds = crate::ui_helpers::filtered_install_indices(app);
                    if inds.is_empty() {
                        return false;
                    }
                    if let Some(sel) = app.install_state.selected() {
                        let new = sel.saturating_sub(1);
                        app.install_state.select(Some(new));
                    }
                }
                KeyCode::Char('/') => {
                    app.pane_find = Some(String::new());
                }
                KeyCode::Enter => {
                    if !app.install_list.is_empty() {
                        crate::install::spawn_install_all(&app.install_list, app.dry_run);
                    }
                }
                KeyCode::Esc => {
                    app.focus = Focus::Search;
                }
                KeyCode::Tab => {
                    // Install -> Recent (cycle)
                    if app.history_state.selected().is_none() && !app.recent.is_empty() {
                        app.history_state.select(Some(0));
                    }
                    app.focus = Focus::Recent;
                    crate::ui_helpers::trigger_recent_preview(app, preview_tx);
                }
                KeyCode::BackTab => {
                    app.focus = Focus::Recent;
                }
                KeyCode::Left => {
                    // Install -> Search (adjacent)
                    app.focus = Focus::Search;
                }
                KeyCode::Right => { /* no-op: rightmost pane */ }
                KeyCode::Delete if ke.modifiers.contains(KeyModifiers::SHIFT) => {
                    app.install_list.clear();
                    app.install_state.select(None);
                    app.install_dirty = true;
                }
                KeyCode::Delete => {
                    let inds = crate::ui_helpers::filtered_install_indices(app);
                    if inds.is_empty() {
                        return false;
                    }
                    if let Some(vsel) = app.install_state.selected() {
                        let i = inds.get(vsel).copied().unwrap_or(0);
                        if i < app.install_list.len() {
                            app.install_list.remove(i);
                            app.install_dirty = true;
                            let vis_len = inds.len().saturating_sub(1); // one less visible
                            if vis_len == 0 {
                                app.install_state.select(None);
                            } else {
                                let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                                app.install_state.select(Some(new_sel));
                            }
                        }
                    }
                }
                KeyCode::Up => {
                    let inds = crate::ui_helpers::filtered_install_indices(app);
                    if inds.is_empty() {
                        return false;
                    }
                    if let Some(sel) = app.install_state.selected() {
                        let new = sel.saturating_sub(1);
                        app.install_state.select(Some(new));
                    }
                }
                KeyCode::Down => {
                    let inds = crate::ui_helpers::filtered_install_indices(app);
                    if inds.is_empty() {
                        return false;
                    }
                    let sel = app.install_state.selected().unwrap_or(0);
                    let max = inds.len().saturating_sub(1);
                    let new = std::cmp::min(sel + 1, max);
                    app.install_state.select(Some(new));
                }
                _ => {}
            }
            return false;
        }

        // Normal mode (Search focused)
        let KeyEvent {
            code, modifiers, ..
        } = ke;
        match (code, modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Tab, _) => {
                // Search -> Install (cycle)
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = Focus::Install;
            }
            (KeyCode::BackTab, _) => {
                // Search -> Install (unchanged)
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = Focus::Install;
            }
            (KeyCode::Right, _) => {
                // Search -> Install (adjacent)
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = Focus::Install;
            }
            (KeyCode::Left, _) => {
                // Search -> Recent (adjacent)
                if app.history_state.selected().is_none() && !app.recent.is_empty() {
                    app.history_state.select(Some(0));
                }
                app.focus = Focus::Recent;
                crate::ui_helpers::trigger_recent_preview(app, preview_tx);
            }
            (KeyCode::Char(' '), _) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    let _ = add_tx.send(item);
                }
            }
            (KeyCode::Backspace, _) => {
                app.input.pop();
                app.last_input_change = std::time::Instant::now();
                app.last_saved_value = None;
                send_query(app, query_tx);
            }
            (KeyCode::Char('\n') | KeyCode::Enter, _) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    crate::install::spawn_install(&item, None, app.dry_run);
                }
            }
            (KeyCode::Char(ch), _) => {
                app.input.push(ch);
                app.last_input_change = std::time::Instant::now();
                app.last_saved_value = None;
                send_query(app, query_tx);
            }
            (KeyCode::Up, _) => move_sel_cached(app, -1, details_tx),
            (KeyCode::Down, _) => move_sel_cached(app, 1, details_tx),
            (KeyCode::PageUp, _) => move_sel_cached(app, -10, details_tx),
            (KeyCode::PageDown, _) => move_sel_cached(app, 10, details_tx),
            _ => {}
        }
    }
    false
}

fn refresh_selected_details(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    if let Some(item) = app.results.get(app.selected).cloned() {
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}
