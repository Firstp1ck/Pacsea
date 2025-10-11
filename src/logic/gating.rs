use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

use crate::state::AppState;

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
