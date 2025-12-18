use std::collections::HashSet;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

use crossterm::event::Event as CEvent;
use rand::Rng;
use tokio::{
    sync::mpsc,
    sync::oneshot,
    time::{Duration, sleep},
};

use crate::index as pkgindex;
use crate::sources;
use crate::state::ArchStatusColor;
use crate::state::types::{NewsFeedSource, NewsSortMode};

/// What: Spawns Arch status worker that fetches status once at startup and periodically.
///
/// Inputs:
/// - `status_tx`: Channel sender for Arch status updates
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Fetches Arch status text once at startup
/// - Periodically refreshes Arch status every 120 seconds
fn spawn_status_worker(status_tx: &mpsc::UnboundedSender<(String, ArchStatusColor)>) {
    // Fetch Arch status text once at startup
    let status_tx_once = status_tx.clone();
    tokio::spawn(async move {
        if let Ok((txt, color)) = sources::fetch_arch_status_text().await {
            let _ = status_tx_once.send((txt, color));
        }
    });

    // Periodically refresh Arch status every 120 seconds
    let status_tx_periodic = status_tx.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(120)).await;
            if let Ok((txt, color)) = sources::fetch_arch_status_text().await {
                let _ = status_tx_periodic.send((txt, color));
            }
        }
    });
}

/// What: Ensures installed packages set is populated, refreshing caches if needed.
///
/// Inputs:
/// - `installed`: Initial set of installed package names
///
/// Output:
/// - `HashSet<String>` with installed package names (refreshed if needed)
///
/// Details:
/// - If the initial set is empty, refreshes installed and explicit caches
/// - Returns refreshed set if available, otherwise returns original set
async fn ensure_installed_set(installed: HashSet<String>) -> HashSet<String> {
    if installed.is_empty() {
        crate::index::refresh_installed_cache().await;
        crate::index::refresh_explicit_cache(crate::state::InstalledPackagesMode::AllExplicit)
            .await;
        let refreshed: HashSet<String> = pkgindex::explicit_names().into_iter().collect();
        if !refreshed.is_empty() {
            return refreshed;
        }
    }
    installed
}

/// What: Filters news feed items by source type based on startup news preferences.
///
/// Inputs:
/// - `feed`: Vector of news feed items to filter
/// - `prefs`: Theme settings containing startup news preferences
///
/// Output:
/// - Filtered vector of news feed items
///
/// Details:
/// - Filters items based on whether each source type is enabled in preferences
fn filter_news_by_source(
    feed: Vec<crate::state::types::NewsFeedItem>,
    prefs: &crate::theme::Settings,
) -> Vec<crate::state::types::NewsFeedItem> {
    feed.into_iter()
        .filter(|item| match item.source {
            crate::state::types::NewsFeedSource::ArchNews => prefs.startup_news_show_arch_news,
            crate::state::types::NewsFeedSource::SecurityAdvisory => {
                prefs.startup_news_show_advisories
            }
            crate::state::types::NewsFeedSource::InstalledPackageUpdate => {
                prefs.startup_news_show_pkg_updates
            }
            crate::state::types::NewsFeedSource::AurPackageUpdate => {
                prefs.startup_news_show_aur_updates
            }
            crate::state::types::NewsFeedSource::AurComment => prefs.startup_news_show_aur_comments,
        })
        .collect()
}

/// What: Filters news feed items by maximum age in days.
///
/// Inputs:
/// - `feed`: Vector of news feed items to filter
/// - `max_age_days`: Optional maximum age in days
///
/// Output:
/// - Filtered vector of news feed items
///
/// Details:
/// - If `max_age_days` is Some, filters out items older than the cutoff date
/// - If `max_age_days` is None, returns all items unchanged
fn filter_news_by_age(
    feed: Vec<crate::state::types::NewsFeedItem>,
    max_age_days: Option<u32>,
) -> Vec<crate::state::types::NewsFeedItem> {
    if let Some(max_days) = max_age_days {
        let cutoff_date = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(i64::from(max_days)))
            .map(|dt| dt.format("%Y-%m-%d").to_string());
        #[allow(clippy::unnecessary_map_or)]
        feed.into_iter()
            .filter(|item| {
                cutoff_date
                    .as_ref()
                    .map_or(true, |cutoff| &item.date >= cutoff)
            })
            .collect()
    } else {
        feed
    }
}

/// What: Filters out already-read news items by URL.
///
/// Inputs:
/// - `feed`: Vector of news feed items to filter
/// - `read_urls`: Set of already-read news URLs
///
/// Output:
/// - Filtered vector containing only unread items
///
/// Details:
/// - Removes items whose URL is in the `read_urls` set
fn filter_unread_news(
    feed: Vec<crate::state::types::NewsFeedItem>,
    read_urls: &HashSet<String>,
) -> Vec<crate::state::types::NewsFeedItem> {
    #[allow(clippy::unnecessary_map_or)]
    feed.into_iter()
        .filter(|item| {
            item.url
                .as_ref()
                .map_or(true, |url| !read_urls.contains(url))
        })
        .collect()
}

