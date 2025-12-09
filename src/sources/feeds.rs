//! Aggregated news feed fetcher (Arch news + security advisories).
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::BuildHasher;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::state::types::AurComment;
use crate::state::types::{NewsFeedItem, NewsFeedSource, NewsSortMode};
use crate::util::parse_update_entry;
use tracing::{debug, info, warn};

/// Result type alias for news feed fetching operations.
type Result<T> = super::Result<T>;

/// What: Input context for fetching a combined news feed.
///
/// Inputs:
/// - `limit`: Maximum number of items per source.
/// - `include_*`: Source toggles.
/// - `installed_filter`: Optional installed-package set for scoping.
/// - `installed_only`: Whether to restrict advisories to installed packages.
/// - `sort_mode`: Sort order.
/// - `seen_pkg_versions`: Last-seen map for package updates.
/// - `seen_aur_comments`: Last-seen map for AUR comments.
///
/// Output:
/// - Mutable references updated in place alongside returned feed items.
///
/// Details:
/// - Hashers are generic to remain compatible with caller-supplied maps.
#[allow(clippy::struct_excessive_bools)]
pub struct NewsFeedContext<'a, HS, HV, HC>
where
    HS: BuildHasher + Send + Sync + 'static,
    HV: BuildHasher + Send + Sync + 'static,
    HC: BuildHasher + Send + Sync + 'static,
{
    /// Emit all sources even on first run (bypasses baseline gating).
    pub force_emit_all: bool,
    /// Optional path to `available_updates.txt` for filtering noisy first-run emissions.
    pub updates_list_path: Option<PathBuf>,
    /// Maximum number of items per source.
    pub limit: usize,
    /// Whether to include Arch news RSS posts.
    pub include_arch_news: bool,
    /// Whether to include security advisories.
    pub include_advisories: bool,
    /// Whether to include installed package updates.
    pub include_pkg_updates: bool,
    /// Whether to include installed AUR comments.
    pub include_aur_comments: bool,
    /// Optional installed-package filter set.
    pub installed_filter: Option<&'a HashSet<String, HS>>,
    /// Whether to restrict advisories to installed packages.
    pub installed_only: bool,
    /// Sort mode for the resulting feed.
    pub sort_mode: NewsSortMode,
    /// Last-seen versions map (updated in place).
    pub seen_pkg_versions: &'a mut HashMap<String, String, HV>,
    /// Last-seen AUR comments map (updated in place).
    pub seen_aur_comments: &'a mut HashMap<String, String, HC>,
}

/// What: Append Arch news items when enabled.
///
/// Inputs:
/// - `include_arch_news`: Toggle to include Arch news.
/// - `limit`: Maximum items to fetch.
/// - `items`: Accumulator to extend.
///
/// Output: Result indicating success/failure of fetch; always extends items when successful.
async fn append_arch_news(
    include_arch_news: bool,
    limit: usize,
    items: &mut Vec<NewsFeedItem>,
) -> Result<()> {
    if !include_arch_news {
        return Ok(());
    }
    match super::fetch_arch_news(limit).await {
        Ok(news) => items.extend(news.into_iter().map(|n| NewsFeedItem {
            id: n.url.clone(),
            date: n.date,
            title: n.title,
            summary: None,
            url: Some(n.url),
            source: NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        })),
        Err(e) => {
            warn!(error = %e, "arch news fetch failed; continuing without Arch news");
        }
    }
    Ok(())
}

