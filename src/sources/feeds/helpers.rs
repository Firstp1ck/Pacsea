//! Helper functions for building feed items, date parsing, and utilities.
use std::collections::HashMap;
use std::fs;
use std::hash::BuildHasher;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::state::types::{AurComment, NewsFeedItem, NewsFeedSource};
use crate::util::parse_update_entry;

use super::rate_limit::{
    check_circuit_breaker, extract_retry_after_from_error, increase_archlinux_backoff,
    rate_limit_archlinux, record_circuit_breaker_outcome, reset_archlinux_backoff,
};
use super::updates::{AurVersionInfo, FetchDateResult};

/// What: Build a feed item for an official package update.
///
/// Inputs:
/// - `pkg`: Official package metadata (includes repo/arch for links).
/// - `last_seen`: Previously seen version (if any) for summary formatting.
/// - `old_version`: Old version from updates list (if available).
/// - `remote_version`: Current version detected in the official index.
/// - `pkg_date`: Optional package date string.
///
/// Output:
/// - `NewsFeedItem` representing the update.
///
/// Details:
/// - Prefers package metadata date (last update/build); falls back to today when unavailable.
/// - Includes repo/arch link when available.
pub(super) fn build_official_update_item(
    pkg: &crate::state::PackageItem,
    last_seen: Option<&String>,
    old_version: Option<&str>,
    remote_version: &str,
    pkg_date: Option<String>,
) -> NewsFeedItem {
    let date = pkg_date.unwrap_or_else(|| Utc::now().date_naive().to_string());
    let url = if let crate::state::Source::Official { repo, arch } = &pkg.source {
        let repo_lc = repo.to_lowercase();
        let arch_slug = if arch.is_empty() {
            std::env::consts::ARCH
        } else {
            arch.as_str()
        };
        Some(format!(
            "https://archlinux.org/packages/{repo}/{arch}/{name}/",
            repo = repo_lc,
            arch = arch_slug,
            name = pkg.name
        ))
    } else {
        None
    };
    // Only show summary if there's an actual version change (old != new)
    let summary = old_version
        .and_then(|prev| {
            if prev == remote_version {
                None
            } else {
                Some(format!("{prev} → {remote_version}"))
            }
        })
        .or_else(|| {
            last_seen.and_then(|prev| {
                if prev == remote_version {
                    None
                } else {
                    Some(format!("{prev} → {remote_version}"))
                }
            })
        });
    // Simplified title: just the package name
    NewsFeedItem {
        id: format!("pkg-update:official:{}:{remote_version}", pkg.name),
        date,
        title: pkg.name.clone(),
        summary,
        url,
        source: NewsFeedSource::InstalledPackageUpdate,
        severity: None,
        packages: vec![pkg.name.clone()],
    }
}

/// What: Build a feed item for an AUR package update.
///
/// Inputs:
/// - `pkg`: AUR version info (name, version, optional last-modified timestamp).
/// - `last_seen`: Previously seen version, if any.
/// - `old_version`: Old version from updates list (if available).
/// - `remote_version`: Current version.
///
/// Output:
/// - `NewsFeedItem` representing the update.
///
/// Details:
/// - Uses last-modified timestamp for the date when available, otherwise today.
pub(super) fn build_aur_update_item(
    pkg: &AurVersionInfo,
    last_seen: Option<&String>,
    old_version: Option<&str>,
    remote_version: &str,
) -> NewsFeedItem {
    let date = pkg
        .last_modified
        .and_then(ts_to_date_string)
        .unwrap_or_else(|| Utc::now().date_naive().to_string());
    let summary = old_version
        .map(|prev| format!("{prev} → {remote_version}"))
        .or_else(|| last_seen.map(|prev| format!("{prev} → {remote_version}")))
        .or_else(|| Some(remote_version.to_string()));
    NewsFeedItem {
        id: format!("pkg-update:aur:{}:{remote_version}", pkg.name),
        date,
        title: format!("{} updated to {remote_version}", pkg.name),
        summary,
        url: Some(format!("https://aur.archlinux.org/packages/{}", pkg.name)),
        source: NewsFeedSource::AurPackageUpdate,
        severity: None,
        packages: vec![pkg.name.clone()],
    }
}