/// What: Spawns startup news worker that fetches and filters news items for startup popup.
///
/// Inputs:
/// - `news_tx`: Channel sender for startup news updates
/// - `news_read_urls`: Set of already-read news URLs
/// - `news_seen_pkg_versions`: Map of seen package versions
/// - `news_seen_aur_comments`: Map of seen AUR comments
/// - `last_startup_timestamp`: Previous TUI startup time for incremental updates
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Fetches news items based on startup news preferences
/// - Filters by source type, max age, and read status
/// - Sends filtered items to the news channel
fn spawn_startup_news_worker(
    news_tx: &mpsc::UnboundedSender<Vec<crate::state::types::NewsFeedItem>>,
    news_read_urls: &HashSet<String>,
    news_seen_pkg_versions: &std::collections::HashMap<String, String>,
    news_seen_aur_comments: &std::collections::HashMap<String, String>,
    last_startup_timestamp: Option<&str>,
    completion_tx: Option<oneshot::Sender<()>>,
) {
    let prefs = crate::theme::settings();
    if !prefs.startup_news_configured {
        // If startup news is not configured, signal completion immediately
        if let Some(tx) = completion_tx {
            let _ = tx.send(());
        }
        return;
    }

    let news_tx_once = news_tx.clone();
    let read_urls = news_read_urls.clone();
    let installed: HashSet<String> = pkgindex::explicit_names().into_iter().collect();
    let mut seen_versions = news_seen_pkg_versions.clone();
    let mut seen_aur_comments = news_seen_aur_comments.clone();
    let last_startup = last_startup_timestamp.map(str::to_owned);
    tracing::info!(
        read_urls = read_urls.len(),
        last_startup = ?last_startup,
        "queueing startup news fetch (startup)"
    );
    tokio::spawn(async move {
        // Use random jitter (0-500ms) before startup news fetch
        // Keep this short since the startup popup should appear quickly
        let jitter_ms = rand::rng().random_range(0..=500_u64);
        if jitter_ms > 0 {
            tracing::info!(jitter_ms, "staggering startup news fetch");
            tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
        }
        tracing::info!("startup news fetch task started");
        let optimized_max_age = sources::optimize_max_age_for_startup(
            last_startup.as_deref(),
            prefs.startup_news_max_age_days,
        );
        let installed_set = ensure_installed_set(installed).await;
        let include_pkg_updates =
            prefs.startup_news_show_pkg_updates || prefs.startup_news_show_aur_updates;
        #[allow(clippy::items_after_statements)]
        const STARTUP_NEWS_LIMIT: usize = 20;
        let updates_limit =
            if prefs.startup_news_show_pkg_updates && prefs.startup_news_show_aur_updates {
                STARTUP_NEWS_LIMIT * 2
            } else {
                STARTUP_NEWS_LIMIT
            };
        let ctx = sources::NewsFeedContext {
            force_emit_all: true,
            updates_list_path: Some(crate::theme::lists_dir().join("available_updates.txt")),
            limit: updates_limit,
            include_arch_news: prefs.startup_news_show_arch_news,
            include_advisories: prefs.startup_news_show_advisories,
            include_pkg_updates,
            include_aur_comments: prefs.startup_news_show_aur_comments,
            installed_filter: Some(&installed_set),
            installed_only: false,
            sort_mode: NewsSortMode::DateDesc,
            seen_pkg_versions: &mut seen_versions,
            seen_aur_comments: &mut seen_aur_comments,
            max_age_days: optimized_max_age,
        };
        tracing::info!(
            limit = updates_limit,
            include_arch_news = prefs.startup_news_show_arch_news,
            include_advisories = prefs.startup_news_show_advisories,
            include_pkg_updates,
            include_aur_comments = prefs.startup_news_show_aur_comments,
            configured_max_age = ?prefs.startup_news_max_age_days,
            optimized_max_age = ?optimized_max_age,
            installed_count = installed_set.len(),
            "starting startup news fetch"
        );
        match sources::fetch_news_feed(ctx).await {
            Ok(feed) => {
                tracing::info!(
                    total_items = feed.len(),
                    "startup news fetch completed successfully"
                );
                let source_filtered = filter_news_by_source(feed, &prefs);
                let filtered = filter_news_by_age(source_filtered, prefs.startup_news_max_age_days);
                let unread = filter_unread_news(filtered, &read_urls);
                tracing::info!(
                    unread_count = unread.len(),
                    "sending startup news items to channel"
                );
                match news_tx_once.send(unread) {
                    Ok(()) => {
                        tracing::info!("startup news items sent to channel successfully");
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "failed to send startup news items to channel (receiver dropped?)"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "startup news fetch failed");
                tracing::info!("sending empty array to clear loading flag after fetch error");
                let _ = news_tx_once.send(Vec::new());
            }
        }
        // Signal completion to allow aggregated feed fetch to proceed
        if let Some(tx) = completion_tx {
            let _ = tx.send(());
        }
    });
}

/// What: Spawns aggregated news feed worker that fetches combined news feed.
///
/// Inputs:
/// - `news_feed_tx`: Channel sender for aggregated news feed
/// - `news_seen_pkg_versions`: Map of seen package versions
/// - `news_seen_aur_comments`: Map of seen AUR comments
/// - `completion_rx`: Optional oneshot receiver to wait for startup news fetch completion
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Fetches aggregated news feed (Arch news + security advisories + package updates + AUR comments)
/// - Sends feed payload to the news feed channel
/// - Waits for startup news fetch to complete before starting to prevent concurrent archlinux.org requests
fn spawn_aggregated_news_feed_worker(
    news_feed_tx: &mpsc::UnboundedSender<crate::state::types::NewsFeedPayload>,
    news_seen_pkg_versions: &std::collections::HashMap<String, String>,
    news_seen_aur_comments: &std::collections::HashMap<String, String>,
    completion_rx: Option<oneshot::Receiver<()>>,
) {
    let news_feed_tx_once = news_feed_tx.clone();
    let installed: HashSet<String> = pkgindex::explicit_names().into_iter().collect();
    let mut seen_versions = news_seen_pkg_versions.clone();
    let mut seen_aur_comments = news_seen_aur_comments.clone();
    tracing::info!(
        installed_names = installed.len(),
        "queueing combined news feed fetch (startup)"
    );
    tokio::spawn(async move {
        // Wait for startup news fetch to complete before starting aggregated feed fetch
        // This prevents concurrent requests to archlinux.org which can cause rate limiting/blocking
        if let Some(rx) = completion_rx {
            tracing::info!(
                "waiting for startup news fetch to complete before starting aggregated feed fetch"
            );
            let _ = rx.await; // Wait for startup fetch completion signal
            // Add a small additional delay after startup fetch completes to ensure clean separation
            let additional_delay_ms = rand::rng().random_range(500..=1500_u64);
            tracing::info!(
                additional_delay_ms,
                "additional delay after startup fetch completion"
            );
            tokio::time::sleep(Duration::from_millis(additional_delay_ms)).await;
        } else {
            // Fallback: use fixed delay if no completion signal is provided
            // This should not happen in normal operation, but provides safety
            let base_delay_ms = 10000_u64; // Increased to 10 seconds as fallback
            let jitter_ms = rand::rng().random_range(0..=2000_u64);
            let stagger_ms = base_delay_ms + jitter_ms;
            tracing::warn!(
                stagger_ms,
                "no completion signal available, using fallback delay for aggregated feed fetch"
            );
            tokio::time::sleep(Duration::from_millis(stagger_ms)).await;
        }
        let installed_set = ensure_installed_set(installed).await;
        let ctx = sources::NewsFeedContext {
            force_emit_all: true,
            updates_list_path: Some(crate::theme::lists_dir().join("available_updates.txt")),
            limit: 50,
            include_arch_news: true,
            include_advisories: true,
            include_pkg_updates: true,
            include_aur_comments: true,
            installed_filter: Some(&installed_set),
            installed_only: false,
            sort_mode: NewsSortMode::DateDesc,
            seen_pkg_versions: &mut seen_versions,
            seen_aur_comments: &mut seen_aur_comments,
            max_age_days: None, // Main feed doesn't use date filtering
        };
        match sources::fetch_news_feed(ctx).await {
            Ok(feed) => {
                let arch_ct = feed
                    .iter()
                    .filter(|i| matches!(i.source, NewsFeedSource::ArchNews))
                    .count();
                let adv_ct = feed
                    .iter()
                    .filter(|i| matches!(i.source, NewsFeedSource::SecurityAdvisory))
                    .count();
                tracing::info!(
                    total = feed.len(),
                    arch = arch_ct,
                    advisories = adv_ct,
                    installed_names = installed_set.len(),
                    "news feed fetched"
                );
                if feed.is_empty() {
                    tracing::warn!(
                        installed_names = installed_set.len(),
                        "news feed is empty after fetch"
                    );
                }
                let payload = crate::state::types::NewsFeedPayload {
                    items: feed,
                    seen_pkg_versions: seen_versions,
                    seen_aur_comments,
                };
                if let Err(e) = news_feed_tx_once.send(payload) {
                    tracing::warn!(error = ?e, "failed to send news feed to channel");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to fetch news feed");
            }
        }
    });
}

/// What: Spawns announcement worker that fetches remote announcement from GitHub Gist.
///
/// Inputs:
/// - `announcement_tx`: Channel sender for remote announcement updates
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Fetches remote announcement from hardcoded Gist URL
/// - Sends announcement to channel if successfully fetched and parsed
fn spawn_announcement_worker(
    announcement_tx: &mpsc::UnboundedSender<crate::announcements::RemoteAnnouncement>,
) {
    let announcement_tx_once = announcement_tx.clone();
    // Hardcoded Gist URL for remote announcements
    let url = "https://gist.githubusercontent.com/Firstp1ck/d2e6016b8d7a90f813a582078208e9bd/raw/announcement.json".to_string();
    tokio::spawn(async move {
        tracing::info!(url = %url, "fetching remote announcement");
        match reqwest::get(&url).await {
            Ok(response) => {
                tracing::debug!(
                    status = response.status().as_u16(),
                    "announcement fetch response received"
                );
                match response
                    .json::<crate::announcements::RemoteAnnouncement>()
                    .await
                {
                    Ok(json) => {
                        tracing::info!(id = %json.id, "announcement fetched successfully");
                        let _ = announcement_tx_once.send(json);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to parse announcement JSON");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "failed to fetch announcement");
            }
        }
    });
}

/// What: Spawns index update worker for Windows platform.
///
/// Inputs:
/// - `official_index_path`: Path to official package index
/// - `net_err_tx`: Channel sender for network errors
/// - `index_notify_tx`: Channel sender for index update notifications
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Windows-specific: saves mirrors and builds index via Arch API
#[cfg(windows)]
fn spawn_index_update_worker(
    official_index_path: &std::path::Path,
    net_err_tx: &mpsc::UnboundedSender<String>,
    index_notify_tx: &mpsc::UnboundedSender<()>,
) {
    let repo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repository");
    let index_path = official_index_path.to_path_buf();
    let net_err = net_err_tx.clone();
    let index_notify = index_notify_tx.clone();
    tokio::spawn(async move {
        crate::index::refresh_windows_mirrors_and_index(
            index_path,
            repo_dir,
            net_err,
            index_notify,
        )
        .await;
    });
}

/// What: Spawns index update worker for non-Windows platforms.
///
/// Inputs:
/// - `headless`: When `true`, skip index update
/// - `official_index_path`: Path to official package index
/// - `net_err_tx`: Channel sender for network errors
/// - `index_notify_tx`: Channel sender for index update notifications
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Updates package index in background
/// - Skips in headless mode to avoid slow network/disk operations
#[cfg(not(windows))]
fn spawn_index_update_worker(
    headless: bool,
    official_index_path: &std::path::Path,
    net_err_tx: &mpsc::UnboundedSender<String>,
    index_notify_tx: &mpsc::UnboundedSender<()>,
) {
    if headless {
        return;
    }
    let index_path = official_index_path.to_path_buf();
    let net_err = net_err_tx.clone();
    let index_notify = index_notify_tx.clone();
    tokio::spawn(async move {
        pkgindex::update_in_background(index_path, net_err, index_notify).await;
    });
}

/// What: Spawns cache refresh worker that refreshes pacman caches.
///
/// Inputs:
/// - `installed_packages_mode`: Filter mode for installed packages
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Refreshes installed and explicit package caches
/// - Uses the configured installed packages mode
fn spawn_cache_refresh_worker(installed_packages_mode: crate::state::InstalledPackagesMode) {
    let mode = installed_packages_mode;
    tokio::spawn(async move {
        pkgindex::refresh_installed_cache().await;
        pkgindex::refresh_explicit_cache(mode).await;
    });
}

/// What: Spawns periodic updates worker that checks for package updates at intervals.
///
/// Inputs:
/// - `updates_tx`: Channel sender for package updates
/// - `updates_refresh_interval`: Refresh interval in seconds
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Checks for updates once at startup
/// - Periodically refreshes updates list at configured interval
fn spawn_periodic_updates_worker(
    updates_tx: &mpsc::UnboundedSender<(usize, Vec<String>)>,
    updates_refresh_interval: u64,
) {
    spawn_updates_worker(updates_tx.clone());

    let updates_tx_periodic = updates_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(updates_refresh_interval));
        // Skip the first tick to avoid immediate refresh after startup
        interval.tick().await;
        loop {
            interval.tick().await;
            spawn_updates_worker(updates_tx_periodic.clone());
        }
    });
}

/// What: Spawns tick worker that sends tick events every 200ms.
///
/// Inputs:
/// - `tick_tx`: Channel sender for tick events
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Sends tick events every 200ms to drive UI updates
fn spawn_tick_worker(tick_tx: &mpsc::UnboundedSender<()>) {
    let tick_tx_bg = tick_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            interval.tick().await;
            let _ = tick_tx_bg.send(());
        }
    });
}

