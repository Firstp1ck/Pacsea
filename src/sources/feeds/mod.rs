//! Aggregated news feed fetcher (Arch news + security advisories).
mod cache;
mod helpers;
mod news_fetch;
mod rate_limit;
mod updates;

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::path::PathBuf;

use crate::state::types::{NewsFeedItem, NewsSortMode, severity_rank};
use tracing::{info, warn};

use helpers::load_update_versions;
use news_fetch::fetch_slow_sources;
use updates::{fetch_installed_aur_comments, fetch_installed_updates};

/// Result type alias for news feed fetching operations.
type Result<T> = super::Result<T>;

/// What: Calculate optimal `max_age_days` based on last startup timestamp.
///
/// Inputs:
/// - `last_startup`: Optional timestamp in `YYYYMMDD:HHMMSS` format.
/// - `default_max_age`: Default max age in days if no optimization applies.
///
/// Output:
/// - Optimized `max_age_days` value, or `None` to fetch all.
///
/// Details:
/// - If last startup was within 1 hour: use 1 day (recent data likely cached)
/// - If last startup was within 24 hours: use 2 days
/// - If last startup was within 7 days: use configured `max_age` or 7 days
/// - Otherwise: use configured `max_age`
/// - This reduces unnecessary fetching when the app was recently used.
/// - NOTE: This only affects Arch news and advisories date filtering.
///   Package updates are ALWAYS fetched fresh to detect new packages and version changes.
#[must_use]
pub fn optimize_max_age_for_startup(
    last_startup: Option<&str>,
    default_max_age: Option<u32>,
) -> Option<u32> {
    let Some(ts) = last_startup else {
        // No previous startup recorded, use default
        return default_max_age;
    };

    // Parse timestamp: YYYYMMDD:HHMMSS
    let parsed = chrono::NaiveDateTime::parse_from_str(ts, "%Y%m%d:%H%M%S").ok();
    let Some(last_dt) = parsed else {
        tracing::debug!(timestamp = %ts, "failed to parse last startup timestamp");
        return default_max_age;
    };

    let now = chrono::Local::now().naive_local();
    let elapsed = now.signed_duration_since(last_dt);

    if elapsed.num_hours() < 1 {
        // Very recent startup (< 1 hour): minimal fresh fetch needed
        info!(
            hours_since_last = elapsed.num_hours(),
            "recent startup detected, using minimal fetch window"
        );
        Some(1)
    } else if elapsed.num_hours() < 24 {
        // Within last day: use 2 days to be safe
        info!(
            hours_since_last = elapsed.num_hours(),
            "startup within 24h, using 2-day fetch window"
        );
        Some(2)
    } else if elapsed.num_days() < 7 {
        // Within last week: use configured or 7 days
        let optimized = default_max_age.map_or(7, |d| d.min(7));
        info!(
            days_since_last = elapsed.num_days(),
            optimized_max_age = optimized,
            "startup within 7 days, using optimized fetch window"
        );
        Some(optimized)
    } else {
        // More than a week: use configured max_age
        default_max_age
    }
}

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
/// - `max_age_days`: Optional maximum age in days for filtering items (enables early filtering).
///
/// Output:
/// - Mutable references updated in place alongside returned feed items.
///
/// Details:
/// - Hashers are generic to remain compatible with caller-supplied maps.
/// - `max_age_days` enables early date filtering during fetch to improve performance.
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
    /// Optional maximum age in days for early date filtering during fetch.
    pub max_age_days: Option<u32>,
}

/// Configuration for fetching fast sources.
struct FastSourcesConfig<'a, HS, HV, HC> {
    /// Whether to fetch package updates.
    include_pkg_updates: bool,
    /// Whether to fetch AUR comments.
    include_aur_comments: bool,
    /// Optional set of installed package names.
    installed_filter: Option<&'a HashSet<String, HS>>,
    /// Maximum items per source.
    limit: usize,
    /// Last-seen versions map (updated in place).
    seen_pkg_versions: &'a mut HashMap<String, String, HV>,
    /// Last-seen AUR comments map (updated in place).
    seen_aur_comments: &'a mut HashMap<String, String, HC>,
    /// Whether to emit all items regardless of last-seen.
    force_emit_all: bool,
    /// Optional pre-loaded update versions.
    updates_versions: Option<&'a HashMap<String, (String, String)>>,
}

