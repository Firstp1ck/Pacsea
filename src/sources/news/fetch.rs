//! News fetching functionality with HTTP client and error handling.

use crate::sources::news::cache::{ARTICLE_CACHE, ARTICLE_CACHE_TTL_SECONDS, ArticleCacheEntry};
use crate::sources::news::parse::parse_arch_news_html;
use crate::sources::news::utils::is_archlinux_url;
use crate::sources::news::{
    aur::extract_aur_pkg_from_url,
    cache::{load_article_entry_from_disk_cache, save_article_to_disk_cache},
};
use crate::state::NewsItem;
use reqwest;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Result type alias for Arch Linux news fetching operations.
type Result<T> = super::Result<T>;

/// Shared HTTP client with connection pooling for news content fetching.
/// Connection pooling is enabled by default in `reqwest::Client`.
/// Uses browser-like headers to work with archlinux.org's `DDoS` protection.
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue};
    let mut headers = HeaderMap::new();
    // Browser-like Accept header
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    );
    // Accept-Language header for completeness
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        // Firefox-like User-Agent with Pacsea identifier for transparency
        .user_agent(format!(
            "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0 Pacsea/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .default_headers(headers)
        .build()
        .expect("Failed to create HTTP client")
});

/// What: Fetch recent Arch Linux news items with optional early date filtering.
///
/// Input:
/// - `limit`: Maximum number of items to return (best-effort)
/// - `cutoff_date`: Optional date string (YYYY-MM-DD) for early filtering
///
/// Output: `Ok(Vec<NewsItem>)` with date/title/url; `Err` on network or parse failures
///
/// # Errors
/// - Returns `Err` when network request fails (curl execution error)
/// - Returns `Err` when RSS feed cannot be fetched from Arch Linux website
/// - Returns `Err` when response body cannot be decoded as UTF-8
///
/// Details: Downloads the Arch Linux news RSS feed and iteratively parses `<item>` blocks,
/// extracting `<title>`, `<link>`, and `<pubDate>`. The `pubDate` value is normalized to a
/// date-only form via `strip_time_and_tz`. If `cutoff_date` is provided, stops fetching when
/// items exceed the date limit.
pub async fn fetch_arch_news(limit: usize, cutoff_date: Option<&str>) -> Result<Vec<NewsItem>> {
    use crate::sources::news::utils::{extract_between, strip_time_and_tz};

    let url = "https://archlinux.org/feeds/news/";
    // Use shorter timeout (10s connect, 15s max) to avoid blocking on slow/unreachable servers
    let body = tokio::task::spawn_blocking(move || {
        crate::util::curl::curl_text_with_args(
            url,
            &["--connect-timeout", "10", "--max-time", "15"],
        )
    })
    .await?
    .map_err(|e| {
        warn!(error = %e, "failed to fetch arch news feed");
        e
    })?;
    info!(bytes = body.len(), "fetched arch news feed");
    let mut items: Vec<NewsItem> = Vec::new();
    let mut pos = 0;
    while items.len() < limit {
        if let Some(start) = body[pos..].find("<item>") {
            let s = pos + start;
            let end = body[s..].find("</item>").map_or(body.len(), |e| s + e + 7);
            let chunk = &body[s..end];
            let title = extract_between(chunk, "<title>", "</title>").unwrap_or_default();
            let link = extract_between(chunk, "<link>", "</link>").unwrap_or_default();
            let raw_date = extract_between(chunk, "<pubDate>", "</pubDate>")
                .map(|d| d.trim().to_string())
                .unwrap_or_default();
            let date = strip_time_and_tz(&raw_date);
            // Early date filtering: stop if item is older than cutoff_date
            if let Some(cutoff) = cutoff_date
                && date.as_str() < cutoff
            {
                break;
            }
            items.push(NewsItem {
                date,
                title,
                url: link,
            });
            pos = end;
        } else {
            break;
        }
    }
    info!(count = items.len(), "parsed arch news feed");
    Ok(items)
}

