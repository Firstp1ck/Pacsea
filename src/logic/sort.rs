//! Result sorting with selection preservation across sort modes.
//!
//! Implements cache-based O(n) reordering for sort mode switching between cacheable modes.
//! `BestMatches` mode is query-dependent and always performs full O(n log n) sort.

use crate::state::{AppState, PackageItem, SortMode, Source};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static COMPUTE_REPO_INDICES_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static COMPUTE_AUR_INDICES_CALLS: AtomicUsize = AtomicUsize::new(0);

/// What: Compute a signature hash for the results list to validate cache validity.
///
/// Inputs:
/// - `results`: Slice of package items to compute signature for.
///
/// Output:
/// - Returns an order-insensitive `u64` hash based on package names.
///
/// Details:
/// - Used to detect when results have changed, invalidating cached sort orders.
/// - Order-insensitive so mode switches do not invalidate caches.
fn compute_results_signature(results: &[PackageItem]) -> u64 {
    // Collect and canonicalize names to be order-insensitive.
    let mut names: Vec<&str> = results.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();

    let mut hasher = DefaultHasher::new();
    names.len().hash(&mut hasher);

    // Mix first/last to avoid hashing full list twice.
    if let Some(first) = names.first() {
        first.hash(&mut hasher);
    }
    if let Some(last) = names.last() {
        last.hash(&mut hasher);
    }

    // Aggregate individual name hashes in an order-insensitive way.
    let mut aggregate: u64 = 0;
    for name in names {
        let mut nh = DefaultHasher::new();
        name.hash(&mut nh);
        aggregate ^= nh.finish();
    }
    aggregate.hash(&mut hasher);

    hasher.finish()
}

/// What: Reorder results vector using cached indices.
///
/// Inputs:
/// - `results`: Mutable reference to results vector.
/// - `indices`: Slice of indices representing the desired sort order.
///
/// Output:
/// - Reorders `results` in-place according to `indices`.
///
/// Details:
/// - Performs O(n) reordering instead of O(n log n) sorting.
/// - Invalid indices are filtered out safely.
fn reorder_from_indices(results: &mut Vec<PackageItem>, indices: &[usize]) {
    let reordered: Vec<PackageItem> = indices
        .iter()
        .filter_map(|&i| results.get(i).cloned())
        .collect();
    *results = reordered;
}

