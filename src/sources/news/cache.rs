//! Cache management for article content.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Cache entry for article content with timestamp.
pub struct ArticleCacheEntry {
    /// Cached article content.
    pub content: String,
    /// Timestamp when the cache entry was created.
    pub timestamp: Instant,
    /// `ETag` from last response (for conditional requests).
    pub etag: Option<String>,
    /// `Last-Modified` date from last response (for conditional requests).
    pub last_modified: Option<String>,
}

/// Disk cache entry for article content with Unix timestamp (for serialization).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ArticleDiskCacheEntry {
    /// Cached article content.
    pub content: String,
    /// Unix timestamp (seconds since epoch) when the cache was saved.
    pub saved_at: i64,
    /// `ETag` from last response (for conditional requests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// `Last-Modified` date from last response (for conditional requests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

/// In-memory cache for article content.
/// Key: URL string
/// TTL: 15 minutes (same as news feed)
pub static ARTICLE_CACHE: LazyLock<Mutex<HashMap<String, ArticleCacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// Cache TTL in seconds (15 minutes, same as news feed).
pub const ARTICLE_CACHE_TTL_SECONDS: u64 = 900;

/// What: Get the disk cache TTL in seconds from settings.
///
/// Inputs: None
///
/// Output: TTL in seconds (defaults to 14 days = 1209600 seconds).
///
/// Details:
/// - Reads `news_cache_ttl_days` from settings and converts to seconds.
/// - Minimum is 1 day to prevent excessive network requests.
pub fn article_disk_cache_ttl_seconds() -> i64 {
    let days = crate::theme::settings().news_cache_ttl_days.max(1);
    i64::from(days) * 86400 // days to seconds
}

/// What: Get the path to the article content disk cache file.
///
/// Inputs: None
///
/// Output:
/// - `PathBuf` to the cache file.
pub fn article_disk_cache_path() -> std::path::PathBuf {
    crate::theme::lists_dir().join("news_article_cache.json")
}

/// What: Load cached article entry from disk if available and not expired.
///
/// Inputs:
/// - `url`: URL of the article to load from cache.
///
/// Output:
/// - `Some(ArticleDiskCacheEntry)` if valid cache exists, `None` otherwise.
///
/// Details:
/// - Returns `None` if file doesn't exist, is corrupted, or cache is older than configured TTL.
pub fn load_article_entry_from_disk_cache(url: &str) -> Option<ArticleDiskCacheEntry> {
    let path = article_disk_cache_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let cache: HashMap<String, ArticleDiskCacheEntry> = serde_json::from_str(&content).ok()?;
    let entry = cache.get(url)?.clone();
    let now = chrono::Utc::now().timestamp();
    let age = now - entry.saved_at;
    let ttl = article_disk_cache_ttl_seconds();
    if age < ttl {
        info!(
            url,
            age_hours = age / 3600,
            ttl_days = ttl / 86400,
            "loaded article from disk cache"
        );
        Some(entry)
    } else {
        debug!(
            url,
            age_hours = age / 3600,
            ttl_days = ttl / 86400,
            "article disk cache expired"
        );
        None
    }
}

/// What: Save article content to disk cache with current timestamp.
///
/// Inputs:
/// - `url`: URL of the article.
/// - `content`: Article content to cache.
/// - `etag`: Optional `ETag` from response.
/// - `last_modified`: Optional `Last-Modified` date from response.
///
/// Details:
/// - Writes to disk asynchronously to avoid blocking.
/// - Logs errors but does not propagate them.
/// - Updates existing cache file, adding or updating the entry for this URL.
pub fn save_article_to_disk_cache(
    url: &str,
    content: &str,
    etag: Option<String>,
    last_modified: Option<String>,
) {
    let path = article_disk_cache_path();
    // Load existing cache or create new
    let mut cache: HashMap<String, ArticleDiskCacheEntry> = std::fs::read_to_string(&path)
        .map_or_else(
            |_| HashMap::new(),
            |file_content| serde_json::from_str(&file_content).unwrap_or_default(),
        );
    // Update or insert entry
    cache.insert(
        url.to_string(),
        ArticleDiskCacheEntry {
            content: content.to_string(),
            saved_at: chrono::Utc::now().timestamp(),
            etag,
            last_modified,
        },
    );
    // Save back to disk
    match serde_json::to_string_pretty(&cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!(error = %e, url, "failed to write article disk cache");
            } else {
                debug!(url, "saved article to disk cache");
            }
        }
        Err(e) => warn!(error = %e, url, "failed to serialize article disk cache"),
    }
}
