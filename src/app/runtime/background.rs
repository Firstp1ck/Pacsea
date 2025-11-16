use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crossterm::event::Event as CEvent;
use tokio::{
    select,
    sync::mpsc,
    time::{Duration, sleep},
};

use crate::index as pkgindex;
use crate::sources;
use crate::sources::fetch_details;
use crate::state::*;
use crate::util::{match_rank, repo_order};

/// What: Channel definitions for runtime communication.
///
/// Details:
/// - Contains all channel senders and receivers used for communication
///   between the main event loop and background workers
#[allow(dead_code)]
pub struct Channels {
    pub event_tx: mpsc::UnboundedSender<CEvent>,
    pub event_rx: mpsc::UnboundedReceiver<CEvent>,
    pub event_thread_cancelled: Arc<AtomicBool>,
    pub search_result_tx: mpsc::UnboundedSender<SearchResults>,
    pub results_rx: mpsc::UnboundedReceiver<SearchResults>,
    pub details_req_tx: mpsc::UnboundedSender<PackageItem>,
    pub details_res_tx: mpsc::UnboundedSender<PackageDetails>,
    pub details_res_rx: mpsc::UnboundedReceiver<PackageDetails>,
    pub tick_tx: mpsc::UnboundedSender<()>,
    pub tick_rx: mpsc::UnboundedReceiver<()>,
    pub net_err_tx: mpsc::UnboundedSender<String>,
    pub net_err_rx: mpsc::UnboundedReceiver<String>,
    pub preview_tx: mpsc::UnboundedSender<PackageItem>,
    pub preview_rx: mpsc::UnboundedReceiver<PackageItem>,
    pub add_tx: mpsc::UnboundedSender<PackageItem>,
    pub add_rx: mpsc::UnboundedReceiver<PackageItem>,
    pub index_notify_tx: mpsc::UnboundedSender<()>,
    pub index_notify_rx: mpsc::UnboundedReceiver<()>,
    pub pkgb_req_tx: mpsc::UnboundedSender<PackageItem>,
    pub pkgb_res_tx: mpsc::UnboundedSender<(String, String)>,
    pub pkgb_res_rx: mpsc::UnboundedReceiver<(String, String)>,
    pub status_tx: mpsc::UnboundedSender<(String, ArchStatusColor)>,
    pub status_rx: mpsc::UnboundedReceiver<(String, ArchStatusColor)>,
    pub news_tx: mpsc::UnboundedSender<Vec<NewsItem>>,
    pub news_rx: mpsc::UnboundedReceiver<Vec<NewsItem>>,
    pub deps_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    pub deps_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::DependencyInfo>>,
    pub deps_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::DependencyInfo>>,
    pub files_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    pub files_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::PackageFileInfo>>,
    pub files_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::PackageFileInfo>>,
    pub services_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    pub services_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::ServiceImpact>>,
    pub services_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::ServiceImpact>>,
    pub sandbox_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    pub sandbox_res_tx: mpsc::UnboundedSender<Vec<crate::logic::sandbox::SandboxInfo>>,
    pub sandbox_res_rx: mpsc::UnboundedReceiver<Vec<crate::logic::sandbox::SandboxInfo>>,
    pub summary_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    pub summary_res_tx: mpsc::UnboundedSender<crate::logic::preflight::PreflightSummaryOutcome>,
    pub summary_res_rx: mpsc::UnboundedReceiver<crate::logic::preflight::PreflightSummaryOutcome>,
    pub query_tx: mpsc::UnboundedSender<QueryInput>,
}