/// What: Append security advisories when enabled, respecting installed-only filter.
///
/// Inputs:
/// - `include_advisories`: Toggle to include advisories.
/// - `limit`: Maximum items to fetch.
/// - `installed_filter`: Optional installed set for filtering.
/// - `installed_only`: Whether to drop advisories unrelated to installed packages.
/// - `items`: Accumulator to extend.
///
/// Output: Result indicating success/failure of fetch; always extends items when successful.
async fn append_advisories<S>(
    include_advisories: bool,
    limit: usize,
    installed_filter: Option<&HashSet<String, S>>,
    installed_only: bool,
    items: &mut Vec<NewsFeedItem>,
) -> Result<()>
where
    S: BuildHasher + Send + Sync + 'static,
{
    if !include_advisories {
        return Ok(());
    }
    match super::fetch_security_advisories(limit).await {
        Ok(advisories) => {
            for adv in advisories {
                if installed_only
                    && let Some(set) = installed_filter
                    && !adv.packages.iter().any(|p| set.contains(p))
                {
                    continue;
                }
                items.push(adv);
            }
        }
        Err(e) => {
            warn!(error = %e, "security advisories fetch failed; continuing without advisories");
        }
    }
    Ok(())
}

/// What: Fetch combined news feed (Arch news, advisories, installed updates, AUR comments) and sort.
///
/// Inputs:
/// - `limit`: Maximum items per source (best-effort).
/// - `include_arch_news`: Whether to fetch Arch news RSS.
/// - `include_advisories`: Whether to fetch security advisories.
/// - `include_pkg_updates`: Whether to include installed package update items.
/// - `include_aur_comments`: Whether to include installed AUR comment items.
/// - `installed_filter`: Optional set of installed package names for scoping advisories/updates.
/// - `installed_only`: Whether to drop advisories unrelated to installed packages.
/// - `sort_mode`: Selected sort mode for results.
/// - `seen_pkg_versions`: Last-seen versions (updated in place for dedupe).
/// - `seen_aur_comments`: Last-seen comment IDs (updated in place for dedupe).
///
/// Output:
/// - `Ok(Vec<NewsFeedItem>)` combined and sorted by selected mode.
///
/// Details:
/// - Advisories are filtered to installed packages when `installed_filter` is provided and
///   `installed_only` is true.
/// - Update/comment items are emitted only when last-seen markers indicate new data; maps are
///   refreshed regardless to establish a baseline.
///
/// # Errors
/// - Network failures fetching sources
/// - JSON parse errors from upstream feeds
pub async fn fetch_news_feed<HS, HV, HC>(
    ctx: NewsFeedContext<'_, HS, HV, HC>,
) -> Result<Vec<NewsFeedItem>>
where
    HS: BuildHasher + Send + Sync + 'static,
    HV: BuildHasher + Send + Sync + 'static,
    HC: BuildHasher + Send + Sync + 'static,
{
    let NewsFeedContext {
        limit,
        include_arch_news,
        include_advisories,
        include_pkg_updates,
        include_aur_comments,
        installed_filter,
        installed_only,
        sort_mode,
        seen_pkg_versions,
        seen_aur_comments,
        force_emit_all,
        updates_list_path,
    } = ctx;
    info!(
        limit,
        include_arch_news,
        include_advisories,
        include_pkg_updates,
        include_aur_comments,
        installed_only,
        installed_filter = installed_filter.is_some(),
        sort_mode = ?sort_mode,
        "fetch_news_feed start"
    );
    let mut items: Vec<NewsFeedItem> = Vec::new();
    append_arch_news(include_arch_news, limit, &mut items).await?;
    append_advisories(
        include_advisories,
        limit,
        installed_filter,
        installed_only,
        &mut items,
    )
    .await?;
    let updates_versions = if force_emit_all {
        load_update_versions(updates_list_path.as_ref())
    } else {
        None
    };

    if include_pkg_updates {
        if let Some(installed) = installed_filter {
            if installed.is_empty() {
                warn!("include_pkg_updates set but installed set is empty; skipping updates");
            } else {
                match fetch_installed_updates(
                    installed,
                    limit,
                    seen_pkg_versions,
                    force_emit_all,
                    updates_versions.as_ref(),
                )
                .await
                {
                    Ok(updates) => {
                        items.extend(updates);
                    }
                    Err(e) => warn!(error = %e, "installed package updates fetch failed"),
                }
            }
        } else {
            warn!("include_pkg_updates set but installed_filter missing; skipping updates");
        }
    }
    if include_aur_comments {
        if let Some(installed) = installed_filter {
            if installed.is_empty() {
                warn!("include_aur_comments set but installed set is empty; skipping comments");
            } else {
                match fetch_installed_aur_comments(
                    installed,
                    limit,
                    seen_aur_comments,
                    force_emit_all,
                )
                .await
                {
                    Ok(comments) => items.extend(comments),
                    Err(e) => warn!(error = %e, "installed AUR comments fetch failed"),
                }
            }
        } else {
            warn!("include_aur_comments set but installed_filter missing; skipping comments");
        }
    }
    sort_news_items(&mut items, sort_mode);
    info!(
        total = items.len(),
        arch = items
            .iter()
            .filter(|i| matches!(i.source, NewsFeedSource::ArchNews))
            .count(),
        advisories = items
            .iter()
            .filter(|i| matches!(i.source, NewsFeedSource::SecurityAdvisory))
            .count(),
        updates = items
            .iter()
            .filter(|i| {
                matches!(
                    i.source,
                    NewsFeedSource::InstalledPackageUpdate | NewsFeedSource::AurPackageUpdate
                )
            })
            .count(),
        aur_comments = items
            .iter()
            .filter(|i| matches!(i.source, NewsFeedSource::AurComment))
            .count(),
        "fetch_news_feed success"
    );
    Ok(items)
}

