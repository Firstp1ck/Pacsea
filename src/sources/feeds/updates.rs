//! Package updates fetching (official and AUR).
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use futures::stream::{self, StreamExt};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::state::types::NewsFeedItem;

use super::Result;
use super::cache::{AUR_COMMENTS_CACHE, SKIP_CACHE_TTL_SECONDS, UPDATES_CACHE};
use super::helpers::{
    build_aur_update_item, build_official_update_item, fetch_official_package_date,
    normalize_pkg_date, update_seen_for_comments,
};
use super::rate_limit::rate_limit;

/// What: Result of fetching an official package date.
///
/// Inputs: None (enum variants).
///
/// Output: Indicates whether the fetch succeeded, failed with cached fallback, or needs retry.
///
/// Details:
/// - `Success(date)`: Fetch succeeded with the date.
/// - `CachedFallback(date)`: Fetch failed but cached date was available.
/// - `NeedsRetry`: Fetch failed, no cache available, should retry later.
#[derive(Debug, Clone)]
pub(super) enum FetchDateResult {
    /// Fetch succeeded with the date from network.
    Success(Option<String>),
    /// Fetch failed but cached date was available.
    CachedFallback(Option<String>),
    /// Fetch failed with no cache, should retry later.
    NeedsRetry,
}

/// Cache for AUR JSON changes per package.
/// Key: package name, Value: formatted change description.
static AUR_JSON_CHANGES_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Cache for official package JSON changes per package.
/// Key: package name, Value: formatted change description.
pub(super) static OFFICIAL_JSON_CHANGES_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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
/// - `updates_versions`: Optional map of update versions (used for version information only, not for filtering).
/// - `force_emit_all`: Whether to emit all packages regardless of version changes.
/// - `remaining`: Remaining slots for updates.
///
/// Output:
/// - Tuple of (`official_candidates`, `aur_candidates`, `new_packages_count`, `updated_packages_count`, `baseline_only_count`, `remaining`)
///
/// Details:
/// - All installed packages are checked, regardless of whether they appear in `updates_versions`.
/// - `updates_versions` is used only to provide version information (old/new versions) when available.
/// - Packages are shown if they are new (not previously tracked) or have version changes.
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
            // Always emit new packages (not previously tracked) and version changes
            // Note: updates_versions is used only for version information, not for filtering
            let should_emit = remaining > 0 && (force_emit_all || has_version_change);
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
/// - `updates_versions`: Optional map of update versions (used for version information only, not for filtering).
/// - `force_emit_all`: Whether to emit all packages regardless of version changes.
/// - `remaining`: Remaining slots for updates.
///
/// Output:
/// - Tuple of (`items`, `new_packages_count`, `updated_packages_count`, `baseline_only_count`, `remaining`)
///
/// Details:
/// - All AUR packages are checked, regardless of whether they appear in `updates_versions`.
/// - `updates_versions` is used only to provide version information (old/new versions) when available.
/// - Packages are shown if they are new (not previously tracked) or have version changes.
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
        // Always emit new packages (not previously tracked) and version changes
        // Note: updates_versions is used only for version information, not for filtering
        let should_emit = remaining > 0 && (force_emit_all || has_version_change);
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

/// Maximum retry attempts per package before giving up.
const MAX_RETRIES_PER_PACKAGE: u8 = 3;

/// Base delay in milliseconds between retry attempts (increases with each retry).
const RETRY_BASE_DELAY_MS: u64 = 10_000; // 10 seconds base

/// Delay multiplier for exponential backoff.
const RETRY_DELAY_MULTIPLIER: u64 = 2;

/// What: Candidate with retry tracking for background processing.
#[derive(Clone)]
struct BackgroundRetryCandidate {
    /// Package name for logging.
    pkg_name: String,
    /// Repository slug.
    repo_slug: String,
    /// Architecture slug.
    arch_slug: String,
    /// Number of retry attempts so far.
    retry_count: u8,
}

