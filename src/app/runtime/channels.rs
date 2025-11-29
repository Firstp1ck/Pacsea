use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crossterm::event::Event as CEvent;
use tokio::sync::mpsc;

use crate::state::{
    ArchStatusColor, NewsItem, PackageDetails, PackageItem, QueryInput, SearchResults,
};

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
    pub comments_req_tx: mpsc::UnboundedSender<String>,
    pub comments_res_tx:
        mpsc::UnboundedSender<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    pub comments_res_rx:
        mpsc::UnboundedReceiver<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    pub status_tx: mpsc::UnboundedSender<(String, ArchStatusColor)>,
    pub status_rx: mpsc::UnboundedReceiver<(String, ArchStatusColor)>,
    pub news_tx: mpsc::UnboundedSender<Vec<NewsItem>>,
    pub news_rx: mpsc::UnboundedReceiver<Vec<NewsItem>>,
    pub updates_tx: mpsc::UnboundedSender<(usize, Vec<String>)>,
    pub updates_rx: mpsc::UnboundedReceiver<(usize, Vec<String>)>,
    pub deps_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    pub deps_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::DependencyInfo>>,
    pub deps_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::DependencyInfo>>,
    pub files_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    pub files_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::PackageFileInfo>>,
    pub files_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::PackageFileInfo>>,
    pub services_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    pub services_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::ServiceImpact>>,
    pub services_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::ServiceImpact>>,
    pub sandbox_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    pub sandbox_res_tx: mpsc::UnboundedSender<Vec<crate::logic::sandbox::SandboxInfo>>,
    pub sandbox_res_rx: mpsc::UnboundedReceiver<Vec<crate::logic::sandbox::SandboxInfo>>,
    pub summary_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    pub summary_res_tx: mpsc::UnboundedSender<crate::logic::preflight::PreflightSummaryOutcome>,
    pub summary_res_rx: mpsc::UnboundedReceiver<crate::logic::preflight::PreflightSummaryOutcome>,
    pub executor_req_tx: mpsc::UnboundedSender<crate::install::ExecutorRequest>,
    pub executor_res_rx: mpsc::UnboundedReceiver<crate::install::ExecutorOutput>,
    pub post_summary_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    pub post_summary_res_rx: mpsc::UnboundedReceiver<crate::logic::summary::PostSummaryData>,
    pub query_tx: mpsc::UnboundedSender<QueryInput>,
}

/// What: Event channel pair and cancellation flag.
struct EventChannels {
    tx: mpsc::UnboundedSender<CEvent>,
    rx: mpsc::UnboundedReceiver<CEvent>,
    cancelled: Arc<AtomicBool>,
}

/// What: Search-related channels.
struct SearchChannels {
    result_tx: mpsc::UnboundedSender<SearchResults>,
    results_rx: mpsc::UnboundedReceiver<SearchResults>,
    query_tx: mpsc::UnboundedSender<QueryInput>,
    query_rx: mpsc::UnboundedReceiver<QueryInput>,
}

/// What: Package details channels.
struct DetailsChannels {
    req_tx: mpsc::UnboundedSender<PackageItem>,
    req_rx: mpsc::UnboundedReceiver<PackageItem>,
    res_tx: mpsc::UnboundedSender<PackageDetails>,
    res_rx: mpsc::UnboundedReceiver<PackageDetails>,
}

/// What: Preflight-related channels (dependencies, files, services, sandbox, summary).
struct PreflightChannels {
    deps_req_tx: mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    deps_req_rx: mpsc::UnboundedReceiver<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    deps_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::DependencyInfo>>,
    deps_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::DependencyInfo>>,
    files_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    files_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    files_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::PackageFileInfo>>,
    files_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::PackageFileInfo>>,
    services_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    services_req_rx:
        mpsc::UnboundedReceiver<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    services_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::ServiceImpact>>,
    services_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::ServiceImpact>>,
    sandbox_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    sandbox_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    sandbox_res_tx: mpsc::UnboundedSender<Vec<crate::logic::sandbox::SandboxInfo>>,
    sandbox_res_rx: mpsc::UnboundedReceiver<Vec<crate::logic::sandbox::SandboxInfo>>,
    summary_req_tx: mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    summary_req_rx:
        mpsc::UnboundedReceiver<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    summary_res_tx: mpsc::UnboundedSender<crate::logic::preflight::PreflightSummaryOutcome>,
    summary_res_rx: mpsc::UnboundedReceiver<crate::logic::preflight::PreflightSummaryOutcome>,
}

