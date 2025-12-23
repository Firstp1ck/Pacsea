//! News and advisories fetching with caching.
use std::collections::HashSet;
use std::hash::BuildHasher;
use std::time::{Duration, Instant};

use crate::state::types::{NewsFeedItem, NewsFeedSource};
use tracing::{info, warn};

use super::Result;
use super::cache::{
    CACHE_TTL_SECONDS, CacheEntry, NEWS_CACHE, load_from_disk_cache, save_to_disk_cache,
};
use super::rate_limit::{
    extract_retry_after_from_error, increase_archlinux_backoff, rate_limit, rate_limit_archlinux,
    reset_archlinux_backoff, retry_with_backoff, set_network_error,
};

/// What: Fetch Arch news items with optional early date filtering and caching.
///
/// Inputs:
/// - `limit`: Maximum items to fetch.
/// - `cutoff_date`: Optional date string (YYYY-MM-DD) for early filtering.
///
/// Output: Vector of `NewsFeedItem` representing Arch news.
///
/// Details:
/// - If `cutoff_date` is provided, stops fetching when items exceed the date limit.
/// - Uses in-memory cache with 15-minute TTL to avoid redundant fetches.
/// - Falls back to disk cache (configurable TTL, default 14 days) if in-memory cache misses.
/// - Saves fetched data to both in-memory and disk caches.
pub(super) async fn append_arch_news(
    limit: usize,
    cutoff_date: Option<&str>,
) -> Result<Vec<NewsFeedItem>> {
    const SOURCE: &str = "arch_news";

    // 1. Check in-memory cache first (fastest, 15-minute TTL)
    if cutoff_date.is_none()
        && let Ok(cache) = NEWS_CACHE.lock()
        && let Some(entry) = cache.get(SOURCE)
        && entry.timestamp.elapsed().as_secs() < CACHE_TTL_SECONDS
    {
        info!("using in-memory cached arch news");
        return Ok(entry.data.clone());
    }

    // 2. Check disk cache (24-hour TTL) - useful after app restart
    if cutoff_date.is_none()
        && let Some(disk_data) = load_from_disk_cache(SOURCE)
    {
        // Populate in-memory cache from disk
        if let Ok(mut cache) = NEWS_CACHE.lock() {
            cache.insert(
                SOURCE.to_string(),
                CacheEntry {
                    data: disk_data.clone(),
                    timestamp: Instant::now(),
                },
            );
        }
        return Ok(disk_data);
    }

    // 3. Fetch from network - NO retries for arch news to avoid long delays
    // If archlinux.org is blocked/slow, we skip it rather than retrying
    let fetch_result: std::result::Result<
        Vec<crate::state::types::NewsItem>,
        Box<dyn std::error::Error + Send + Sync>,
    > = {
        // Acquire semaphore permit and hold it during the request
        // This ensures only one archlinux.org request is in flight at a time
        let _permit = rate_limit_archlinux().await;
        let result = crate::sources::fetch_arch_news(limit, cutoff_date).await;
        // Check for HTTP 429/503 and update backoff for future requests
        if let Err(ref e) = result {
            let error_str = e.to_string();
            let retry_after_seconds = extract_retry_after_from_error(&error_str);
            if error_str.contains("429") || error_str.contains("503") {
                if let Some(retry_after) = retry_after_seconds {
                    warn!(
                        retry_after_seconds = retry_after,
                        "HTTP {} detected, noting Retry-After for future requests",
                        if error_str.contains("429") {
                            "429"
                        } else {
                            "503"
                        }
                    );
                    // Use Retry-After value for backoff on future requests
                    increase_archlinux_backoff(Some(retry_after));
                } else {
                    warn!(
                        "HTTP {} detected, increasing backoff for future requests",
                        if error_str.contains("429") {
                            "429"
                        } else {
                            "503"
                        }
                    );
                    // Increase backoff for future requests
                    increase_archlinux_backoff(None);
                }
            } else {
                // For other errors (timeout, network), only mild backoff increase
                increase_archlinux_backoff(None);
            }
        }
        result
    };
    match fetch_result {
        Ok(news) => {
            // Reset backoff after successful request
            reset_archlinux_backoff();
            let items: Vec<NewsFeedItem> = news
                .into_iter()
                .map(|n| NewsFeedItem {
                    id: n.url.clone(),
                    date: n.date,
                    title: n.title,
                    summary: None,
                    url: Some(n.url),
                    source: NewsFeedSource::ArchNews,
                    severity: None,
                    packages: Vec::new(),
                })
                .collect();
            // Cache the result (only if no cutoff_date)
            if cutoff_date.is_none() {
                // Save to in-memory cache
                if let Ok(mut cache) = NEWS_CACHE.lock() {
                    cache.insert(
                        SOURCE.to_string(),
                        CacheEntry {
                            data: items.clone(),
                            timestamp: Instant::now(),
                        },
                    );
                }
                // Save to disk cache for persistence across restarts
                save_to_disk_cache(SOURCE, &items);
            }
            Ok(items)
        }
        Err(e) => {
            warn!(error = %e, "arch news fetch failed");
            set_network_error();
            // Increase backoff after failure
            increase_archlinux_backoff(None);
            // Graceful degradation: try in-memory cache first
            if let Ok(cache) = NEWS_CACHE.lock()
                && let Some(entry) = cache.get(SOURCE)
            {
                info!(
                    cached_items = entry.data.len(),
                    age_secs = entry.timestamp.elapsed().as_secs(),
                    "using stale in-memory cached arch news due to fetch failure"
                );
                return Ok(entry.data.clone());
            }
            // Then try disk cache (ignores TTL for fallback)
            if let Some(disk_data) = load_from_disk_cache(SOURCE) {
                info!(
                    cached_items = disk_data.len(),
                    "using disk cached arch news due to fetch failure"
                );
                return Ok(disk_data);
            }
            Err(e)
        }
    }
}

