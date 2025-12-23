use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crossterm::event::Event as CEvent;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{Duration, sleep};

use crate::index as pkgindex;
use crate::sources;
use crate::state::ArchStatusColor;

use crate::app::runtime::workers::news;
use crate::app::runtime::workers::updates;

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
/// - `news_incremental_tx`: Channel sender for incremental background news items
/// - `announcement_tx`: Channel sender for remote announcement updates
/// - `tick_tx`: Channel sender for tick events
/// - `news_read_ids`: Set of already-read news IDs
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
    news_incremental_tx: &mpsc::UnboundedSender<crate::state::types::NewsFeedItem>,
    announcement_tx: &mpsc::UnboundedSender<crate::announcements::RemoteAnnouncement>,
    tick_tx: &mpsc::UnboundedSender<()>,
    news_read_ids: &std::collections::HashSet<String>,
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
        news::spawn_startup_news_worker(
            news_tx,
            news_read_ids,
            news_read_urls,
            news_seen_pkg_versions,
            news_seen_aur_comments,
            last_startup_timestamp,
            Some(completion_tx),
        );
        news::spawn_aggregated_news_feed_worker(
            news_feed_tx,
            news_incremental_tx,
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
        updates::spawn_periodic_updates_worker(updates_tx, updates_refresh_interval);
    }

    // Spawn tick worker (always runs)
    spawn_tick_worker(tick_tx);

    // Spawn faillock worker (skip in headless mode)
    if !headless {
        spawn_faillock_worker(tick_tx);
    }
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
