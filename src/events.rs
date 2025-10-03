//! Event handling layer for Pacsea's TUI.
//!
//! This module centralizes keyboard and mouse input handling for the
//! three-pane interface:
//!
//! - Search (center): query input and results navigation
//! - Recent (left): previously used queries
//! - Install (right): pending install list and confirmation modal
//!
//! High-level behavior:
//!
//! - Converts raw `crossterm` events into mutations on [`AppState`]
//! - Coordinates background requests via async channels (query/details/preview/add)
//! - Implements pane-local search ("/") and Vim-like navigation ("j"/"k")
//! - Manages modal dialogs (alert and install confirmation)
//! - Opens package URLs on mouse click
//!
//! All functions in this module are synchronous and manipulate the provided
//! mutable [`AppState`]. Any long-running work is delegated to other modules or
//! spawned tasks to keep input handling responsive.
use crossterm::event::{Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

use crate::{
    logic::{move_sel_cached, send_query},
    state::{AppState, Focus, PackageItem, QueryInput},
};
use crate::theme::reload_theme;

/// Advance selection in the Recent pane to the next or previous item matching
/// the current pane-find pattern.
///
/// - Matching is case-insensitive and performed against the raw recent query
///   text.
/// - Search wraps around and respects the current filtered view (via
///   `filtered_recent_indices`).
/// - When no pattern is set or the list is empty, the function is a no-op.
///
/// The function updates `app.history_state` in place and does not emit any
/// I/O. Callers typically follow a successful move by triggering a preview
/// update.
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
        } else if vi == 0 {
            n - 1
        } else {
            vi - 1
        };
        let i = inds[vi];
        if let Some(s) = app.recent.get(i)
            && s.to_lowercase().contains(&pattern.to_lowercase())
        {
            app.history_state.select(Some(vi));
            break;
        }
    }
}

/// Advance selection in the Install pane to the next or previous item whose
/// name or description matches the pane-find pattern.
///
/// - Case-insensitive matching against both `name` and `description`
/// - Respects filtered indices and wraps around
/// - No effect when the pattern or list is empty
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
        } else if vi == 0 {
            n - 1
        } else {
            vi - 1
        };
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

/// Dispatch a single input event, mutating [`AppState`] and coordinating
/// background work via the provided channels.
///
/// Returns `true` to signal the application should exit (e.g., `Esc` or
/// `Ctrl+C` in supported contexts); otherwise returns `false`.
///
/// Arguments:
///
/// - `ev`: A raw `crossterm` event (keyboard or mouse)
/// - `app`: Mutable application state to be updated
/// - `query_tx`: Sends search queries when input changes
/// - `details_tx`: Requests details for the currently selected result
/// - `preview_tx`: Requests preview for the selected recent query
/// - `add_tx`: Adds selected result(s) to the install list
///
/// Behavior overview:
///
/// - Only key presses (`KeyEventKind::Press`) are handled; other key event
///   kinds are ignored.
/// - Modal handling has precedence and captures `Enter`/`Esc` while a modal is
///   open. Confirmation can trigger installs.
/// - Pane focus controls which bindings are active:
///   - Recent: `j`/`k` or arrows to move; `Enter` to load into Search; `Space`
///     to add first match to install; `/` to start pane-find.
///   - Install: `j`/`k` or arrows to move; `Delete` to remove; `Shift+Delete`
///     to clear all; `Enter` to open confirmation modal; `/` to pane-find.
///   - Search: text input edits query and sends it; `Space` adds selection to
///     install; arrows/PageUp/PageDown move selection; `Enter` opens single
///     item confirmation.
/// - Focus switching: `Tab`/`BackTab`/`Left`/`Right` move focus between panes
///   while ensuring a valid selection where applicable.
/// - Mouse: A left-click inside the stored `url_button_rect` attempts to open
///   the details URL using `xdg-open` on a background thread.
///
/// Concurrency and side effects:
///
/// - May spawn a Tokio task to resolve a recent query to a concrete package
///   when pressing `Space` in the Recent pane.
/// - Sends messages over provided channels; failures are ignored to keep input
///   handling robust in the face of downstream shutdowns.
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
            crate::state::Modal::None => {}
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
                                if let Some(item) =
                                    crate::ui_helpers::fetch_first_match_for_query(q).await
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
                        // Open confirmation modal listing all items to be installed
                        app.modal = crate::state::Modal::ConfirmInstall {
                            items: app.install_list.clone(),
                        };
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
            // Reload theme from disk
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                match reload_theme() {
                    Ok(()) => {
                        // trigger a UI refresh by sending a tick via side-effect: no direct channel here,
                        // but returning false will cause next loop draw; also any state change will repaint.
                    }
                    Err(msg) => {
                        app.modal = crate::state::Modal::Alert { message: msg };
                    }
                }
            }
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
                    // Confirm single install
                    app.modal = crate::state::Modal::ConfirmInstall { items: vec![item] };
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
    // Mouse click handling for URL (always enabled)
    if let CEvent::Mouse(m) = ev {
        if let crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) = m.kind
        {
            if let Some((x, y, w, h)) = app.url_button_rect {
                let mx = m.column;
                let my = m.row;
                if mx >= x && mx < x + w && my >= y && my < y + h {
                    if !app.details.url.is_empty() {
                        let url = app.details.url.clone();
                        std::thread::spawn(move || {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(url)
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                        });
                    }
                }
            }
        }
        return false;
    }
    false
}

/// Ensure `app.details` reflects the currently selected result.
///
/// If details are cached for the selected item's name, updates `app.details`
/// from cache synchronously. Otherwise sends a details request over
/// `details_tx` for asynchronous population.
fn refresh_selected_details(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    if let Some(item) = app.results.get(app.selected).cloned() {
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}
