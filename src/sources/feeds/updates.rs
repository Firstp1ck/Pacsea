//! Package updates fetching (official and AUR).
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::time::Instant;

use futures::stream::{self, StreamExt};
use tracing::{debug, info, warn};

use crate::state::types::NewsFeedItem;

use super::Result;
use super::cache::{AUR_COMMENTS_CACHE, SKIP_CACHE_TTL_SECONDS, UPDATES_CACHE};
use super::helpers::{
    build_aur_update_item, build_official_update_item, fetch_official_package_date,
    update_seen_for_comments,
};
use super::rate_limit::rate_limit;

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
pub(super) struct AurVersionInfo {
    /// Package name.
    pub name: String,
    /// Latest version string.
    pub version: String,
    /// Optional last-modified timestamp from AUR.
    pub last_modified: Option<i64>,
}

/// What: Helper container for official package update processing with bounded concurrency.
#[derive(Clone)]
struct OfficialCandidate {
    /// Original order in the installed list to keep stable rendering.
    order: usize,
    /// Package metadata from the official index.
    pkg: crate::state::PackageItem,
    /// Previously seen version (if any).
    last_seen: Option<String>,
    /// Old version string captured from updates list (if available).
    old_version: Option<String>,
    /// Current remote version.
    remote_version: String,
}

/// What: Process official packages and build candidates for update items.
///
/// Inputs:
/// - `installed_sorted`: Sorted list of installed package names.
/// - `seen_pkg_versions`: Last-seen versions map (mutated).
/// - `updates_versions`: Optional map of update versions.
/// - `force_emit_all`: Whether to emit all packages regardless of version changes.
/// - `remaining`: Remaining slots for updates.
///
/// Output:
/// - Tuple of (`official_candidates`, `aur_candidates`, `new_packages_count`, `updated_packages_count`, `baseline_only_count`, `remaining`)
fn process_official_packages<HV>(
    installed_sorted: &[String],
    seen_pkg_versions: &mut HashMap<String, String, HV>,
    updates_versions: Option<&HashMap<String, (String, String)>>,
    force_emit_all: bool,
    mut remaining: usize,
) -> (
    Vec<OfficialCandidate>,
    Vec<String>,
    usize,
    usize,
    usize,
    usize,
)
where
    HV: BuildHasher,
{
    let mut aur_candidates: Vec<String> = Vec::new();
    let mut official_candidates: Vec<OfficialCandidate> = Vec::new();
    let mut baseline_only = 0usize;
    let mut new_packages = 0usize;
    let mut updated_packages = 0usize;

    for name in installed_sorted {
        if let Some(pkg) = crate::index::find_package_by_name(name) {
            let (old_version_opt, remote_version) = updates_versions
                .and_then(|m| m.get(&pkg.name))
                .map_or((None, pkg.version.as_str()), |(old_v, new_v)| {
                    (Some(old_v.as_str()), new_v.as_str())
                });
            let remote_version = remote_version.to_string();
            let last_seen = seen_pkg_versions.insert(pkg.name.clone(), remote_version.clone());
            let is_new_package = last_seen.is_none();
            let has_version_change = last_seen.as_ref() != Some(&remote_version);
            let allow = updates_versions.is_none_or(|m| m.contains_key(&pkg.name));
            // Always emit new packages (not previously tracked) and version changes
            let should_emit = remaining > 0 && allow && (force_emit_all || has_version_change);
            if should_emit {
                if is_new_package {
                    new_packages = new_packages.saturating_add(1);
                } else if has_version_change {
                    updated_packages = updated_packages.saturating_add(1);
                }
                let order = official_candidates.len();
                official_candidates.push(OfficialCandidate {
                    order,
                    pkg: pkg.clone(),
                    last_seen,
                    old_version: old_version_opt.map(str::to_string),
                    remote_version,
                });
                remaining = remaining.saturating_sub(1);
            } else {
                baseline_only = baseline_only.saturating_add(1);
            }
        } else {
            aur_candidates.push(name.clone());
        }
    }

    (
        official_candidates,
        aur_candidates,
        new_packages,
        updated_packages,
        baseline_only,
        remaining,
    )
}