/// What: Spawns faillock check worker that triggers tick events every minute.
///
/// Inputs:
/// - `tick_tx`: Channel sender for tick events
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Triggers tick events every 60 seconds to update faillock status in UI
fn spawn_faillock_worker(tick_tx: &mpsc::UnboundedSender<()>) {
    let faillock_tx = tick_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        // Skip the first tick to avoid immediate check after startup
        interval.tick().await;
        loop {
            interval.tick().await;
            // Trigger a tick to update faillock status in the UI
            let _ = faillock_tx.send(());
        }
    });
}

/// What: Spawn background workers for status, news, announcements, and tick events.
///
/// Inputs:
/// - `headless`: When `true`, skip terminal-dependent operations
/// - `status_tx`: Channel sender for Arch status updates
/// - `news_tx`: Channel sender for Arch news updates
/// - `news_feed_tx`: Channel sender for aggregated news feed (Arch news + advisories)
/// - `announcement_tx`: Channel sender for remote announcement updates
/// - `tick_tx`: Channel sender for tick events
/// - `news_read_urls`: Set of already-read news URLs
/// - `official_index_path`: Path to official package index
/// - `net_err_tx`: Channel sender for network errors
/// - `index_notify_tx`: Channel sender for index update notifications
/// - `updates_tx`: Channel sender for package updates
/// - `updates_refresh_interval`: Refresh interval in seconds for pacman -Qu and AUR helper checks
/// - `installed_packages_mode`: Filter mode for installed packages (leaf only vs all explicit)
/// - `get_announcement`: Whether to fetch remote announcements from GitHub Gist
/// - `last_startup_timestamp`: Previous TUI startup time (`YYYYMMDD:HHMMSS`) for incremental updates
///
/// Details:
/// - Fetches Arch status text once at startup and periodically every 120 seconds
/// - Fetches Arch news once at startup, filtering out already-read items
/// - Fetches remote announcement once at startup if URL is configured
/// - Updates package index in background (Windows vs non-Windows handling)
/// - Refreshes pacman caches (installed, explicit) using the configured installed packages mode
/// - Spawns tick worker that sends events every 200ms
/// - Checks for available package updates once at startup and periodically at configured interval
#[allow(clippy::too_many_arguments)]
pub fn spawn_auxiliary_workers(
    headless: bool,
    status_tx: &mpsc::UnboundedSender<(String, ArchStatusColor)>,
    news_tx: &mpsc::UnboundedSender<Vec<crate::state::types::NewsFeedItem>>,
    news_feed_tx: &mpsc::UnboundedSender<crate::state::types::NewsFeedPayload>,
    announcement_tx: &mpsc::UnboundedSender<crate::announcements::RemoteAnnouncement>,
    tick_tx: &mpsc::UnboundedSender<()>,
    news_read_urls: &std::collections::HashSet<String>,
    news_seen_pkg_versions: &std::collections::HashMap<String, String>,
    news_seen_aur_comments: &std::collections::HashMap<String, String>,
    official_index_path: &std::path::Path,
    net_err_tx: &mpsc::UnboundedSender<String>,
    index_notify_tx: &mpsc::UnboundedSender<()>,
    updates_tx: &mpsc::UnboundedSender<(usize, Vec<String>)>,
    updates_refresh_interval: u64,
    installed_packages_mode: crate::state::InstalledPackagesMode,
    get_announcement: bool,
    last_startup_timestamp: Option<&str>,
) {
    tracing::info!(
        headless,
        get_announcement,
        updates_refresh_interval,
        "auxiliary workers starting"
    );

    // Spawn status worker (skip in headless mode)
    if !headless {
        spawn_status_worker(status_tx);
    }

    // Handle news workers
    if headless {
        tracing::info!("headless mode: skipping news/advisory fetch and announcements");
        // In headless mode, send empty array to news channel to ensure event loop doesn't hang
        let news_tx_headless = news_tx.clone();
        tokio::spawn(async move {
            tracing::debug!("headless mode: sending empty news array to clear any pending waits");
            let _ = news_tx_headless.send(Vec::new());
        });
    } else {
        // Create a oneshot channel to coordinate startup and aggregated news fetches
        // This prevents concurrent requests to archlinux.org which can cause rate limiting/blocking
        let (completion_tx, completion_rx) = oneshot::channel();
        spawn_startup_news_worker(
            news_tx,
            news_read_urls,
            news_seen_pkg_versions,
            news_seen_aur_comments,
            last_startup_timestamp,
            Some(completion_tx),
        );
        spawn_aggregated_news_feed_worker(
            news_feed_tx,
            news_seen_pkg_versions,
            news_seen_aur_comments,
            Some(completion_rx),
        );
    }

    // Spawn announcement worker (skip in headless mode)
    if !headless && get_announcement {
        spawn_announcement_worker(announcement_tx);
    }

    // Spawn index update worker (platform-specific)
    #[cfg(windows)]
    spawn_index_update_worker(official_index_path, net_err_tx, index_notify_tx);
    #[cfg(not(windows))]
    spawn_index_update_worker(headless, official_index_path, net_err_tx, index_notify_tx);

    // Spawn cache refresh worker (skip in headless mode)
    if !headless {
        spawn_cache_refresh_worker(installed_packages_mode);
    }

    // Spawn periodic updates worker (skip in headless mode)
    if !headless {
        spawn_periodic_updates_worker(updates_tx, updates_refresh_interval);
    }

    // Spawn tick worker (always runs)
    spawn_tick_worker(tick_tx);

    // Spawn faillock worker (skip in headless mode)
    if !headless {
        spawn_faillock_worker(tick_tx);
    }
}