/// What: Fetch fast sources (package updates and AUR comments) in parallel.
///
/// Inputs:
/// - `config`: Configuration struct containing all fetch parameters.
///
/// Output:
/// - Tuple of (`updates_result`, `comments_result`).
///
/// Details:
/// - Fetches both sources in parallel for better performance.
/// - Returns empty vectors on errors (graceful degradation).
async fn fetch_fast_sources<HS, HV, HC>(
    config: FastSourcesConfig<'_, HS, HV, HC>,
) -> (
    std::result::Result<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>,
    std::result::Result<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>,
)
where
    HS: BuildHasher + Send + Sync + 'static,
    HV: BuildHasher + Send + Sync + 'static,
    HC: BuildHasher + Send + Sync + 'static,
{
    tokio::join!(
        async {
            if config.include_pkg_updates {
                if let Some(installed) = config.installed_filter {
                    if installed.is_empty() {
                        warn!(
                            "include_pkg_updates set but installed set is empty; skipping updates"
                        );
                        Ok::<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>(Vec::new())
                    } else {
                        info!(
                            "fetching package updates: installed_count={}, limit={}",
                            installed.len(),
                            config.limit
                        );
                        let result = fetch_installed_updates(
                            installed,
                            config.limit,
                            config.seen_pkg_versions,
                            config.force_emit_all,
                            config.updates_versions,
                        )
                        .await;
                        match &result {
                            Ok(updates) => {
                                info!("package updates fetch completed: items={}", updates.len());
                            }
                            Err(e) => {
                                warn!(error = %e, "installed package updates fetch failed");
                            }
                        }
                        match result {
                            Ok(updates) => Ok(updates),
                            Err(_e) => Ok::<
                                Vec<NewsFeedItem>,
                                Box<dyn std::error::Error + Send + Sync>,
                            >(Vec::new()),
                        }
                    }
                } else {
                    warn!("include_pkg_updates set but installed_filter missing; skipping updates");
                    Ok::<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>(Vec::new())
                }
            } else {
                Ok::<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>(Vec::new())
            }
        },
        async {
            if config.include_aur_comments {
                if let Some(installed) = config.installed_filter {
                    if installed.is_empty() {
                        warn!(
                            "include_aur_comments set but installed set is empty; skipping comments"
                        );
                        Ok::<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>(Vec::new())
                    } else {
                        info!(
                            "fetching AUR comments: installed_count={}, limit={}",
                            installed.len(),
                            config.limit
                        );
                        let result = fetch_installed_aur_comments(
                            installed,
                            config.limit,
                            config.seen_aur_comments,
                            config.force_emit_all,
                        )
                        .await;
                        match &result {
                            Ok(comments) => {
                                info!("AUR comments fetch completed: items={}", comments.len());
                            }
                            Err(e) => {
                                warn!(error = %e, "installed AUR comments fetch failed");
                            }
                        }
                        match result {
                            Ok(comments) => Ok(comments),
                            Err(_e) => Ok::<
                                Vec<NewsFeedItem>,
                                Box<dyn std::error::Error + Send + Sync>,
                            >(Vec::new()),
                        }
                    }
                } else {
                    warn!(
                        "include_aur_comments set but installed_filter missing; skipping comments"
                    );
                    Ok::<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>(Vec::new())
                }
            } else {
                Ok::<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>(Vec::new())
            }
        }
    )
}

/// What: Combine feed results from all sources into a single sorted vector.
///
/// Inputs:
/// - `arch_result`: Arch news fetch result.
/// - `advisories_result`: Advisories fetch result.
/// - `updates_result`: Package updates fetch result.
/// - `comments_result`: AUR comments fetch result.
/// - `sort_mode`: Sort mode for the final result.
///
/// Output:
/// - Combined and sorted vector of news feed items.
///
/// Details:
/// - Gracefully handles errors by logging warnings and continuing.
/// - Sorts items according to the specified sort mode.
fn combine_feed_results(
    arch_result: std::result::Result<Vec<NewsFeedItem>, Box<dyn std::error::Error + Send + Sync>>,
    advisories_result: std::result::Result<
        Vec<NewsFeedItem>,
        Box<dyn std::error::Error + Send + Sync>,
    >,
    updates_result: std::result::Result<
        Vec<NewsFeedItem>,
        Box<dyn std::error::Error + Send + Sync>,
    >,
    comments_result: std::result::Result<
        Vec<NewsFeedItem>,
        Box<dyn std::error::Error + Send + Sync>,
    >,
    sort_mode: NewsSortMode,
) -> Vec<NewsFeedItem> {
    let mut items: Vec<NewsFeedItem> = Vec::new();
    match arch_result {
        Ok(mut arch_items) => items.append(&mut arch_items),
        Err(e) => warn!(error = %e, "arch news fetch failed; continuing without Arch news"),
    }
    match advisories_result {
        Ok(mut adv_items) => items.append(&mut adv_items),
        Err(e) => warn!(error = %e, "advisories fetch failed; continuing without advisories"),
    }
    match updates_result {
        Ok(mut upd_items) => items.append(&mut upd_items),
        Err(e) => warn!(error = %e, "updates fetch failed; continuing without updates"),
    }
    match comments_result {
        Ok(mut cmt_items) => items.append(&mut cmt_items),
        Err(e) => warn!(error = %e, "comments fetch failed; continuing without comments"),
    }
    sort_news_items(&mut items, sort_mode);
    items
}

