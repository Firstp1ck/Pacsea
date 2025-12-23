use std::collections::HashSet;

use rand::Rng;
use tokio::{sync::mpsc, sync::oneshot, time::Duration};

use crate::index as pkgindex;
use crate::sources;
use crate::state::types::{NewsFeedSource, NewsSortMode};

/// What: Ensures installed packages set is populated, refreshing caches if needed.
///
/// Inputs:
/// - `installed`: Initial set of installed package names
///
/// Output:
/// - `HashSet<String>` with installed package names (refreshed if needed)
///
/// Details:
/// - If the initial set is empty, refreshes installed and explicit caches
/// - Returns refreshed set if available, otherwise returns original set
pub async fn ensure_installed_set(installed: HashSet<String>) -> HashSet<String> {
    if installed.is_empty() {
        crate::index::refresh_installed_cache().await;
        crate::index::refresh_explicit_cache(crate::state::InstalledPackagesMode::AllExplicit)
            .await;
        let refreshed: HashSet<String> = pkgindex::explicit_names().into_iter().collect();
        if !refreshed.is_empty() {
            return refreshed;
        }
    }
    installed
}

/// What: Filters news feed items by source type based on startup news preferences.
///
/// Inputs:
/// - `feed`: Vector of news feed items to filter
/// - `prefs`: Theme settings containing startup news preferences
///
/// Output:
/// - Filtered vector of news feed items
///
/// Details:
/// - Filters items based on whether each source type is enabled in preferences
pub fn filter_news_by_source(
    feed: Vec<crate::state::types::NewsFeedItem>,
    prefs: &crate::theme::Settings,
) -> Vec<crate::state::types::NewsFeedItem> {
    feed.into_iter()
        .filter(|item| match item.source {
            crate::state::types::NewsFeedSource::ArchNews => prefs.startup_news_show_arch_news,
            crate::state::types::NewsFeedSource::SecurityAdvisory => {
                prefs.startup_news_show_advisories
            }
            crate::state::types::NewsFeedSource::InstalledPackageUpdate => {
                prefs.startup_news_show_pkg_updates
            }
            crate::state::types::NewsFeedSource::AurPackageUpdate => {
                prefs.startup_news_show_aur_updates
            }
            crate::state::types::NewsFeedSource::AurComment => prefs.startup_news_show_aur_comments,
        })
        .collect()
}

/// What: Filters news feed items by maximum age in days.
///
/// Inputs:
/// - `feed`: Vector of news feed items to filter
/// - `max_age_days`: Optional maximum age in days
///
/// Output:
/// - Filtered vector of news feed items
///
/// Details:
/// - If `max_age_days` is Some, filters out items older than the cutoff date
/// - If `max_age_days` is None, returns all items unchanged
pub fn filter_news_by_age(
    feed: Vec<crate::state::types::NewsFeedItem>,
    max_age_days: Option<u32>,
) -> Vec<crate::state::types::NewsFeedItem> {
    if let Some(max_days) = max_age_days {
        let cutoff_date = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(i64::from(max_days)))
            .map(|dt| dt.format("%Y-%m-%d").to_string());
        #[allow(clippy::unnecessary_map_or)]
        feed.into_iter()
            .filter(|item| {
                cutoff_date
                    .as_ref()
                    .map_or(true, |cutoff| &item.date >= cutoff)
            })
            .collect()
    } else {
        feed
    }
}

/// What: Filters out already-read news items by ID and URL.
///
/// Inputs:
/// - `feed`: Vector of news feed items to filter
/// - `read_ids`: Set of already-read news IDs
/// - `read_urls`: Set of already-read news URLs
///
/// Output:
/// - Filtered vector containing only unread items
///
/// Details:
/// - Removes items whose ID is in the `read_ids` set or whose URL is in the `read_urls` set
/// - Package updates and AUR comments are tracked by ID, while Arch news items are tracked by URL
pub fn filter_unread_news(
    feed: Vec<crate::state::types::NewsFeedItem>,
    read_ids: &HashSet<String>,
    read_urls: &HashSet<String>,
) -> Vec<crate::state::types::NewsFeedItem> {
    feed.into_iter()
        .filter(|item| {
            !read_ids.contains(&item.id)
                && item.url.as_ref().is_none_or(|url| !read_urls.contains(url))
        })
        .collect()
}