/// What: Check which AUR helper is available (paru or yay).
///
/// Output:
/// - Tuple of (`has_paru`, `has_yay`, `helper_name`)
fn check_aur_helper() -> (bool, bool, &'static str) {
    use std::process::{Command, Stdio};

    let has_paru = Command::new("paru")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok();

    let has_yay = if has_paru {
        false
    } else {
        Command::new("yay")
            .args(["--version"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .is_ok()
    };

    let helper = if has_paru { "paru" } else { "yay" };
    if has_paru || has_yay {
        tracing::debug!("Using {} to check for AUR updates", helper);
    }

    (has_paru, has_yay, helper)
}

/// What: Check if fakeroot is available on the system.
///
/// Output:
/// - `true` if fakeroot is available, `false` otherwise
///
/// Details:
/// - Fakeroot is required to sync a temporary pacman database without root
#[cfg(not(target_os = "windows"))]
fn has_fakeroot() -> bool {
    use std::process::{Command, Stdio};

    Command::new("fakeroot")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

/// What: Check if checkupdates is available on the system.
///
/// Output:
/// - `true` if checkupdates is available, `false` otherwise
///
/// Details:
/// - checkupdates (from pacman-contrib) can check for updates without root
/// - It automatically syncs the database and doesn't require fakeroot
#[cfg(not(target_os = "windows"))]
fn has_checkupdates() -> bool {
    use std::process::{Command, Stdio};

    Command::new("checkupdates")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

/// What: Get the current user's UID by reading /proc/self/status.
///
/// Output:
/// - `Some(u32)` with the UID if successful
/// - `None` if unable to read the UID
///
/// Details:
/// - Reads /proc/self/status and parses the Uid line
/// - Returns the real UID (first value on the Uid line)
#[cfg(not(target_os = "windows"))]
fn get_uid() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("Uid:") {
            // Format: "Uid:\treal\teffective\tsaved\tfs"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse().ok();
            }
        }
    }
    None
}

/// What: Set up a temporary pacman database directory for safe update checks.
///
/// Output:
/// - `Some(PathBuf)` with the temp database path if setup succeeds
/// - `None` if setup fails
///
/// Details:
/// - Creates `/tmp/pacsea-db-{UID}/` directory
/// - Creates a symlink from `local` to `/var/lib/pacman/local`
/// - The symlink allows pacman to know which packages are installed
/// - Directory is kept for reuse across subsequent checks
#[cfg(not(target_os = "windows"))]
fn setup_temp_db() -> Option<std::path::PathBuf> {
    // Get current user ID
    let uid = get_uid()?;
    let temp_db = std::path::PathBuf::from(format!("/tmp/pacsea-db-{uid}"));

    // Create directory if needed
    if let Err(e) = std::fs::create_dir_all(&temp_db) {
        tracing::warn!("Failed to create temp database directory: {}", e);
        return None;
    }

    // Create symlink to local database (skip if exists)
    let local_link = temp_db.join("local");
    if !local_link.exists()
        && let Err(e) = std::os::unix::fs::symlink("/var/lib/pacman/local", &local_link)
    {
        tracing::warn!("Failed to create symlink to local database: {}", e);
        return None;
    }

    Some(temp_db)
}

/// What: Sync the temporary pacman database with remote repositories.
///
/// Inputs:
/// - `temp_db`: Path to the temporary database directory
///
/// Output:
/// - `true` if sync succeeds, `false` otherwise
///
/// Details:
/// - Uses fakeroot to run `pacman -Sy` without root privileges
/// - Syncs only the temporary database, not the system database
/// - Uses `--logfile /dev/null` to prevent log file creation
/// - Logs stderr on failure to help diagnose sync issues
#[cfg(not(target_os = "windows"))]
fn sync_temp_db(temp_db: &std::path::Path) -> bool {
    use std::process::{Command, Stdio};

    let output = Command::new("fakeroot")
        .args(["--", "pacman", "-Sy", "--dbpath"])
        .arg(temp_db)
        .args(["--logfile", "/dev/null"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            // Log stderr to help diagnose sync failures
            let stderr = String::from_utf8_lossy(&o.stderr);
            if !stderr.trim().is_empty() {
                tracing::warn!(
                    "Temp database sync failed (exit code: {:?}): {}",
                    o.status.code(),
                    stderr.trim()
                );
            }
            false
        }
        Err(e) => {
            tracing::warn!("Failed to execute fakeroot pacman -Sy: {}", e);
            false
        }
    }
}

/// What: Parse packages from pacman -Qu output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `old_version`, `new_version`) tuples
///
/// Details:
/// - Parses `"package-name old_version -> new_version"` format
fn parse_checkupdates(output: &[u8]) -> Vec<(String, String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package-name old_version -> new_version" format
                trimmed.find(" -> ").and_then(|arrow_pos| {
                    let before_arrow = &trimmed[..arrow_pos];
                    let after_arrow = &trimmed[arrow_pos + 4..];
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let old_version = parts[1..].join(" "); // In case version has spaces
                        let new_version = after_arrow.trim().to_string();
                        Some((name, old_version, new_version))
                    } else {
                        None
                    }
                })
            }
        })
        .collect()
}