impl Channels {
    /// What: Create all channels used for runtime communication.
    ///
    /// Inputs:
    /// - `index_path`: Path to official package index (for search worker)
    ///
    /// Output:
    /// - Returns a `Channels` struct with all senders and receivers initialized
    pub fn new(index_path: std::path::PathBuf) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<CEvent>();
        let event_thread_cancelled = Arc::new(AtomicBool::new(false));
        let (search_result_tx, results_rx) = mpsc::unbounded_channel::<SearchResults>();
        let (details_req_tx, details_req_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (details_res_tx, details_res_rx) = mpsc::unbounded_channel::<PackageDetails>();
        let (tick_tx, tick_rx) = mpsc::unbounded_channel::<()>();
        let (net_err_tx, net_err_rx) = mpsc::unbounded_channel::<String>();
        let (preview_tx, preview_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (add_tx, add_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (index_notify_tx, index_notify_rx) = mpsc::unbounded_channel::<()>();
        let (pkgb_req_tx, pkgb_req_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_res_tx, pkgb_res_rx) = mpsc::unbounded_channel::<(String, String)>();
        let (status_tx, status_rx) = mpsc::unbounded_channel::<(String, ArchStatusColor)>();
        let (news_tx, news_rx) = mpsc::unbounded_channel::<Vec<NewsItem>>();
        let (deps_req_tx, deps_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
        let (deps_res_tx, deps_res_rx) =
            mpsc::unbounded_channel::<Vec<crate::state::modal::DependencyInfo>>();
        let (files_req_tx, files_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
        let (files_res_tx, files_res_rx) =
            mpsc::unbounded_channel::<Vec<crate::state::modal::PackageFileInfo>>();
        let (services_req_tx, services_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
        let (services_res_tx, services_res_rx) =
            mpsc::unbounded_channel::<Vec<crate::state::modal::ServiceImpact>>();
        let (sandbox_req_tx, sandbox_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
        let (sandbox_res_tx, sandbox_res_rx) =
            mpsc::unbounded_channel::<Vec<crate::logic::sandbox::SandboxInfo>>();
        let (summary_req_tx, summary_req_rx) =
            mpsc::unbounded_channel::<(Vec<PackageItem>, crate::state::modal::PreflightAction)>();
        let (summary_res_tx, summary_res_rx) =
            mpsc::unbounded_channel::<crate::logic::preflight::PreflightSummaryOutcome>();
        let (query_tx, query_rx) = mpsc::unbounded_channel::<QueryInput>();

        // Spawn background workers
        spawn_details_worker(net_err_tx.clone(), details_req_rx, details_res_tx.clone());
        spawn_pkgbuild_worker(pkgb_req_rx, pkgb_res_tx.clone());
        spawn_dependency_worker(deps_req_rx, deps_res_tx.clone());
        spawn_file_worker(files_req_rx, files_res_tx.clone());
        spawn_service_worker(services_req_rx, services_res_tx.clone());
        spawn_sandbox_worker(sandbox_req_rx, sandbox_res_tx.clone());
        spawn_summary_worker(summary_req_rx, summary_res_tx.clone());
        spawn_search_worker(
            query_rx,
            search_result_tx.clone(),
            net_err_tx.clone(),
            index_path,
        );

        Channels {
            event_tx,
            event_rx,
            event_thread_cancelled,
            search_result_tx,
            results_rx,
            details_req_tx,
            details_res_tx,
            details_res_rx,
            tick_tx,
            tick_rx,
            net_err_tx,
            net_err_rx,
            preview_tx,
            preview_rx,
            add_tx,
            add_rx,
            index_notify_tx,
            index_notify_rx,
            pkgb_req_tx,
            pkgb_res_tx,
            pkgb_res_rx,
            status_tx,
            status_rx,
            news_tx,
            news_rx,
            deps_req_tx,
            deps_res_tx,
            deps_res_rx,
            files_req_tx,
            files_res_tx,
            files_res_rx,
            services_req_tx,
            services_res_tx,
            services_res_rx,
            sandbox_req_tx,
            sandbox_res_tx,
            sandbox_res_rx,
            summary_req_tx,
            summary_res_tx,
            summary_res_rx,
            query_tx,
        }
    }
}

/// What: Spawn background worker for batched package details fetching.
///
/// Inputs:
/// - `net_err_tx`: Channel sender for network errors
/// - `details_req_rx`: Channel receiver for detail requests
/// - `details_res_tx`: Channel sender for detail responses
///
/// Details:
/// - Batches requests within a 120ms window to reduce network calls
/// - Deduplicates requests by package name
/// - Filters out disallowed packages
fn spawn_details_worker(
    net_err_tx: mpsc::UnboundedSender<String>,
    mut details_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    details_res_tx: mpsc::UnboundedSender<PackageDetails>,
) {
    let net_err_tx_details = net_err_tx.clone();
    tokio::spawn(async move {
        const DETAILS_BATCH_WINDOW_MS: u64 = 120;
        loop {
            let first = match details_req_rx.recv().await {
                Some(i) => i,
                None => break,
            };
            let mut batch: Vec<PackageItem> = vec![first];
            loop {
                tokio::select! {
                    Some(next) = details_req_rx.recv() => { batch.push(next); }
                    _ = sleep(Duration::from_millis(DETAILS_BATCH_WINDOW_MS)) => { break; }
                }
            }
            use std::collections::HashSet;
            let mut seen: HashSet<String> = HashSet::new();
            let mut ordered: Vec<PackageItem> = Vec::with_capacity(batch.len());
            for it in batch.into_iter() {
                if seen.insert(it.name.clone()) {
                    ordered.push(it);
                }
            }
            for it in ordered.into_iter() {
                if !crate::logic::is_allowed(&it.name) {
                    continue;
                }
                match fetch_details(it.clone()).await {
                    Ok(details) => {
                        let _ = details_res_tx.send(details);
                    }
                    Err(e) => {
                        let msg = match it.source {
                            Source::Official { .. } => format!(
                                "Official package details unavailable for {}: {}",
                                it.name, e
                            ),
                            Source::Aur => {
                                format!("AUR package details unavailable for {}: {}", it.name, e)
                            }
                        };
                        let _ = net_err_tx_details.send(msg);
                    }
                }
            }
        }
    });
}

/// What: Spawn background worker for PKGBUILD fetching.
///
/// Inputs:
/// - `pkgb_req_rx`: Channel receiver for PKGBUILD requests
/// - `pkgb_res_tx`: Channel sender for PKGBUILD responses
fn spawn_pkgbuild_worker(
    mut pkgb_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    pkgb_res_tx: mpsc::UnboundedSender<(String, String)>,
) {
    tokio::spawn(async move {
        while let Some(item) = pkgb_req_rx.recv().await {
            let name = item.name.clone();
            match sources::fetch_pkgbuild_fast(&item).await {
                Ok(txt) => {
                    let _ = pkgb_res_tx.send((name, txt));
                }
                Err(e) => {
                    let _ = pkgb_res_tx.send((name, format!("Failed to fetch PKGBUILD: {e}")));
                }
            }
        }
    });
}

/// What: Spawn background worker for dependency resolution.
///
/// Inputs:
/// - `deps_req_rx`: Channel receiver for dependency resolution requests
/// - `deps_res_tx`: Channel sender for dependency resolution responses
///
/// Details:
/// - Runs blocking dependency resolution in a thread pool
/// - Always sends a result, even if the task panics, to ensure flags are reset
fn spawn_dependency_worker(
    mut deps_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    deps_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::DependencyInfo>>,
) {
    let deps_res_tx_bg = deps_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = deps_req_rx.recv().await {
            // Run blocking dependency resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = deps_res_tx_bg.clone();
            let res_tx_error = deps_res_tx_bg.clone(); // Clone for error handling
            let handle = tokio::task::spawn_blocking(move || {
                let deps = crate::logic::deps::resolve_dependencies(&items_clone);
                let _ = res_tx.send(deps);
            });
            // CRITICAL: Always await and send a result, even if task panics
            // This ensures deps_resolving flag is always reset
            tokio::spawn(async move {
                match handle.await {
                    Ok(_) => {
                        // Task completed successfully, result already sent
                        tracing::debug!("[Runtime] Dependency resolution task completed");
                    }
                    Err(e) => {
                        // Task panicked - send empty result to reset flag
                        tracing::error!("[Runtime] Dependency resolution task panicked: {:?}", e);
                        let _ = res_tx_error.send(Vec::new());
                    }
                }
            });
        }
        tracing::debug!("[Runtime] Dependency resolution worker exiting (channel closed)");
    });
}

/// What: Spawn background worker for file resolution.
///
/// Inputs:
/// - `files_req_rx`: Channel receiver for file resolution requests
/// - `files_res_tx`: Channel sender for file resolution responses
fn spawn_file_worker(
    mut files_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    files_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::PackageFileInfo>>,
) {
    let files_res_tx_bg = files_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = files_req_rx.recv().await {
            // Run blocking file resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = files_res_tx_bg.clone();
            tokio::task::spawn_blocking(move || {
                let files = crate::logic::files::resolve_file_changes(
                    &items_clone,
                    crate::state::modal::PreflightAction::Install,
                );
                tracing::debug!(
                    "[Background] Sending file result: {} entries for packages: {:?}",
                    files.len(),
                    files.iter().map(|f| &f.name).collect::<Vec<_>>()
                );
                for file_info in &files {
                    tracing::debug!(
                        "[Background] Package '{}' - total={}, new={}, changed={}, removed={}, config={}",
                        file_info.name,
                        file_info.total_count,
                        file_info.new_count,
                        file_info.changed_count,
                        file_info.removed_count,
                        file_info.config_count
                    );
                }
                if let Err(e) = res_tx.send(files) {
                    tracing::error!("[Background] Failed to send file result: {}", e);
                } else {
                    tracing::debug!("[Background] Successfully sent file result");
                }
            });
        }
    });
}

/// What: Spawn background worker for service impact resolution.
///
/// Inputs:
/// - `services_req_rx`: Channel receiver for service resolution requests
/// - `services_res_tx`: Channel sender for service resolution responses
fn spawn_service_worker(
    mut services_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    services_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::ServiceImpact>>,
) {
    let services_res_tx_bg = services_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = services_req_rx.recv().await {
            // Run blocking service resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = services_res_tx_bg.clone();
            tokio::task::spawn_blocking(move || {
                let services = crate::logic::services::resolve_service_impacts(
                    &items_clone,
                    crate::state::modal::PreflightAction::Install,
                );
                let _ = res_tx.send(services);
            });
        }
    });
}