/// What: Process AUR packages and build update items.
///
/// Inputs:
/// - `aur_info`: AUR package information.
/// - `seen_pkg_versions`: Last-seen versions map (mutated).
/// - `updates_versions`: Optional map of update versions.
/// - `force_emit_all`: Whether to emit all packages regardless of version changes.
/// - `remaining`: Remaining slots for updates.
///
/// Output:
/// - Tuple of (`items`, `new_packages_count`, `updated_packages_count`, `baseline_only_count`, `remaining`)
fn process_aur_packages<HV>(
    aur_info: Vec<AurVersionInfo>,
    seen_pkg_versions: &mut HashMap<String, String, HV>,
    updates_versions: Option<&HashMap<String, (String, String)>>,
    force_emit_all: bool,
    mut remaining: usize,
) -> (Vec<NewsFeedItem>, usize, usize, usize, usize)
where
    HV: BuildHasher,
{
    let mut items = Vec::new();
    let mut aur_new_packages = 0usize;
    let mut aur_updated_packages = 0usize;
    let mut baseline_only = 0usize;

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
        let is_new_package = last_seen.is_none();
        let has_version_change = last_seen.as_ref() != Some(&remote_version);
        let allow = updates_versions.is_none_or(|m| m.contains_key(&pkg.name));
        // Always emit new packages (not previously tracked) and version changes
        let should_emit = remaining > 0 && allow && (force_emit_all || has_version_change);
        if should_emit {
            if is_new_package {
                aur_new_packages = aur_new_packages.saturating_add(1);
            } else if has_version_change {
                aur_updated_packages = aur_updated_packages.saturating_add(1);
            }
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

    (
        items,
        aur_new_packages,
        aur_updated_packages,
        baseline_only,
        remaining,
    )
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
    // Apply rate limiting before network request
    rate_limit().await;
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

/// What: Fetch installed package updates (official and AUR) using cached indexes and AUR RPC.
///
/// Inputs:
/// - `installed`: Set of installed package names (explicit cache).
/// - `limit`: Maximum number of update items to emit.
/// - `seen_pkg_versions`: Last-seen versions map (mutated for persistence).
/// - `force_emit_all`: Whether to emit all packages regardless of version changes.
/// - `updates_versions`: Optional pre-loaded update versions.
///
/// Output:
/// - Vector of `NewsFeedItem` describing version bumps for installed packages.
///
/// Details:
/// - Emits when last-seen is missing or differs; updates maps for persistence.
/// - New packages (not previously tracked) are always emitted regardless of optimization settings.
pub(super) async fn fetch_installed_updates<HS, HV>(
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
    // Check if we can use cached results (skip if last fetch was < 5 minutes ago)
    if let Ok(cache_guard) = UPDATES_CACHE.lock()
        && let Some((cached_items, last_fetch)) = cache_guard.as_ref()
        && last_fetch.elapsed().as_secs() < SKIP_CACHE_TTL_SECONDS
    {
        info!(
            "fetch_installed_updates: using cached results (age={}s, items={})",
            last_fetch.elapsed().as_secs(),
            cached_items.len()
        );
        return Ok(cached_items.clone());
    }

    debug!(
        "fetch_installed_updates: starting, installed_count={}, limit={}, force_emit_all={}",
        installed.len(),
        limit,
        force_emit_all
    );
    let mut items = Vec::new();
    let mut installed_sorted: Vec<String> = installed.iter().cloned().collect();
    installed_sorted.sort();

    debug!(
        "fetch_installed_updates: processing {} installed packages",
        installed_sorted.len()
    );
    // Process official packages
    let (
        official_candidates,
        aur_candidates,
        new_packages,
        updated_packages,
        baseline_only,
        remaining,
    ) = process_official_packages(
        &installed_sorted,
        seen_pkg_versions,
        updates_versions,
        force_emit_all,
        limit,
    );
    info!(
        "fetch_installed_updates: official scan complete, new_packages={}, updated_packages={}, baseline_only={}",
        new_packages, updated_packages, baseline_only
    );

    // Fetch official package dates with rate-limited concurrency.
    // Note: Although buffer_unordered(5) allows 5 tasks in flight, the archlinux.org
    // rate limiter semaphore serializes actual HTTP requests to 1 at a time.
    // This prevents IP blocking while still allowing task scheduling overhead.
    if !official_candidates.is_empty() {
        debug!(
            "fetch_installed_updates: fetching dates for {} official packages (rate-limited)",
            official_candidates.len()
        );
        let mut official_items = stream::iter(official_candidates)
            .map(|candidate| async move {
                let date = fetch_official_package_date(&candidate.pkg).await;
                (
                    candidate.order,
                    build_official_update_item(
                        &candidate.pkg,
                        candidate.last_seen.as_ref(),
                        candidate.old_version.as_deref(),
                        &candidate.remote_version,
                        date,
                    ),
                )
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await;
        official_items.sort_by_key(|(order, _)| *order);
        for (_, item) in official_items {
            items.push(item);
        }
        debug!(
            "fetch_installed_updates: official packages processed, items={}, aur_candidates={}, remaining={}",
            items.len(),
            aur_candidates.len(),
            remaining
        );
    }

    if remaining == 0 || aur_candidates.is_empty() {
        debug!(
            "fetch_installed_updates: returning early, remaining={}, aur_candidates_empty={}",
            remaining,
            aur_candidates.is_empty()
        );
        return Ok(items);
    }

    debug!(
        "fetch_installed_updates: fetching AUR versions for {} candidates",
        aur_candidates.len()
    );
    let aur_info = fetch_aur_versions(&aur_candidates).await?;
    debug!(
        "fetch_installed_updates: fetched {} AUR package versions",
        aur_info.len()
    );
    // Process AUR packages
    let (mut aur_items, aur_new_packages, aur_updated_packages, aur_baseline_only, _remaining) =
        process_aur_packages(
            aur_info,
            seen_pkg_versions,
            updates_versions,
            force_emit_all,
            remaining,
        );
    items.append(&mut aur_items);
    let baseline_only = baseline_only.saturating_add(aur_baseline_only);

    info!(
        emitted = items.len(),
        new_packages,
        updated_packages,
        aur_new_packages,
        aur_updated_packages,
        baseline_only,
        installed_total = installed.len(),
        aur_candidates = aur_candidates.len(),
        "installed update feed built"
    );

    // Cache results for 5-minute skip
    if let Ok(mut cache_guard) = UPDATES_CACHE.lock() {
        *cache_guard = Some((items.clone(), Instant::now()));
    }

    Ok(items)
}

/// What: Fetch latest AUR comments for installed AUR packages and emit unseen ones.
///
/// Inputs:
/// - `installed`: Set of installed package names (explicit cache).
/// - `limit`: Maximum number of comment feed items to emit.
/// - `seen_aur_comments`: Last-seen comment identifier per package (mutated).
/// - `force_emit_all`: Whether to emit all comments regardless of seen state.
///
/// Output:
/// - Vector of `NewsFeedItem` representing new comments.
///
/// Details:
/// - Only considers packages not present in the official index (assumed AUR).
/// - Uses first-seen gating to avoid flooding on initial run.
pub(super) async fn fetch_installed_aur_comments<HS, HC>(
    installed: &HashSet<String, HS>,
    limit: usize,
    seen_aur_comments: &mut HashMap<String, String, HC>,
    force_emit_all: bool,
) -> Result<Vec<NewsFeedItem>>
where
    HS: BuildHasher + Send + Sync + 'static,
    HC: BuildHasher + Send + Sync + 'static,
{
    // Check if we can use cached results (skip if last fetch was < 5 minutes ago)
    if let Ok(cache_guard) = AUR_COMMENTS_CACHE.lock()
        && let Some((cached_items, last_fetch)) = cache_guard.as_ref()
        && last_fetch.elapsed().as_secs() < SKIP_CACHE_TTL_SECONDS
    {
        info!(
            "fetch_installed_aur_comments: using cached results (age={}s, items={})",
            last_fetch.elapsed().as_secs(),
            cached_items.len()
        );
        return Ok(cached_items.clone());
    }

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
        match crate::sources::fetch_aur_comments(pkgname.clone()).await {
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

    // Cache results for 5-minute skip
    if let Ok(mut cache_guard) = AUR_COMMENTS_CACHE.lock() {
        *cache_guard = Some((items.clone(), Instant::now()));
    }

    Ok(items)
}
