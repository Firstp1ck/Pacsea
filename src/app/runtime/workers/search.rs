use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use tokio::{
    select,
    sync::mpsc,
    time::{Duration, sleep},
};

use crate::index as pkgindex;
use crate::sources;
use crate::state::{PackageItem, QueryInput, SearchResults};
use crate::util::{fuzzy_match_rank_with_matcher, match_rank, repo_order};

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
                let items = handle_empty_query(&index_path);
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

            let query = latest;
            let tx = search_result_tx.clone();
            let err_tx = net_err_tx_search.clone();
            let ipath = index_path.clone();
            tokio::spawn(async move {
                let (items, errors) = process_search_query(&query.text, query.fuzzy, &ipath).await;
                for e in errors {
                    let _ = err_tx.send(e);
                }
                let _ = tx.send(SearchResults {
                    id: query.id,
                    items,
                });
            });
        }
    });
}

/// What: Handle empty query by returning all official packages.
///
/// Inputs:
/// - `index_path`: Path to official package index
///
/// Output:
/// - Sorted and deduplicated list of all official packages
///
/// Details:
/// - Fetches all official packages
/// - Sorts by repo order (core > extra > others), then by name
/// - Deduplicates by package name, preferring earlier entries
fn handle_empty_query(index_path: &Path) -> Vec<PackageItem> {
    let mut items = pkgindex::all_official_or_fetch(index_path);
    sort_by_repo_and_name(&mut items);
    deduplicate_items(&mut items);
    items
}

/// What: Process a search query and return sorted results.
///
/// Inputs:
/// - `query_text`: Search query text
/// - `fuzzy_mode`: Whether to use fuzzy matching
/// - `index_path`: Path to official package index
///
/// Output:
/// - Tuple of (sorted and deduplicated list of matching packages, network errors)
///
/// Details:
/// - Ensures official index is loaded
/// - Searches official packages
/// - Fetches and filters AUR packages
/// - Combines, scores, and sorts results
async fn process_search_query(
    query_text: &str,
    fuzzy_mode: bool,
    index_path: &Path,
) -> (Vec<PackageItem>, Vec<String>) {
    if crate::index::all_official().is_empty() {
        let _ = crate::index::all_official_or_fetch(index_path);
    }
    let official_results = pkgindex::search_official(query_text, fuzzy_mode);
    let (aur_items, errors) = sources::fetch_all_with_errors(query_text.to_string()).await;

    let mut items_with_scores = official_results;
    score_aur_items(&mut items_with_scores, aur_items, query_text, fuzzy_mode);
    sort_scored_items(&mut items_with_scores, query_text, fuzzy_mode);

    let mut items: Vec<PackageItem> = items_with_scores
        .into_iter()
        .map(|(item, _)| item)
        .collect();
    deduplicate_items(&mut items);
    (items, errors)
}

/// What: Score and add AUR items to the results list.
///
/// Inputs:
/// - `items_with_scores`: Mutable vector of (item, score) tuples to extend
/// - `aur_items`: List of AUR package items
/// - `query_text`: Search query text
/// - `fuzzy_mode`: Whether to use fuzzy matching
///
/// Details:
/// - In fuzzy mode: filters and scores AUR items using fuzzy matching
/// - In normal mode: adds all AUR items with placeholder score
#[allow(clippy::ptr_arg)] // Need &mut Vec to extend the vector
fn score_aur_items(
    items_with_scores: &mut Vec<(PackageItem, Option<i64>)>,
    aur_items: Vec<PackageItem>,
    query_text: &str,
    fuzzy_mode: bool,
) {
    if fuzzy_mode {
        let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
        let aur_scored: Vec<(PackageItem, Option<i64>)> = aur_items
            .into_iter()
            .filter_map(|item| {
                fuzzy_match_rank_with_matcher(&item.name, query_text, &matcher)
                    .map(|score| (item, Some(score)))
            })
            .collect();
        items_with_scores.extend(aur_scored);
    } else {
        let aur_scored: Vec<(PackageItem, Option<i64>)> =
            aur_items.into_iter().map(|item| (item, Some(0))).collect();
        items_with_scores.extend(aur_scored);
    }
}