/// What: Utility channels (tick, network errors, preview, add, index notify, PKGBUILD, status, news).
struct UtilityChannels {
    tick_tx: mpsc::UnboundedSender<()>,
    tick_rx: mpsc::UnboundedReceiver<()>,
    net_err_tx: mpsc::UnboundedSender<String>,
    net_err_rx: mpsc::UnboundedReceiver<String>,
    preview_tx: mpsc::UnboundedSender<PackageItem>,
    preview_rx: mpsc::UnboundedReceiver<PackageItem>,
    add_tx: mpsc::UnboundedSender<PackageItem>,
    add_rx: mpsc::UnboundedReceiver<PackageItem>,
    index_notify_tx: mpsc::UnboundedSender<()>,
    index_notify_rx: mpsc::UnboundedReceiver<()>,
    pkgb_req_tx: mpsc::UnboundedSender<PackageItem>,
    pkgb_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    pkgb_res_tx: mpsc::UnboundedSender<(String, String)>,
    pkgb_res_rx: mpsc::UnboundedReceiver<(String, String)>,
    comments_req_tx: mpsc::UnboundedSender<String>,
    comments_req_rx: mpsc::UnboundedReceiver<String>,
    comments_res_tx:
        mpsc::UnboundedSender<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    comments_res_rx:
        mpsc::UnboundedReceiver<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    status_tx: mpsc::UnboundedSender<(String, ArchStatusColor)>,
    status_rx: mpsc::UnboundedReceiver<(String, ArchStatusColor)>,
    news_tx: mpsc::UnboundedSender<Vec<NewsItem>>,
    news_rx: mpsc::UnboundedReceiver<Vec<NewsItem>>,
    updates_tx: mpsc::UnboundedSender<(usize, Vec<String>)>,
    updates_rx: mpsc::UnboundedReceiver<(usize, Vec<String>)>,
    executor_req_tx: mpsc::UnboundedSender<crate::install::ExecutorRequest>,
    executor_req_rx: mpsc::UnboundedReceiver<crate::install::ExecutorRequest>,
    executor_res_tx: mpsc::UnboundedSender<crate::install::ExecutorOutput>,
    executor_res_rx: mpsc::UnboundedReceiver<crate::install::ExecutorOutput>,
    post_summary_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    post_summary_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    post_summary_res_tx: mpsc::UnboundedSender<crate::logic::summary::PostSummaryData>,
    post_summary_res_rx: mpsc::UnboundedReceiver<crate::logic::summary::PostSummaryData>,
}

/// What: Create event channels.
///
/// Output:
/// - Returns event channels and cancellation flag
fn create_event_channels() -> EventChannels {
    let (tx, rx) = mpsc::unbounded_channel::<CEvent>();
    let cancelled = Arc::new(AtomicBool::new(false));
    EventChannels { tx, rx, cancelled }
}

/// What: Create search-related channels.
///
/// Output:
/// - Returns search channels
fn create_search_channels() -> SearchChannels {
    let (result_tx, results_rx) = mpsc::unbounded_channel::<SearchResults>();
    let (query_tx, query_rx) = mpsc::unbounded_channel::<QueryInput>();
    SearchChannels {
        result_tx,
        results_rx,
        query_tx,
        query_rx,
    }
}