/// What: Convert a Unix timestamp to a `YYYY-MM-DD` string.
///
/// Inputs:
/// - `ts`: Unix timestamp in seconds.
///
/// Output:
/// - `Some(date)` when conversion succeeds; `None` on invalid timestamp.
///
/// Details:
/// - Uses UTC date component only.
pub(super) fn ts_to_date_string(ts: i64) -> Option<String> {
    DateTime::<Utc>::from_timestamp(ts, 0).map(|dt| dt.date_naive().to_string())
}

/// What: Build list of architecture candidates to try for package JSON.
///
/// Inputs:
/// - `arch`: The architecture from the package source (may be empty).
///
/// Output:
/// - Vector of architectures to try, in order of preference.
///
/// Details:
/// - If arch is empty or `x86_64`, tries both `x86_64` and "any" (for arch-independent packages).
/// - If arch is "any", only tries "any".
/// - Otherwise tries the specific arch, then "any" as fallback.
fn build_arch_candidates(arch: &str) -> Vec<String> {
    if arch.is_empty() || arch.eq_ignore_ascii_case("x86_64") {
        vec!["x86_64".to_string(), "any".to_string()]
    } else if arch.eq_ignore_ascii_case("any") {
        vec!["any".to_string()]
    } else {
        vec![arch.to_string(), "any".to_string()]
    }
}

/// What: Build list of repository candidates to try for package JSON.
///
/// Inputs:
/// - `repo`: The repository from the package source (may be empty).
///
/// Output:
/// - Vector of repositories to try, in order of preference.
///
/// Details:
/// - If repo is empty, tries core and extra.
/// - Otherwise tries the specified repo first, then others as fallback.
fn build_repo_candidates(repo: &str) -> Vec<String> {
    if repo.is_empty() {
        vec!["core".to_string(), "extra".to_string()]
    } else {
        let repo_lower = repo.to_lowercase();
        if repo_lower == "core" {
            vec!["core".to_string(), "extra".to_string()]
        } else if repo_lower == "extra" {
            vec!["extra".to_string(), "core".to_string()]
        } else {
            // For other repos (multilib, etc.), try specified first, then core/extra
            vec![repo_lower, "extra".to_string(), "core".to_string()]
        }
    }
}

/// What: Try fetching package JSON from multiple repo/arch combinations.
///
/// Inputs:
/// - `name`: Package name.
/// - `repo_candidates`: List of repositories to try.
/// - `arch_candidates`: List of architectures to try.
///
/// Output:
/// - Result containing either the JSON value or an error string.
///
/// Details:
/// - Tries each combination until one succeeds.
/// - On 404, tries the next combination.
/// - On other errors (rate limiting, etc.), returns the error immediately.
async fn try_fetch_package_json(
    name: &str,
    repo_candidates: &[String],
    arch_candidates: &[String],
) -> Result<serde_json::Value, String> {
    let mut last_error = String::new();

    for repo in repo_candidates {
        for arch in arch_candidates {
            let url = format!("https://archlinux.org/packages/{repo}/{arch}/{name}/json/",);

            let fetch_result = tokio::time::timeout(
                tokio::time::Duration::from_millis(2000),
                tokio::task::spawn_blocking({
                    let url = url.clone();
                    move || crate::util::curl::curl_json(&url)
                }),
            )
            .await;

            match fetch_result {
                Ok(Ok(Ok(json))) => {
                    tracing::debug!(
                        package = %name,
                        repo = %repo,
                        arch = %arch,
                        "successfully fetched package JSON"
                    );
                    return Ok(json);
                }
                Ok(Ok(Err(e))) => {
                    let error_str = e.to_string();
                    // 404 means wrong repo/arch combo - try next one
                    if error_str.contains("404") {
                        tracing::debug!(
                            package = %name,
                            repo = %repo,
                            arch = %arch,
                            "package not found at this URL, trying next candidate"
                        );
                        last_error = error_str;
                        continue;
                    }
                    // Other errors (rate limiting, server errors) - return immediately
                    return Err(error_str);
                }
                Ok(Err(e)) => {
                    // spawn_blocking failed
                    return Err(format!("task join error: {e}"));
                }
                Err(_) => {
                    // Timeout
                    return Err("timeout".to_string());
                }
            }
        }
    }

    // All candidates returned 404
    Err(last_error)
}