/// What: Sort items based on fuzzy mode.
///
/// Inputs:
/// - `items_with_scores`: Mutable slice of (item, score) tuples
/// - `query_text`: Search query text
/// - `fuzzy_mode`: Whether to use fuzzy matching
///
/// Details:
/// - In fuzzy mode: sorts by fuzzy score (higher first), then repo order, then name
/// - In normal mode: sorts by match rank, then repo order, then name
fn sort_scored_items(
    items_with_scores: &mut [(PackageItem, Option<i64>)],
    query_text: &str,
    fuzzy_mode: bool,
) {
    if fuzzy_mode {
        sort_items_fuzzy(items_with_scores);
    } else {
        sort_items_normal(items_with_scores, query_text);
    }
}

/// What: Sort items in fuzzy mode by score, repo order, and name.
///
/// Inputs:
/// - `items_with_scores`: Mutable slice of (item, score) tuples
///
/// Details:
/// - Higher fuzzy scores come first
/// - Then sorted by repo order (official before AUR)
/// - Finally sorted by name (case-insensitive)
fn sort_items_fuzzy(items_with_scores: &mut [(PackageItem, Option<i64>)]) {
    items_with_scores.sort_by(|a, b| match (a.1, b.1) {
        (Some(sa), Some(sb)) => match sb.cmp(&sa) {
            std::cmp::Ordering::Equal => compare_by_repo_and_name(&a.0, &b.0),
            other => other,
        },
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => compare_by_repo_and_name(&a.0, &b.0),
    });
}

/// What: Sort items in normal mode by match rank, repo order, and name.
///
/// Inputs:
/// - `items_with_scores`: Mutable slice of (item, score) tuples
/// - `query_text`: Search query text
///
/// Details:
/// - Lower match ranks come first (exact > prefix > substring > no match)
/// - Then sorted by repo order (official before AUR)
/// - Finally sorted by name (case-insensitive)
fn sort_items_normal(items_with_scores: &mut [(PackageItem, Option<i64>)], query_text: &str) {
    let query_lower = query_text.trim().to_lowercase();
    items_with_scores.sort_by(|a, b| {
        let oa = repo_order(&a.0.source);
        let ob = repo_order(&b.0.source);
        if oa != ob {
            return oa.cmp(&ob);
        }
        let ra = match_rank(&a.0.name, &query_lower);
        let rb = match_rank(&b.0.name, &query_lower);
        if ra != rb {
            return ra.cmp(&rb);
        }
        a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase())
    });
}

/// What: Compare two packages by repo order and name.
///
/// Inputs:
/// - `a`: First package item
/// - `b`: Second package item
///
/// Output:
/// - Ordering comparison result
///
/// Details:
/// - First compares by repo order (official before AUR)
/// - Then compares by name (case-insensitive)
fn compare_by_repo_and_name(a: &PackageItem, b: &PackageItem) -> std::cmp::Ordering {
    let oa = repo_order(&a.source);
    let ob = repo_order(&b.source);
    oa.cmp(&ob)
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
}

/// What: Sort items by repo order and name.
///
/// Inputs:
/// - `items`: Mutable slice of package items
///
/// Details:
/// - Sorts by repo order (core > extra > others > AUR)
/// - Then sorts by name (case-insensitive)
fn sort_by_repo_and_name(items: &mut [PackageItem]) {
    items.sort_by(compare_by_repo_and_name);
}

/// What: Deduplicate items by package name, keeping first occurrence.
///
/// Inputs:
/// - `items`: Mutable reference to list of package items
///
/// Details:
/// - Removes duplicate packages based on case-insensitive name comparison
/// - Keeps the first occurrence (preferring earlier entries in sort order)
fn deduplicate_items(items: &mut Vec<PackageItem>) {
    let mut seen = HashSet::new();
    items.retain(|p| seen.insert(p.name.to_lowercase()));
}
