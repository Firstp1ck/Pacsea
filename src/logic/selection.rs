use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// What: Move the selection by `delta` and coordinate detail loading policies.
///
/// Inputs:
/// - `app`: Mutable application state (results, selection, caches, scroll heuristics).
/// - `delta`: Signed offset to apply to the current selection index.
/// - `details_tx`: Channel used to request lazy loading of package details.
/// - `comments_tx`: Channel used to request AUR package comments.
///
/// Output:
/// - Updates selection-related state, potentially sends detail requests, and adjusts gating flags.
///
/// # Panics
/// - Panics if `abs_delta_usize` exceeds `u32::MAX` when converting to `u32`
/// - May panic if `app.list_state.select` is called with an invalid index (depends on the list state implementation)
///
/// Details:
/// - Clamps the selection to valid bounds, refreshes placeholder metadata, and reuses cached entries.
/// - Schedules PKGBUILD reloads when necessary and tracks scroll velocity to throttle prefetching.
/// - Updates comments when package changes and comments are visible (only for AUR packages).
/// - Switches between selected-only gating during fast scrolls and wide ring prefetch for slower navigation.
pub fn move_sel_cached(
    app: &mut AppState,
    delta: isize,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
) {
    if app.results.is_empty() {
        return;
    }
    let len = isize::try_from(app.results.len()).unwrap_or(isize::MAX);
    let mut idx = isize::try_from(app.selected).unwrap_or(0) + delta;
    if idx < 0 {
        idx = 0;
    }
    if idx >= len {
        idx = len - 1;
    }
    app.selected = usize::try_from(idx).unwrap_or(0);
    app.list_state.select(Some(app.selected));
    if let Some(item) = app.results.get(app.selected).cloned() {
        // Focus details on the currently selected item only
        app.details_focus = Some(item.name.clone());

        // Update details pane immediately with a placeholder reflecting the selection
        app.details.name.clone_from(&item.name);
        app.details.version.clone_from(&item.version);
        app.details.description.clear();
        match &item.source {
            crate::state::Source::Official { repo, arch } => {
                app.details.repository.clone_from(repo);
                app.details.architecture.clone_from(arch);
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

        // Auto-update comments if visible and for a different package (only for AUR packages)
        if app.comments_visible && matches!(item.source, crate::state::Source::Aur) {
            let needs_update = app
                .comments_package_name
                .as_deref()
                .is_none_or(|cached_name| cached_name != item.name.as_str());
            if needs_update {
                // Check if we have cached comments for this package
                if app
                    .comments_package_name
                    .as_ref()
                    .is_some_and(|cached_name| {
                        cached_name == &item.name && !app.comments.is_empty()
                    })
                {
                    // Use cached comments, just reset scroll
                    app.comments_scroll = 0;
                } else {
                    // Request new comments
                    app.comments.clear();
                    app.comments_package_name = None;
                    app.comments_fetched_at = None;
                    app.comments_scroll = 0;
                    app.comments_loading = true;
                    app.comments_error = None;
                    let _ = comments_tx.send(item.name.clone());
                }
            }
        }
    }

    // Debounce ring prefetch when scrolling fast (>5 items cumulatively)
    let abs_delta_usize: usize = if delta < 0 {
        usize::try_from(-delta).unwrap_or(0)
    } else {
        usize::try_from(delta).unwrap_or(0)
    };
    if abs_delta_usize > 0 {
        let add = u32::try_from(abs_delta_usize.min(u32::MAX as usize))
            .expect("value is bounded by u32::MAX");
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
            out_of_date: None,
            orphaned: false,
        }
    }

    #[tokio::test]
    /// What: Move selection with bounds, placeholder details, and request flow.
    ///
    /// Inputs:
    /// - `app`: Results list seeded with one AUR and one official package, initial selection at index 0.
    /// - `tx`: Unbounded channel capturing detail fetch requests while deltas of +1, -100, and 0 are applied.
    ///
    /// Output:
    /// - Mutates `app` so indices clamp within bounds, details placeholders reflect the active selection, and a fetch request emits when switching to the official entry.
    ///
    /// Details:
    /// - Uses a timeout on the receiver to assert the async request is produced and verifies placeholder data resets when returning to the AUR result.
    async fn move_sel_cached_clamps_and_requests_details() {
        let mut app = crate::state::AppState {
            results: vec![
                crate::state::PackageItem {
                    name: "aur1".into(),
                    version: "1".into(),
                    description: String::new(),
                    source: crate::state::Source::Aur,
                    popularity: None,
                    out_of_date: None,
                    orphaned: false,
                },
                item_official("pkg2", "core"),
            ],
            selected: 0,
            ..Default::default()
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (comments_tx, _comments_rx) = tokio::sync::mpsc::unbounded_channel();
        move_sel_cached(&mut app, 1, &tx, &comments_tx);
        assert_eq!(app.selected, 1);
        assert_eq!(app.details.repository.to_lowercase(), "core");
        assert_eq!(app.details.architecture.to_lowercase(), "x86_64");
        let got = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
            .await
            .ok()
            .flatten();
        assert!(got.is_some());
        move_sel_cached(&mut app, -100, &tx, &comments_tx);
        assert_eq!(app.selected, 0);
        move_sel_cached(&mut app, 0, &tx, &comments_tx);
        assert_eq!(app.details.repository, "AUR");
        assert_eq!(app.details.architecture, "any");
    }

    #[tokio::test]
    /// What: Ensure cached details suppress additional fetch requests.
    ///
    /// Inputs:
    /// - Results containing the cached package and an existing entry in `details_cache`.
    ///
    /// Output:
    /// - No message emitted on the channel and `app.details` populated from the cache.
    ///
    /// Details:
    /// - Confirms `move_sel_cached` short-circuits when cache contains the selected package.
    async fn move_sel_cached_uses_details_cache() {
        let mut app = crate::state::AppState::default();
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
        let (comments_tx, _comments_rx) = tokio::sync::mpsc::unbounded_channel();
        move_sel_cached(&mut app, 0, &tx, &comments_tx);
        let none = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
            .await
            .ok()
            .flatten();
        assert!(none.is_none());
        assert_eq!(app.details.name, "pkg");
    }

    #[test]
    /// What: Verify fast-scroll gating requests ring prefetch and locks selection.
    ///
    /// Inputs:
    /// - `app`: Populated results list with selection moved near the end to trigger fast-scroll logic.
    ///
    /// Output:
    /// - `need_ring_prefetch` flag set, `ring_resume_at` populated, and allowed set restricted to the
    ///   selected package.
    ///
    /// Details:
    /// - Simulates a large positive index jump and ensures gating functions mark the correct state and
    ///   enforce selection-only access.
    fn fast_scroll_sets_gating_and_defers_ring() {
        let mut app = crate::state::AppState {
            results: vec![
                item_official("a", "core"),
                item_official("b", "extra"),
                item_official("c", "extra"),
                item_official("d", "extra"),
                item_official("e", "extra"),
                item_official("f", "extra"),
                item_official("g", "extra"),
            ],
            ..Default::default()
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<crate::state::PackageItem>();
        let (comments_tx, _comments_rx) = tokio::sync::mpsc::unbounded_channel();
        move_sel_cached(&mut app, 6, &tx, &comments_tx);
        assert!(app.need_ring_prefetch);
        assert!(app.ring_resume_at.is_some());
        crate::logic::set_allowed_only_selected(&app);
        assert!(crate::logic::is_allowed(&app.results[app.selected].name));
    }
}
