use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::logic::send_query;
use crate::state::{AppState, PackageItem, QueryInput};

use super::utils::{char_count, find_in_recent, matches_any, refresh_selected_details};

/// What: Handle key events while in pane-find mode for the Recent pane.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state
/// - `preview_tx`: Channel to request preview of the selected recent item
///
/// Output:
/// - `true` if the key was handled, `false` otherwise
///
/// Details:
/// - Handles Enter (jump to next match), Esc (exit find mode), Backspace (delete char), and Char (append char).
fn handle_recent_find_mode(
    ke: &KeyEvent,
    app: &mut AppState,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
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
        _ => return false,
    }
    true
}

/// What: Move selection in the Recent pane up or down.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `down`: `true` to move down, `false` to move up
/// - `preview_tx`: Channel to request preview of the selected recent item
///
/// Output:
/// - `true` if selection was moved, `false` if the list is empty
///
/// Details:
/// - Moves selection within the filtered view and triggers preview.
fn move_recent_selection(
    app: &mut AppState,
    down: bool,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    let inds = crate::ui::helpers::filtered_recent_indices(app);
    if inds.is_empty() {
        return false;
    }
    let sel = app.history_state.selected().unwrap_or(0);
    let max = inds.len().saturating_sub(1);
    let new = if down {
        std::cmp::min(sel + 1, max)
    } else {
        sel.saturating_sub(1)
    };
    app.history_state.select(Some(new));
    crate::ui::helpers::trigger_recent_preview(app, preview_tx);
    true
}

/// What: Transition focus from Recent pane to Search pane.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request details when focus moves back to Search
/// - `activate_normal_mode`: If `true`, activate Search Normal mode
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - Switches focus to Search and refreshes details for the selected result.
fn transition_to_search(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    activate_normal_mode: bool,
) {
    app.focus = crate::state::Focus::Search;
    if activate_normal_mode {
        app.search_normal_mode = true;
    }
    if !matches!(app.app_mode, crate::state::types::AppMode::News) {
        refresh_selected_details(app, details_tx);
    }
}

/// What: Handle wrap-around navigation from Recent (leftmost) to Install (rightmost) pane.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request details for the focused item
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - In installed-only mode, lands on the Remove subpane when wrapping.
/// - Otherwise, lands on the Install list.
fn handle_recent_to_install_wrap(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        app.focus = crate::state::Focus::Install;
    } else if app.installed_only_mode {
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

/// What: Clear all entries from the Recent list.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - Clears the recent list, deselects any item, and marks the list as dirty.
fn clear_recent_list(app: &mut AppState) {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        app.news_recent.clear();
        app.history_state.select(None);
        app.news_recent_dirty = true;
    } else {
        app.recent.clear();
        app.history_state.select(None);
        app.recent_dirty = true;
    }
}

/// What: Remove the currently selected item from the Recent list.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `preview_tx`: Channel to request preview of the selected recent item
///
/// Output:
/// - `true` if an item was removed, `false` if the list is empty or no item is selected
///
/// Details:
/// - Removes the selected item and adjusts selection to remain valid.
fn remove_recent_item(app: &mut AppState, preview_tx: &mpsc::UnboundedSender<PackageItem>) -> bool {
    let inds = crate::ui::helpers::filtered_recent_indices(app);
    if inds.is_empty() {
        return false;
    }
    let Some(vsel) = app.history_state.selected() else {
        return false;
    };
    let i = inds.get(vsel).copied().unwrap_or(0);
    let removed = if matches!(app.app_mode, crate::state::types::AppMode::News) {
        let res = app.remove_news_recent_at(i).is_some();
        if res {
            app.news_recent_dirty = true;
        }
        res
    } else {
        let res = app.remove_recent_at(i).is_some();
        if res {
            app.recent_dirty = true;
        }
        res
    };
    if !removed {
        return false;
    }
    let vis_len = inds.len().saturating_sub(1); // now one less visible
    if vis_len == 0 {
        app.history_state.select(None);
    } else {
        let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
        app.history_state.select(Some(new_sel));
        if !matches!(app.app_mode, crate::state::types::AppMode::News) {
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
    }
    true
}

/// What: Get the selected recent query string.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - `Some(String)` if a valid selection exists, `None` otherwise
///
/// Details:
/// - Returns the query string at the currently selected visible index.
fn get_selected_recent_query(app: &AppState) -> Option<String> {
    let inds = crate::ui::helpers::filtered_recent_indices(app);
    if inds.is_empty() {
        return None;
    }
    let vsel = app.history_state.selected()?;
    let i = inds.get(vsel).copied()?;
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        app.news_recent_value_at(i)
    } else {
        app.recent_value_at(i)
    }
}

/// What: Handle Enter key to use the selected recent query.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send a new query to Search
///
/// Output:
/// - `true` if a query was used, `false` if no valid selection exists
///
/// Details:
/// - Copies the selected recent query into Search, positions caret at end, and triggers a new search.
fn handle_recent_enter(app: &mut AppState, query_tx: &mpsc::UnboundedSender<QueryInput>) -> bool {
    let Some(q) = get_selected_recent_query(app) else {
        return false;
    };
    app.focus = crate::state::Focus::Search;
    app.last_input_change = std::time::Instant::now();
    app.last_saved_value = None;
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        app.news_search_input = q.clone();
        app.input = q;
        app.search_caret = char_count(&app.news_search_input);
        app.search_select_anchor = None;
        app.refresh_news_results();
    } else {
        app.input = q;
        // Position caret at end and clear selection
        app.search_caret = char_count(&app.input);
        app.search_select_anchor = None;
        send_query(app, query_tx);
    }
    true
}

