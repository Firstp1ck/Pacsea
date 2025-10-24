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

/// What: Check whether details loading is currently allowed for a package name.
///
/// Inputs:
/// - `name`: Package name to test
///
/// Output:
/// - `true` when the name is currently allowed; otherwise `false` (or `true` if the lock fails).
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
    /// What: Allowed set behavior for only-selected and ring modes
    ///
    /// - Input: Results with selected index; toggle only-selected then ring radius 1
    /// - Output: Only selected allowed first; after ring, neighbors allowed
    fn allowed_only_selected_and_ring() {
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        app.results = vec![
            item_official("a", "core"),
            item_official("b", "extra"),
            item_official("c", "extra"),
            item_official("d", "other"),
        ];
        app.selected = 1;
        set_allowed_only_selected(&app);
        assert!(is_allowed("b"));
        assert!(!is_allowed("a") || !is_allowed("c") || !is_allowed("d"));

        set_allowed_ring(&app, 1);
        assert!(is_allowed("a") || is_allowed("c"));
    }
}
