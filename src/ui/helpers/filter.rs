//! Filtering utilities for pane-specific index calculations.
//!
//! This module provides functions for filtering indices in the Recent and Install panes
//! based on pane-find queries.

use crate::state::{AppState, Focus};

/// What: Produce visible indices into `app.recent` considering pane-find when applicable.
///
/// Inputs:
/// - `app`: Application state (focus, `pane_find`, recent list)
///
/// Output:
/// - Vector of indices in ascending order without modifying application state.
///
/// # Panics
/// - Panics if `pane_find` is `Some` but becomes `None` between the check and the `expect` call (should not happen in single-threaded usage)
///
/// Details:
/// - Applies pane find filtering only when the Recent pane is focused and the finder string is
///   non-empty; otherwise returns the full range.
#[must_use]
pub fn filtered_recent_indices(app: &AppState) -> Vec<usize> {
    let apply =
        matches!(app.focus, Focus::Recent) && app.pane_find.as_ref().is_some_and(|s| !s.is_empty());
    let recents = app.recent_values();
    if !apply {
        return (0..recents.len()).collect();
    }
    let pat = app
        .pane_find
        .as_ref()
        .expect("pane_find should be Some when apply is true")
        .to_lowercase();
    recents
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if s.to_lowercase().contains(&pat) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

/// What: Produce visible indices into `app.install_list` with optional pane-find filtering.
///
/// Inputs:
/// - `app`: Application state (focus, `pane_find`, install list)
///
/// Output:
/// - Vector of indices in ascending order without modifying application state.
///
/// # Panics
/// - Panics if `pane_find` is `Some` but becomes `None` between the check and the `expect` call (should not happen in single-threaded usage)
///
/// Details:
/// - Restricts matches to name or description substrings when the Install pane is focused and a
///   pane-find expression is active; otherwise surfaces all indices.
#[must_use]
pub fn filtered_install_indices(app: &AppState) -> Vec<usize> {
    let apply = matches!(app.focus, Focus::Install)
        && app.pane_find.as_ref().is_some_and(|s| !s.is_empty());
    if !apply {
        return (0..app.install_list.len()).collect();
    }
    let pat = app
        .pane_find
        .as_ref()
        .expect("pane_find should be Some when apply is true")
        .to_lowercase();
    app.install_list
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let name = p.name.to_lowercase();
            let desc = p.description.to_lowercase();
            if name.contains(&pat) || desc.contains(&pat) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}
