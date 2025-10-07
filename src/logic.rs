//! Interaction logic for selection movement, search dispatching, and
//! ring-based details prefetching.
//!
//! This module centralizes the non-UI behavior that reacts to user navigation
//! and search input:
//! - Maintains an "allowed" set of package names to throttle background
//!   details fetching when the user scrolls quickly.
//! - Provides ring-based prefetch to load details for neighbors around the
//!   current selection, improving perceived responsiveness.
//! - Sends debounced/throttled search queries with monotonically increasing ids
//!   so the UI can ignore stale responses.
//! - Manages the install list with simple de-duplication and cursor behavior.
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem, SortMode, Source};

// Global allowed-names gating for details loading
use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};
/// Lazily-initialized global set of package names that are permitted to have
/// details fetched right now.
///
/// The set is updated by `set_allowed_only_selected` and `set_allowed_ring`
/// to balance responsiveness against network and CPU usage when the user is
/// scrolling rapidly.
fn allowed_set() -> &'static RwLock<HashSet<String>> {
    static ALLOWED: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    ALLOWED.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Returns whether details loading is currently allowed for the given package
/// `name`.
///
/// If the lock cannot be acquired, this conservatively returns `true` to avoid
/// blocking the UI with spurious denials.
pub fn is_allowed(name: &str) -> bool {
    allowed_set()
        .read()
        .ok()
        .map(|s| s.contains(name))
        .unwrap_or(true)
}

/// Restrict details loading to only the currently selected package.
///
/// This is used during fast scrolling to minimize wasted work. If there is no
/// current selection, this function is a no-op.
pub fn set_allowed_only_selected(app: &AppState) {
    if let Some(sel) = app.results.get(app.selected)
        && let Ok(mut w) = allowed_set().write()
    {
        w.clear();
        w.insert(sel.name.clone());
    }
}

/// Allow details loading for a "ring" around the current selection.
///
/// The ring consists of the selected package plus up to `radius` neighbors
/// above and below (clamped to list bounds). This enables background prefetch
/// for items the user is likely to navigate to next.
pub fn set_allowed_ring(app: &AppState, radius: usize) {
    let mut ring: HashSet<String> = HashSet::new();
    if let Some(sel) = app.results.get(app.selected) {
        ring.insert(sel.name.clone());
    }
    let len = app.results.len();
    let mut step = 1usize;
    while step <= radius {
        if let Some(i) = app.selected.checked_sub(step)
            && let Some(it) = app.results.get(i)
        {
            ring.insert(it.name.clone());
        }
        let below = app.selected + step;
        if below < len
            && let Some(it) = app.results.get(below)
        {
            ring.insert(it.name.clone());
        }
        step += 1;
    }
    if let Ok(mut w) = allowed_set().write() {
        *w = ring;
    }
}

/// Send the current query text over the search channel with a fresh id.
///
/// Side effects on `app`:
/// - Increments and records `next_query_id`
/// - Updates `latest_query_id` to the id sent
///
/// The id allows the receiver to tag results so the UI can discard any stale
/// responses that arrive out of order.
pub fn send_query(app: &mut AppState, query_tx: &mpsc::UnboundedSender<crate::state::QueryInput>) {
    let id = app.next_query_id;
    app.next_query_id += 1;
    app.latest_query_id = id;
    let _ = query_tx.send(crate::state::QueryInput {
        id,
        text: app.input.clone(),
    });
}

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
        set_allowed_only_selected(app);
        app.ring_resume_at =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        return;
    }
    if app.scroll_moves > 5 {
        app.need_ring_prefetch = true;
        set_allowed_only_selected(app);
        app.ring_resume_at =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        return;
    }

    // For small/slow scrolls, allow ring and prefetch immediately
    set_allowed_ring(app, 30);
    ring_prefetch_from_selected(app, details_tx);
}

/// Prefetch details for items near the current selection, alternating above and
/// below within a fixed radius.
///
/// Only enqueues requests for names allowed by `is_allowed` and not already in
/// the cache. This function is designed to be cheap and safe to call often.
pub fn ring_prefetch_from_selected(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    let len_u = app.results.len();
    if len_u == 0 {
        return;
    }
    let max_radius: usize = 30;
    let mut step: usize = 1;
    loop {
        let mut progressed = false;
        if let Some(i) = app.selected.checked_sub(step) {
            if let Some(it) = app.results.get(i).cloned()
                && is_allowed(&it.name)
                && !app.details_cache.contains_key(&it.name)
            {
                let _ = details_tx.send(it);
            }
            progressed = true;
        }
        let below = app.selected + step;
        if below < len_u {
            if let Some(it) = app.results.get(below).cloned()
                && is_allowed(&it.name)
                && !app.details_cache.contains_key(&it.name)
            {
                let _ = details_tx.send(it);
            }
            progressed = true;
        }
        if step >= max_radius || !progressed {
            break;
        }
        step += 1;
    }
}