/// What: Spawn background worker for sandbox resolution.
///
/// Inputs:
/// - `sandbox_req_rx`: Channel receiver for sandbox resolution requests
/// - `sandbox_res_tx`: Channel sender for sandbox resolution responses
///
/// Details:
/// - Uses async version for parallel HTTP fetches
fn spawn_sandbox_worker(
    mut sandbox_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    sandbox_res_tx: mpsc::UnboundedSender<Vec<crate::logic::sandbox::SandboxInfo>>,
) {
    let sandbox_res_tx_bg = sandbox_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = sandbox_req_rx.recv().await {
            // Use async version for parallel HTTP fetches
            let items_clone = items.clone();
            let res_tx = sandbox_res_tx_bg.clone();
            tokio::spawn(async move {
                let sandbox_info =
                    crate::logic::sandbox::resolve_sandbox_info_async(&items_clone).await;
                tracing::debug!(
                    "[Background] Sending sandbox result: {} entries for packages: {:?}",
                    sandbox_info.len(),
                    sandbox_info
                        .iter()
                        .map(|s| &s.package_name)
                        .collect::<Vec<_>>()
                );
                if let Err(e) = res_tx.send(sandbox_info) {
                    tracing::error!("[Background] Failed to send sandbox result: {}", e);
                } else {
                    tracing::debug!("[Background] Successfully sent sandbox result");
                }
            });
        }
    });
}