/// What: Fetch security advisories with optional early date filtering and caching.
///
/// Inputs:
/// - `limit`: Maximum items to fetch.
/// - `installed_filter`: Optional installed set for filtering.
/// - `installed_only`: Whether to drop advisories unrelated to installed packages.
/// - `cutoff_date`: Optional date string (YYYY-MM-DD) for early filtering.
///
/// Output: Vector of `NewsFeedItem` representing security advisories.
///
/// Details:
/// - If `cutoff_date` is provided, stops fetching when items exceed the date limit.
/// - Uses in-memory cache with 15-minute TTL to avoid redundant fetches.
/// - Falls back to disk cache (configurable TTL, default 14 days) if in-memory cache misses.
/// - Note: Cache key includes `installed_only` flag to handle different filtering needs.
pub(super) async fn append_advisories<S>(
    limit: usize,
    installed_filter: Option<&HashSet<String, S>>,
    installed_only: bool,
    cutoff_date: Option<&str>,
) -> Result<Vec<NewsFeedItem>>
where
    S: BuildHasher + Send + Sync + 'static,
{
    const SOURCE: &str = "advisories";

    // 1. Check in-memory cache first (fastest, 15-minute TTL)
    if cutoff_date.is_none()
        && !installed_only
        && let Ok(cache) = NEWS_CACHE.lock()
        && let Some(entry) = cache.get(SOURCE)
        && entry.timestamp.elapsed().as_secs() < CACHE_TTL_SECONDS
    {
        info!("using in-memory cached advisories");
        return Ok(entry.data.clone());
    }

    // 2. Check disk cache (24-hour TTL) - useful after app restart
    if cutoff_date.is_none()
        && !installed_only
        && let Some(disk_data) = load_from_disk_cache(SOURCE)
    {
        // Populate in-memory cache from disk
        if let Ok(mut cache) = NEWS_CACHE.lock() {
            cache.insert(
                SOURCE.to_string(),
                CacheEntry {
                    data: disk_data.clone(),
                    timestamp: Instant::now(),
                },
            );
        }
        return Ok(disk_data);
    }

    // 3. Fetch from network with retry and exponential backoff
    rate_limit().await;
    let fetch_result = retry_with_backoff(
        || async {
            rate_limit().await;
            crate::sources::fetch_security_advisories(limit, cutoff_date).await
        },
        2, // Max 2 retries (3 total attempts)
    )
    .await;
    match fetch_result {
        Ok(advisories) => {
            let mut filtered = Vec::new();
            for adv in advisories {
                if installed_only
                    && let Some(set) = installed_filter
                    && !adv.packages.iter().any(|p| set.contains(p))
                {
                    continue;
                }
                filtered.push(adv);
            }
            // Cache the result (only if no cutoff_date and not installed_only)
            if cutoff_date.is_none() && !installed_only {
                // Save to in-memory cache
                if let Ok(mut cache) = NEWS_CACHE.lock() {
                    cache.insert(
                        SOURCE.to_string(),
                        CacheEntry {
                            data: filtered.clone(),
                            timestamp: Instant::now(),
                        },
                    );
                }
                // Save to disk cache for persistence across restarts
                save_to_disk_cache(SOURCE, &filtered);
            }
            Ok(filtered)
        }
        Err(e) => {
            warn!(error = %e, "security advisories fetch failed");
            set_network_error();
            // Graceful degradation: try in-memory cache first
            if let Ok(cache) = NEWS_CACHE.lock()
                && let Some(entry) = cache.get(SOURCE)
            {
                info!(
                    cached_items = entry.data.len(),
                    age_secs = entry.timestamp.elapsed().as_secs(),
                    "using stale in-memory cached advisories due to fetch failure"
                );
                return Ok(entry.data.clone());
            }
            // Then try disk cache (ignores TTL for fallback)
            if let Some(disk_data) = load_from_disk_cache(SOURCE) {
                info!(
                    cached_items = disk_data.len(),
                    "using disk cached advisories due to fetch failure"
                );
                return Ok(disk_data);
            }
            Err(e)
        }
    }
}