/// What: Parse packages from checkupdates output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `new_version`) tuples
///
/// Details:
/// - Parses "package-name version" format (checkupdates only shows new version)
/// - Old version must be retrieved separately from installed packages
fn parse_checkupdates_tool(output: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package-name version" format
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0].to_string();
                    let new_version = parts[1..].join(" "); // In case version has spaces
                    Some((name, new_version))
                } else {
                    None
                }
            }
        })
        .collect()
}

/// What: Get installed version of a package.
///
/// Inputs:
/// - `package_name`: Name of the package
///
/// Output:
/// - `Some(version)` if package is installed, `None` otherwise
///
/// Details:
/// - Uses `pacman -Q` to get the installed version
fn get_installed_version(package_name: &str) -> Option<String> {
    use std::process::{Command, Stdio};

    let output = Command::new("pacman")
        .args(["-Q", package_name])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        // Format: "package-name version"
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 2 {
            Some(parts[1..].join(" "))
        } else {
            None
        }
    } else {
        None
    }
}

/// What: Parse packages from -Qua output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `old_version`, `new_version`) tuples
///
/// Details:
/// - Parses "package old -> new" format
fn parse_qua(output: &[u8]) -> Vec<(String, String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package old -> new" format
                trimmed.find(" -> ").and_then(|arrow_pos| {
                    let before_arrow = &trimmed[..arrow_pos];
                    let after_arrow = &trimmed[arrow_pos + 4..];
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let old_version = parts[1..].join(" "); // In case version has spaces
                        let new_version = after_arrow.trim().to_string();
                        Some((name, old_version, new_version))
                    } else {
                        None
                    }
                })
            }
        })
        .collect()
}

