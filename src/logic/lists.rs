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
