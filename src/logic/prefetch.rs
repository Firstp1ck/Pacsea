use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// What: Prefetch details for items near the current selection (alternating above/below).
///
/// Inputs:
/// - `app`: Mutable application state (`results`, `selected`, `details_cache`)
/// - `details_tx`: Channel to enqueue detail requests
///
/// Output:
/// - Enqueues requests for allowed, uncached neighbors within a fixed radius; no return value.
///
/// Details:
/// - Respects `logic::is_allowed` and skips names present in the cache; designed to be cheap.
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

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    /// What: Ensure prefetching emits no requests when results are empty.
    ///
    /// Inputs:
    /// - Application state with zero search results.
    ///
    /// Output:
    /// - No messages received on the details channel within the timeout window.
    ///
    /// Details:
    /// - Uses a short timeout to confirm no unexpected sends occur during the async loop.
    async fn prefetch_noop_on_empty_results() {
        let _guard = crate::global_test_mutex_lock();
        let mut app = AppState::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        ring_prefetch_from_selected(&mut app, &tx);
        let none = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
            .await
            .ok()
            .flatten();
        assert!(none.is_none());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    /// What: Verify prefetch honours allowed gating and avoids cached entries.
    ///
    /// Inputs:
    /// - Results list of three packages with varying allowed states and cache contents.
    ///
    /// Output:
    /// - No requests when only the selected item is allowed; afterwards only uncached, allowed neighbor is dispatched.
    ///
    /// Details:
    /// - Toggles `set_allowed_only_selected` and `set_allowed_ring`, updating the cache between passes to target specific neighbours.
    async fn prefetch_respects_allowed_and_cache() {
        let _guard = crate::global_test_mutex_lock();
        let mut app = AppState {
            results: vec![
                item_official("a", "core"),
                item_official("b", "extra"),
                item_official("c", "extra"),
            ],
            selected: 1,
            ..Default::default()
        };
        // Disallow b/c except selected, and cache one neighbor
        crate::logic::set_allowed_only_selected(&app);
        app.details_cache.insert(
            "c".into(),
            crate::state::PackageDetails {
                name: "c".into(),
                ..Default::default()
            },
        );
        let (tx, mut rx) = mpsc::unbounded_channel();
        ring_prefetch_from_selected(&mut app, &tx);
        // With only-selected allowed, neighbors shouldn't be sent
        let none = tokio::time::timeout(std::time::Duration::from_millis(60), rx.recv())
            .await
            .ok()
            .flatten();
        assert!(none.is_none());

        // Now allow ring and clear cache for b, keep c cached
        app.details_cache.clear();
        app.details_cache.insert(
            "c".into(),
            crate::state::PackageDetails {
                name: "c".into(),
                ..Default::default()
            },
        );
        crate::logic::set_allowed_ring(&app, 1);
        ring_prefetch_from_selected(&mut app, &tx);
        // Expect only 'a' (above neighbor) to be sent; 'c' is cached
        let sent = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .ok()
            .flatten()
            .expect("one sent");
        assert_eq!(sent.name, "a");
        let none2 = tokio::time::timeout(std::time::Duration::from_millis(60), rx.recv())
            .await
            .ok()
            .flatten();
        assert!(none2.is_none());
    }
}
