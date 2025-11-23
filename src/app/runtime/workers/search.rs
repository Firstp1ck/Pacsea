use std::time::Instant;

use tokio::{
    select,
    sync::mpsc,
    time::{Duration, sleep},
};

use crate::index as pkgindex;
use crate::sources;
use crate::state::{QueryInput, SearchResults};
use crate::util::{match_rank, repo_order};

/// What: Spawn background worker for search queries.
///
/// Inputs:
/// - `query_rx`: Channel receiver for search queries
/// - `search_result_tx`: Channel sender for search results
/// - `net_err_tx`: Channel sender for network errors
/// - `index_path`: Path to official package index
///
/// Details:
/// - Debounces queries with 250ms window
/// - Enforces minimum 300ms interval between searches
/// - Handles empty queries by returning all official packages
/// - Searches both official and AUR repositories
pub fn spawn_search_worker(
    mut query_rx: mpsc::UnboundedReceiver<QueryInput>,
    search_result_tx: mpsc::UnboundedSender<SearchResults>,
    net_err_tx: &mpsc::UnboundedSender<String>,
    index_path: std::path::PathBuf,
) {
    let net_err_tx_search = net_err_tx.clone();
    tokio::spawn(async move {
        const DEBOUNCE_MS: u64 = 250;
        const MIN_INTERVAL_MS: u64 = 300;
        let mut last_sent = Instant::now()
            .checked_sub(Duration::from_millis(MIN_INTERVAL_MS))
            .unwrap_or_else(Instant::now);
        loop {
            let Some(mut latest) = query_rx.recv().await else {
                break;
            };
            loop {
                select! { Some(new_q) = query_rx.recv() => { latest = new_q; } () = sleep(Duration::from_millis(DEBOUNCE_MS)) => { break; } }
            }
            if latest.text.trim().is_empty() {
                let mut items = pkgindex::all_official_or_fetch(&index_path);
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
                // Deduplicate by package name, preferring earlier entries (core > extra > others)
                {
                    use std::collections::HashSet;
                    let mut seen = HashSet::new();
                    items.retain(|p| seen.insert(p.name.to_lowercase()));
                }
                let _ = search_result_tx.send(SearchResults {
                    id: latest.id,
                    items,
                });
                continue;
            }
            let elapsed = last_sent.elapsed();
            if elapsed < Duration::from_millis(MIN_INTERVAL_MS) {
                sleep(Duration::from_millis(MIN_INTERVAL_MS) - elapsed).await;
            }
            last_sent = Instant::now();

            let qtext = latest.text.clone();
            let fuzzy_mode = latest.fuzzy;
            let sid = latest.id;
            let tx = search_result_tx.clone();
            let err_tx = net_err_tx_search.clone();
            let ipath = index_path.clone();
            tokio::spawn(async move {
                if crate::index::all_official().is_empty() {
                    let _ = crate::index::all_official_or_fetch(&ipath);
                }
                let official_results = pkgindex::search_official(&qtext, fuzzy_mode);
                let q_for_net = qtext.clone();
                let (aur_items, errors) = sources::fetch_all_with_errors(q_for_net).await;

                // Collect all items with their fuzzy scores if in fuzzy mode
                let mut items_with_scores: Vec<(crate::state::PackageItem, Option<i64>)> =
                    official_results;

                // Filter and score AUR items
                if fuzzy_mode {
                    // In fuzzy mode, filter AUR items by fuzzy match and keep scores
                    // AUR API returns substring matches, but we re-filter with fuzzy matching
                    let aur_scored: Vec<(crate::state::PackageItem, Option<i64>)> = aur_items
                        .into_iter()
                        .filter_map(|item| {
                            crate::util::fuzzy_match_rank(&item.name, &qtext)
                                .map(|score| (item, Some(score)))
                        })
                        .collect();
                    items_with_scores.extend(aur_scored);
                } else {
                    // In normal mode, AUR API already filters by substring match
                    // Add all AUR items with placeholder score for consistent sorting
                    let aur_scored: Vec<(crate::state::PackageItem, Option<i64>)> = aur_items
                        .into_iter()
                        .map(|item| (item, Some(0))) // Placeholder score for AUR items
                        .collect();
                    items_with_scores.extend(aur_scored);
                }

                // Sort items based on fuzzy mode
                if fuzzy_mode {
                    // In fuzzy mode, sort by fuzzy score first (higher = better), then repo order
                    items_with_scores.sort_by(|a, b| {
                        match (a.1, b.1) {
                            (Some(sa), Some(sb)) => {
                                // Higher score first
                                match sb.cmp(&sa) {
                                    std::cmp::Ordering::Equal => {
                                        let oa = repo_order(&a.0.source);
                                        let ob = repo_order(&b.0.source);
                                        if oa != ob {
                                            return oa.cmp(&ob);
                                        }
                                        a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase())
                                    }
                                    other => other,
                                }
                            }
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (None, None) => {
                                let oa = repo_order(&a.0.source);
                                let ob = repo_order(&b.0.source);
                                if oa != ob {
                                    return oa.cmp(&ob);
                                }
                                a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase())
                            }
                        }
                    });
                } else {
                    // Normal mode: use existing match_rank logic
                    let ql = qtext.trim().to_lowercase();
                    items_with_scores.sort_by(|a, b| {
                        let oa = repo_order(&a.0.source);
                        let ob = repo_order(&b.0.source);
                        if oa != ob {
                            return oa.cmp(&ob);
                        }
                        let ra = match_rank(&a.0.name, &ql);
                        let rb = match_rank(&b.0.name, &ql);
                        if ra != rb {
                            return ra.cmp(&rb);
                        }
                        a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase())
                    });
                }

                // Extract items from scored tuples
                let mut items: Vec<crate::state::PackageItem> = items_with_scores
                    .into_iter()
                    .map(|(item, _)| item)
                    .collect();

                // Deduplicate by package name, preferring earlier entries (official over AUR)
                {
                    use std::collections::HashSet;
                    let mut seen = HashSet::new();
                    items.retain(|p| seen.insert(p.name.to_lowercase()));
                }
                for e in errors {
                    let _ = err_tx.send(e);
                }
                let _ = tx.send(SearchResults { id: sid, items });
            });
        }
    });
}
