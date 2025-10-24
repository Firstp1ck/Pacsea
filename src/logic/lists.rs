use crate::state::{AppState, PackageItem};

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
    app.last_install_change = Some(std::time::Instant::now());
    // Always keep cursor on top after adding
    app.install_state.select(Some(0));
}

/// Add a `PackageItem` to the remove list if it is not already present.
///
/// Inserts at the front and selects the first entry to keep it visible.
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

/// Add a `PackageItem` to the downgrade list if it is not already present.
///
/// Inserts at the front and selects the first entry to keep it visible.
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
    /// What: Add items to install list with case-insensitive dedup and selection
    ///
    /// - Input: Two items with same name differing in case
    /// - Output: Only one entry remains; list marked dirty; selection at 0
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
    /// What: Add items to remove list with case-insensitive dedup and selection
    ///
    /// - Input: Two items with same name differing in case
    /// - Output: Only one entry remains; selection at 0
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
    /// What: Add items to downgrade list with case-insensitive dedup and selection
    ///
    /// - Input: Two items with same name differing in case
    /// - Output: Only one entry remains; selection at 0
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
