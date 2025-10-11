use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// Move the selection by `delta` and update cached details and prefetch policy.
///
/// Behavior:
/// - Clamps the selection to the valid range and updates the list state.
/// - Focuses the details pane on the newly selected item and immediately shows
///   a placeholder based on known metadata.
/// - If cached details are present, uses them; otherwise requests loading via
///   `details_tx`.
/// - Tracks cumulative scroll moves to detect fast scrolling.
/// - During fast scrolls, temporarily tightens the allowed set to only the
///   current selection and defers ring prefetch for ~200ms.
/// - For small scrolls, expands allowed set to a 30-item ring and begins
///   background prefetch.
pub fn move_sel_cached(
    app: &mut AppState,
    delta: isize,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    if app.results.is_empty() {
        return;
    }
    let len = app.results.len() as isize;
    let mut idx = app.selected as isize + delta;
    if idx < 0 {
        idx = 0;
    }
    if idx >= len {
        idx = len - 1;
    }
    app.selected = idx as usize;
    app.list_state.select(Some(app.selected));
    if let Some(item) = app.results.get(app.selected).cloned() {
        // Focus details on the currently selected item only
        app.details_focus = Some(item.name.clone());

        // Update details pane immediately with a placeholder reflecting the selection
        app.details.name = item.name.clone();
        app.details.version = item.version.clone();
        app.details.description.clear();
        match &item.source {
            crate::state::Source::Official { repo, arch } => {
                app.details.repository = repo.clone();
                app.details.architecture = arch.clone();
            }
            crate::state::Source::Aur => {
                app.details.repository = "AUR".to_string();
                app.details.architecture = "any".to_string();
            }
        }

        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }

    // Debounce ring prefetch when scrolling fast (>5 items cumulatively)
    let abs_delta_usize: usize = if delta < 0 {
        (-delta) as usize
    } else {
        delta as usize
    };
    if abs_delta_usize > 0 {
        let add = abs_delta_usize.min(u32::MAX as usize) as u32;
        app.scroll_moves = app.scroll_moves.saturating_add(add);
    }
    if app.need_ring_prefetch {
        // tighten allowed set to only current selection during fast scroll
        crate::logic::set_allowed_only_selected(app);
        app.ring_resume_at =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        return;
    }
    if app.scroll_moves > 5 {
        app.need_ring_prefetch = true;
        crate::logic::set_allowed_only_selected(app);
        app.ring_resume_at =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        return;
    }

    // For small/slow scrolls, allow ring and prefetch immediately
    crate::logic::set_allowed_ring(app, 30);
    crate::logic::ring_prefetch_from_selected(app, details_tx);
}