/// What: Spawns startup news worker that fetches and filters news items for startup popup.
///
/// Inputs:
/// - `news_tx`: Channel sender for startup news updates
/// - `news_read_ids`: Set of already-read news IDs
/// - `news_read_urls`: Set of already-read news URLs
/// - `news_seen_pkg_versions`: Map of seen package versions
/// - `news_seen_aur_comments`: Map of seen AUR comments
/// - `last_startup_timestamp`: Previous TUI startup time for incremental updates
/// - `completion_tx`: Optional oneshot sender to signal completion
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Fetches news items based on startup news preferences
/// - Filters by source type, max age, and read status (by both ID and URL)
/// - Sends filtered items to the news channel
pub fn spawn_startup_news_worker(
    news_tx: &mpsc::UnboundedSender<Vec<crate::state::types::NewsFeedItem>>,
    news_read_ids: &HashSet<String>,
    news_read_urls: &HashSet<String>,
    news_seen_pkg_versions: &std::collections::HashMap<String, String>,
    news_seen_aur_comments: &std::collections::HashMap<String, String>,
    last_startup_timestamp: Option<&str>,
    completion_tx: Option<oneshot::Sender<()>>,
) {
    let prefs = crate::theme::settings();
    if !prefs.startup_news_configured {
        // If startup news is not configured, signal completion immediately
        if let Some(tx) = completion_tx {
            let _ = tx.send(());
        }
        return;
    }

    let news_tx_once = news_tx.clone();
    let read_ids = news_read_ids.clone();
    let read_urls = news_read_urls.clone();
    let installed: HashSet<String> = pkgindex::explicit_names().into_iter().collect();
    let mut seen_versions = news_seen_pkg_versions.clone();
    let mut seen_aur_comments = news_seen_aur_comments.clone();
    let last_startup = last_startup_timestamp.map(str::to_owned);
    tracing::info!(
        read_ids = read_ids.len(),
        read_urls = read_urls.len(),
        last_startup = ?last_startup,
        "queueing startup news fetch (startup)"
    );
    tokio::spawn(async move {
        // Use random jitter (0-500ms) before startup news fetch
        // Keep this short since the startup popup should appear quickly
        let jitter_ms = rand::rng().random_range(0..=500_u64);
        if jitter_ms > 0 {
            tracing::info!(jitter_ms, "staggering startup news fetch");
            tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
        }
        tracing::info!("startup news fetch task started");
        let optimized_max_age = sources::optimize_max_age_for_startup(
            last_startup.as_deref(),
            prefs.startup_news_max_age_days,
        );
        let installed_set = ensure_installed_set(installed).await;
        let include_pkg_updates =
            prefs.startup_news_show_pkg_updates || prefs.startup_news_show_aur_updates;
        #[allow(clippy::items_after_statements)]
        const STARTUP_NEWS_LIMIT: usize = 20;
        let updates_limit =
            if prefs.startup_news_show_pkg_updates && prefs.startup_news_show_aur_updates {
                STARTUP_NEWS_LIMIT * 2
            } else {
                STARTUP_NEWS_LIMIT
            };
        let ctx = sources::NewsFeedContext {
            force_emit_all: true,
            updates_list_path: Some(crate::theme::lists_dir().join("available_updates.txt")),
            limit: updates_limit,
            include_arch_news: prefs.startup_news_show_arch_news,
            include_advisories: prefs.startup_news_show_advisories,
            include_pkg_updates,
            include_aur_comments: prefs.startup_news_show_aur_comments,
            installed_filter: Some(&installed_set),
            installed_only: false,
            sort_mode: NewsSortMode::DateDesc,
            seen_pkg_versions: &mut seen_versions,
            seen_aur_comments: &mut seen_aur_comments,
            max_age_days: optimized_max_age,
        };
        tracing::info!(
            limit = updates_limit,
            include_arch_news = prefs.startup_news_show_arch_news,
            include_advisories = prefs.startup_news_show_advisories,
            include_pkg_updates,
            include_aur_comments = prefs.startup_news_show_aur_comments,
            configured_max_age = ?prefs.startup_news_max_age_days,
            optimized_max_age = ?optimized_max_age,
            installed_count = installed_set.len(),
            "starting startup news fetch"
        );
        match sources::fetch_news_feed(ctx).await {
            Ok(feed) => {
                tracing::info!(
                    total_items = feed.len(),
                    "startup news fetch completed successfully"
                );
                let source_filtered = filter_news_by_source(feed, &prefs);
                let filtered = filter_news_by_age(source_filtered, prefs.startup_news_max_age_days);
                let unread = filter_unread_news(filtered, &read_ids, &read_urls);
                tracing::info!(
                    unread_count = unread.len(),
                    "sending startup news items to channel"
                );
                match news_tx_once.send(unread) {
                    Ok(()) => {
                        tracing::info!("startup news items sent to channel successfully");
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "failed to send startup news items to channel (receiver dropped?)"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "startup news fetch failed");
                tracing::info!("sending empty array to clear loading flag after fetch error");
                let _ = news_tx_once.send(Vec::new());
            }
        }
        // Signal completion to allow aggregated feed fetch to proceed
        if let Some(tx) = completion_tx {
            let _ = tx.send(());
        }
    });
}

/// What: Spawns aggregated news feed worker that fetches combined news feed.
///
/// Inputs:
/// - `news_feed_tx`: Channel sender for aggregated news feed
/// - `news_incremental_tx`: Channel sender for incremental background news items
/// - `news_seen_pkg_versions`: Map of seen package versions
/// - `news_seen_aur_comments`: Map of seen AUR comments
/// - `completion_rx`: Optional oneshot receiver to wait for startup news fetch completion
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Fetches aggregated news feed (Arch news + security advisories + package updates + AUR comments)
/// - Sends feed payload to the news feed channel
/// - Spawns background continuation task to fetch remaining items after initial limit
/// - Waits for startup news fetch to complete before starting to prevent concurrent archlinux.org requests
pub fn spawn_aggregated_news_feed_worker(
    news_feed_tx: &mpsc::UnboundedSender<crate::state::types::NewsFeedPayload>,
    news_incremental_tx: &mpsc::UnboundedSender<crate::state::types::NewsFeedItem>,
    news_seen_pkg_versions: &std::collections::HashMap<String, String>,
    news_seen_aur_comments: &std::collections::HashMap<String, String>,
    completion_rx: Option<oneshot::Receiver<()>>,
) {
    let news_feed_tx_once = news_feed_tx.clone();
    let news_incremental_tx_clone = news_incremental_tx.clone();
    let installed: HashSet<String> = pkgindex::explicit_names().into_iter().collect();
    let mut seen_versions = news_seen_pkg_versions.clone();
    let mut seen_aur_comments = news_seen_aur_comments.clone();
    tracing::info!(
        installed_names = installed.len(),
        "queueing combined news feed fetch (startup)"
    );
    tokio::spawn(async move {
        // Wait for startup news fetch to complete before starting aggregated feed fetch
        // This prevents concurrent requests to archlinux.org which can cause rate limiting/blocking
        if let Some(rx) = completion_rx {
            tracing::info!(
                "waiting for startup news fetch to complete before starting aggregated feed fetch"
            );
            let _ = rx.await; // Wait for startup fetch completion signal
            // Add a small additional delay after startup fetch completes to ensure clean separation
            let additional_delay_ms = rand::rng().random_range(500..=1500_u64);
            tracing::info!(
                additional_delay_ms,
                "additional delay after startup fetch completion"
            );
            tokio::time::sleep(Duration::from_millis(additional_delay_ms)).await;
        } else {
            // Fallback: use fixed delay if no completion signal is provided
            // This should not happen in normal operation, but provides safety
            let base_delay_ms = 10000_u64; // Increased to 10 seconds as fallback
            let jitter_ms = rand::rng().random_range(0..=2000_u64);
            let stagger_ms = base_delay_ms + jitter_ms;
            tracing::warn!(
                stagger_ms,
                "no completion signal available, using fallback delay for aggregated feed fetch"
            );
            tokio::time::sleep(Duration::from_millis(stagger_ms)).await;
        }
        let installed_set = ensure_installed_set(installed).await;
        let ctx = sources::NewsFeedContext {
            force_emit_all: true,
            updates_list_path: Some(crate::theme::lists_dir().join("available_updates.txt")),
            limit: 50,
            include_arch_news: true,
            include_advisories: true,
            include_pkg_updates: true,
            include_aur_comments: true,
            installed_filter: Some(&installed_set),
            installed_only: false,
            sort_mode: NewsSortMode::DateDesc,
            seen_pkg_versions: &mut seen_versions,
            seen_aur_comments: &mut seen_aur_comments,
            max_age_days: None, // Main feed doesn't use date filtering
        };
        match sources::fetch_news_feed(ctx).await {
            Ok(feed) => {
                let arch_ct = feed
                    .iter()
                    .filter(|i| matches!(i.source, NewsFeedSource::ArchNews))
                    .count();
                let adv_ct = feed
                    .iter()
                    .filter(|i| matches!(i.source, NewsFeedSource::SecurityAdvisory))
                    .count();
                tracing::info!(
                    total = feed.len(),
                    arch = arch_ct,
                    advisories = adv_ct,
                    installed_names = installed_set.len(),
                    "news feed fetched"
                );
                if feed.is_empty() {
                    tracing::warn!(
                        installed_names = installed_set.len(),
                        "news feed is empty after fetch"
                    );
                }
                let payload = crate::state::types::NewsFeedPayload {
                    items: feed.clone(),
                    seen_pkg_versions: seen_versions,
                    seen_aur_comments,
                };
                tracing::info!(
                    items_count = feed.len(),
                    "sending aggregated news feed payload to channel"
                );
                if let Err(e) = news_feed_tx_once.send(payload) {
                    tracing::warn!(error = ?e, "failed to send news feed to channel");
                } else {
                    tracing::info!("aggregated news feed payload sent successfully");
                    // Spawn background continuation task to fetch remaining items
                    let initial_ids: HashSet<String> = feed.iter().map(|i| i.id.clone()).collect();
                    spawn_news_continuation_worker(
                        news_incremental_tx_clone.clone(),
                        installed_set.clone(),
                        initial_ids,
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to fetch news feed");
            }
        }
    });
}

/// What: Spawns background worker to continue fetching news items after initial limit.
///
/// Inputs:
/// - `news_incremental_tx`: Channel sender for incremental news items
/// - `installed_set`: Set of installed package names
/// - `initial_ids`: Set of item IDs already sent in initial batch
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Fetches remaining items from all news sources (no limit)
/// - Sends one item per second to the channel
/// - Skips items already in `initial_ids`
fn spawn_news_continuation_worker(
    news_incremental_tx: mpsc::UnboundedSender<crate::state::types::NewsFeedItem>,
    installed_set: HashSet<String>,
    initial_ids: HashSet<String>,
) {
    tokio::spawn(async move {
        tracing::info!(
            initial_count = initial_ids.len(),
            "starting news continuation worker"
        );

        // Wait a bit before starting continuation to let UI settle
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Fetch continuation items from sources (high limit to get everything)
        let continuation_items =
            sources::fetch_continuation_items(&installed_set, &initial_ids).await;

        match continuation_items {
            Ok(items) => {
                tracing::info!(
                    count = items.len(),
                    "continuation worker received items to send"
                );
                for item in items {
                    // Skip if already sent in initial batch
                    if initial_ids.contains(&item.id) {
                        continue;
                    }
                    // Send item to channel
                    if let Err(e) = news_incremental_tx.send(item.clone()) {
                        tracing::warn!(error = ?e, "failed to send incremental news item");
                        break;
                    }
                    // Throttle: 1 item per second
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                tracing::info!("news continuation worker completed");
            }
            Err(e) => {
                tracing::warn!(error = %e, "news continuation fetch failed");
            }
        }
    });
}