/// What: Sort news feed items by the specified mode.
///
/// Inputs:
/// - `items`: Mutable slice of news feed items to sort.
/// - `mode`: Sort mode (date descending, etc.).
///
/// Output: Items are sorted in place.
///
/// Details: Sorts news items according to the specified sort mode.
fn sort_news_items(items: &mut [NewsFeedItem], mode: NewsSortMode) {
    match mode {
        NewsSortMode::DateDesc => items.sort_by(|a, b| b.date.cmp(&a.date)),
        NewsSortMode::DateAsc => items.sort_by(|a, b| a.date.cmp(&b.date)),
        NewsSortMode::Title => {
            items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        }
        NewsSortMode::SourceThenTitle => items.sort_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
    }
}

/// What: Fetch installed package updates (official and AUR) using cached indexes and AUR RPC.
///
/// Inputs:
/// - `installed`: Set of installed package names (explicit cache).
/// - `limit`: Maximum number of update items to emit.
/// - `seen_pkg_versions`: Last-seen versions map (mutated for persistence).
///
/// Output:
/// - Vector of `NewsFeedItem` describing version bumps for installed packages.
///
/// Details:
/// - Emits when last-seen is missing or differs; updates maps for persistence.
async fn fetch_installed_updates<HS, HV>(
    installed: &HashSet<String, HS>,
    limit: usize,
    seen_pkg_versions: &mut HashMap<String, String, HV>,
    force_emit_all: bool,
    updates_versions: Option<&HashMap<String, (String, String)>>,
) -> Result<Vec<NewsFeedItem>>
where
    HS: BuildHasher + Send + Sync + 'static,
    HV: BuildHasher + Send + Sync + 'static,
{
    let mut items = Vec::new();
    let mut remaining = limit;
    let mut aur_candidates: Vec<String> = Vec::new();
    let mut installed_sorted: Vec<String> = installed.iter().cloned().collect();
    installed_sorted.sort();
    let mut baseline_only = 0usize;

    for name in installed_sorted {
        if let Some(pkg) = crate::index::find_package_by_name(&name) {
            let (old_version_opt, remote_version) = updates_versions
                .and_then(|m| m.get(&pkg.name))
                .map_or((None, pkg.version.as_str()), |(old_v, new_v)| {
                    (Some(old_v.as_str()), new_v.as_str())
                });
            let remote_version = remote_version.to_string();
            let last_seen = seen_pkg_versions.insert(pkg.name.clone(), remote_version.clone());
            let allow = updates_versions.is_none_or(|m| m.contains_key(&pkg.name));
            let should_emit = remaining > 0
                && allow
                && (force_emit_all || last_seen.as_ref() != Some(&remote_version));
            if should_emit {
                let pkg_date = fetch_official_package_date(&pkg).await;
                items.push(build_official_update_item(
                    &pkg,
                    last_seen.as_ref(),
                    old_version_opt,
                    &remote_version,
                    pkg_date,
                ));
                remaining = remaining.saturating_sub(1);
            } else {
                baseline_only = baseline_only.saturating_add(1);
            }
        } else {
            aur_candidates.push(name);
        }
    }

    if remaining == 0 || aur_candidates.is_empty() {
        return Ok(items);
    }

    let aur_info = fetch_aur_versions(&aur_candidates).await?;
    for pkg in aur_info {
        if remaining == 0 {
            break;
        }
        let (old_version_opt, remote_version) = updates_versions
            .and_then(|m| m.get(&pkg.name))
            .map_or((None, pkg.version.as_str()), |(old_v, new_v)| {
                (Some(old_v.as_str()), new_v.as_str())
            });
        let remote_version = remote_version.to_string();
        let last_seen = seen_pkg_versions.insert(pkg.name.clone(), remote_version.clone());
        let allow = updates_versions.is_none_or(|m| m.contains_key(&pkg.name));
        let should_emit = remaining > 0
            && allow
            && (force_emit_all || last_seen.as_ref() != Some(&remote_version));
        if should_emit {
            items.push(build_aur_update_item(
                &pkg,
                last_seen.as_ref(),
                old_version_opt,
                &remote_version,
            ));
            remaining = remaining.saturating_sub(1);
        } else {
            baseline_only = baseline_only.saturating_add(1);
        }
    }

    debug!(
        emitted = items.len(),
        baseline_only,
        installed_total = installed.len(),
        aur_candidates = aur_candidates.len(),
        "installed update feed built"
    );
    Ok(items)
}