/// What: Fetch the full article content from an Arch news URL.
///
/// Inputs:
/// - `url`: The news article URL (e.g., `https://archlinux.org/news/...`)
///
/// Output:
/// - `Ok(String)` with the article text content; `Err` on network/parse failure.
///
/// # Errors
/// - Network fetch failures
/// - HTML parsing failures
///
/// Details:
/// - For AUR package URLs, fetches and renders AUR comments instead.
/// - For Arch news URLs, checks cache first (15-minute in-memory, 14-day disk TTL).
/// - Applies rate limiting for archlinux.org URLs to prevent aggressive fetching.
/// - Fetches the HTML page and extracts content from the article body.
/// - Strips HTML tags and normalizes whitespace.
/// - Caches successful fetches in both in-memory and disk caches.
pub async fn fetch_news_content(url: &str) -> Result<String> {
    use crate::sources::news::aur::render_aur_comments;

    if let Some(pkg) = extract_aur_pkg_from_url(url) {
        let comments = crate::sources::fetch_aur_comments(pkg.clone()).await?;
        let rendered = render_aur_comments(&pkg, &comments);
        return Ok(rendered);
    }

    // 1. Check in-memory cache first (fastest, 15-minute TTL)
    let cached_entry: Option<ArticleCacheEntry> = if let Ok(cache) = ARTICLE_CACHE.lock()
        && let Some(entry) = cache.get(url)
        && entry.timestamp.elapsed().as_secs() < ARTICLE_CACHE_TTL_SECONDS
    {
        info!(url, "using in-memory cached article content");
        return Ok(entry.content.clone());
    } else {
        None
    };

    // 2. Check disk cache (14-day TTL) - useful after app restart
    let disk_entry = load_article_entry_from_disk_cache(url);
    if let Some(ref entry) = disk_entry {
        // Populate in-memory cache from disk
        if let Ok(mut cache) = ARTICLE_CACHE.lock() {
            cache.insert(
                url.to_string(),
                ArticleCacheEntry {
                    content: entry.content.clone(),
                    timestamp: Instant::now(),
                    etag: entry.etag.clone(),
                    last_modified: entry.last_modified.clone(),
                },
            );
        }
        return Ok(entry.content.clone());
    }

    // 3. Check circuit breaker before making request (no network call)
    let endpoint_pattern = crate::sources::feeds::extract_endpoint_pattern(url);
    if let Err(e) = crate::sources::feeds::check_circuit_breaker(&endpoint_pattern) {
        warn!(url, endpoint_pattern, error = %e, "circuit breaker blocking request");
        // Try to return cached content if available
        if let Some(cached) = cached_entry {
            return Ok(cached.content);
        }
        if let Some(disk) = disk_entry {
            return Ok(disk.content);
        }
        return Err(e);
    }

    // 4. Apply rate limiting and acquire semaphore for archlinux.org URLs
    // The permit is held during the entire request to serialize access
    let _permit = if is_archlinux_url(url) {
        Some(crate::sources::feeds::rate_limit_archlinux().await)
    } else {
        None
    };

    // 5. Fetch from network with conditional requests
    let url_owned = url.to_string();
    let url_for_log = url_owned.clone();
    // Get cached ETag/Last-Modified for conditional request
    let cached_etag = cached_entry
        .as_ref()
        .and_then(|e: &ArticleCacheEntry| e.etag.as_ref())
        .or_else(|| disk_entry.as_ref().and_then(|e| e.etag.as_ref()))
        .cloned();
    let cached_last_modified = cached_entry
        .as_ref()
        .and_then(|e: &ArticleCacheEntry| e.last_modified.as_ref())
        .or_else(|| disk_entry.as_ref().and_then(|e| e.last_modified.as_ref()))
        .cloned();

    // Fetch from network with conditional requests using reqwest (connection pooling)
    let client = HTTP_CLIENT.clone();
    let mut request = client.get(&url_owned);

    // Add conditional request headers if we have cached ETag/Last-Modified
    if let Some(ref etag) = cached_etag {
        request = request.header("If-None-Match", etag);
    }
    if let Some(ref last_mod) = cached_last_modified {
        request = request.header("If-Modified-Since", last_mod);
    }

    let http_response = request.send().await.map_err(|e| {
        warn!(error = %e, url = %url_for_log, "failed to fetch news content");
        // Record failure in circuit breaker
        crate::sources::feeds::record_circuit_breaker_outcome(&endpoint_pattern, false);
        Box::<dyn std::error::Error + Send + Sync>::from(format!("Network error: {e}"))
    })?;

    let status = http_response.status();
    let status_code = status.as_u16();

    // Handle 304 Not Modified - return cached content
    if status_code == 304 {
        info!(
            url,
            "server returned 304 Not Modified, using cached content"
        );
        if let Some(cached) = cached_entry {
            return Ok(cached.content);
        }
        if let Some(disk) = disk_entry {
            return Ok(disk.content);
        }
        // Fallback: should not happen, but handle gracefully
        warn!(url, "304 response but no cached content available");
        return Err("304 Not Modified but no cache available".into());
    }

    // Extract ETag and Last-Modified from response headers before consuming body
    let etag = http_response
        .headers()
        .get("etag")
        .and_then(|h| h.to_str().ok())
        .map(ToString::to_string);
    let last_modified = http_response
        .headers()
        .get("last-modified")
        .and_then(|h| h.to_str().ok())
        .map(ToString::to_string);

    // Check for HTTP errors
    if status.is_client_error() || status.is_server_error() {
        // Record failure in circuit breaker
        crate::sources::feeds::record_circuit_breaker_outcome(&endpoint_pattern, false);
        return Err(handle_http_error(status, status_code, &http_response).into());
    }

    let body = http_response.text().await.map_err(|e| {
        warn!(error = %e, url = %url_for_log, "failed to read response body");
        Box::<dyn std::error::Error + Send + Sync>::from(format!("Failed to read response: {e}"))
    })?;

    info!(url, bytes = body.len(), "fetched news page");

    // Record success in circuit breaker
    crate::sources::feeds::record_circuit_breaker_outcome(&endpoint_pattern, true);

    // Extract article content from HTML
    let content = parse_arch_news_html(&body, Some(url));
    let parsed_len = content.len();
    if parsed_len == 0 {
        warn!(url, "parsed news content is empty");
    } else {
        info!(url, parsed_len, "parsed news content");
    }

    // 5. Cache the result with ETag/Last-Modified
    // Save to in-memory cache
    if let Ok(mut cache) = ARTICLE_CACHE.lock() {
        cache.insert(
            url.to_string(),
            ArticleCacheEntry {
                content: content.clone(),
                timestamp: Instant::now(),
                etag: etag.clone(),
                last_modified: last_modified.clone(),
            },
        );
    }
    // Save to disk cache for persistence across restarts
    save_article_to_disk_cache(url, &content, etag, last_modified);

    Ok(content)
}