/// What: Fetch slow sources (Arch news and advisories) in parallel with timeout.
///
/// Inputs:
/// - `include_arch_news`: Whether to fetch Arch news.
/// - `include_advisories`: Whether to fetch advisories.
/// - `limit`: Maximum items per source.
/// - `installed_filter`: Optional set of installed package names.
/// - `installed_only`: Whether to restrict advisories to installed packages.
/// - `cutoff_date`: Optional date cutoff for filtering.
///
/// Output:
/// - Tuple of (`arch_result`, `advisories_result`).
///
/// Details:
/// - Applies 30-second timeout to match HTTP client timeout.
/// - Returns empty vectors on timeout or errors (graceful degradation).
pub(super) async fn fetch_slow_sources<HS>(
    include_arch_news: bool,
    include_advisories: bool,
    limit: usize,
    installed_filter: Option<&HashSet<String, HS>>,
    installed_only: bool,
    cutoff_date: Option<&str>,
) -> (
    std::result::Result<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>,
    std::result::Result<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>,
)
where
    HS: BuildHasher + Send + Sync + 'static,
{
    let arch_result: std::result::Result<
        Vec<NewsFeedItem>,
        Box<dyn std::error::Error + Send + Sync>,
    > = if include_arch_news {
        info!("fetching arch news...");
        tokio::time::timeout(
            Duration::from_secs(30),
            append_arch_news(limit, cutoff_date),
        )
        .await
        .map_or_else(
            |_| {
                warn!("arch news fetch timed out after 30s, continuing without arch news");
                Err("Arch news fetch timeout".into())
            },
            |result| {
                info!(
                    "arch news fetch completed: items={}",
                    result.as_ref().map(Vec::len).unwrap_or(0)
                );
                result
            },
        )
    } else {
        Ok(Vec::new())
    };

    let advisories_result: std::result::Result<
        Vec<NewsFeedItem>,
        Box<dyn std::error::Error + Send + Sync>,
    > = if include_advisories {
        info!("fetching advisories...");
        tokio::time::timeout(
            Duration::from_secs(30),
            append_advisories(limit, installed_filter, installed_only, cutoff_date),
        )
        .await
        .map_or_else(
            |_| {
                warn!("advisories fetch timed out after 30s, continuing without advisories");
                Err("Advisories fetch timeout".into())
            },
            |result| {
                info!(
                    "advisories fetch completed: items={}",
                    result.as_ref().map(Vec::len).unwrap_or(0)
                );
                result
            },
        )
    } else {
        Ok(Vec::new())
    };

    (arch_result, advisories_result)
}