/// What: Handle Space key to add the selected recent query as a best-effort match to install list.
///
/// Inputs:
/// - `app`: Application state
/// - `add_tx`: Channel to enqueue adding a best-effort match to the install list
///
/// Output:
/// - `true` if a task was spawned, `false` if no valid selection exists
///
/// Details:
/// - Asynchronously resolves a best-effort match and enqueues it to the install list.
fn handle_recent_space(app: &AppState, add_tx: &mpsc::UnboundedSender<PackageItem>) -> bool {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        return false;
    }
    let Some(q) = get_selected_recent_query(app) else {
        return false;
    };
    let tx = add_tx.clone();
    tokio::spawn(async move {
        if let Some(item) = crate::ui::helpers::fetch_first_match_for_query(q).await {
            let _ = tx.send(item);
        }
    });
    true
}

/// What: Handle key events while the Recent pane (left column) is focused.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state (recent list, selection, find pattern)
/// - `query_tx`: Channel to send a new query to Search when Enter is pressed
/// - `details_tx`: Channel to request details when focus moves back to Search
/// - `preview_tx`: Channel to request preview of the selected recent item
/// - `add_tx`: Channel to enqueue adding a best-effort match to the install list
///
/// Output:
/// - `true` to request application exit (e.g., Ctrl+C); `false` to continue.
///
/// Details:
/// - In-pane find: `/` enters find mode; typing edits the pattern; Enter jumps to next match;
///   Esc cancels. Matches are case-insensitive on recent query strings.
/// - Navigation: `j/k` or `Down/Up` move selection within the filtered view and trigger preview.
/// - Use item: `Enter` copies the selected recent query into Search and triggers a new search.
/// - Add item: Space resolves a best-effort match asynchronously and enqueues it to install list.
/// - Removal: Configured keys (`recent_remove`/`recent_clear`) remove one/all entries.
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
    if app.pane_find.is_some() && handle_recent_find_mode(&ke, app, preview_tx) {
        return false; // Key was handled in find mode
    }

    let km = &app.keymap;

    match ke.code {
        KeyCode::Char('j') | KeyCode::Down => {
            move_recent_selection(app, true, preview_tx);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            move_recent_selection(app, false, preview_tx);
        }
        KeyCode::Char('/') => {
            app.pane_find = Some(String::new());
        }
        KeyCode::Esc => {
            transition_to_search(app, details_tx, true);
        }
        code if matches_any(&ke, &km.pane_next) && code == ke.code => {
            transition_to_search(app, details_tx, false);
        }
        KeyCode::Left => {
            handle_recent_to_install_wrap(app, details_tx);
        }
        KeyCode::Right => {
            transition_to_search(app, details_tx, false);
        }
        code if matches_any(&ke, &km.recent_clear) && code == ke.code => {
            clear_recent_list(app);
        }
        code if matches_any(&ke, &km.recent_remove) && code == ke.code => {
            remove_recent_item(app, preview_tx);
        }
        KeyCode::Char(' ') => {
            handle_recent_space(app, add_tx);
        }
        KeyCode::Enter => {
            handle_recent_enter(app, query_tx);
        }
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Provide a fresh `AppState` tailored for recent-pane tests without repeated boilerplate.
    ///
    /// Inputs:
    /// - None (relies on `Default::default()` for deterministic initial state).
    ///
    /// Output:
    /// - New `AppState` ready for mutating within individual recent-pane scenarios.
    ///
    /// Details:
    /// - Keeps tests concise by centralizing setup of the baseline application state.
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Exercise recent-pane find mode from entry through exit.
    ///
    /// Inputs:
    /// - Key sequence `'/ '`, `'a'`, `Enter`, `Esc` routed through the handler.
    ///
    /// Output:
    /// - `pane_find` initialises, captures search text, Enter triggers preview, and Escape clears the mode.
    ///
    /// Details:
    /// - Verifies the state transitions without asserting on query side-effects.
    fn recent_pane_find_flow() {
        let mut app = new_app();
        let (qtx, _qrx) = mpsc::unbounded_channel::<QueryInput>();
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();

        // Enter find mode
        let _ = handle_recent_key(
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &ptx,
            &atx,
        );
        assert_eq!(app.pane_find.as_deref(), Some(""));
        // Type 'a'
        let _ = handle_recent_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &ptx,
            &atx,
        );
        assert_eq!(app.pane_find.as_deref(), Some("a"));
        // Press Enter
        let _ = handle_recent_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &ptx,
            &atx,
        );
        // Exit find with Esc
        let _ = handle_recent_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &ptx,
            &atx,
        );
        assert!(app.pane_find.is_none());
    }

    #[test]
    /// What: Confirm Enter on a recent entry restores the search query and emits a request.
    ///
    /// Inputs:
    /// - Recent list with a single item selected and an `Enter` key event.
    ///
    /// Output:
    /// - Focus switches to `Search`, the input field reflects the selection, and a query message is queued.
    ///
    /// Details:
    /// - Uses unbounded channels to capture the emitted query without running async tasks.
    fn recent_enter_uses_query() {
        let mut app = new_app();
        app.load_recent_items(&["ripgrep".to_string()]);
        app.history_state.select(Some(0));
        let (qtx, mut qrx) = mpsc::unbounded_channel::<QueryInput>();
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let _ = handle_recent_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &ptx,
            &atx,
        );
        assert!(matches!(app.focus, crate::state::Focus::Search));
        let msg = qrx.try_recv().ok();
        assert!(msg.is_some());
        assert_eq!(app.input, "ripgrep");
    }
}