/// What: Fetch and normalize the last update/build date for an official package.
///
/// Inputs:
/// - `pkg`: Package item expected to originate from an official repository.
///
/// Output:
/// - `FetchDateResult` indicating success, cached fallback, or needs retry.
///
/// Details:
/// - Queries the Arch package JSON endpoint, preferring `last_update` then `build_date`.
/// - Tries multiple repo/arch combinations if the first one returns 404.
/// - Falls back to cached JSON when network errors occur.
/// - Returns `NeedsRetry` when fetch fails and no cache is available.
/// - Uses rate limiting to prevent IP blocking by archlinux.org.
/// - Applies circuit breaker pattern to avoid overwhelming the server during outages.
pub(super) async fn fetch_official_package_date(
    pkg: &crate::state::PackageItem,
) -> FetchDateResult {
    let crate::state::Source::Official { repo, arch } = &pkg.source else {
        return FetchDateResult::Success(None);
    };
    let endpoint_pattern = "/packages/*/json/";

    // Build list of repo/arch candidates to try
    // Some packages use "any" arch instead of x86_64, and may be in different repos
    let repo_slug = repo.to_lowercase();
    let arch_candidates = build_arch_candidates(arch);
    let repo_candidates = build_repo_candidates(&repo_slug);

    // Use first candidate for cache path (most likely to be correct)
    let first_arch = arch_candidates.first().map_or("x86_64", String::as_str);
    let first_repo = repo_candidates
        .first()
        .map_or("extra", |s| s.as_str())
        .to_lowercase();
    let cache_path = crate::sources::feeds::updates::official_json_cache_path(
        &first_repo,
        first_arch,
        &pkg.name,
    );

    // Check circuit breaker before making request
    if let Err(e) = check_circuit_breaker(endpoint_pattern) {
        tracing::debug!(
            package = %pkg.name,
            error = %e,
            "circuit breaker blocking package date fetch, trying cached JSON"
        );
        // Fall back to cached JSON if available, otherwise needs retry
        return extract_date_from_cached_json(&cache_path)
            .map_or(FetchDateResult::NeedsRetry, |date| {
                FetchDateResult::CachedFallback(Some(date))
            });
    }

    // Apply rate limiting before request to prevent IP blocking
    let _permit = rate_limit_archlinux().await;

    // Try each repo/arch combination until one succeeds
    let result = try_fetch_package_json(&pkg.name, &repo_candidates, &arch_candidates).await;

    match result {
        Ok(json) => {
            // Success: reset backoff and record success
            reset_archlinux_backoff();
            record_circuit_breaker_outcome(endpoint_pattern, true);

            // Save and compare JSON for change detection
            let old_json = crate::sources::feeds::updates::load_official_json_cache(&cache_path);

            // Compare with previous JSON if it exists
            if let Some(old_json) = old_json
                && let Some(change_desc) =
                    crate::sources::feeds::updates::compare_official_json_changes(
                        &old_json, &json, &pkg.name,
                    )
            {
                // Store changes in cache
                if let Ok(mut cache) =
                    crate::sources::feeds::updates::OFFICIAL_JSON_CHANGES_CACHE.lock()
                {
                    cache.insert(pkg.name.clone(), change_desc);
                }
            }

            // Save the JSON to disk (after comparison)
            if let Err(e) =
                crate::sources::feeds::updates::save_official_json_cache(&cache_path, &json)
            {
                tracing::debug!(
                    error = %e,
                    path = ?cache_path,
                    "failed to save official package JSON cache"
                );
            }

            // last_update is at the top level, not inside pkg
            // Check top-level first, then fall back to pkg object
            let date = extract_date_from_pkg_json(&json)
                .or_else(|| json.get("pkg").and_then(extract_date_from_pkg_json));
            FetchDateResult::Success(date)
        }
        Err(error_str) => {
            // Check for HTTP 404 - all repo/arch combinations returned not found
            // This means the package truly doesn't exist in the JSON API
            if error_str.contains("404") {
                tracing::debug!(
                    package = %pkg.name,
                    "package not found in any repository JSON API (may be a virtual package)"
                );
                // 404s are NOT failures for circuit breaker - they're expected for some packages
                // Record as success to not trip circuit breaker
                record_circuit_breaker_outcome(endpoint_pattern, true);
                // Return Success(None) to indicate "no date available" permanently
                return FetchDateResult::Success(None);
            }
            // Check for rate limiting errors (429, 502, 503, 504) or timeout
            if error_str.contains("429")
                || error_str.contains("502")
                || error_str.contains("503")
                || error_str.contains("504")
            {
                let retry_after = extract_retry_after_from_error(&error_str);
                increase_archlinux_backoff(retry_after);
                tracing::warn!(
                    package = %pkg.name,
                    error = %error_str,
                    "rate limited fetching official package date"
                );
            } else if error_str.contains("timeout") {
                // Timeout - mild backoff increase
                increase_archlinux_backoff(None);
                tracing::debug!(
                    package = %pkg.name,
                    "timeout fetching official package date, trying cached JSON"
                );
            } else {
                increase_archlinux_backoff(None);
                tracing::warn!(
                    package = %pkg.name,
                    error = %error_str,
                    "failed to fetch official package date"
                );
            }
            record_circuit_breaker_outcome(endpoint_pattern, false);
            // Fall back to cached JSON if available, otherwise needs retry
            cached_fallback_or_retry(&cache_path)
        }
    }
}