/// What: Fetch official package dates and spawn background retries for failures.
///
/// Inputs:
/// - `candidates`: List of official package candidates to fetch dates for.
///
/// Output:
/// - Vector of (order, `NewsFeedItem`) tuples - returned immediately.
///
/// Details:
/// - Performs initial fetch for all candidates concurrently.
/// - Returns immediately with successful fetches and cached fallbacks.
/// - Items needing retry use cached date or today's date initially.
/// - Spawns a background task to process retries conservatively.
/// - Background retries update the JSON cache for future fetches.
async fn fetch_official_dates_with_retry(
    candidates: Vec<OfficialCandidate>,
) -> Vec<(usize, NewsFeedItem)> {
    let mut retry_queue: Vec<BackgroundRetryCandidate> = Vec::new();
    let mut official_items: Vec<(usize, NewsFeedItem)> = Vec::new();

    // First pass: fetch all packages concurrently
    let fetch_results: Vec<(OfficialCandidate, FetchDateResult)> = stream::iter(candidates)
        .map(|candidate| async move {
            let result = fetch_official_package_date(&candidate.pkg).await;
            (candidate, result)
        })
        .buffer_unordered(5)
        .collect::<Vec<_>>()
        .await;

    for (candidate, result) in fetch_results {
        match result {
            FetchDateResult::Success(date) | FetchDateResult::CachedFallback(date) => {
                let item = build_official_update_item(
                    &candidate.pkg,
                    candidate.last_seen.as_ref(),
                    candidate.old_version.as_deref(),
                    &candidate.remote_version,
                    date,
                );
                official_items.push((candidate.order, item));
            }
            FetchDateResult::NeedsRetry => {
                // Use today's date for now, queue for background retry
                debug!(
                    package = %candidate.pkg.name,
                    "package needs retry, using today's date and queuing for background retry"
                );
                let item = build_official_update_item(
                    &candidate.pkg,
                    candidate.last_seen.as_ref(),
                    candidate.old_version.as_deref(),
                    &candidate.remote_version,
                    None, // Today's date for now
                );
                official_items.push((candidate.order, item));

                // Extract info for background retry
                if let crate::state::Source::Official { repo, arch } = &candidate.pkg.source {
                    let repo_slug = repo.to_lowercase();
                    let arch_slug = if arch.is_empty() {
                        std::env::consts::ARCH.to_string()
                    } else {
                        arch.clone()
                    };
                    retry_queue.push(BackgroundRetryCandidate {
                        pkg_name: candidate.pkg.name.clone(),
                        repo_slug,
                        arch_slug,
                        retry_count: 0,
                    });
                }
            }
        }
    }

    // Spawn background retry task if there are items to retry
    if !retry_queue.is_empty() {
        info!(
            "spawning background retry task for {} packages",
            retry_queue.len()
        );
        tokio::spawn(process_retry_queue_background(retry_queue));
    }

    official_items
}

