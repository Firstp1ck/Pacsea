//! Cache management for news feeds (in-memory and disk).
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use crate::state::types::NewsFeedItem;
use tracing::{debug, info, warn};

/// Cache entry with data and timestamp (in-memory).
pub(super) struct CacheEntry {
    /// Cached news feed items.
    pub data: Vec<NewsFeedItem>,
    /// Timestamp when the cache entry was created.
    pub timestamp: Instant,
}

/// Disk cache entry with data and Unix timestamp (for serialization).
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub(super) struct DiskCacheEntry {
    /// Cached news feed items.
    pub data: Vec<NewsFeedItem>,
    /// Unix timestamp (seconds since epoch) when the cache was saved.
    pub saved_at: i64,
}

/// Simple in-memory cache for Arch news and advisories.
/// Key: source type (`"arch_news"` or `"advisories"`)
/// TTL: 15 minutes
pub(super) static NEWS_CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// Cache TTL in seconds (15 minutes).
pub(super) const CACHE_TTL_SECONDS: u64 = 900;

/// Type alias for skip cache entry (results + timestamp).
pub(super) type SkipCacheEntry = Option<(Vec<NewsFeedItem>, Instant)>;

/// Cache for package updates results (time-based skip).
/// Stores last fetch results and timestamp to avoid re-fetching within 5 minutes.
pub(super) static UPDATES_CACHE: LazyLock<Mutex<SkipCacheEntry>> =
    LazyLock::new(|| Mutex::new(None));

/// Cache for AUR comments results (time-based skip).
/// Stores last fetch results and timestamp to avoid re-fetching within 5 minutes.
pub(super) static AUR_COMMENTS_CACHE: LazyLock<Mutex<SkipCacheEntry>> =
    LazyLock::new(|| Mutex::new(None));

/// Skip cache TTL in seconds (5 minutes) - if last fetch was within this time, use cached results.
pub(super) const SKIP_CACHE_TTL_SECONDS: u64 = 300;

/// What: Get the disk cache TTL in seconds from settings.
///
/// Inputs: None
///
/// Output: TTL in seconds (defaults to 14 days = 1209600 seconds).
///
/// Details:
/// - Reads `news_cache_ttl_days` from settings and converts to seconds.
/// - Minimum is 1 day to prevent excessive network requests.
pub(super) fn disk_cache_ttl_seconds() -> i64 {
    let days = crate::theme::settings().news_cache_ttl_days.max(1);
    i64::from(days) * 86400 // days to seconds
}

/// What: Get the path to a disk cache file for a specific source.
///
/// Inputs:
/// - `source`: Cache source identifier (`"arch_news"` or `"advisories"`).
///
/// Output:
/// - `PathBuf` to the cache file.
pub(super) fn disk_cache_path(source: &str) -> std::path::PathBuf {
    crate::theme::lists_dir().join(format!("{source}_cache.json"))
}

/// What: Load cached data from disk if available and not expired.
///
/// Inputs:
/// - `source`: Cache source identifier.
///
/// Output:
/// - `Some(Vec<NewsFeedItem>)` if valid cache exists, `None` otherwise.
///
/// Details:
/// - Returns `None` if file doesn't exist, is corrupted, or cache is older than configured TTL.
pub(super) fn load_from_disk_cache(source: &str) -> Option<Vec<NewsFeedItem>> {
    let path = disk_cache_path(source);
    let content = std::fs::read_to_string(&path).ok()?;
    let entry: DiskCacheEntry = serde_json::from_str(&content).ok()?;
    let now = chrono::Utc::now().timestamp();
    let age = now - entry.saved_at;
    let ttl = disk_cache_ttl_seconds();
    if age < ttl {
        info!(
            source,
            items = entry.data.len(),
            age_hours = age / 3600,
            ttl_days = ttl / 86400,
            "loaded from disk cache"
        );
        Some(entry.data)
    } else {
        debug!(
            source,
            age_hours = age / 3600,
            ttl_days = ttl / 86400,
            "disk cache expired"
        );
        None
    }
}

/// What: Save data to disk cache with current timestamp.
///
/// Inputs:
/// - `source`: Cache source identifier.
/// - `data`: News feed items to cache.
///
/// Details:
/// - Writes to disk asynchronously to avoid blocking.
/// - Logs errors but does not propagate them.
pub(super) fn save_to_disk_cache(source: &str, data: &[NewsFeedItem]) {
    let entry = DiskCacheEntry {
        data: data.to_vec(),
        saved_at: chrono::Utc::now().timestamp(),
    };
    let path = disk_cache_path(source);
    match serde_json::to_string_pretty(&entry) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!(error = %e, source, "failed to write disk cache");
            } else {
                debug!(source, items = data.len(), "saved to disk cache");
            }
        }
        Err(e) => warn!(error = %e, source, "failed to serialize disk cache"),
    }
}