/// What: Return cached fallback or needs retry result.
///
/// Inputs:
/// - `cache_path`: Path to the cached JSON file.
///
/// Output:
/// - `FetchDateResult::CachedFallback` if cache exists; `NeedsRetry` otherwise.
///
/// Details:
/// - Helper to reduce code duplication in error handling paths.
fn cached_fallback_or_retry(cache_path: &std::path::Path) -> FetchDateResult {
    extract_date_from_cached_json(cache_path).map_or(FetchDateResult::NeedsRetry, |date| {
        FetchDateResult::CachedFallback(Some(date))
    })
}

/// What: Extract date from cached official package JSON.
///
/// Inputs:
/// - `cache_path`: Path to the cached JSON file.
///
/// Output:
/// - `Some(YYYY-MM-DD)` if cached JSON exists and date can be extracted; `None` otherwise.
///
/// Details:
/// - Used as fallback when network requests fail.
fn extract_date_from_cached_json(cache_path: &std::path::Path) -> Option<String> {
    let cached_json = crate::sources::feeds::updates::load_official_json_cache(cache_path)?;
    extract_date_from_pkg_json(&cached_json)
        .or_else(|| cached_json.get("pkg").and_then(extract_date_from_pkg_json))
}

/// What: Extract a normalized date from Arch package JSON metadata.
///
/// Inputs:
/// - `obj`: JSON value from the package endpoint (`pkg` object or root).
///
/// Output:
/// - `Some(YYYY-MM-DD)` when `last_update` or `build_date` can be parsed; `None` otherwise.
///
/// Details:
/// - Prefers `last_update`; falls back to `build_date`.
pub(super) fn extract_date_from_pkg_json(obj: &Value) -> Option<String> {
    obj.get("last_update")
        .and_then(Value::as_str)
        .and_then(normalize_pkg_date)
        .or_else(|| {
            obj.get("build_date")
                .and_then(Value::as_str)
                .and_then(normalize_pkg_date)
        })
}

/// What: Normalize a package timestamp string to `YYYY-MM-DD`.
///
/// Inputs:
/// - `raw`: Date/time string (RFC3339 with optional fractional seconds, or `YYYY-MM-DD HH:MM UTC` formats).
///
/// Output:
/// - `Some(YYYY-MM-DD)` when parsing succeeds; `None` on invalid inputs.
///
/// Details:
/// - Handles Arch JSON date formats including milliseconds (`2025-12-15T19:30:14.422Z`).
/// - Falls back to simple date prefix extraction for other formats.
pub(super) fn normalize_pkg_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    // Try RFC3339 parsing (handles formats with and without fractional seconds)
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.date_naive().to_string());
    }
    // Try parsing with explicit format for fractional seconds
    if let Ok(dt) = DateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S%.fZ") {
        return Some(dt.date_naive().to_string());
    }
    // Try standard UTC format
    if let Ok(dt) = DateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M %Z") {
        return Some(dt.date_naive().to_string());
    }
    // Fallback: extract date prefix if it looks like YYYY-MM-DD
    let prefix = trimmed.chars().take(10).collect::<String>();
    if prefix.len() == 10
        && prefix.as_bytes()[4] == b'-'
        && prefix.as_bytes()[7] == b'-'
        && prefix[..4].chars().all(|c| c.is_ascii_digit())
        && prefix[5..7].chars().all(|c| c.is_ascii_digit())
        && prefix[8..10].chars().all(|c| c.is_ascii_digit())
    {
        return Some(prefix);
    }
    None
}