/// What: Sort results by best match rank based on query.
///
/// Inputs:
/// - `results`: Mutable reference to results vector.
/// - `query`: Search query string for match ranking.
///
/// Output:
/// - Sorts results in-place by match rank (lower is better), with repo order and name as tiebreakers.
///
/// Details:
/// - Used for `BestMatches` sort mode. Query-dependent, so cannot be cached.
fn sort_best_matches(results: &mut [PackageItem], query: &str) {
    let ql = query.trim().to_lowercase();
    results.sort_by(|a, b| {
        let ra = crate::util::match_rank(&a.name, &ql);
        let rb = crate::util::match_rank(&b.name, &ql);
        if ra != rb {
            return ra.cmp(&rb);
        }
        // Tiebreak: keep pacman repo order first to keep layout familiar
        let oa = crate::util::repo_order(&a.source);
        let ob = crate::util::repo_order(&b.source);
        if oa != ob {
            return oa.cmp(&ob);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
}

/// What: Compute sort order indices for repo-then-name sorting.
///
/// Inputs:
/// - `results`: Slice of package items.
///
/// Output:
/// - Returns vector of indices representing sorted order.
///
/// Details:
/// - Used to populate cache without modifying the original results.
fn compute_repo_then_name_indices(results: &[PackageItem]) -> Vec<usize> {
    #[cfg(test)]
    COMPUTE_REPO_INDICES_CALLS.fetch_add(1, Ordering::Relaxed);

    let mut indices: Vec<usize> = (0..results.len()).collect();
    indices.sort_by(|&i, &j| {
        let a = &results[i];
        let b = &results[j];
        let oa = crate::util::repo_order(&a.source);
        let ob = crate::util::repo_order(&b.source);
        if oa != ob {
            return oa.cmp(&ob);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    indices
}

/// What: Compute sort order indices for AUR-popularity-then-official sorting.
///
/// Inputs:
/// - `results`: Slice of package items.
///
/// Output:
/// - Returns vector of indices representing sorted order.
///
/// Details:
/// - Used to populate cache without modifying the original results.
fn compute_aur_popularity_then_official_indices(results: &[PackageItem]) -> Vec<usize> {
    #[cfg(test)]
    COMPUTE_AUR_INDICES_CALLS.fetch_add(1, Ordering::Relaxed);

    let mut indices: Vec<usize> = (0..results.len()).collect();
    indices.sort_by(|&i, &j| {
        let a = &results[i];
        let b = &results[j];
        // AUR first
        let aur_a = matches!(a.source, Source::Aur);
        let aur_b = matches!(b.source, Source::Aur);
        if aur_a != aur_b {
            return aur_b.cmp(&aur_a); // true before false
        }
        if aur_a && aur_b {
            // Desc popularity for AUR
            let pa = a.popularity.unwrap_or(0.0);
            let pb = b.popularity.unwrap_or(0.0);
            if (pa - pb).abs() > f64::EPSILON {
                return pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal);
            }
        } else {
            // Both official: keep pacman order (repo_order), then name
            let oa = crate::util::repo_order(&a.source);
            let ob = crate::util::repo_order(&b.source);
            if oa != ob {
                return oa.cmp(&ob);
            }
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
    indices
}

/// What: Apply the currently selected sorting mode to `app.results` in-place.
///
/// Inputs:
/// - `app`: Mutable application state (`results`, `selected`, `input`, `sort_mode`)
///
/// Output:
/// - Sorts `app.results` and preserves selection by name when possible; otherwise clamps index.
///
/// Details:
/// - Uses cache-based O(n) reordering when switching between cacheable modes (`RepoThenName` and `AurPopularityThenOfficial`).
/// - Performs full O(n log n) sort when cache is invalid or for `BestMatches` mode.
/// - Populates both cache orders eagerly after full sort to enable instant mode switching.
pub fn sort_results_preserve_selection(app: &mut AppState) {
    if app.results.is_empty() {
        return;
    }
    let prev_name = app.results.get(app.selected).map(|p| p.name.clone());

    // Compute current signature to check cache validity
    let current_sig = compute_results_signature(&app.results);

    // Check if cache is valid and we can use O(n) reordering
    let cache_valid = app.sort_cache_signature == Some(current_sig);

    match app.sort_mode {
        SortMode::RepoThenName => {
            if cache_valid {
                if let Some(ref indices) = app.sort_cache_repo_name {
                    // Cache hit: O(n) reorder
                    reorder_from_indices(&mut app.results, indices);
                } else {
                    // Cache miss: compute indices from current state, then reorder
                    let indices = compute_repo_then_name_indices(&app.results);
                    reorder_from_indices(&mut app.results, &indices);
                }
            } else {
                // Cache invalid: compute indices from current state, then reorder
                let indices = compute_repo_then_name_indices(&app.results);
                reorder_from_indices(&mut app.results, &indices);
            }
            // Re-anchor caches to current order to keep future switches correct.
            app.sort_cache_repo_name = Some((0..app.results.len()).collect());
            app.sort_cache_aur_popularity =
                Some(compute_aur_popularity_then_official_indices(&app.results));
            app.sort_cache_signature = Some(current_sig);
        }
        SortMode::AurPopularityThenOfficial => {
            if cache_valid {
                if let Some(ref indices) = app.sort_cache_aur_popularity {
                    // Cache hit: O(n) reorder
                    reorder_from_indices(&mut app.results, indices);
                } else {
                    // Cache miss: compute indices from current state, then reorder
                    let indices = compute_aur_popularity_then_official_indices(&app.results);
                    reorder_from_indices(&mut app.results, &indices);
                }
            } else {
                // Cache invalid: compute indices from current state, then reorder
                let indices = compute_aur_popularity_then_official_indices(&app.results);
                reorder_from_indices(&mut app.results, &indices);
            }
            // Re-anchor caches to current order to keep future switches correct.
            app.sort_cache_repo_name = Some(compute_repo_then_name_indices(&app.results));
            app.sort_cache_aur_popularity = Some((0..app.results.len()).collect());
            app.sort_cache_signature = Some(current_sig);
        }
        SortMode::BestMatches => {
            // BestMatches is query-dependent, always do full sort and don't cache
            sort_best_matches(&mut app.results, &app.input);
            // Clear mode-specific caches since BestMatches can't use them
            app.sort_cache_repo_name = None;
            app.sort_cache_aur_popularity = None;
            app.sort_cache_signature = None;
        }
    }

    // Restore selection by name
    if let Some(name) = prev_name {
        if let Some(pos) = app.results.iter().position(|p| p.name == name) {
            app.selected = pos;
            app.list_state.select(Some(pos));
        } else {
            app.selected = app.selected.min(app.results.len().saturating_sub(1));
            app.list_state.select(Some(app.selected));
        }
    }
}

/// What: Invalidate all sort caches.
///
/// Inputs:
/// - `app`: Mutable application state.
///
/// Output:
/// - Clears all sort cache fields.
///
/// Details:
/// - Should be called when results change (new search, filter change, etc.).
pub fn invalidate_sort_caches(app: &mut AppState) {
    app.sort_cache_repo_name = None;
    app.sort_cache_aur_popularity = None;
    app.sort_cache_signature = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    /// What: Reset compute index call counters used for instrumentation in tests.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Clears the atomic counters to zero.
    ///
    /// Details:
    /// - Keeps tests isolated by removing cross-test coupling from shared state.
    fn reset_compute_counters() {
        COMPUTE_REPO_INDICES_CALLS.store(0, Ordering::SeqCst);
        COMPUTE_AUR_INDICES_CALLS.store(0, Ordering::SeqCst);
    }

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
    fn item_aur(name: &str, pop: Option<f64>) -> crate::state::PackageItem {
        crate::state::PackageItem {
            name: name.to_string(),
            version: "1.0".to_string(),
            description: format!("{name} desc"),
            source: crate::state::Source::Aur,
            popularity: pop,
            out_of_date: None,
            orphaned: false,
        }
    }

    #[test]
    /// What: Confirm sorting preserves the selected index while adjusting order across modes, including relevance matching.
    ///
    /// Inputs:
    /// - Mixed list of official and AUR results.
    /// - Sort mode toggled through `RepoThenName`, `AurPopularityThenOfficial`, and `BestMatches` with input `"bb"`.
    ///
    /// Output:
    /// - Selection remains on the prior package and ordering reflects repo priority, popularity preference, and match rank, respectively.
    ///
    /// Details:
    /// - Ensures the UI behaviour stays predictable when users toggle sort modes after highlighting a result.
    fn sort_preserve_selection_and_best_matches() {
        let mut app = AppState {
            results: vec![
                item_aur("zzz", Some(1.0)),
                item_official("aaa", "core"),
                item_official("bbb", "extra"),
                item_aur("ccc", Some(10.0)),
            ],
            selected: 2,
            sort_mode: SortMode::RepoThenName,
            ..Default::default()
        };
        app.list_state.select(Some(2));
        sort_results_preserve_selection(&mut app);
        assert_eq!(
            app.results
                .iter()
                .filter(|p| matches!(p.source, Source::Official { .. }))
                .count(),
            2
        );
        assert_eq!(app.results[app.selected].name, "bbb");

        app.sort_mode = SortMode::AurPopularityThenOfficial;
        sort_results_preserve_selection(&mut app);
        let aur_first = &app.results[0];
        assert!(matches!(aur_first.source, Source::Aur));

        app.input = "bb".into();
        app.sort_mode = SortMode::BestMatches;
        sort_results_preserve_selection(&mut app);
        assert!(
            app.results
                .iter()
                .position(|p| p.name.contains("bb"))
                .expect("should find package containing 'bb' in test data")
                <= 1
        );
    }

    #[test]
    /// What: Validate `BestMatches` tiebreakers prioritise repo order before lexicographic name sorting.
    ///
    /// Inputs:
    /// - Three official packages whose names share the `alpha` prefix across `core` and `extra` repos.
    ///
    /// Output:
    /// - Sorted list begins with the `core` repo entry, followed by `extra` items in name order.
    ///
    /// Details:
    /// - Captures the layered tiebreak logic to catch regressions if repo precedence changes.
    fn sort_bestmatches_tiebreak_repo_then_name() {
        let mut app = AppState {
            results: vec![
                item_official("alpha2", "extra"),
                item_official("alpha1", "extra"),
                item_official("alpha_core", "core"),
            ],
            input: "alpha".into(),
            sort_mode: SortMode::BestMatches,
            ..Default::default()
        };
        sort_results_preserve_selection(&mut app);
        let names: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["alpha_core", "alpha1", "alpha2"]);
    }

    #[test]
    /// What: Ensure results signature is order-insensitive but content-sensitive.
    ///
    /// Inputs:
    /// - Same set of packages in different orders.
    /// - A variant with an extra package.
    ///
    /// Output:
    /// - Signatures match for permutations and differ when content changes.
    ///
    /// Details:
    /// - Guards cache reuse when switching sort modes without masking real result changes.
    fn results_signature_is_order_insensitive() {
        let base = vec![
            item_official("aaa", "core"),
            item_official("bbb", "extra"),
            item_official("ccc", "community"),
        ];
        let permuted = vec![
            item_official("ccc", "community"),
            item_official("aaa", "core"),
            item_official("bbb", "extra"),
        ];
        let mut extended = permuted.clone();
        extended.push(item_official("ddd", "community"));

        let sig_base = compute_results_signature(&base);
        let sig_permuted = compute_results_signature(&permuted);
        let sig_extended = compute_results_signature(&extended);

        assert_eq!(sig_base, sig_permuted);
        assert_ne!(sig_base, sig_extended);
    }

    #[test]
    /// What: Ensure the AUR popularity sort orders helpers by descending popularity with deterministic tie-breaks.
    ///
    /// Inputs:
    /// - AUR items sharing the same popularity value and official entries from different repos.
    ///
    /// Output:
    /// - AUR items sorted by name when popularity ties, followed by official packages prioritising `core` before `extra`.
    ///
    /// Details:
    /// - Verifies the composite comparator remains stable for UI diffs and regression detection.
    fn sort_aur_popularity_and_official_tiebreaks() {
        let mut app = AppState {
            results: vec![
                item_aur("aurB", Some(1.0)),
                item_aur("aurA", Some(1.0)),
                item_official("z_off", "core"),
                item_official("a_off", "extra"),
            ],
            sort_mode: SortMode::AurPopularityThenOfficial,
            ..Default::default()
        };
        sort_results_preserve_selection(&mut app);
        let names: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["aurA", "aurB", "z_off", "a_off"]);
    }

    #[test]
    /// What: Verify cache invalidation clears all sort cache fields.
    ///
    /// Inputs:
    /// - `AppState` with manually set cache fields.
    ///
    /// Output:
    /// - All cache fields are `None` after invalidation.
    ///
    /// Details:
    /// - Tests that `invalidate_sort_caches` properly clears all cache state.
    fn sort_cache_invalidation() {
        let mut app = AppState {
            results: vec![
                item_official("pkg1", "core"),
                item_official("pkg2", "extra"),
            ],
            sort_mode: SortMode::RepoThenName,
            sort_cache_signature: Some(12345),
            sort_cache_repo_name: Some(vec![0, 1]),
            sort_cache_aur_popularity: Some(vec![1, 0]),
            ..Default::default()
        };

        // Invalidate cache
        invalidate_sort_caches(&mut app);
        assert!(app.sort_cache_signature.is_none());
        assert!(app.sort_cache_repo_name.is_none());
        assert!(app.sort_cache_aur_popularity.is_none());
    }

    #[test]
    /// What: Verify `BestMatches` mode does not populate mode-specific caches.
    ///
    /// Inputs:
    /// - Results list sorted with `BestMatches` mode.
    ///
    /// Output:
    /// - Mode-specific caches remain `None` for `BestMatches`.
    ///
    /// Details:
    /// - `BestMatches` depends on the query and should not cache mode-specific indices.
    fn sort_bestmatches_no_mode_cache() {
        let mut app = AppState {
            results: vec![
                item_official("alpha", "core"),
                item_official("beta", "extra"),
            ],
            input: "alph".into(),
            sort_mode: SortMode::BestMatches,
            ..Default::default()
        };

        sort_results_preserve_selection(&mut app);

        // BestMatches should not populate mode-specific caches
        assert!(app.sort_cache_repo_name.is_none());
        assert!(app.sort_cache_aur_popularity.is_none());
    }

    #[test]
    /// What: Verify cache hit path uses O(n) reordering when cache is valid.
    ///
    /// Inputs:
    /// - Results with valid cache signature and cached indices for `RepoThenName`.
    ///
    /// Output:
    /// - Results are reordered using cached indices without full sort.
    ///
    /// Details:
    /// - Tests that cache-based optimization works correctly.
    fn sort_cache_hit_repo_then_name() {
        let mut app = AppState {
            results: vec![
                item_official("zzz", "extra"),
                item_official("aaa", "core"),
                item_official("bbb", "core"),
            ],
            sort_mode: SortMode::RepoThenName,
            ..Default::default()
        };

        // First sort to populate cache
        sort_results_preserve_selection(&mut app);
        let first_sort_order: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        let cached_sig = app.sort_cache_signature;

        // Change to different order
        app.sort_mode = SortMode::AurPopularityThenOfficial;
        sort_results_preserve_selection(&mut app);

        // Switch back - should use cache
        app.sort_mode = SortMode::RepoThenName;
        sort_results_preserve_selection(&mut app);
        let second_sort_order: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();

        // Should match first sort order
        assert_eq!(first_sort_order, second_sort_order);
        assert_eq!(app.sort_cache_signature, cached_sig);
    }

    #[test]
    /// What: Verify cache miss path performs full sort when results change.
    ///
    /// Inputs:
    /// - Results with cached signature that doesn't match current results.
    ///
    /// Output:
    /// - Full sort is performed and cache is repopulated.
    ///
    /// Details:
    /// - Tests that cache invalidation works correctly.
    fn sort_cache_miss_on_results_change() {
        let mut app = AppState {
            results: vec![item_official("aaa", "core"), item_official("bbb", "extra")],
            sort_mode: SortMode::RepoThenName,
            ..Default::default()
        };

        // First sort to populate cache
        sort_results_preserve_selection(&mut app);
        let old_sig = app.sort_cache_signature;

        // Change results (simulating new search)
        app.results = vec![item_official("ccc", "core"), item_official("ddd", "extra")];

        // Sort again - should detect cache miss and repopulate
        sort_results_preserve_selection(&mut app);
        let new_sig = app.sort_cache_signature;

        // Signature should be different
        assert_ne!(old_sig, new_sig);
        assert!(app.sort_cache_repo_name.is_some());
        assert!(app.sort_cache_aur_popularity.is_some());
    }

    #[test]
    /// What: Ensure cache invalidation only computes current-mode indices once while rebuilding caches.
    ///
    /// Inputs:
    /// - Results with a deliberately mismatched cache signature to force invalidation.
    ///
    /// Output:
    /// - Current-mode index computation runs once; cross-mode cache computation still occurs once after reorder.
    ///
    /// Details:
    /// - Guards against redundant index work when cache signatures are stale.
    fn sort_cache_invalid_computes_indices_once() {
        reset_compute_counters();
        let mut app = AppState {
            results: vec![item_official("bbb", "extra"), item_official("aaa", "core")],
            sort_mode: SortMode::RepoThenName,
            ..Default::default()
        };

        // Force signature mismatch to hit invalidation path.
        let sig = compute_results_signature(&app.results);
        app.sort_cache_signature = Some(sig.wrapping_add(1));

        sort_results_preserve_selection(&mut app);

        assert_eq!(
            COMPUTE_REPO_INDICES_CALLS.load(Ordering::SeqCst),
            1,
            "repo indices should be computed exactly once on cache invalidation"
        );
        assert_eq!(
            COMPUTE_AUR_INDICES_CALLS.load(Ordering::SeqCst),
            1,
            "aur indices should be recomputed once to re-anchor caches"
        );
        let names: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["aaa", "bbb"]);
    }

    #[test]
    /// What: Verify switching between cacheable modes uses cached indices.
    ///
    /// Inputs:
    /// - Results sorted in `RepoThenName` mode with populated caches.
    ///
    /// Output:
    /// - Switching to `AurPopularityThenOfficial` uses cached indices for O(n) reordering.
    ///
    /// Details:
    /// - Tests the main optimization: instant mode switching via cache.
    fn sort_cache_mode_switching() {
        let mut app = AppState {
            results: vec![
                item_aur("low_pop", Some(1.0)),
                item_official("core_pkg", "core"),
                item_aur("high_pop", Some(10.0)),
                item_official("extra_pkg", "extra"),
            ],
            sort_mode: SortMode::RepoThenName,
            ..Default::default()
        };

        // Initial sort - populates both caches
        sort_results_preserve_selection(&mut app);
        assert!(app.sort_cache_repo_name.is_some());
        assert!(app.sort_cache_aur_popularity.is_some());
        let repo_order: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();

        // Switch to AUR popularity - should use cache
        app.sort_mode = SortMode::AurPopularityThenOfficial;
        sort_results_preserve_selection(&mut app);
        let _aur_order: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        // AUR packages should be first
        assert!(matches!(app.results[0].source, Source::Aur));

        // Switch back to repo - should use cache
        app.sort_mode = SortMode::RepoThenName;
        sort_results_preserve_selection(&mut app);
        let repo_order_again: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        assert_eq!(repo_order, repo_order_again);
    }
}