/// What: Process retry queue in the background (conservative, one at a time).
///
/// Inputs:
/// - `retry_queue`: Initial list of packages needing retry.
///
/// Output:
/// - None (updates JSON cache on disk for successful retries).
///
/// Details:
/// - Processes retries sequentially with exponential backoff delays.
/// - Failed retries go back to the end of the queue.
/// - Each package can retry up to `MAX_RETRIES_PER_PACKAGE` times.
/// - Successful retries update the JSON cache for future fetches.
async fn process_retry_queue_background(initial_queue: Vec<BackgroundRetryCandidate>) {
    use std::collections::VecDeque;

    let mut retry_queue: VecDeque<BackgroundRetryCandidate> = initial_queue.into_iter().collect();

    info!(
        "background retry task started with {} packages",
        retry_queue.len()
    );

    while let Some(mut retry_item) = retry_queue.pop_front() {
        retry_item.retry_count += 1;

        // Calculate delay with exponential backoff
        let delay_ms = RETRY_BASE_DELAY_MS
            * RETRY_DELAY_MULTIPLIER
                .saturating_pow(u32::from(retry_item.retry_count).saturating_sub(1));
        info!(
            package = %retry_item.pkg_name,
            retry_attempt = retry_item.retry_count,
            queue_remaining = retry_queue.len(),
            delay_ms,
            "background retry: waiting before attempt"
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

        // Fetch the JSON to update cache
        let result = fetch_official_json_for_cache(
            &retry_item.pkg_name,
            &retry_item.repo_slug,
            &retry_item.arch_slug,
        )
        .await;

        match result {
            Ok(()) => {
                info!(
                    package = %retry_item.pkg_name,
                    retry_attempt = retry_item.retry_count,
                    "background retry succeeded, cache updated"
                );
            }
            Err(needs_retry) if needs_retry => {
                if retry_item.retry_count < MAX_RETRIES_PER_PACKAGE {
                    // Add back to the END of the queue for later retry
                    debug!(
                        package = %retry_item.pkg_name,
                        retry_attempt = retry_item.retry_count,
                        "background retry failed, adding back to end of queue"
                    );
                    retry_queue.push_back(retry_item);
                } else {
                    warn!(
                        package = %retry_item.pkg_name,
                        max_retries = MAX_RETRIES_PER_PACKAGE,
                        "background retry: all attempts exhausted"
                    );
                }
            }
            Err(_) => {
                // Non-retryable error (e.g., used cache)
                debug!(
                    package = %retry_item.pkg_name,
                    "background retry: completed (cache or non-retryable)"
                );
            }
        }
    }

    info!("background retry task completed");
}

/// What: Fetch official package JSON and save to cache (for background retry).
///
/// Inputs:
/// - `pkg_name`: Package name.
/// - `repo_slug`: Repository slug (lowercase).
/// - `arch_slug`: Architecture slug.
///
/// Output:
/// - `Ok(())` on success (cache updated).
/// - `Err(true)` if fetch failed and should retry.
/// - `Err(false)` if fetch failed but no retry needed.
///
/// Details:
/// - Applies rate limiting and circuit breaker checks.
/// - Saves JSON to disk cache on success.
async fn fetch_official_json_for_cache(
    pkg_name: &str,
    repo_slug: &str,
    arch_slug: &str,
) -> std::result::Result<(), bool> {
    use super::rate_limit::{
        check_circuit_breaker, increase_archlinux_backoff, rate_limit_archlinux,
        record_circuit_breaker_outcome, reset_archlinux_backoff,
    };

    let url = format!("https://archlinux.org/packages/{repo_slug}/{arch_slug}/{pkg_name}/json/",);
    let endpoint_pattern = "/packages/*/json/";
    let cache_path = official_json_cache_path(repo_slug, arch_slug, pkg_name);

    // Check circuit breaker
    if check_circuit_breaker(endpoint_pattern).is_err() {
        debug!(
            package = %pkg_name,
            "background retry: circuit breaker blocking"
        );
        return Err(true); // Should retry later
    }

    // Apply rate limiting
    let _permit = rate_limit_archlinux().await;

    // Fetch with timeout (longer for background)
    let result = tokio::time::timeout(
        tokio::time::Duration::from_millis(5000),
        tokio::task::spawn_blocking({
            let url = url.clone();
            move || crate::util::curl::curl_json(&url)
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(json))) => {
            reset_archlinux_backoff();
            record_circuit_breaker_outcome(endpoint_pattern, true);

            // Save to cache
            if let Err(e) = save_official_json_cache(&cache_path, &json) {
                debug!(
                    error = %e,
                    package = %pkg_name,
                    "background retry: failed to save cache"
                );
            }
            Ok(())
        }
        Ok(Ok(Err(e))) => {
            increase_archlinux_backoff(None);
            record_circuit_breaker_outcome(endpoint_pattern, false);
            debug!(
                package = %pkg_name,
                error = %e,
                "background retry: fetch failed"
            );
            Err(true) // Should retry
        }
        Ok(Err(e)) => {
            increase_archlinux_backoff(None);
            record_circuit_breaker_outcome(endpoint_pattern, false);
            debug!(
                package = %pkg_name,
                error = ?e,
                "background retry: task join failed"
            );
            Err(true) // Should retry
        }
        Err(_) => {
            increase_archlinux_backoff(None);
            record_circuit_breaker_outcome(endpoint_pattern, false);
            debug!(package = %pkg_name, "background retry: timeout");
            Err(true) // Should retry
        }
    }
}

/// What: Get the path to the AUR JSON cache directory.
///
/// Inputs: None.
///
/// Output:
/// - `PathBuf` pointing to the cache directory.
///
/// Details:
/// - Uses the lists directory from theme configuration.
#[must_use]
fn aur_json_cache_dir() -> PathBuf {
    crate::theme::lists_dir().join("aur_json_cache")
}

/// What: Get the path to a cached AUR JSON file for a set of packages.
///
/// Inputs:
/// - `pkgnames`: Package names to generate cache key from.
///
/// Output:
/// - `PathBuf` to the cache file.
///
/// Details:
/// - Creates a deterministic filename from sorted package names.
fn aur_json_cache_path(pkgnames: &[String]) -> PathBuf {
    let mut sorted = pkgnames.to_vec();
    sorted.sort();
    let key = sorted.join(",");
    // Create a safe filename from the key
    let safe_key = key
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ',' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    aur_json_cache_dir().join(format!("{safe_key}.json"))
}

/// What: Load previously cached AUR JSON from disk.
///
/// Inputs:
/// - `cache_path`: Path to the cache file.
///
/// Output:
/// - `Some(Value)` if cache exists and is valid JSON; `None` otherwise.
///
/// Details:
/// - Returns `None` on file read errors or JSON parse errors.
fn load_aur_json_cache(cache_path: &PathBuf) -> Option<Value> {
    let data = std::fs::read_to_string(cache_path).ok()?;
    serde_json::from_str::<Value>(&data).ok()
}

/// What: Save AUR JSON response to disk cache.
///
/// Inputs:
/// - `cache_path`: Path where to save the cache.
/// - `json`: JSON value to save.
///
/// Output:
/// - `Ok(())` on success, `Err` on failure.
///
/// Details:
/// - Creates parent directories if they don't exist.
/// - Saves pretty-printed JSON for readability.
fn save_aur_json_cache(cache_path: &PathBuf, json: &Value) -> std::io::Result<()> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pretty = serde_json::to_string_pretty(json)?;
    std::fs::write(cache_path, pretty)
}