/// What: Create package details channels.
///
/// Output:
/// - Returns details channels
fn create_details_channels() -> DetailsChannels {
    let (req_tx, req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (res_tx, res_rx) = mpsc::unbounded_channel::<PackageDetails>();
    DetailsChannels {
        req_tx,
        req_rx,
        res_tx,
        res_rx,
    }
}

/// What: Create preflight-related channels.
///
/// Output:
/// - Returns preflight channels
fn create_preflight_channels() -> PreflightChannels {
    let (deps_req_tx, deps_req_rx) =
        mpsc::unbounded_channel::<(Vec<PackageItem>, crate::state::modal::PreflightAction)>();
    let (deps_res_tx, deps_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::state::modal::DependencyInfo>>();
    let (files_req_tx, files_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (files_res_tx, files_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::state::modal::PackageFileInfo>>();
    let (services_req_tx, services_req_rx) =
        mpsc::unbounded_channel::<(Vec<PackageItem>, crate::state::modal::PreflightAction)>();
    let (services_res_tx, services_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::state::modal::ServiceImpact>>();
    let (sandbox_req_tx, sandbox_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (sandbox_res_tx, sandbox_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::logic::sandbox::SandboxInfo>>();
    let (summary_req_tx, summary_req_rx) =
        mpsc::unbounded_channel::<(Vec<PackageItem>, crate::state::modal::PreflightAction)>();
    let (summary_res_tx, summary_res_rx) =
        mpsc::unbounded_channel::<crate::logic::preflight::PreflightSummaryOutcome>();
    PreflightChannels {
        deps_req_tx,
        deps_req_rx,
        deps_res_tx,
        deps_res_rx,
        files_req_tx,
        files_req_rx,
        files_res_tx,
        files_res_rx,
        services_req_tx,
        services_req_rx,
        services_res_tx,
        services_res_rx,
        sandbox_req_tx,
        sandbox_req_rx,
        sandbox_res_tx,
        sandbox_res_rx,
        summary_req_tx,
        summary_req_rx,
        summary_res_tx,
        summary_res_rx,
    }
}

/// What: Create utility channels.
///
/// Output:
/// - Returns utility channels
fn create_utility_channels() -> UtilityChannels {
    let (tick_tx, tick_rx) = mpsc::unbounded_channel::<()>();
    let (net_err_tx, net_err_rx) = mpsc::unbounded_channel::<String>();
    let (preview_tx, preview_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (add_tx, add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (index_notify_tx, index_notify_rx) = mpsc::unbounded_channel::<()>();
    let (pkgb_req_tx, pkgb_req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_res_tx, pkgb_res_rx) = mpsc::unbounded_channel::<(String, String)>();
    let (comments_req_tx, comments_req_rx) = mpsc::unbounded_channel::<String>();
    let (comments_res_tx, comments_res_rx) =
        mpsc::unbounded_channel::<(String, Result<Vec<crate::state::types::AurComment>, String>)>();
    let (status_tx, status_rx) = mpsc::unbounded_channel::<(String, ArchStatusColor)>();
    let (news_tx, news_rx) = mpsc::unbounded_channel::<Vec<NewsItem>>();
    let (updates_tx, updates_rx) = mpsc::unbounded_channel::<(usize, Vec<String>)>();
    let (executor_req_tx, executor_req_rx) =
        mpsc::unbounded_channel::<crate::install::ExecutorRequest>();
    let (executor_res_tx, executor_res_rx) =
        mpsc::unbounded_channel::<crate::install::ExecutorOutput>();
    let (post_summary_req_tx, post_summary_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (post_summary_res_tx, post_summary_res_rx) =
        mpsc::unbounded_channel::<crate::logic::summary::PostSummaryData>();
    UtilityChannels {
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
        pkgb_req_rx,
        pkgb_res_tx,
        pkgb_res_rx,
        comments_req_tx,
        comments_req_rx,
        comments_res_tx,
        comments_res_rx,
        status_tx,
        status_rx,
        news_tx,
        news_rx,
        updates_tx,
        updates_rx,
        executor_req_tx,
        executor_req_rx,
        executor_res_tx,
        executor_res_rx,
        post_summary_req_tx,
        post_summary_req_rx,
        post_summary_res_tx,
        post_summary_res_rx,
    }
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
        let event_channels = create_event_channels();
        let search_channels = create_search_channels();
        let details_channels = create_details_channels();
        let preflight_channels = create_preflight_channels();
        let utility_channels = create_utility_channels();

        // Spawn background workers
        crate::app::runtime::workers::details::spawn_details_worker(
            &utility_channels.net_err_tx,
            details_channels.req_rx,
            details_channels.res_tx.clone(),
        );
        crate::app::runtime::workers::details::spawn_pkgbuild_worker(
            utility_channels.pkgb_req_rx,
            utility_channels.pkgb_res_tx.clone(),
        );
        crate::app::runtime::workers::comments::spawn_comments_worker(
            utility_channels.comments_req_rx,
            utility_channels.comments_res_tx.clone(),
        );
        crate::app::runtime::workers::preflight::spawn_dependency_worker(
            preflight_channels.deps_req_rx,
            preflight_channels.deps_res_tx.clone(),
        );
        crate::app::runtime::workers::preflight::spawn_file_worker(
            preflight_channels.files_req_rx,
            preflight_channels.files_res_tx.clone(),
        );
        crate::app::runtime::workers::preflight::spawn_service_worker(
            preflight_channels.services_req_rx,
            preflight_channels.services_res_tx.clone(),
        );
        crate::app::runtime::workers::preflight::spawn_sandbox_worker(
            preflight_channels.sandbox_req_rx,
            preflight_channels.sandbox_res_tx.clone(),
        );
        crate::app::runtime::workers::preflight::spawn_summary_worker(
            preflight_channels.summary_req_rx,
            preflight_channels.summary_res_tx.clone(),
        );
        crate::app::runtime::workers::search::spawn_search_worker(
            search_channels.query_rx,
            search_channels.result_tx.clone(),
            &utility_channels.net_err_tx,
            index_path,
        );
        crate::app::runtime::workers::executor::spawn_executor_worker(
            utility_channels.executor_req_rx,
            utility_channels.executor_res_tx.clone(),
        );
        spawn_post_summary_worker(
            utility_channels.post_summary_req_rx,
            utility_channels.post_summary_res_tx.clone(),
        );

        Self {
            event_tx: event_channels.tx,
            event_rx: event_channels.rx,
            event_thread_cancelled: event_channels.cancelled,
            search_result_tx: search_channels.result_tx,
            results_rx: search_channels.results_rx,
            details_req_tx: details_channels.req_tx,
            details_res_tx: details_channels.res_tx,
            details_res_rx: details_channels.res_rx,
            tick_tx: utility_channels.tick_tx,
            tick_rx: utility_channels.tick_rx,
            net_err_tx: utility_channels.net_err_tx,
            net_err_rx: utility_channels.net_err_rx,
            preview_tx: utility_channels.preview_tx,
            preview_rx: utility_channels.preview_rx,
            add_tx: utility_channels.add_tx,
            add_rx: utility_channels.add_rx,
            index_notify_tx: utility_channels.index_notify_tx,
            index_notify_rx: utility_channels.index_notify_rx,
            pkgb_req_tx: utility_channels.pkgb_req_tx,
            pkgb_res_tx: utility_channels.pkgb_res_tx,
            pkgb_res_rx: utility_channels.pkgb_res_rx,
            comments_req_tx: utility_channels.comments_req_tx,
            comments_res_tx: utility_channels.comments_res_tx,
            comments_res_rx: utility_channels.comments_res_rx,
            status_tx: utility_channels.status_tx,
            status_rx: utility_channels.status_rx,
            news_tx: utility_channels.news_tx,
            news_rx: utility_channels.news_rx,
            updates_tx: utility_channels.updates_tx,
            updates_rx: utility_channels.updates_rx,
            deps_req_tx: preflight_channels.deps_req_tx,
            deps_res_tx: preflight_channels.deps_res_tx,
            deps_res_rx: preflight_channels.deps_res_rx,
            files_req_tx: preflight_channels.files_req_tx,
            files_res_tx: preflight_channels.files_res_tx,
            files_res_rx: preflight_channels.files_res_rx,
            services_req_tx: preflight_channels.services_req_tx,
            services_res_tx: preflight_channels.services_res_tx,
            services_res_rx: preflight_channels.services_res_rx,
            sandbox_req_tx: preflight_channels.sandbox_req_tx,
            sandbox_res_tx: preflight_channels.sandbox_res_tx,
            sandbox_res_rx: preflight_channels.sandbox_res_rx,
            summary_req_tx: preflight_channels.summary_req_tx,
            summary_res_tx: preflight_channels.summary_res_tx,
            summary_res_rx: preflight_channels.summary_res_rx,
            executor_req_tx: utility_channels.executor_req_tx,
            executor_res_rx: utility_channels.executor_res_rx,
            post_summary_req_tx: utility_channels.post_summary_req_tx,
            post_summary_res_rx: utility_channels.post_summary_res_rx,
            query_tx: search_channels.query_tx,
        }
    }
}

/// What: Spawn background worker for post-summary computation.
///
/// Inputs:
/// - `req_rx`: Channel receiver for post-summary requests (package items)
/// - `res_tx`: Channel sender for post-summary results
///
/// Details:
/// - Runs `compute_post_summary` in a blocking task to avoid blocking the UI
fn spawn_post_summary_worker(
    mut req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    res_tx: mpsc::UnboundedSender<crate::logic::summary::PostSummaryData>,
) {
    tokio::spawn(async move {
        while let Some(items) = req_rx.recv().await {
            let res_tx = res_tx.clone();
            tokio::task::spawn_blocking(move || {
                let data = crate::logic::compute_post_summary(&items);
                let _ = res_tx.send(data);
            });
        }
    });
}
