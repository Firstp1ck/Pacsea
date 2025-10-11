use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

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
                && crate::logic::is_allowed(&it.name)
                && !app.details_cache.contains_key(&it.name)
            {
                let _ = details_tx.send(it);
            }
            progressed = true;
        }
        let below = app.selected + step;
        if below < len_u {
            if let Some(it) = app.results.get(below).cloned()
                && crate::logic::is_allowed(&it.name)
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