/// What: Process pacman -Qu or checkupdates output and add packages to collections.
///
/// Inputs:
/// - `output`: Command output result
/// - `is_checkupdates_tool`: `true` if output is from checkupdates tool, `false` if from pacman -Qu
/// - `packages_map`: Mutable `HashMap` to store formatted package strings
/// - `packages_set`: Mutable `HashSet` to track unique package names
fn process_checkupdates_output(
    output: Result<std::process::Output, std::io::Error>,
    is_checkupdates_tool: bool,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
) {
    match output {
        Ok(output) => {
            let exit_code = output.status.code();
            if output.status.success() {
                if is_checkupdates_tool {
                    // Parse checkupdates output (package-name version format)
                    let packages = parse_checkupdates_tool(&output.stdout);
                    let count = packages.len();

                    for (name, new_version) in packages {
                        // Get old version from installed packages
                        let old_version =
                            get_installed_version(&name).unwrap_or_else(|| "unknown".to_string());
                        // Format: "name - old_version -> name - new_version"
                        let formatted = format!("{name} - {old_version} -> {name} - {new_version}");
                        packages_map.insert(name.clone(), formatted);
                        packages_set.insert(name);
                    }

                    tracing::debug!(
                        "checkupdates completed successfully (exit code: {:?}): found {} packages from official repos",
                        exit_code,
                        count
                    );
                } else {
                    // Parse pacman -Qu output (package-name old_version -> new_version format)
                    let packages = parse_checkupdates(&output.stdout);
                    let count = packages.len();

                    for (name, old_version, new_version) in packages {
                        // Format: "name - old_version -> name - new_version"
                        let formatted = format!("{name} - {old_version} -> {name} - {new_version}");
                        packages_map.insert(name.clone(), formatted);
                        packages_set.insert(name);
                    }

                    tracing::debug!(
                        "pacman -Qu completed successfully (exit code: {:?}): found {} packages from official repos",
                        exit_code,
                        count
                    );
                }
            } else if output.status.code() == Some(1) {
                // Exit code 1 is normal (no updates)
                if is_checkupdates_tool {
                    tracing::debug!(
                        "checkupdates returned exit code 1 (no updates available in official repos)"
                    );
                } else {
                    tracing::debug!(
                        "pacman -Qu returned exit code 1 (no updates available in official repos)"
                    );
                }
            } else {
                // Other exit codes are errors
                let stderr = String::from_utf8_lossy(&output.stderr);
                if is_checkupdates_tool {
                    tracing::warn!(
                        "checkupdates command failed with exit code: {:?}, stderr: {}",
                        exit_code,
                        stderr.trim()
                    );
                } else {
                    tracing::warn!("pacman -Qu command failed with exit code: {:?}", exit_code);
                }
            }
        }
        Err(e) => {
            if is_checkupdates_tool {
                tracing::warn!("Failed to execute checkupdates: {}", e);
            } else {
                tracing::warn!("Failed to execute pacman -Qu: {}", e);
            }
        }
    }
}