/// What: Fetch latest AUR comments for installed AUR packages and emit unseen ones.
///
/// Inputs:
/// - `installed`: Set of installed package names (explicit cache).
/// - `limit`: Maximum number of comment feed items to emit.
/// - `seen_aur_comments`: Last-seen comment identifier per package (mutated).
///
/// Output:
/// - Vector of `NewsFeedItem` representing new comments.
///
/// Details:
/// - Only considers packages not present in the official index (assumed AUR).
/// - Uses first-seen gating to avoid flooding on initial run.
async fn fetch_installed_aur_comments<HS, HC>(
    installed: &HashSet<String, HS>,
    limit: usize,
    seen_aur_comments: &mut HashMap<String, String, HC>,
    force_emit_all: bool,
) -> Result<Vec<NewsFeedItem>>
where
    HS: BuildHasher + Send + Sync + 'static,
    HC: BuildHasher + Send + Sync + 'static,
{
    let mut items = Vec::new();
    if limit == 0 {
        return Ok(items);
    }
    let mut aur_names: Vec<String> = installed
        .iter()
        .filter_map(|name| {
            if crate::index::find_package_by_name(name).is_some() {
                None
            } else {
                Some(name.clone())
            }
        })
        .collect();
    aur_names.sort();
    let mut baseline_only = 0usize;

    for pkgname in &aur_names {
        if items.len() >= limit {
            break;
        }
        match super::fetch_aur_comments(pkgname.clone()).await {
            Ok(comments) => {
                if comments.is_empty() {
                    continue;
                }
                let newly_seen = update_seen_for_comments(
                    pkgname,
                    &comments,
                    seen_aur_comments,
                    limit.saturating_sub(items.len()),
                    force_emit_all,
                );
                if newly_seen.is_empty() {
                    baseline_only = baseline_only.saturating_add(1);
                }
                items.extend(newly_seen);
            }
            Err(e) => warn!(error = %e, pkg = %pkgname, "failed to fetch AUR comments"),
        }
    }

    debug!(
        candidates = aur_names.len(),
        emitted = items.len(),
        baseline_only,
        "installed AUR comments feed built"
    );
    Ok(items)
}

