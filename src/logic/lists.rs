use crate::state::{AppState, PackageItem};

/// What: Add a `PackageItem` to the install list if it is not already present.
///
/// Inputs:
/// - `app`: Mutable application state (`install_list` and selection)
/// - `item`: Package to add
///
/// Output:
/// - Inserts at the front on success, marks list dirty, and selects index 0; no-op on dedup.
///
/// Details:
/// - Updates `last_install_change` to support UI throttling of follow-up actions.
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
    app.last_install_change = Some(std::time::Instant::now());
    // Always keep cursor on top after adding
    app.install_state.select(Some(0));
}

/// What: Add a `PackageItem` to the remove list if it is not already present.
///
/// Inputs:
/// - `app`: Mutable application state (`remove_list` and selection)
/// - `item`: Package to add
///
/// Output:
/// - Inserts at the front and selects index 0; no-op on dedup.
///
/// Details:
/// - Leaves `remove_list` order deterministic by always pushing new entries to the head.
pub fn add_to_remove_list(app: &mut AppState, item: PackageItem) {
    if app
        .remove_list
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&item.name))
    {
        return;
    }
    app.remove_list.insert(0, item);
    app.remove_state.select(Some(0));
}

/// What: Add a `PackageItem` to the downgrade list if it is not already present.
///
/// Inputs:
/// - `app`: Mutable application state (`downgrade_list` and selection)
/// - `item`: Package to add
///
/// Output:
/// - Inserts at the front and selects index 0; no-op on dedup.
///
/// Details:
/// - Ensures repeated requests for the same package keep the cursor anchored at the newest item.
pub fn add_to_downgrade_list(app: &mut AppState, item: PackageItem) {
    if app
        .downgrade_list
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&item.name))
    {
        return;
    }
    app.downgrade_list.insert(0, item);
    app.downgrade_state.select(Some(0));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_official(name: &str, repo: &str) -> PackageItem {
        PackageItem {
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
    /// What: Ensure the install list deduplicates entries case-insensitively and updates selection state.
    ///
    /// Inputs:
    /// - Two package items whose names differ only by casing.
    ///
    /// Output:
    /// - Install list contains a single entry, marked dirty, with the selection pointing at index `0`.
    ///
    /// Details:
    /// - Exercises the guard path preventing duplicate installs and verifies the UI selection remains anchored on insert.
    fn add_to_install_list_behavior() {
        let mut app = AppState {
            ..Default::default()
        };
        add_to_install_list(&mut app, item_official("pkg1", "core"));
        add_to_install_list(&mut app, item_official("Pkg1", "core"));
        assert_eq!(app.install_list.len(), 1);
        assert!(app.install_dirty);
        assert_eq!(app.install_state.selected(), Some(0));
    }

    #[test]
    /// What: Confirm the remove list enforces case-insensitive uniqueness and selection updates.
    ///
    /// Inputs:
    /// - Two package items whose names differ only by casing.
    ///
    /// Output:
    /// - Remove list retains a single item and its selection index becomes `0`.
    ///
    /// Details:
    /// - Protects against regressions where duplicates might shift the selection or leak into the list.
    fn add_to_remove_list_behavior() {
        let mut app = AppState {
            ..Default::default()
        };
        add_to_remove_list(&mut app, item_official("pkg1", "extra"));
        add_to_remove_list(&mut app, item_official("Pkg1", "extra"));
        assert_eq!(app.remove_list.len(), 1);
        assert_eq!(app.remove_state.selected(), Some(0));
    }

    #[test]
    /// What: Verify the downgrade list rejects duplicate names regardless of case and updates selection.
    ///
    /// Inputs:
    /// - Two package items whose names differ only by casing.
    ///
    /// Output:
    /// - Downgrade list contains one item and the selection index resolves to `0`.
    ///
    /// Details:
    /// - Ensures repeated downgrade requests do not reorder the cursor unexpectedly.
    fn add_to_downgrade_list_behavior() {
        let mut app = AppState {
            ..Default::default()
        };
        add_to_downgrade_list(&mut app, item_official("PkgX", "extra"));
        add_to_downgrade_list(&mut app, item_official("pkgx", "extra"));
        assert_eq!(app.downgrade_list.len(), 1);
        assert_eq!(app.downgrade_state.selected(), Some(0));
    }
}
