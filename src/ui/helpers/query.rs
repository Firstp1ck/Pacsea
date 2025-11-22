//! Query resolution and preview utilities.
//!
//! This module provides functions for resolving query strings to packages and triggering
//! asynchronous preview fetches.

use super::filter::filtered_recent_indices;
use crate::state::AppState;

/// What: Resolve a free-form query string to a best-effort matching package.
///
/// Inputs:
/// - `q`: Query string to resolve
///
/// Output:
/// - `Some(PackageItem)` per the priority rules below; `None` if nothing usable is found.
///
/// Details (selection priority):
///   1) Exact-name match from the official index;
///   2) Exact-name match from AUR;
///   3) First official result;
///   4) Otherwise, first AUR result.
///
/// Performs network I/O for AUR; tolerates errors.
pub async fn fetch_first_match_for_query(q: String) -> Option<crate::state::PackageItem> {
    // Prefer exact match from official index, then from AUR, else first official, then first AUR
    let official = crate::index::search_official(&q);
    if let Some(off) = official
        .iter()
        .find(|it| it.name.eq_ignore_ascii_case(&q))
        .cloned()
    {
        return Some(off);
    }
    let (aur, _errors) = crate::sources::fetch_all_with_errors(q.clone()).await;
    if let Some(a) = aur
        .iter()
        .find(|it| it.name.eq_ignore_ascii_case(&q))
        .cloned()
    {
        return Some(a);
    }
    if let Some(off) = official.first().cloned() {
        return Some(off);
    }
    aur.into_iter().next()
}

/// What: Trigger an asynchronous preview fetch for the selected Recent query when applicable.
///
/// Inputs:
/// - `app`: Application state (focus, selection, recent list)
/// - `preview_tx`: Channel to send the preview `PackageItem`
///
/// Output:
/// - Spawns a task to resolve and send a preview item; no return payload; exits early when inapplicable.
///
/// Details:
/// - Requires: focus on Recent, a valid selection within the filtered view, and a query string present.
/// - Resolves via [`fetch_first_match_for_query`] and sends over `preview_tx`; ignores send errors.
pub fn trigger_recent_preview(
    app: &AppState,
    preview_tx: &tokio::sync::mpsc::UnboundedSender<crate::state::PackageItem>,
) {
    if !matches!(app.focus, crate::state::Focus::Recent) {
        return;
    }
    let Some(idx) = app.history_state.selected() else {
        return;
    };
    let inds = filtered_recent_indices(app);
    if idx >= inds.len() {
        return;
    }
    let Some(q) = app.recent.get(inds[idx]).cloned() else {
        return;
    };
    let tx = preview_tx.clone();
    tokio::spawn(async move {
        if let Some(item) = fetch_first_match_for_query(q).await {
            let _ = tx.send(item);
        }
    });
}