/// What: Spawn background worker for preflight summary computation.
///
/// Inputs:
/// - `summary_req_rx`: Channel receiver for summary computation requests
/// - `summary_res_tx`: Channel sender for summary computation responses
///
/// Details:
/// - Runs blocking summary computation in a thread pool
/// - Always sends a result, even if the task panics, to avoid breaking the UI
fn spawn_summary_worker(
    mut summary_req_rx: mpsc::UnboundedReceiver<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
    summary_res_tx: mpsc::UnboundedSender<crate::logic::preflight::PreflightSummaryOutcome>,
) {
    let summary_res_tx_bg = summary_res_tx.clone();
    tokio::spawn(async move {
        while let Some((items, action)) = summary_req_rx.recv().await {
            // Run blocking summary computation in a thread pool
            let items_clone = items.clone();
            let res_tx = summary_res_tx_bg.clone();
            let res_tx_error = summary_res_tx_bg.clone();
            let handle = tokio::task::spawn_blocking(move || {
                let summary =
                    crate::logic::preflight::compute_preflight_summary(&items_clone, action);
                let _ = res_tx.send(summary);
            });
            // CRITICAL: Always await and send a result, even if task panics
            tokio::spawn(async move {
                match handle.await {
                    Ok(_) => {
                        // Task completed successfully, result already sent
                        tracing::debug!("[Runtime] Preflight summary computation task completed");
                    }
                    Err(e) => {
                        // Task panicked - send minimal result to reset flag
                        tracing::error!(
                            "[Runtime] Preflight summary computation task panicked: {:?}",
                            e
                        );
                        // Create a minimal summary to avoid breaking the UI
                        let minimal_summary = crate::logic::preflight::PreflightSummaryOutcome {
                            summary: crate::state::modal::PreflightSummaryData {
                                packages: Vec::new(),
                                package_count: 0,
                                aur_count: 0,
                                download_bytes: 0,
                                install_delta_bytes: 0,
                                risk_score: 0,
                                risk_level: crate::state::modal::RiskLevel::Low,
                                risk_reasons: Vec::new(),
                                major_bump_packages: Vec::new(),
                                core_system_updates: Vec::new(),
                                pacnew_candidates: 0,
                                pacsave_candidates: 0,
                                config_warning_packages: Vec::new(),
                                service_restart_units: Vec::new(),
                                summary_warnings: vec!["Summary computation failed".to_string()],
                                summary_notes: Vec::new(),
                            },
                            header: crate::state::modal::PreflightHeaderChips {
                                package_count: 0,
                                download_bytes: 0,
                                install_delta_bytes: 0,
                                aur_count: 0,
                                risk_score: 0,
                                risk_level: crate::state::modal::RiskLevel::Low,
                            },
                        };
                        let _ = res_tx_error.send(minimal_summary);
                    }
                }
            });
        }
        tracing::debug!("[Runtime] Preflight summary computation worker exiting (channel closed)");
    });
}

