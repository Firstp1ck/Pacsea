use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

pub fn send_query(app: &mut AppState, query_tx: &mpsc::UnboundedSender<crate::state::QueryInput>) {
    let id = app.next_query_id;
    app.next_query_id += 1;
    app.latest_query_id = id;
    let _ = query_tx.send(crate::state::QueryInput {
        id,
        text: app.input.clone(),
    });
}

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
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

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
