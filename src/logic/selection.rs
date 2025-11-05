use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// Move the selection by `delta` and update cached details and prefetch policy.
///
/// Behavior:
/// - Clamps the selection to the valid range and updates the list state.
/// - Focuses the details pane on the newly selected item and immediately shows
///   a placeholder based on known metadata.
/// - If cached details are present, uses them; otherwise requests loading via
///   `details_tx`.
/// - If PKGBUILD is visible and for a different package, schedules a debounced reload.
/// - Tracks cumulative scroll moves to detect fast scrolling.
/// - During fast scrolls, temporarily tightens the allowed set to only the
///   current selection and defers ring prefetch for ~200ms.
/// - For small scrolls, expands allowed set to a 30-item ring and begins
///   background prefetch.
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
        // Focus details on the currently selected item only
        app.details_focus = Some(item.name.clone());

        // Update details pane immediately with a placeholder reflecting the selection
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
            let _ = details_tx.send(item.clone());
        }

        // Auto-reload PKGBUILD if visible and for a different package (with debounce)
        if app.pkgb_visible {
            let needs_reload = app.pkgb_package_name.as_deref() != Some(item.name.as_str());
            if needs_reload {
                // Instead of immediately loading, schedule a debounced reload
                app.pkgb_reload_requested_at = Some(std::time::Instant::now());
                app.pkgb_reload_requested_for = Some(item.name.clone());
                app.pkgb_text = None; // Clear old PKGBUILD while loading
            }
        }
    }

    // Debounce ring prefetch when scrolling fast (>5 items cumulatively)
    let abs_delta_usize: usize = if delta < 0 {
        (-delta) as usize
    } else {
        delta as usize
    };
    if abs_delta_usize > 0 {
        let add = abs_delta_usize.min(u32::MAX as usize) as u32;
        app.scroll_moves = app.scroll_moves.saturating_add(add);
    }
    if app.need_ring_prefetch {
        // tighten allowed set to only current selection during fast scroll
        crate::logic::set_allowed_only_selected(app);
        app.ring_resume_at =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        return;
    }
    if app.scroll_moves > 5 {
        app.need_ring_prefetch = true;
        crate::logic::set_allowed_only_selected(app);
        app.ring_resume_at =
            Some(std::time::Instant::now() + std::time::Duration::from_millis(200));
        return;
    }

    // For small/slow scrolls, allow ring and prefetch immediately
    crate::logic::set_allowed_ring(app, 30);
    crate::logic::ring_prefetch_from_selected(app, details_tx);
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

    #[tokio::test]
    /// What: Move selection with bounds, placeholder details, and request flow
    ///
    /// - Input: Results with AUR and official; delta +1, -100, 0
    /// - Output: Clamped indices; details placeholders set; request sent when uncached
    async fn move_sel_cached_clamps_and_requests_details() {
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        app.results = vec![
            crate::state::PackageItem {
                name: "aur1".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Aur,
                popularity: None,
            },
            item_official("pkg2", "core"),
        ];
        app.selected = 0;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        move_sel_cached(&mut app, 1, &tx);
        assert_eq!(app.selected, 1);
        assert_eq!(app.details.repository.to_lowercase(), "core");
        assert_eq!(app.details.architecture.to_lowercase(), "x86_64");
        let got = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
            .await
            .ok()
            .flatten();
        assert!(got.is_some());
        move_sel_cached(&mut app, -100, &tx);
        assert_eq!(app.selected, 0);
        move_sel_cached(&mut app, 0, &tx);
        assert_eq!(app.details.repository, "AUR");
        assert_eq!(app.details.architecture, "any");
    }

    #[tokio::test]
    /// What: Reuse details from cache to avoid sending a new request
    ///
    /// - Input: Results with cached details for selected item
    /// - Output: No message sent; details filled from cache
    async fn move_sel_cached_uses_details_cache() {
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        let pkg = item_official("pkg", "core");
        app.results = vec![pkg.clone()];
        app.details_cache.insert(
            pkg.name.clone(),
            crate::state::PackageDetails {
                repository: "core".into(),
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                architecture: "x86_64".into(),
                ..Default::default()
            },
        );
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        move_sel_cached(&mut app, 0, &tx);
        let none = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
            .await
            .ok()
            .flatten();
        assert!(none.is_none());
        assert_eq!(app.details.name, "pkg");
    }

    #[test]
    /// What: Fast scroll gating triggers ring deferral and selected-only allowance
    ///
    /// - Input: Large positive delta; ring prefetch state
    /// - Output: need_ring_prefetch set; ring_resume_at present; only selected allowed
    fn fast_scroll_sets_gating_and_defers_ring() {
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        app.results = vec![
            item_official("a", "core"),
            item_official("b", "extra"),
            item_official("c", "extra"),
            item_official("d", "extra"),
            item_official("e", "extra"),
            item_official("f", "extra"),
            item_official("g", "extra"),
        ];
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<crate::state::PackageItem>();
        move_sel_cached(&mut app, 6, &tx);
        assert!(app.need_ring_prefetch);
        assert!(app.ring_resume_at.is_some());
        crate::logic::set_allowed_only_selected(&app);
        assert!(crate::logic::is_allowed(&app.results[app.selected].name));
    }
}