/// What: Process -Qua output and add packages to collections.
///
/// Inputs:
/// - `result`: Command output result
/// - `helper`: Helper name for logging
/// - `packages_map`: Mutable `HashMap` to store formatted package strings
/// - `packages_set`: Mutable `HashSet` to track unique package names
fn process_qua_output(
    result: Option<Result<std::process::Output, std::io::Error>>,
    helper: &str,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
) {
    if let Some(result) = result {
        match result {
            Ok(output) => {
                let exit_code = output.status.code();
                if output.status.success() {
                    let packages = parse_qua(&output.stdout);
                    let count = packages.len();
                    let before_count = packages_set.len();

                    for (name, old_version, new_version) in packages {
                        // Format: "name - old_version -> name - new_version"
                        let formatted = format!("{name} - {old_version} -> {name} - {new_version}");
                        packages_map.insert(name.clone(), formatted);
                        packages_set.insert(name);
                    }

                    let after_count = packages_set.len();
                    tracing::debug!(
                        "{} -Qua completed successfully (exit code: {:?}): found {} packages from AUR, {} total ({} new)",
                        helper,
                        exit_code,
                        count,
                        after_count,
                        after_count - before_count
                    );
                } else if output.status.code() == Some(1) {
                    // Exit code 1 is normal (no updates)
                    tracing::debug!(
                        "{} -Qua returned exit code 1 (no updates available in AUR)",
                        helper
                    );
                } else {
                    // Other exit codes are errors
                    tracing::warn!(
                        "{} -Qua command failed with exit code: {:?}",
                        helper,
                        exit_code
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to execute {} -Qua: {}", helper, e);
            }
        }
    } else {
        tracing::debug!("No AUR helper available, skipping AUR updates check");
    }
}

/// Static mutex to prevent concurrent update checks.
///
/// What: Tracks whether an update check is currently in progress.
///
/// Details:
/// - Uses `OnceLock` for lazy initialization
/// - Uses `tokio::sync::Mutex` for async-safe synchronization
/// - Prevents overlapping file writes to `available_updates.txt`
static UPDATE_CHECK_IN_PROGRESS: OnceLock<tokio::sync::Mutex<bool>> = OnceLock::new();

/// What: Spawn background worker to check for available package updates.
///
/// Inputs:
/// - `updates_tx`: Channel sender for updates (count, sorted list)
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Uses a temporary database to safely check for updates without modifying the system
/// - Syncs the temp database with `fakeroot pacman -Sy` if fakeroot is available
/// - Falls back to `pacman -Qu` (stale local DB) if fakeroot is not available
/// - Executes `yay -Qua` or `paru -Qua` for AUR updates
/// - Removes duplicates using `HashSet`
/// - Sorts package names alphabetically
/// - Saves list to `~/.config/pacsea/lists/available_updates.txt`
/// - Sends `(count, sorted_list)` via channel
/// - Uses synchronization to prevent concurrent update checks and file writes
#[allow(clippy::too_many_lines)] // Complex function handling multiple update check methods
pub fn spawn_updates_worker(updates_tx: mpsc::UnboundedSender<(usize, Vec<String>)>) {
    let updates_tx_once = updates_tx;

    tokio::spawn(async move {
        // Get mutex reference inside async block
        let mutex = UPDATE_CHECK_IN_PROGRESS.get_or_init(|| tokio::sync::Mutex::new(false));

        // Check if update check is already in progress
        let mut in_progress = mutex.lock().await;
        if *in_progress {
            tracing::debug!("Update check already in progress, skipping concurrent call");
            return;
        }

        // Set flag to indicate update check is in progress
        *in_progress = true;
        drop(in_progress); // Release lock before blocking operation

        let result = tokio::task::spawn_blocking(move || {
            use std::collections::HashSet;
            use std::process::{Command, Stdio};

            tracing::debug!("Starting update check");

            let (has_paru, has_yay, helper) = check_aur_helper();

            // Try safe update check with temp database (non-Windows only)
            #[cfg(not(target_os = "windows"))]
            let (temp_db_path, use_checkupdates_tool) = {
                let db_result = if has_fakeroot() {
                    tracing::debug!("fakeroot is available, setting up temp database");
                    setup_temp_db().and_then(|temp_db| {
                        tracing::debug!("Syncing temporary database at {:?}", temp_db);
                        if sync_temp_db(&temp_db) {
                            tracing::debug!("Temp database sync successful");
                            Some(temp_db)
                        } else {
                            tracing::warn!("Temp database sync failed");
                            None
                        }
                    })
                } else {
                    tracing::debug!("fakeroot not available");
                    None
                };

                // If temp database sync failed, try checkupdates as fallback
                if db_result.is_none() && has_checkupdates() {
                    tracing::debug!("Temp database sync failed, trying checkupdates as fallback");
                    (None, true)
                } else if db_result.is_none() {
                    tracing::warn!("Temp database sync failed and checkupdates not available, falling back to pacman -Qu (may show stale results)");
                    (None, false)
                } else {
                    (db_result, false)
                }
            };

            // Execute update check command
            #[cfg(not(target_os = "windows"))]
            let (output_checkupdates, is_checkupdates_tool) = if use_checkupdates_tool {
                tracing::debug!("Executing: checkupdates (automatically syncs database)");
                (
                    Command::new("checkupdates")
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output(),
                    true,
                )
            } else if let Some(db_path) = temp_db_path.as_ref() {
                tracing::debug!(
                    "Executing: pacman -Qu --dbpath {:?} (using synced temp database)",
                    db_path
                );
                (
                    Command::new("pacman")
                        .args(["-Qu", "--dbpath"])
                        .arg(db_path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                    false,
                )
            } else {
                tracing::debug!("Executing: pacman -Qu (using system database - may be stale)");
                (
                    Command::new("pacman")
                        .args(["-Qu"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                    false,
                )
            };

            #[cfg(target_os = "windows")]
            let (output_checkupdates, is_checkupdates_tool) = {
                tracing::debug!("Executing: pacman -Qu (Windows fallback)");
                (
                    Command::new("pacman")
                        .args(["-Qu"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                    false,
                )
            };

            // Execute -Qua command (AUR) - only if helper is available
            let output_qua = if has_paru {
                tracing::debug!("Executing: paru -Qua (AUR updates)");
                Some(
                    Command::new("paru")
                        .args(["-Qua"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                )
            } else if has_yay {
                tracing::debug!("Executing: yay -Qua (AUR updates)");
                Some(
                    Command::new("yay")
                        .args(["-Qua"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                )
            } else {
                tracing::debug!("No AUR helper available (paru/yay), skipping AUR updates check");
                None
            };

            // Collect packages from both commands
            // Use HashMap to store: package_name -> formatted_string
            // Use HashSet to track unique package names for deduplication
            let mut packages_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut packages_set = HashSet::new();

            // Parse pacman -Qu or checkupdates output (official repos)
            #[cfg(target_os = "windows")]
            let is_checkupdates_tool = false;
            process_checkupdates_output(
                output_checkupdates,
                is_checkupdates_tool,
                &mut packages_map,
                &mut packages_set,
            );

            // Parse -Qua output (AUR)
            process_qua_output(output_qua, helper, &mut packages_map, &mut packages_set);

            // Convert to Vec of formatted strings, sorted by package name
            let mut package_names: Vec<String> = packages_set.into_iter().collect();
            package_names.sort_unstable();

            let packages: Vec<String> = package_names
                .iter()
                .filter_map(|name| packages_map.get(name).cloned())
                .collect();

            let count = packages.len();
            tracing::debug!(
                "Update check completed: found {} total available updates (after deduplication)",
                count
            );

            // Save to file
            let lists_dir = crate::theme::lists_dir();
            let updates_file = lists_dir.join("available_updates.txt");
            if let Err(e) = std::fs::write(&updates_file, packages.join("\n")) {
                tracing::warn!("Failed to save updates list to file: {}", e);
            } else {
                tracing::debug!("Saved updates list to {:?}", updates_file);
            }

            // Return count and package names (for display) - not the formatted strings
            (count, package_names)
        })
        .await;

        // Reset flag when done (even on error)
        let mutex = UPDATE_CHECK_IN_PROGRESS.get_or_init(|| tokio::sync::Mutex::new(false));
        let mut in_progress = mutex.lock().await;
        *in_progress = false;
        drop(in_progress);

        match result {
            Ok((count, list)) => {
                let _ = updates_tx_once.send((count, list));
            }
            Err(e) => {
                tracing::error!("Updates worker task panicked: {:?}", e);
                let _ = updates_tx_once.send((0, Vec::new()));
            }
        }
    });
}

/// What: Spawn event reading thread for terminal input.
///
/// Inputs:
/// - `headless`: When `true`, skip spawning the thread
/// - `event_tx`: Channel sender for terminal events
/// - `event_thread_cancelled`: Atomic flag to signal thread cancellation
///
/// Details:
/// - Spawns a blocking thread that polls for terminal events
/// - Checks cancellation flag periodically to allow immediate exit
/// - Uses 50ms poll timeout to balance responsiveness and CPU usage
pub fn spawn_event_thread(
    headless: bool,
    event_tx: mpsc::UnboundedSender<CEvent>,
    event_thread_cancelled: Arc<AtomicBool>,
) {
    if !headless {
        let event_tx_for_thread = event_tx;
        let cancelled = event_thread_cancelled;
        std::thread::spawn(move || {
            loop {
                // Check cancellation flag first for immediate exit
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                // Use poll with timeout to allow periodic cancellation checks
                // This prevents blocking indefinitely when exit is requested
                match crossterm::event::poll(std::time::Duration::from_millis(50)) {
                    Ok(true) => {
                        // Event available, read it
                        if let Ok(ev) = crossterm::event::read() {
                            // Check cancellation again before sending
                            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }
                            // Check if channel is still open before sending
                            // When receiver is dropped (on exit), send will fail
                            if event_tx_for_thread.send(ev).is_err() {
                                // Channel closed, exit thread
                                break;
                            }
                        }
                        // ignore transient read errors and continue
                    }
                    Ok(false) => {
                        // No event available, check cancellation flag
                        // This allows the thread to exit promptly when exit is requested
                        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                    }
                    Err(_) => {
                        // Poll error, check cancellation before continuing
                        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::parse_checkupdates;

    /// What: Test that pacman -Qu parsing correctly extracts old and new versions.
    ///
    /// Inputs:
    /// - Sample pacman -Qu output with format `"package-name old_version -> new_version"`
    ///
    /// Output:
    /// - Verifies that `old_version` and `new_version` are correctly parsed and different
    ///
    /// Details:
    /// - Tests parsing of pacman -Qu output format
    #[test]
    fn test_parse_checkupdates_extracts_correct_versions() {
        let test_cases = vec![
            ("bat 0.26.0-1 -> 0.26.0-2", "bat", "0.26.0-1", "0.26.0-2"),
            (
                "comgr 2:6.4.4-2 -> 2:7.1.0-1",
                "comgr",
                "2:6.4.4-2",
                "2:7.1.0-1",
            ),
            (
                "composable-kernel 6.4.4-1 -> 7.1.0-1",
                "composable-kernel",
                "6.4.4-1",
                "7.1.0-1",
            ),
        ];

        for (input, expected_name, expected_old, expected_new) in test_cases {
            let output = input.as_bytes();
            let entries = parse_checkupdates(output);

            assert_eq!(entries.len(), 1, "Failed to parse: {input}");
            let (name, old_version, new_version) = &entries[0];
            assert_eq!(name, expected_name, "Wrong name for: {input}");
            assert_eq!(old_version, expected_old, "Wrong old_version for: {input}");
            assert_eq!(new_version, expected_new, "Wrong new_version for: {input}");
        }
    }

    /// What: Test that pacman -Qu parsing handles multiple packages.
    ///
    /// Inputs:
    /// - Multi-line pacman -Qu output
    ///
    /// Output:
    /// - Verifies that all packages are parsed correctly
    #[test]
    fn test_parse_checkupdates_multiple_packages() {
        let input = "bat 0.26.0-1 -> 0.26.0-2\ncomgr 2:6.4.4-2 -> 2:7.1.0-1\n";
        let output = input.as_bytes();
        let entries = parse_checkupdates(output);

        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            (
                "bat".to_string(),
                "0.26.0-1".to_string(),
                "0.26.0-2".to_string()
            )
        );
        assert_eq!(
            entries[1],
            (
                "comgr".to_string(),
                "2:6.4.4-2".to_string(),
                "2:7.1.0-1".to_string()
            )
        );
    }
}