/// What: Compare two AUR package JSON objects and generate a change description.
///
/// Inputs:
/// - `old_json`: Previous JSON object for the package.
/// - `new_json`: Current JSON object for the package.
/// - `pkg_name`: Package name for context.
///
/// Output:
/// - `Some(String)` with formatted changes if differences found; `None` if identical.
///
/// Details:
/// - Compares key fields like Version, Description, Maintainer, etc.
/// - Formats changes in a human-readable way.
fn compare_aur_json_changes(old_json: &Value, new_json: &Value, pkg_name: &str) -> Option<String> {
    let mut changes = Vec::new();

    // Compare Version
    let old_version = old_json.get("Version").and_then(Value::as_str);
    let new_version = new_json.get("Version").and_then(Value::as_str);
    if old_version != new_version
        && let (Some(old_v), Some(new_v)) = (old_version, new_version)
        && old_v != new_v
    {
        changes.push(format!("Version: {old_v} → {new_v}"));
    }

    // Compare Description
    let old_desc = old_json.get("Description").and_then(Value::as_str);
    let new_desc = new_json.get("Description").and_then(Value::as_str);
    if old_desc != new_desc
        && let (Some(old_d), Some(new_d)) = (old_desc, new_desc)
        && old_d != new_d
    {
        changes.push("Description changed".to_string());
    }

    // Compare Maintainer
    let old_maintainer = old_json.get("Maintainer").and_then(Value::as_str);
    let new_maintainer = new_json.get("Maintainer").and_then(Value::as_str);
    if old_maintainer != new_maintainer
        && let (Some(old_m), Some(new_m)) = (old_maintainer, new_maintainer)
        && old_m != new_m
    {
        changes.push(format!("Maintainer: {old_m} → {new_m}"));
    }

    // Compare URL
    let old_url = old_json.get("URL").and_then(Value::as_str);
    let new_url = new_json.get("URL").and_then(Value::as_str);
    if old_url != new_url
        && let (Some(old_u), Some(new_u)) = (old_url, new_url)
        && old_u != new_u
    {
        changes.push("URL changed".to_string());
    }

    // Compare License
    let old_license = old_json.get("License").and_then(Value::as_array);
    let new_license = new_json.get("License").and_then(Value::as_array);
    if old_license != new_license {
        changes.push("License changed".to_string());
    }

    // Compare Keywords
    let old_keywords = old_json.get("Keywords").and_then(Value::as_array);
    let new_keywords = new_json.get("Keywords").and_then(Value::as_array);
    if old_keywords != new_keywords {
        changes.push("Keywords changed".to_string());
    }

    if changes.is_empty() {
        None
    } else {
        Some(format!(
            "Changes detected for {pkg_name}:\n{}",
            changes.join("\n")
        ))
    }
}

/// What: Get the path to the official package JSON cache directory.
///
/// Inputs: None.
///
/// Output:
/// - `PathBuf` pointing to the cache directory.
///
/// Details:
/// - Uses the lists directory from theme configuration.
#[must_use]
fn official_json_cache_dir() -> PathBuf {
    crate::theme::lists_dir().join("official_json_cache")
}

