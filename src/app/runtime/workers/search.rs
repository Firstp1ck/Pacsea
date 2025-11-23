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
            let sid = latest.id;
            let tx = search_result_tx.clone();
            let err_tx = net_err_tx_search.clone();
            let ipath = index_path.clone();
            tokio::spawn(async move {
                if crate::index::all_official().is_empty() {
                    let _ = crate::index::all_official_or_fetch(&ipath);
                }
                let mut items = pkgindex::search_official(&qtext);
                let q_for_net = qtext.clone();
                let (aur_items, errors) = sources::fetch_all_with_errors(q_for_net).await;
                items.extend(aur_items);
                let ql = qtext.trim().to_lowercase();
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    let ra = match_rank(&a.name, &ql);
                    let rb = match_rank(&b.name, &ql);
                    if ra != rb {
                        return ra.cmp(&rb);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
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
