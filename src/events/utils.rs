use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// Return the number of Unicode scalar values (characters) in the input.
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Convert a character index to a byte index for slicing.
/// If `ci` equals the number of characters, returns `s.len()`.
pub fn byte_index_for_char(s: &str, ci: usize) -> usize {
    let cc = char_count(s);
    if ci == 0 {
        return 0;
    }
    if ci >= cc {
        return s.len();
    }
    s.char_indices().map(|(i, _)| i).nth(ci).unwrap_or(s.len())
}

/// Advance selection in the Recent pane to the next or previous item matching
/// the current pane-find pattern.
pub fn find_in_recent(app: &mut AppState, forward: bool) {
    let Some(pattern) = app.pane_find.clone() else {
        return;
    };
    let inds = crate::ui::helpers::filtered_recent_indices(app);
    if inds.is_empty() {
        return;
    }
    let start = app.history_state.selected().unwrap_or(0);
    let mut vi = start;
    let n = inds.len();
    for _ in 0..n {
        vi = if forward {
            (vi + 1) % n
        } else if vi == 0 {
            n - 1
        } else {
            vi - 1
        };
        let i = inds[vi];
        if let Some(s) = app.recent.get(i)
            && s.to_lowercase().contains(&pattern.to_lowercase())
        {
            app.history_state.select(Some(vi));
            break;
        }
    }
}

/// Advance selection in the Install pane to the next or previous item whose
/// name or description matches the pane-find pattern.
pub fn find_in_install(app: &mut AppState, forward: bool) {
    let Some(pattern) = app.pane_find.clone() else {
        return;
    };
    let inds = crate::ui::helpers::filtered_install_indices(app);
    if inds.is_empty() {
        return;
    }
    let start = app.install_state.selected().unwrap_or(0);
    let mut vi = start;
    let n = inds.len();
    for _ in 0..n {
        vi = if forward {
            (vi + 1) % n
        } else if vi == 0 {
            n - 1
        } else {
            vi - 1
        };
        let i = inds[vi];
        if let Some(p) = app.install_list.get(i)
            && (p.name.to_lowercase().contains(&pattern.to_lowercase())
                || p.description
                    .to_lowercase()
                    .contains(&pattern.to_lowercase()))
        {
            app.install_state.select(Some(vi));
            break;
        }
    }
}

/// Ensure `app.details` reflects the currently selected result.
pub fn refresh_selected_details(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    if let Some(item) = app.results.get(app.selected).cloned() {
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

/// Ensure `app.details` reflects the currently selected item in the Install pane.
pub fn refresh_install_details(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    let Some(vsel) = app.install_state.selected() else {
        return;
    };
    let inds = crate::ui::helpers::filtered_install_indices(app);
    if inds.is_empty() || vsel >= inds.len() {
        return;
    }
    let i = inds[vsel];
    if let Some(item) = app.install_list.get(i).cloned() {
        // Focus details on the install selection
        app.details_focus = Some(item.name.clone());

        // Provide an immediate placeholder reflecting the selection
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
}

/// Ensure `app.details` reflects the currently selected item in the Remove pane.
pub fn refresh_remove_details(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    let Some(vsel) = app.remove_state.selected() else {
        return;
    };
    if app.remove_list.is_empty() || vsel >= app.remove_list.len() {
        return;
    }
    if let Some(item) = app.remove_list.get(vsel).cloned() {
        app.details_focus = Some(item.name.clone());
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
}

/// Ensure `app.details` reflects the currently selected item in the Downgrade pane.
pub fn refresh_downgrade_details(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    let Some(vsel) = app.downgrade_state.selected() else {
        return;
    };
    if app.downgrade_list.is_empty() || vsel >= app.downgrade_list.len() {
        return;
    }
    if let Some(item) = app.downgrade_list.get(vsel).cloned() {
        app.details_focus = Some(item.name.clone());
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
}