/// What: Minimal AUR version record for update feed generation.
///
/// Inputs:
/// - `name`: Package name.
/// - `version`: Latest version string.
/// - `last_modified`: Optional last modified timestamp (UTC seconds).
///
/// Output:
/// - Data holder used during update feed construction.
///
/// Details:
/// - Derived from AUR RPC v5 info responses.
#[derive(Debug, Clone)]
struct AurVersionInfo {
    /// Package name.
    name: String,
    /// Latest version string.
    version: String,
    /// Optional last-modified timestamp from AUR.
    last_modified: Option<i64>,
}

/// What: Build a feed item for an official package update.
///
/// Inputs:
/// - `pkg`: Official package metadata (includes repo/arch for links).
/// - `last_seen`: Previously seen version (if any) for summary formatting.
/// - `remote_version`: Current version detected in the official index.
///
/// Output:
/// - `NewsFeedItem` representing the update.
///
/// Details:
/// - Prefers package metadata date (last update/build); falls back to today when unavailable.
/// - Includes repo/arch link when available.
fn build_official_update_item(
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
///
/// Output:
/// - `NewsFeedItem` representing the update.
///
/// Details:
/// - Uses last-modified timestamp for the date when available, otherwise today.
fn build_aur_update_item(
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
fn ts_to_date_string(ts: i64) -> Option<String> {
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
async fn fetch_official_package_date(pkg: &crate::state::PackageItem) -> Option<String> {
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
    match tokio::task::spawn_blocking({
        let url = url.clone();
        move || crate::util::curl::curl_json(&url)
    })
    .await
    {
        Ok(Ok(json)) => {
            let obj = json.get("pkg").unwrap_or(&json);
            extract_date_from_pkg_json(obj)
        }
        Ok(Err(e)) => {
            warn!(error = %e, package = %pkg.name, "failed to fetch official package date");
            None
        }
        Err(e) => {
            warn!(error = ?e, package = %pkg.name, "failed to join package date task");
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
fn extract_date_from_pkg_json(obj: &Value) -> Option<String> {
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
fn normalize_pkg_date(raw: &str) -> Option<String> {
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

/// What: Fetch version info for a list of AUR packages via RPC v5.
///
/// Inputs:
/// - `pkgnames`: Package names to query (will be percent-encoded).
///
/// Output:
/// - `Ok(Vec<AurVersionInfo>)` with name/version/last-modified data; empty on empty input.
///
/// Details:
/// - Uses a single multi-arg RPC call to minimize network requests.
/// - Returns an empty list when request or parsing succeeds but yields no results.
async fn fetch_aur_versions(pkgnames: &[String]) -> Result<Vec<AurVersionInfo>> {
    if pkgnames.is_empty() {
        return Ok(Vec::new());
    }
    let args: String = pkgnames
        .iter()
        .map(|n| format!("arg[]={}", crate::util::percent_encode(n)))
        .collect::<Vec<String>>()
        .join("&");
    let url = format!("https://aur.archlinux.org/rpc/v5/info?{args}");
    let resp = tokio::task::spawn_blocking(move || crate::util::curl::curl_json(&url)).await??;
    let results = resp
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for obj in results {
        if let Some(name) = obj.get("Name").and_then(serde_json::Value::as_str) {
            let version = obj
                .get("Version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            let last_modified = obj.get("LastModified").and_then(serde_json::Value::as_i64);
            out.push(AurVersionInfo {
                name: name.to_string(),
                version,
                last_modified,
            });
        }
    }
    Ok(out)
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
fn normalize_comment_date(date: &str) -> String {
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
fn summarize_comment(content: &str) -> String {
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
fn load_update_versions(path: Option<&PathBuf>) -> Option<HashMap<String, (String, String)>> {
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
///
/// Output:
/// - `Vec<NewsFeedItem>` containing new comment items.
///
/// Details:
/// - Emits from newest to oldest until the previous marker (if any) or allowance is exhausted.
fn update_seen_for_comments<H>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::NewsFeedSource;
    use std::collections::HashMap;

    #[test]
    fn sort_news_items_orders_by_date_desc() {
        let mut items = vec![
            NewsFeedItem {
                id: "1".into(),
                date: "2024-01-02".into(),
                title: "B".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
            NewsFeedItem {
                id: "2".into(),
                date: "2024-01-03".into(),
                title: "A".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
        ];
        sort_news_items(&mut items, NewsSortMode::DateDesc);
        assert_eq!(items.first().map(|i| &i.id), Some(&"2".to_string()));
    }

    #[test]
    fn update_seen_for_comments_emits_on_first_run() {
        let mut seen = HashMap::new();
        let comments = vec![AurComment {
            id: Some("c1".into()),
            author: "a".into(),
            date: "2025-01-01 00:00 (UTC)".into(),
            date_timestamp: Some(0),
            date_url: Some("https://aur.archlinux.org/packages/foo#comment-1".into()),
            content: "hello world".into(),
            pinned: false,
        }];
        let emitted = update_seen_for_comments("foo", &comments, &mut seen, 5, true);
        assert_eq!(emitted.len(), 1, "first run should emit newest comments");
        assert_eq!(emitted[0].id, "aur-comment:foo:c1");
        assert_eq!(seen.get("foo"), Some(&"c1".to_string()));
    }

    #[test]
    fn update_seen_for_comments_emits_until_seen_marker() {
        let mut seen = HashMap::from([("foo".to_string(), "c1".to_string())]);
        let comments = vec![
            AurComment {
                id: Some("c2".into()),
                author: "a".into(),
                date: "2025-01-02 00:00 (UTC)".into(),
                date_timestamp: Some(0),
                date_url: Some("https://aur.archlinux.org/packages/foo#comment-2".into()),
                content: "second".into(),
                pinned: false,
            },
            AurComment {
                id: Some("c1".into()),
                author: "a".into(),
                date: "2025-01-01 00:00 (UTC)".into(),
                date_timestamp: Some(0),
                date_url: Some("https://aur.archlinux.org/packages/foo#comment-1".into()),
                content: "first".into(),
                pinned: false,
            },
        ];
        let emitted = update_seen_for_comments("foo", &comments, &mut seen, 5, false);
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].id, "aur-comment:foo:c2");
        assert_eq!(seen.get("foo"), Some(&"c2".to_string()));
    }

    #[test]
    fn normalize_pkg_date_handles_rfc3339_and_utc_formats() {
        assert_eq!(
            normalize_pkg_date("2025-12-07T11:09:38Z"),
            Some("2025-12-07".to_string())
        );
        assert_eq!(
            normalize_pkg_date("2025-12-07 11:09 UTC"),
            Some("2025-12-07".to_string())
        );
    }

    #[test]
    fn extract_date_from_pkg_json_prefers_last_update() {
        let val = serde_json::json!({
            "pkg": {
                "last_update": "2025-12-07T11:09:38Z",
                "build_date": "2024-01-01T00:00:00Z"
            }
        });
        let Some(pkg) = val.get("pkg") else {
            panic!("pkg key missing");
        };
        let date = extract_date_from_pkg_json(pkg);
        assert_eq!(date, Some("2025-12-07".to_string()));
    }

    #[test]
    fn build_official_update_item_uses_metadata_date_when_available() {
        let pkg = crate::state::PackageItem {
            name: "xterm".into(),
            version: "1".into(),
            description: "term".into(),
            source: crate::state::Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };
        let item =
            build_official_update_item(&pkg, None, Some("1"), "2", Some("2025-12-07".into()));
        assert_eq!(item.date, "2025-12-07");
    }
}
