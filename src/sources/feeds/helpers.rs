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
use super::updates::AurVersionInfo;

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
    let summary = old_version
        .map(|prev| format!("{prev} → {remote_version}"))
        .or_else(|| last_seen.map(|prev| format!("{prev} → {remote_version}")))
        .or_else(|| Some(remote_version.to_string()));
    NewsFeedItem {
        id: format!("pkg-update:official:{}:{remote_version}", pkg.name),
        date,
        title: format!("{} updated to {remote_version}", pkg.name),
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

/// What: Fetch and normalize the last update/build date for an official package.
///
/// Inputs:
/// - `pkg`: Package item expected to originate from an official repository.
///
/// Output:
/// - `Some(YYYY-MM-DD)` when metadata is available; `None` when unavailable or on fetch error.
///
/// Details:
/// - Queries the Arch package JSON endpoint, preferring `last_update` then `build_date`.
/// - Falls back silently when network or parse errors occur.
/// - Uses rate limiting to prevent IP blocking by archlinux.org.
/// - Applies circuit breaker pattern to avoid overwhelming the server during outages.
pub(super) async fn fetch_official_package_date(pkg: &crate::state::PackageItem) -> Option<String> {
    let crate::state::Source::Official { repo, arch } = &pkg.source else {
        return None;
    };
    let repo_slug = repo.to_lowercase();
    let arch_slug = if arch.is_empty() {
        std::env::consts::ARCH
    } else {
        arch.as_str()
    };
    let url = format!(
        "https://archlinux.org/packages/{repo_slug}/{arch_slug}/{name}/json/",
        name = pkg.name
    );
    let endpoint_pattern = "/packages/*/json/";

    // Check circuit breaker before making request
    if let Err(e) = check_circuit_breaker(endpoint_pattern) {
        tracing::debug!(
            package = %pkg.name,
            error = %e,
            "circuit breaker blocking package date fetch"
        );
        return None;
    }

    // Apply rate limiting before request to prevent IP blocking
    let _permit = rate_limit_archlinux().await;

    // Timeout increased from 500ms to 2000ms to accommodate rate limiting delays
    // Still prioritizes responsiveness - missed dates will use "unknown"
    let result = tokio::time::timeout(
        tokio::time::Duration::from_millis(2000),
        tokio::task::spawn_blocking({
            let url = url.clone();
            move || crate::util::curl::curl_json(&url)
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(json))) => {
            // Success: reset backoff and record success
            reset_archlinux_backoff();
            record_circuit_breaker_outcome(endpoint_pattern, true);
            let obj = json.get("pkg").unwrap_or(&json);
            extract_date_from_pkg_json(obj)
        }
        Ok(Ok(Err(e))) => {
            let error_str = e.to_string();
            // Check for rate limiting errors
            if error_str.contains("429") || error_str.contains("503") {
                let retry_after = extract_retry_after_from_error(&error_str);
                increase_archlinux_backoff(retry_after);
                tracing::warn!(
                    package = %pkg.name,
                    error = %e,
                    "rate limited fetching official package date"
                );
            } else {
                increase_archlinux_backoff(None);
                tracing::warn!(
                    package = %pkg.name,
                    error = %e,
                    "failed to fetch official package date"
                );
            }
            record_circuit_breaker_outcome(endpoint_pattern, false);
            None
        }
        Ok(Err(e)) => {
            increase_archlinux_backoff(None);
            record_circuit_breaker_outcome(endpoint_pattern, false);
            tracing::warn!(
                error = ?e,
                package = %pkg.name,
                "failed to join package date task"
            );
            None
        }
        Err(_) => {
            // Timeout - mild backoff increase
            increase_archlinux_backoff(None);
            record_circuit_breaker_outcome(endpoint_pattern, false);
            tracing::debug!(package = %pkg.name, "timeout fetching official package date");
            None
        }
    }
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
/// - `raw`: Date/time string (RFC3339 or `YYYY-MM-DD HH:MM UTC` formats).
///
/// Output:
/// - `Some(YYYY-MM-DD)` when parsing succeeds; `None` on invalid inputs.
///
/// Details:
/// - Handles Arch JSON date formats (`2025-12-07T11:09:38Z`) and page metadata (`2025-12-07 11:09 UTC`).
pub(super) fn normalize_pkg_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.date_naive().to_string());
    }
    if let Ok(dt) = DateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M %Z") {
        return Some(dt.date_naive().to_string());
    }
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