/// Return type for `prepare_fetch_context` function.
type PrepareFetchContextReturn<'a, HS, HV, HC> = (
    Option<String>,
    Option<HashMap<String, (String, String)>>,
    usize,
    bool,
    bool,
    bool,
    bool,
    Option<&'a HashSet<String, HS>>,
    bool,
    NewsSortMode,
    &'a mut HashMap<String, String, HV>,
    &'a mut HashMap<String, String, HC>,
    bool,
);

/// What: Prepare fetch context and calculate derived values.
///
/// Inputs:
/// - `ctx`: News feed context.
///
/// Output:
/// - Tuple of (`cutoff_date`, `updates_versions`, and extracted context fields).
///
/// Details:
/// - Extracts context fields and calculates cutoff date and update versions.
fn prepare_fetch_context<HS, HV, HC>(
    ctx: NewsFeedContext<'_, HS, HV, HC>,
) -> PrepareFetchContextReturn<'_, HS, HV, HC>
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
        max_age_days,
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
        max_age_days,
        "fetch_news_feed start"
    );

    let cutoff_date = max_age_days.and_then(|days| {
        chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(i64::from(days)))
            .map(|dt| dt.format("%Y-%m-%d").to_string())
    });
    let updates_versions = if force_emit_all {
        load_update_versions(updates_list_path.as_ref())
    } else {
        None
    };

    (
        cutoff_date,
        updates_versions,
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
    )
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
            items.sort_by(|a, b| {
                a.title
                    .to_lowercase()
                    .cmp(&b.title.to_lowercase())
                    .then(b.date.cmp(&a.date))
            });
        }
        NewsSortMode::SourceThenTitle => items.sort_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then(b.date.cmp(&a.date))
                .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        NewsSortMode::SeverityThenDate => items.sort_by(|a, b| {
            let sa = severity_rank(a.severity);
            let sb = severity_rank(b.severity);
            sb.cmp(&sa)
                .then(b.date.cmp(&a.date))
                .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        NewsSortMode::UnreadThenDate => {
            // Fetch pipeline lacks read-state context; fall back to newest-first.
            items.sort_by(|a, b| b.date.cmp(&a.date));
        }
    }
}

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
    let (
        cutoff_date,
        updates_versions,
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
    ) = prepare_fetch_context(ctx);

    info!(
        "starting fetch: arch_news={include_arch_news}, advisories={include_advisories}, pkg_updates={include_pkg_updates}, aur_comments={include_aur_comments}"
    );
    rate_limit::reset_archlinux_backoff();

    // Fetch ALL sources in parallel for best responsiveness:
    // - Fast sources (AUR comments, package updates) run in parallel and complete quickly
    // - Slow sources (arch news, advisories from archlinux.org) run sequentially with each other
    //   but IN PARALLEL with the fast sources, so they don't block everything
    let ((updates_result, comments_result), (arch_result, advisories_result)) = tokio::join!(
        fetch_fast_sources(FastSourcesConfig {
            include_pkg_updates,
            include_aur_comments,
            installed_filter,
            limit,
            seen_pkg_versions,
            seen_aur_comments,
            force_emit_all,
            updates_versions: updates_versions.as_ref(),
        }),
        fetch_slow_sources(
            include_arch_news,
            include_advisories,
            limit,
            installed_filter,
            installed_only,
            cutoff_date.as_deref(),
        )
    );
    info!("fetch completed, combining results...");

    let items = combine_feed_results(
        arch_result,
        advisories_result,
        updates_result,
        comments_result,
        sort_mode,
    );
    info!(
        total = items.len(),
        arch = items
            .iter()
            .filter(|i| matches!(i.source, crate::state::types::NewsFeedSource::ArchNews))
            .count(),
        advisories = items
            .iter()
            .filter(|i| matches!(
                i.source,
                crate::state::types::NewsFeedSource::SecurityAdvisory
            ))
            .count(),
        updates = items
            .iter()
            .filter(|i| {
                matches!(
                    i.source,
                    crate::state::types::NewsFeedSource::InstalledPackageUpdate
                        | crate::state::types::NewsFeedSource::AurPackageUpdate
                )
            })
            .count(),
        aur_comments = items
            .iter()
            .filter(|i| matches!(i.source, crate::state::types::NewsFeedSource::AurComment))
            .count(),
        "fetch_news_feed success"
    );
    Ok(items)
}