/// What: Normalize an AUR comment date string to `YYYY-MM-DD`.
///
/// Inputs:
/// - `date`: Comment date string as displayed (e.g., "2025-01-01 10:00 (UTC)").
///
/// Output:
/// - Normalized date component; falls back to today's date if parsing fails.
///
/// Details:
/// - Splits on whitespace and takes the first token.
pub(super) fn normalize_comment_date(date: &str) -> String {
    date.split_whitespace().next().map_or_else(
        || Utc::now().date_naive().to_string(),
        std::string::ToString::to_string,
    )
}

/// What: Summarize comment content for feed display.
///
/// Inputs:
/// - `content`: Full comment text.
///
/// Output:
/// - Trimmed string capped to 180 characters with ellipsis when truncated.
///
/// Details:
/// - Counts characters (not bytes) to avoid breaking UTF-8 boundaries.
pub(super) fn summarize_comment(content: &str) -> String {
    const MAX: usize = 180;
    if content.chars().count() <= MAX {
        return content.to_string();
    }
    let mut out = content.chars().take(MAX).collect::<String>();
    out.push('…');
    out
}

/// What: Load package names and target versions from `available_updates.txt` if present.
///
/// Inputs:
/// - `path`: Optional path to the updates list file.
///
/// Output:
/// - `Some(HashMap<String, (String, String)>)` mapping package name -> (old, new); `None` on error or empty.
pub(super) fn load_update_versions(
    path: Option<&PathBuf>,
) -> Option<HashMap<String, (String, String)>> {
    let path = path?;
    let data = fs::read_to_string(path).ok()?;
    let mut map: HashMap<String, (String, String)> = HashMap::new();
    for line in data.lines() {
        if let Some((name, old_v, new_v)) = parse_update_entry(line) {
            map.insert(name, (old_v, new_v));
        }
    }
    if map.is_empty() { None } else { Some(map) }
}

/// What: Update last-seen comment map and return new feed items until the last seen marker.
///
/// Inputs:
/// - `pkgname`: Package name associated with the comments.
/// - `comments`: Comments sorted newest-first.
/// - `seen_aur_comments`: Mutable last-seen comment map.
/// - `remaining_allowance`: Maximum number of items to emit.
/// - `force_emit_all`: Whether to emit all comments regardless of seen state.
///
/// Output:
/// - `Vec<NewsFeedItem>` containing new comment items.
///
/// Details:
/// - Emits from newest to oldest until the previous marker (if any) or allowance is exhausted.
pub(super) fn update_seen_for_comments<H>(
    pkgname: &str,
    comments: &[AurComment],
    seen_aur_comments: &mut HashMap<String, String, H>,
    remaining_allowance: usize,
    force_emit_all: bool,
) -> Vec<NewsFeedItem>
where
    H: BuildHasher + Send + Sync + 'static,
{
    let mut emitted = Vec::new();
    let latest_id = comments
        .first()
        .and_then(|c| c.id.clone().or_else(|| c.date_url.clone()));
    let prev_seen = seen_aur_comments.get(pkgname).cloned();
    if let Some(ref latest) = latest_id {
        seen_aur_comments.insert(pkgname.to_string(), latest.clone());
    }
    for comment in comments {
        if emitted.len() >= remaining_allowance {
            break;
        }
        let cid = comment
            .id
            .as_ref()
            .or(comment.date_url.as_ref())
            .unwrap_or(&comment.date);
        if !force_emit_all && prev_seen.as_deref() == Some(cid) {
            break;
        }
        emitted.push(NewsFeedItem {
            id: format!("aur-comment:{pkgname}:{cid}"),
            date: normalize_comment_date(&comment.date),
            title: format!("New AUR comment on {pkgname}"),
            summary: Some(summarize_comment(&comment.content)),
            url: comment.date_url.clone(),
            source: NewsFeedSource::AurComment,
            severity: None,
            packages: vec![pkgname.to_string()],
        });
    }
    emitted
}