/// What: Get the path to a cached official package JSON file.
///
/// Inputs:
/// - `repo`: Repository name.
/// - `arch`: Architecture.
/// - `pkg_name`: Package name.
///
/// Output:
/// - `PathBuf` to the cache file.
///
/// Details:
/// - Creates a deterministic filename from repo, arch, and package name.
#[must_use]
pub fn official_json_cache_path(repo: &str, arch: &str, pkg_name: &str) -> PathBuf {
    let safe_repo = repo
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let safe_arch = arch
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let safe_name = pkg_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    official_json_cache_dir().join(format!("{safe_repo}_{safe_arch}_{safe_name}.json"))
}

/// What: Load previously cached official package JSON from disk.
///
/// Inputs:
/// - `cache_path`: Path to the cache file.
///
/// Output:
/// - `Some(Value)` if cache exists and is valid JSON; `None` otherwise.
///
/// Details:
/// - Returns `None` on file read errors or JSON parse errors.
#[must_use]
pub fn load_official_json_cache(cache_path: &PathBuf) -> Option<Value> {
    let data = std::fs::read_to_string(cache_path).ok()?;
    serde_json::from_str::<Value>(&data).ok()
}

/// What: Save official package JSON response to disk cache.
///
/// Inputs:
/// - `cache_path`: Path where to save the cache.
/// - `json`: JSON value to save.
///
/// Output:
/// - `Ok(())` on success, `Err` on failure.
///
/// Details:
/// - Creates parent directories if they don't exist.
/// - Saves pretty-printed JSON for readability.
pub(super) fn save_official_json_cache(cache_path: &PathBuf, json: &Value) -> std::io::Result<()> {
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pretty = serde_json::to_string_pretty(json)?;
    std::fs::write(cache_path, pretty)
}

/// What: Compare two official package JSON objects and generate a change description.
///
/// Inputs:
/// - `old_json`: Previous JSON object for the package.
/// - `new_json`: Current JSON object for the package.
/// - `pkg_name`: Package name for context.
///
/// Output:
/// - `Some(String)` with formatted changes if differences found; `None` if identical.
///
/// Details:
/// - Compares key fields like version, description, licenses, etc.
/// - Formats changes in a human-readable way.
pub(super) fn compare_official_json_changes(
    old_json: &Value,
    new_json: &Value,
    pkg_name: &str,
) -> Option<String> {
    let mut changes = Vec::new();

    // Get the pkg object from both JSONs
    let old_pkg = old_json.get("pkg").unwrap_or(old_json);
    let new_pkg = new_json.get("pkg").unwrap_or(new_json);

    // Compare Version
    let old_version = old_pkg.get("pkgver").and_then(Value::as_str);
    let new_version = new_pkg.get("pkgver").and_then(Value::as_str);
    if old_version != new_version
        && let (Some(old_v), Some(new_v)) = (old_version, new_version)
        && old_v != new_v
    {
        changes.push(format!("Version: {old_v} → {new_v}"));
    }

    // Compare Description
    let old_desc = old_pkg.get("pkgdesc").and_then(Value::as_str);
    let new_desc = new_pkg.get("pkgdesc").and_then(Value::as_str);
    if old_desc != new_desc
        && let (Some(old_d), Some(new_d)) = (old_desc, new_desc)
        && old_d != new_d
    {
        changes.push("Description changed".to_string());
    }

    // Compare Licenses
    let old_licenses = old_pkg.get("licenses").and_then(Value::as_array);
    let new_licenses = new_pkg.get("licenses").and_then(Value::as_array);
    if old_licenses != new_licenses {
        changes.push("Licenses changed".to_string());
    }

    // Compare URL
    let old_url = old_pkg.get("url").and_then(Value::as_str);
    let new_url = new_pkg.get("url").and_then(Value::as_str);
    if old_url != new_url
        && let (Some(old_u), Some(new_u)) = (old_url, new_url)
        && old_u != new_u
    {
        changes.push("URL changed".to_string());
    }

    // Compare Groups
    let old_groups = old_pkg.get("groups").and_then(Value::as_array);
    let new_groups = new_pkg.get("groups").and_then(Value::as_array);
    if old_groups != new_groups {
        changes.push("Groups changed".to_string());
    }

    // Compare Dependencies
    let old_depends = old_pkg.get("depends").and_then(Value::as_array);
    let new_depends = new_pkg.get("depends").and_then(Value::as_array);
    if old_depends != new_depends {
        changes.push("Dependencies changed".to_string());
    }

    // Compare last_update date (check top-level JSON, not pkg object)
    let old_last_update = old_json.get("last_update").and_then(Value::as_str);
    let new_last_update = new_json.get("last_update").and_then(Value::as_str);
    if old_last_update != new_last_update
        && let (Some(old_date), Some(new_date)) = (old_last_update, new_last_update)
        && old_date != new_date
    {
        // Normalize dates for comparison
        if let (Some(old_norm), Some(new_norm)) =
            (normalize_pkg_date(old_date), normalize_pkg_date(new_date))
            && old_norm != new_norm
        {
            changes.push(format!("Last update: {old_norm} → {new_norm}"));
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(format!(
            "Changes detected for {pkg_name}:\n{}",
            changes.join("\n")
        ))
    }
}

/// What: Get cached JSON changes for an AUR package.
///
/// Inputs:
/// - `pkg_name`: Package name to look up.
///
/// Output:
/// - `Some(String)` with change description if changes were detected; `None` otherwise.
///
/// Details:
/// - Returns changes that were detected during the last `fetch_aur_versions` call.
#[must_use]
pub fn get_aur_json_changes(pkg_name: &str) -> Option<String> {
    AUR_JSON_CHANGES_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(pkg_name).cloned())
}