/// Apply the currently selected sorting mode to `app.results` in-place.
///
/// Preserves the selection by trying to keep the same package name selected
/// after sorting, falling back to index clamping.
pub fn sort_results_preserve_selection(app: &mut AppState) {
    if app.results.is_empty() {
        return;
    }
    let prev_name = app.results.get(app.selected).map(|p| p.name.clone());
    match app.sort_mode {
        SortMode::RepoThenName => {
            app.results.sort_by(|a, b| {
                let oa = crate::util::repo_order(&a.source);
                let ob = crate::util::repo_order(&b.source);
                if oa != ob {
                    return oa.cmp(&ob);
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            });
        }
        SortMode::AurPopularityThenOfficial => {
            app.results.sort_by(|a, b| {
                // AUR first
                let aur_a = matches!(a.source, Source::Aur);
                let aur_b = matches!(b.source, Source::Aur);
                if aur_a != aur_b {
                    return aur_b.cmp(&aur_a); // true before false
                }
                if aur_a && aur_b {
                    // Desc popularity for AUR
                    let pa = a.popularity.unwrap_or(0.0);
                    let pb = b.popularity.unwrap_or(0.0);
                    if (pa - pb).abs() > f64::EPSILON {
                        return pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal);
                    }
                } else {
                    // Both official: keep pacman order (repo_order), then name
                    let oa = crate::util::repo_order(&a.source);
                    let ob = crate::util::repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            });
        }
    }
    if let Some(name) = prev_name {
        if let Some(pos) = app.results.iter().position(|p| p.name == name) {
            app.selected = pos;
            app.list_state.select(Some(pos));
        } else {
            app.selected = app.selected.min(app.results.len().saturating_sub(1));
            app.list_state.select(Some(app.selected));
        }
    }
}

/// Apply current repo/AUR filters to `app.all_results`, write into `app.results`, then sort.
///
/// Attempts to preserve the selection by name; falls back to clamping.
pub fn apply_filters_and_sort_preserve_selection(app: &mut AppState) {
    // Capture previous selected name to preserve when possible
    let prev_name = app.results.get(app.selected).map(|p| p.name.clone());

    // Filter from all_results into results based on toggles
    let mut filtered: Vec<PackageItem> = Vec::with_capacity(app.all_results.len());
    for it in app.all_results.iter().cloned() {
        let include = match &it.source {
            Source::Aur => app.results_filter_show_aur,
            Source::Official { repo, .. } => {
                let r = repo.to_lowercase();
                (r == "core" && app.results_filter_show_core)
                    || (r == "extra" && app.results_filter_show_extra)
                    || (r == "multilib" && app.results_filter_show_multilib)
                    || (!["core", "extra", "multilib"].contains(&r.as_str())
                        // If an official repo is not one of the three, include it only if at least one official filter is on
                        && (app.results_filter_show_core
                            || app.results_filter_show_extra
                            || app.results_filter_show_multilib))
            }
        };
        if include {
            filtered.push(it);
        }
    }
    app.results = filtered;
    // Apply existing sort policy and preserve selection
    sort_results_preserve_selection(app);
    // Restore by name if possible
    if let Some(name) = prev_name {
        if let Some(pos) = app.results.iter().position(|p| p.name == name) {
            app.selected = pos;
            app.list_state.select(Some(pos));
        } else if !app.results.is_empty() {
            app.selected = app.selected.min(app.results.len() - 1);
            app.list_state.select(Some(app.selected));
        } else {
            app.selected = 0;
            app.list_state.select(None);
        }
    } else if app.results.is_empty() {
        app.selected = 0;
        app.list_state.select(None);
    } else {
        app.selected = app.selected.min(app.results.len() - 1);
        app.list_state.select(Some(app.selected));
    }
}

/// Add a `PackageItem` to the install list if it is not already present.
///
/// The insertion happens at the front of the list. The list is de-duplicated by
/// case-insensitive package name. This marks the install list dirty and moves
/// the install list cursor to the first item to keep it visible.
pub fn add_to_install_list(app: &mut AppState, item: PackageItem) {
    if app
        .install_list
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&item.name))
    {
        return;
    }
    app.install_list.insert(0, item);
    app.install_dirty = true;
    // Always keep cursor on top after adding
    app.install_state.select(Some(0));
}
