//! Management of install, remove, and downgrade package lists.

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
/// - Uses `HashSet` for O(1) membership checking instead of linear scan.
pub fn add_to_install_list(app: &mut AppState, item: PackageItem) {
    let name_lower = item.name.to_lowercase();
    if !app.install_list_names.insert(name_lower) {
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
/// - Uses `HashSet` for O(1) membership checking instead of linear scan.
pub fn add_to_remove_list(app: &mut AppState, item: PackageItem) {
    let name_lower = item.name.to_lowercase();
    if !app.remove_list_names.insert(name_lower) {
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
/// - Uses `HashSet` for O(1) membership checking instead of linear scan.
pub fn add_to_downgrade_list(app: &mut AppState, item: PackageItem) {
    let name_lower = item.name.to_lowercase();
    if !app.downgrade_list_names.insert(name_lower) {
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
            out_of_date: None,
            orphaned: false,
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
        let mut app = AppState::default();
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
        let mut app = AppState::default();
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
        let mut app = AppState::default();
        add_to_downgrade_list(&mut app, item_official("PkgX", "extra"));
        add_to_downgrade_list(&mut app, item_official("pkgx", "extra"));
        assert_eq!(app.downgrade_list.len(), 1);
        assert_eq!(app.downgrade_state.selected(), Some(0));
    }

    #[test]
    /// What: Verify `HashSet` synchronization after adding and removing items from install list.
    ///
    /// Inputs:
    /// - Add items to install list, then remove them.
    ///
    /// Output:
    /// - `HashSet` contains names only when items are in the list.
    ///
    /// Details:
    /// - Ensures `HashSet` stays synchronized with the `Vec` for O(1) membership checking.
    fn install_list_hashset_synchronization() {
        let mut app = AppState::default();
        add_to_install_list(&mut app, item_official("pkg1", "core"));
        add_to_install_list(&mut app, item_official("pkg2", "extra"));
        assert!(app.install_list_names.contains("pkg1"));
        assert!(app.install_list_names.contains("pkg2"));
        assert_eq!(app.install_list_names.len(), 2);

        // Remove first item (pkg2 is at index 0 since it was added last)
        // Items are inserted at index 0, so order is: [pkg2, pkg1]
        let removed_name = app.install_list[0].name.to_lowercase();
        app.install_list_names.remove(&removed_name);
        app.install_list.remove(0);
        // After removing pkg2, pkg1 should remain
        assert!(app.install_list_names.contains("pkg1"));
        assert!(!app.install_list_names.contains("pkg2"));
        assert_eq!(app.install_list_names.len(), 1);
    }

    #[test]
    /// What: Verify `HashSet` synchronization after clearing install list.
    ///
    /// Inputs:
    /// - Add items to install list, then clear it.
    ///
    /// Output:
    /// - `HashSet` is empty after clearing.
    ///
    /// Details:
    /// - Ensures `HashSet` is cleared when list is cleared.
    fn install_list_hashset_clear_synchronization() {
        let mut app = AppState::default();
        add_to_install_list(&mut app, item_official("pkg1", "core"));
        add_to_install_list(&mut app, item_official("pkg2", "extra"));
        assert_eq!(app.install_list_names.len(), 2);

        app.install_list.clear();
        app.install_list_names.clear();
        assert!(app.install_list_names.is_empty());
    }
}