/// What: Get cached JSON changes for an official package.
///
/// Inputs:
/// - `pkg_name`: Package name to look up.
///
/// Output:
/// - `Some(String)` with change description if changes were detected; `None` otherwise.
///
/// Details:
/// - Returns changes that were detected during the last `fetch_official_package_date` call.
#[must_use]
pub fn get_official_json_changes(pkg_name: &str) -> Option<String> {
    OFFICIAL_JSON_CHANGES_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(pkg_name).cloned())
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
/// - Saves JSON response to disk and compares with previous version to detect changes.
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

    // Load previous JSON before fetching new one
    let cache_path = aur_json_cache_path(pkgnames);
    let old_json = load_aur_json_cache(&cache_path);

    let resp = tokio::task::spawn_blocking(move || crate::util::curl::curl_json(&url)).await??;

    // Compare with previous JSON if it exists
    if let Some(old_json) = old_json
        && let Some(results_old) = old_json.get("results").and_then(Value::as_array)
        && let Some(results_new) = resp.get("results").and_then(Value::as_array)
    {
        // Create maps for easier lookup
        let old_map: HashMap<String, &Value> = results_old
            .iter()
            .filter_map(|obj| {
                obj.get("Name")
                    .and_then(Value::as_str)
                    .map(|name| (name.to_string(), obj))
            })
            .collect();
        let new_map: HashMap<String, &Value> = results_new
            .iter()
            .filter_map(|obj| {
                obj.get("Name")
                    .and_then(Value::as_str)
                    .map(|name| (name.to_string(), obj))
            })
            .collect();

        // Compare each package
        let mut changes_cache = AUR_JSON_CHANGES_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (pkg_name, new_obj) in &new_map {
            if let Some(old_obj) = old_map.get(pkg_name)
                && let Some(change_desc) = compare_aur_json_changes(old_obj, new_obj, pkg_name)
            {
                changes_cache.insert(pkg_name.clone(), change_desc);
            }
        }
    }

    // Save the full JSON response to disk (after comparison)
    if let Err(e) = save_aur_json_cache(&cache_path, &resp) {
        warn!(error = %e, path = ?cache_path, "failed to save AUR JSON cache");
    } else {
        debug!(path = ?cache_path, "saved AUR JSON cache");
    }

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

    // Fetch official package dates with rate-limited concurrency and retry support.
    if !official_candidates.is_empty() {
        debug!(
            "fetch_installed_updates: fetching dates for {} official packages (rate-limited)",
            official_candidates.len()
        );
        let mut official_items = fetch_official_dates_with_retry(official_candidates).await;
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

    // Only skip AUR processing if there are no AUR candidates
    // Note: We should still process AUR packages even if remaining == 0,
    // because AUR packages deserve representation in the feed alongside official packages
    if aur_candidates.is_empty() {
        debug!("fetch_installed_updates: no AUR candidates, skipping AUR fetch");
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
    // Process AUR packages with their own allocation
    // AUR packages get half of the original limit to ensure representation
    let aur_remaining = limit / 2;
    let (mut aur_items, aur_new_packages, aur_updated_packages, aur_baseline_only, _remaining) =
        process_aur_packages(
            aur_info,
            seen_pkg_versions,
            updates_versions,
            force_emit_all,
            aur_remaining,
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