/// What: Spawn background worker for search queries.
///
/// Inputs:
/// - `query_rx`: Channel receiver for search queries
/// - `search_result_tx`: Channel sender for search results
/// - `net_err_tx`: Channel sender for network errors
/// - `index_path`: Path to official package index
///
/// Details:
/// - Debounces queries with 250ms window
/// - Enforces minimum 300ms interval between searches
/// - Handles empty queries by returning all official packages
/// - Searches both official and AUR repositories
pub fn spawn_search_worker(
    mut query_rx: mpsc::UnboundedReceiver<QueryInput>,
    search_result_tx: mpsc::UnboundedSender<SearchResults>,
    net_err_tx: mpsc::UnboundedSender<String>,
    index_path: std::path::PathBuf,
) {
    let net_err_tx_search = net_err_tx.clone();
    tokio::spawn(async move {
        const DEBOUNCE_MS: u64 = 250;
        const MIN_INTERVAL_MS: u64 = 300;
        let mut last_sent = Instant::now() - Duration::from_millis(MIN_INTERVAL_MS);
        loop {
            let mut latest = match query_rx.recv().await {
                Some(q) => q,
                None => break,
            };
            loop {
                select! { Some(new_q) = query_rx.recv() => { latest = new_q; } _ = sleep(Duration::from_millis(DEBOUNCE_MS)) => { break; } }
            }
            if latest.text.trim().is_empty() {
                let mut items = pkgindex::all_official_or_fetch(&index_path).await;
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
                // Deduplicate by package name, preferring earlier entries (core > extra > others)
                {
                    use std::collections::HashSet;
                    let mut seen = HashSet::new();
                    items.retain(|p| seen.insert(p.name.to_lowercase()));
                }
                let _ = search_result_tx.send(SearchResults {
                    id: latest.id,
                    items,
                });
                continue;
            }
            let elapsed = last_sent.elapsed();
            if elapsed < Duration::from_millis(MIN_INTERVAL_MS) {
                sleep(Duration::from_millis(MIN_INTERVAL_MS) - elapsed).await;
            }
            last_sent = Instant::now();

            let qtext = latest.text.clone();
            let sid = latest.id;
            let tx = search_result_tx.clone();
            let err_tx = net_err_tx_search.clone();
            let ipath = index_path.clone();
            tokio::spawn(async move {
                if crate::index::all_official().is_empty() {
                    let _ = crate::index::all_official_or_fetch(&ipath).await;
                }
                let mut items = pkgindex::search_official(&qtext);
                let q_for_net = qtext.clone();
                let (aur_items, errors) = sources::fetch_all_with_errors(q_for_net).await;
                items.extend(aur_items);
                let ql = qtext.trim().to_lowercase();
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    let ra = match_rank(&a.name, &ql);
                    let rb = match_rank(&b.name, &ql);
                    if ra != rb {
                        return ra.cmp(&rb);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
                // Deduplicate by package name, preferring earlier entries (official over AUR)
                {
                    use std::collections::HashSet;
                    let mut seen = HashSet::new();
                    items.retain(|p| seen.insert(p.name.to_lowercase()));
                }
                for e in errors {
                    let _ = err_tx.send(e);
                }
                let _ = tx.send(SearchResults { id: sid, items });
            });
        }
    });
}

/// What: Spawn background workers for status, news, and tick events.
///
/// Inputs:
/// - `headless`: When `true`, skip terminal-dependent operations
/// - `status_tx`: Channel sender for Arch status updates
/// - `news_tx`: Channel sender for Arch news updates
/// - `tick_tx`: Channel sender for tick events
/// - `news_read_urls`: Set of already-read news URLs
/// - `official_index_path`: Path to official package index
/// - `net_err_tx`: Channel sender for network errors
/// - `index_notify_tx`: Channel sender for index update notifications
///
/// Details:
/// - Fetches Arch status text once at startup and periodically every 120 seconds
/// - Fetches Arch news once at startup, filtering out already-read items
/// - Updates package index in background (Windows vs non-Windows handling)
/// - Refreshes pacman caches (installed, explicit)
/// - Spawns tick worker that sends events every 200ms
#[allow(clippy::too_many_arguments)]
pub fn spawn_auxiliary_workers(
    headless: bool,
    status_tx: mpsc::UnboundedSender<(String, ArchStatusColor)>,
    news_tx: mpsc::UnboundedSender<Vec<NewsItem>>,
    tick_tx: mpsc::UnboundedSender<()>,
    news_read_urls: std::collections::HashSet<String>,
    official_index_path: std::path::PathBuf,
    net_err_tx: mpsc::UnboundedSender<String>,
    index_notify_tx: mpsc::UnboundedSender<()>,
) {
    // Fetch Arch status text once at startup (skip in headless mode to avoid network delays)
    if !headless {
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

    // Fetch Arch news once at startup; show unread items (by URL) if any (skip in headless mode)
    if !headless {
        let news_tx_once = news_tx.clone();
        let read_set = news_read_urls;
        tokio::spawn(async move {
            if let Ok(list) = sources::fetch_arch_news(10).await {
                let unread: Vec<NewsItem> = list
                    .into_iter()
                    .filter(|it| !read_set.contains(&it.url))
                    .collect();
                let _ = news_tx_once.send(unread);
            }
        });
    }

    #[cfg(windows)]
    {
        // Save mirrors into the repository directory in the source tree and build the index via Arch API
        let repo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repository");
        let index_path = official_index_path.clone();
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
    #[cfg(not(windows))]
    {
        // Skip index update in headless mode to avoid slow network/disk operations
        if !headless {
            let index_path = official_index_path.clone();
            let net_err = net_err_tx.clone();
            let index_notify = index_notify_tx.clone();
            tokio::spawn(async move {
                pkgindex::update_in_background(index_path, net_err, index_notify).await;
            });
        }
    }

    // Skip pacman cache refreshes in headless mode to avoid slow process spawning
    if !headless {
        tokio::spawn(async move {
            pkgindex::refresh_installed_cache().await;
            pkgindex::refresh_explicit_cache().await;
        });
    }

    // Spawn tick worker
    let tick_tx_bg = tick_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            interval.tick().await;
            let _ = tick_tx_bg.send(());
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
        let event_tx_for_thread = event_tx.clone();
        let cancelled = event_thread_cancelled.clone();
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
                        match crossterm::event::read() {
                            Ok(ev) => {
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
                            Err(_) => {
                                // ignore transient read errors and continue
                            }
                        }
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