/// What: Handle HTTP error responses and format error messages.
///
/// Inputs:
/// - `status`: HTTP status code object.
/// - `status_code`: HTTP status code as u16.
/// - `http_response`: HTTP response object to extract headers.
///
/// Output:
/// - Formatted error message string.
///
/// Details:
/// - Handles 429 (Too Many Requests) and 503 (Service Unavailable) with Retry-After headers.
/// - Formats generic error messages for other HTTP errors.
fn handle_http_error(
    status: reqwest::StatusCode,
    status_code: u16,
    http_response: &reqwest::Response,
) -> String {
    if status_code == 429 {
        let mut msg = "HTTP 429 Too Many Requests - rate limited by server".to_string();
        if let Some(retry_after) = http_response.headers().get("retry-after")
            && let Ok(retry_str) = retry_after.to_str()
        {
            msg.push_str(" (Retry-After: ");
            msg.push_str(retry_str);
            msg.push(')');
        }
        msg
    } else if status_code == 503 {
        let mut msg = "HTTP 503 Service Unavailable".to_string();
        if let Some(retry_after) = http_response.headers().get("retry-after")
            && let Ok(retry_str) = retry_after.to_str()
        {
            msg.push_str(" (Retry-After: ");
            msg.push_str(retry_str);
            msg.push(')');
        }
        msg
    } else {
        format!("HTTP error: {status}")
    }
}
