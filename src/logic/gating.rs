use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

use crate::state::AppState;

/// What: Lazily construct and return the global set of package names permitted for detail fetching.
///
/// Inputs:
/// - (none): Initializes an `RwLock<HashSet<String>>` on first access.
///
/// Output:
/// - Returns a reference to the lock guarding the allowed-name set.
///
/// Details:
/// - Uses `OnceLock` to avoid race conditions during initialization while keeping lookups fast.
fn allowed_set() -> &'static RwLock<HashSet<String>> {
    static ALLOWED: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    ALLOWED.get_or_init(|| RwLock::new(HashSet::new()))
}

/// What: Check whether details loading is currently allowed for a package name.
///
/// Inputs:
/// - `name`: Package name to test
///
/// Output:
/// - `true` when the name is currently allowed; otherwise `false` (or `true` if the lock fails).
///
/// Details:
/// - Fails open when the read lock cannot be acquired to avoid blocking UI interactions.
#[must_use]
pub fn is_allowed(name: &str) -> bool {
    allowed_set()
        .read()
        .ok()
        .map(|s| s.contains(name))
        .unwrap_or(true)
}

/// What: Restrict details loading to only the currently selected package.
///
/// Inputs:
/// - `app`: Application state to read the current selection from
///
/// Output:
/// - Updates the internal allowed set to contain only the selected package; no-op if none.
///
/// Details:
/// - Clears any previously allowed names to prioritise responsiveness during rapid navigation.
pub fn set_allowed_only_selected(app: &AppState) {
    if let Some(sel) = app.results.get(app.selected)
        && let Ok(mut w) = allowed_set().write()
    {
        w.clear();
        w.insert(sel.name.clone());
    }
}

/// What: Allow details loading for a "ring" around the current selection.
///
/// Inputs:
/// - `app`: Application state to read the current selection and results from
/// - `radius`: Number of neighbors above and below to include
///
/// Output:
/// - Updates the internal allowed set to the ring of names around the selection.
///
/// Details:
/// - Includes the selected package itself and symmetrically expands within bounds while respecting the radius.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item_official(name: &str, repo: &str) -> crate::state::PackageItem {
        crate::state::PackageItem {
            name: name.to_string(),
            version: "1.0".to_string(),
            description: format!("{name} desc"),
            source: crate::state::Source::Official {
                repo: repo.to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        }
    }

    #[test]
    /// What: Check allowed-set helpers toggle between single selection and ring modes.
    ///
    /// Inputs:
    /// - Results array with four packages and selected index set to one.
    ///
    /// Output:
    /// - Only the selected package allowed initially; after calling `set_allowed_ring`, adjacent packages become allowed.
    ///
    /// Details:
    /// - Validates transition between restrictive and radius-based gating policies.
    fn allowed_only_selected_and_ring() {
        let app = crate::state::AppState {
            results: vec![
                item_official("a", "core"),
                item_official("b", "extra"),
                item_official("c", "extra"),
                item_official("d", "other"),
            ],
            selected: 1,
            ..Default::default()
        };
        set_allowed_only_selected(&app);
        assert!(is_allowed("b"));
        assert!(!is_allowed("a") || !is_allowed("c") || !is_allowed("d"));

        set_allowed_ring(&app, 1);
        assert!(is_allowed("a") || is_allowed("c"));
    }
}