/// Limit for continuation fetching (effectively unlimited).
const CONTINUATION_LIMIT: usize = 1000;

/// What: Fetch continuation items for background loading after initial batch.
///
/// Inputs:
/// - `installed`: Set of installed package names.
/// - `initial_ids`: IDs of items already fetched in initial batch.
///
/// Output:
/// - `Ok(Vec<NewsFeedItem>)`: Additional items not in initial batch.
///
/// # Errors
/// - Network errors when fetching from any source.
/// - Parsing errors from upstream feeds.
///
/// Details:
/// - Fetches items from all sources with a high limit (1000).
/// - Filters out items already in `initial_ids`.
/// - Used by background continuation worker to stream additional items to UI.
pub async fn fetch_continuation_items<HS, HI>(
    installed: &HashSet<String, HS>,
    initial_ids: &HashSet<String, HI>,
) -> Result<Vec<NewsFeedItem>>
where
    HS: std::hash::BuildHasher + Send + Sync + 'static,
    HI: std::hash::BuildHasher + Send + Sync,
{
    use crate::state::types::NewsFeedSource;

    info!(
        installed_count = installed.len(),
        initial_count = initial_ids.len(),
        "starting continuation fetch"
    );

    // Fetch from all sources in parallel
    let ((updates_result, comments_result), (arch_result, advisories_result)) = tokio::join!(
        async {
            // Package updates - use fresh seen maps (continuation doesn't track seen state)
            let mut seen_versions: HashMap<String, String> = HashMap::new();
            let mut seen_aur_comments: HashMap<String, String> = HashMap::new();
            let updates = fetch_installed_updates(
                installed,
                CONTINUATION_LIMIT,
                &mut seen_versions,
                true, // force_emit_all
                None,
            )
            .await;
            let comments = fetch_installed_aur_comments(
                installed,
                CONTINUATION_LIMIT,
                &mut seen_aur_comments,
                true, // force_emit_all
            )
            .await;
            (updates, comments)
        },
        fetch_slow_sources(
            true, // include_arch_news
            true, // include_advisories
            CONTINUATION_LIMIT,
            Some(installed),
            false, // installed_only
            None,  // cutoff_date
        )
    );

    let mut items = Vec::new();

    // Add Arch news (filter out already-sent items)
    if let Ok(arch_items) = arch_result {
        for item in arch_items {
            if !initial_ids.contains(&item.id) {
                items.push(item);
            }
        }
    }

    // Add advisories (filter out already-sent items)
    if let Ok(adv_items) = advisories_result {
        for item in adv_items {
            if !initial_ids.contains(&item.id) {
                items.push(item);
            }
        }
    }

    // Add package updates (filter out already-sent items)
    if let Ok(upd_items) = updates_result {
        for item in upd_items {
            if !initial_ids.contains(&item.id) {
                items.push(item);
            }
        }
    }

    // Add AUR comments (filter out already-sent items)
    if let Ok(comment_items) = comments_result {
        for item in comment_items {
            if !initial_ids.contains(&item.id) {
                items.push(item);
            }
        }
    }

    // Sort by date descending
    sort_news_items(&mut items, NewsSortMode::DateDesc);

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
            .filter(|i| matches!(
                i.source,
                NewsFeedSource::InstalledPackageUpdate | NewsFeedSource::AurPackageUpdate
            ))
            .count(),
        "continuation fetch complete"
    );

    Ok(items)
}

// Re-export public functions from submodules
pub use rate_limit::{
    check_circuit_breaker, extract_endpoint_pattern, extract_retry_after_from_error,
    increase_archlinux_backoff, rate_limit_archlinux, record_circuit_breaker_outcome,
    reset_archlinux_backoff, take_network_error,
};
pub use updates::{
    get_aur_json_changes, get_official_json_changes, load_official_json_cache,
    official_json_cache_path,
};

#[cfg(test)]
mod tests;
