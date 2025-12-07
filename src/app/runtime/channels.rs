use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crossterm::event::Event as CEvent;
use tokio::sync::mpsc;

use crate::state::types::NewsFeedItem;
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
    /// Sender for terminal events (keyboard/mouse) from the event reading thread.
    pub event_tx: mpsc::UnboundedSender<CEvent>,
    /// Receiver for terminal events in the main event loop.
    pub event_rx: mpsc::UnboundedReceiver<CEvent>,
    /// Atomic flag to signal cancellation of the event reading thread.
    pub event_thread_cancelled: Arc<AtomicBool>,
    /// Sender for search results from the search worker.
    pub search_result_tx: mpsc::UnboundedSender<SearchResults>,
    /// Receiver for search results in the main event loop.
    pub results_rx: mpsc::UnboundedReceiver<SearchResults>,
    /// Sender for package details requests to the details worker.
    pub details_req_tx: mpsc::UnboundedSender<PackageItem>,
    /// Sender for package details responses from the details worker.
    pub details_res_tx: mpsc::UnboundedSender<PackageDetails>,
    /// Receiver for package details responses in the main event loop.
    pub details_res_rx: mpsc::UnboundedReceiver<PackageDetails>,
    /// Sender for tick events to trigger periodic UI updates.
    pub tick_tx: mpsc::UnboundedSender<()>,
    /// Receiver for tick events in the main event loop.
    pub tick_rx: mpsc::UnboundedReceiver<()>,
    /// Sender for network error messages from background workers.
    pub net_err_tx: mpsc::UnboundedSender<String>,
    /// Receiver for network error messages in the main event loop.
    pub net_err_rx: mpsc::UnboundedReceiver<String>,
    /// Sender for preview requests (package details for Recent pane).
    pub preview_tx: mpsc::UnboundedSender<PackageItem>,
    /// Receiver for preview responses in the main event loop.
    pub preview_rx: mpsc::UnboundedReceiver<PackageItem>,
    /// Sender for adding packages to the install list.
    pub add_tx: mpsc::UnboundedSender<PackageItem>,
    /// Receiver for add requests in the install list handler.
    pub add_rx: mpsc::UnboundedReceiver<PackageItem>,
    /// Sender for index update notifications.
    pub index_notify_tx: mpsc::UnboundedSender<()>,
    /// Receiver for index update notifications in the main event loop.
    pub index_notify_rx: mpsc::UnboundedReceiver<()>,
    /// Sender for PKGBUILD content requests.
    pub pkgb_req_tx: mpsc::UnboundedSender<PackageItem>,
    /// Sender for PKGBUILD content responses (package name, PKGBUILD content).
    pub pkgb_res_tx: mpsc::UnboundedSender<(String, String)>,
    /// Receiver for PKGBUILD content responses in the main event loop.
    pub pkgb_res_rx: mpsc::UnboundedReceiver<(String, String)>,
    /// Sender for AUR comments requests (package name).
    pub comments_req_tx: mpsc::UnboundedSender<String>,
    /// Sender for AUR comments responses (package name, comments or error).
    pub comments_res_tx:
        mpsc::UnboundedSender<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    /// Receiver for AUR comments responses in the main event loop.
    pub comments_res_rx:
        mpsc::UnboundedReceiver<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    /// Sender for Arch Linux status updates (status text, color).
    pub status_tx: mpsc::UnboundedSender<(String, ArchStatusColor)>,
    /// Receiver for Arch Linux status updates in the main event loop.
    pub status_rx: mpsc::UnboundedReceiver<(String, ArchStatusColor)>,
    /// Sender for Arch Linux news items.
    pub news_tx: mpsc::UnboundedSender<Vec<NewsItem>>,
    /// Receiver for Arch Linux news items in the main event loop.
    pub news_rx: mpsc::UnboundedReceiver<Vec<NewsItem>>,
    /// Sender for news feed items.
    pub news_feed_tx: mpsc::UnboundedSender<Vec<NewsFeedItem>>,
    /// Receiver for news feed items in the main event loop.
    pub news_feed_rx: mpsc::UnboundedReceiver<Vec<NewsFeedItem>>,
    /// Request channel for fetching news article content (URL).
    pub news_content_req_tx: mpsc::UnboundedSender<String>,
    /// Response channel for news article content (URL, content).
    pub news_content_res_rx: mpsc::UnboundedReceiver<(String, String)>,
    /// Sender for system updates information (count, package names).
    pub updates_tx: mpsc::UnboundedSender<(usize, Vec<String>)>,
    /// Receiver for system updates information in the main event loop.
    pub updates_rx: mpsc::UnboundedReceiver<(usize, Vec<String>)>,
    /// Sender for remote announcements.
    pub announcement_tx: mpsc::UnboundedSender<crate::announcements::RemoteAnnouncement>,
    /// Receiver for remote announcements in the main event loop.
    pub announcement_rx: mpsc::UnboundedReceiver<crate::announcements::RemoteAnnouncement>,
    /// Sender for dependency resolution requests (packages, action).
    pub deps_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for dependency resolution responses.
    pub deps_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::DependencyInfo>>,
    /// Receiver for dependency resolution responses in the main event loop.
    pub deps_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::DependencyInfo>>,
    /// Sender for file analysis requests (packages, action).
    pub files_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for file analysis responses.
    pub files_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::PackageFileInfo>>,
    /// Receiver for file analysis responses in the main event loop.
    pub files_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::PackageFileInfo>>,
    /// Sender for service impact analysis requests (packages, action).
    pub services_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for service impact analysis responses.
    pub services_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::ServiceImpact>>,
    /// Receiver for service impact analysis responses in the main event loop.
    pub services_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::ServiceImpact>>,
    /// Sender for sandbox analysis requests (packages).
    pub sandbox_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    /// Sender for sandbox analysis responses.
    pub sandbox_res_tx: mpsc::UnboundedSender<Vec<crate::logic::sandbox::SandboxInfo>>,
    /// Receiver for sandbox analysis responses in the main event loop.
    pub sandbox_res_rx: mpsc::UnboundedReceiver<Vec<crate::logic::sandbox::SandboxInfo>>,
    /// Sender for preflight summary requests (packages, action).
    pub summary_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for preflight summary responses.
    pub summary_res_tx: mpsc::UnboundedSender<crate::logic::preflight::PreflightSummaryOutcome>,
    /// Receiver for preflight summary responses in the main event loop.
    pub summary_res_rx: mpsc::UnboundedReceiver<crate::logic::preflight::PreflightSummaryOutcome>,
    /// Sender for executor requests (install/remove/downgrade operations).
    pub executor_req_tx: mpsc::UnboundedSender<crate::install::ExecutorRequest>,
    /// Receiver for executor responses in the main event loop.
    pub executor_res_rx: mpsc::UnboundedReceiver<crate::install::ExecutorOutput>,
    /// Sender for post-summary computation requests (packages, success flag).
    pub post_summary_req_tx: mpsc::UnboundedSender<(Vec<PackageItem>, Option<bool>)>,
    /// Receiver for post-summary computation results in the main event loop.
    pub post_summary_res_rx: mpsc::UnboundedReceiver<crate::logic::summary::PostSummaryData>,
    /// Sender for search queries to the search worker.
    pub query_tx: mpsc::UnboundedSender<QueryInput>,
}

/// What: Event channel pair and cancellation flag.
struct EventChannels {
    /// Sender for terminal events.
    tx: mpsc::UnboundedSender<CEvent>,
    /// Receiver for terminal events.
    rx: mpsc::UnboundedReceiver<CEvent>,
    /// Cancellation flag for event thread.
    cancelled: Arc<AtomicBool>,
}

/// What: Search-related channels.
struct SearchChannels {
    /// Sender for search results.
    result_tx: mpsc::UnboundedSender<SearchResults>,
    /// Receiver for search results.
    results_rx: mpsc::UnboundedReceiver<SearchResults>,
    /// Sender for search queries.
    query_tx: mpsc::UnboundedSender<QueryInput>,
    /// Receiver for search queries.
    query_rx: mpsc::UnboundedReceiver<QueryInput>,
}

/// What: Package details channels.
struct DetailsChannels {
    /// Sender for package details requests.
    req_tx: mpsc::UnboundedSender<PackageItem>,
    /// Receiver for package details requests.
    req_rx: mpsc::UnboundedReceiver<PackageItem>,
    /// Sender for package details responses.
    res_tx: mpsc::UnboundedSender<PackageDetails>,
    /// Receiver for package details responses.
    res_rx: mpsc::UnboundedReceiver<PackageDetails>,
}

/// What: Preflight-related channels (dependencies, files, services, sandbox, summary).
struct PreflightChannels {
    /// Sender for dependency resolution requests.
    deps_req_tx: mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Receiver for dependency resolution requests.
    deps_req_rx: mpsc::UnboundedReceiver<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for dependency resolution responses.
    deps_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::DependencyInfo>>,
    /// Receiver for dependency resolution responses.
    deps_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::DependencyInfo>>,
    /// Sender for file analysis requests.
    files_req_tx: mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Receiver for file analysis requests.
    files_req_rx: mpsc::UnboundedReceiver<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for file analysis responses.
    files_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::PackageFileInfo>>,
    /// Receiver for file analysis responses.
    files_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::PackageFileInfo>>,
    /// Sender for service impact analysis requests.
    services_req_tx:
        mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Receiver for service impact analysis requests.
    services_req_rx:
        mpsc::UnboundedReceiver<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for service impact analysis responses.
    services_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::ServiceImpact>>,
    /// Receiver for service impact analysis responses.
    services_res_rx: mpsc::UnboundedReceiver<Vec<crate::state::modal::ServiceImpact>>,
    /// Sender for sandbox analysis requests.
    sandbox_req_tx: mpsc::UnboundedSender<Vec<PackageItem>>,
    /// Receiver for sandbox analysis requests.
    sandbox_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    /// Sender for sandbox analysis responses.
    sandbox_res_tx: mpsc::UnboundedSender<Vec<crate::logic::sandbox::SandboxInfo>>,
    /// Receiver for sandbox analysis responses.
    sandbox_res_rx: mpsc::UnboundedReceiver<Vec<crate::logic::sandbox::SandboxInfo>>,
    /// Sender for preflight summary requests.
    summary_req_tx: mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Receiver for preflight summary requests.
    summary_req_rx:
        mpsc::UnboundedReceiver<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Sender for preflight summary responses.
    summary_res_tx: mpsc::UnboundedSender<crate::logic::preflight::PreflightSummaryOutcome>,
    /// Receiver for preflight summary responses.
    summary_res_rx: mpsc::UnboundedReceiver<crate::logic::preflight::PreflightSummaryOutcome>,
}

/// What: Utility channels (tick, network errors, preview, add, index notify, PKGBUILD, status, news).
struct UtilityChannels {
    /// Sender for tick events.
    tick_tx: mpsc::UnboundedSender<()>,
    /// Receiver for tick events.
    tick_rx: mpsc::UnboundedReceiver<()>,
    /// Sender for network error messages.
    net_err_tx: mpsc::UnboundedSender<String>,
    /// Receiver for network error messages.
    net_err_rx: mpsc::UnboundedReceiver<String>,
    /// Sender for preview requests.
    preview_tx: mpsc::UnboundedSender<PackageItem>,
    /// Receiver for preview requests.
    preview_rx: mpsc::UnboundedReceiver<PackageItem>,
    /// Sender for add to install list requests.
    add_tx: mpsc::UnboundedSender<PackageItem>,
    /// Receiver for add to install list requests.
    add_rx: mpsc::UnboundedReceiver<PackageItem>,
    /// Sender for index update notifications.
    index_notify_tx: mpsc::UnboundedSender<()>,
    /// Receiver for index update notifications.
    index_notify_rx: mpsc::UnboundedReceiver<()>,
    /// Sender for PKGBUILD requests.
    pkgb_req_tx: mpsc::UnboundedSender<PackageItem>,
    /// Receiver for PKGBUILD requests.
    pkgb_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    /// Sender for PKGBUILD responses.
    pkgb_res_tx: mpsc::UnboundedSender<(String, String)>,
    /// Receiver for PKGBUILD responses.
    pkgb_res_rx: mpsc::UnboundedReceiver<(String, String)>,
    /// Sender for AUR comments requests.
    comments_req_tx: mpsc::UnboundedSender<String>,
    /// Receiver for AUR comments requests.
    comments_req_rx: mpsc::UnboundedReceiver<String>,
    /// Sender for AUR comments responses.
    comments_res_tx:
        mpsc::UnboundedSender<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    /// Receiver for AUR comments responses.
    comments_res_rx:
        mpsc::UnboundedReceiver<(String, Result<Vec<crate::state::types::AurComment>, String>)>,
    /// Sender for Arch Linux status updates.
    status_tx: mpsc::UnboundedSender<(String, ArchStatusColor)>,
    /// Receiver for Arch Linux status updates.
    status_rx: mpsc::UnboundedReceiver<(String, ArchStatusColor)>,
    /// Sender for Arch Linux news items.
    news_tx: mpsc::UnboundedSender<Vec<NewsItem>>,
    /// Receiver for Arch Linux news items.
    news_rx: mpsc::UnboundedReceiver<Vec<NewsItem>>,
    /// Sender for news feed items.
    news_feed_tx: mpsc::UnboundedSender<Vec<NewsFeedItem>>,
    /// Receiver for news feed items.
    news_feed_rx: mpsc::UnboundedReceiver<Vec<NewsFeedItem>>,
    /// Sender for news article content requests.
    news_content_req_tx: mpsc::UnboundedSender<String>,
    /// Receiver for news article content requests.
    news_content_req_rx: mpsc::UnboundedReceiver<String>,
    /// Sender for news article content responses.
    news_content_res_tx: mpsc::UnboundedSender<(String, String)>,
    /// Receiver for news article content responses.
    news_content_res_rx: mpsc::UnboundedReceiver<(String, String)>,
    /// Sender for system updates information.
    updates_tx: mpsc::UnboundedSender<(usize, Vec<String>)>,
    /// Receiver for system updates information.
    updates_rx: mpsc::UnboundedReceiver<(usize, Vec<String>)>,
    /// Sender for remote announcements.
    announcement_tx: mpsc::UnboundedSender<crate::announcements::RemoteAnnouncement>,
    /// Receiver for remote announcements.
    announcement_rx: mpsc::UnboundedReceiver<crate::announcements::RemoteAnnouncement>,
    /// Sender for executor requests.
    executor_req_tx: mpsc::UnboundedSender<crate::install::ExecutorRequest>,
    /// Receiver for executor requests.
    executor_req_rx: mpsc::UnboundedReceiver<crate::install::ExecutorRequest>,
    /// Sender for executor responses.
    executor_res_tx: mpsc::UnboundedSender<crate::install::ExecutorOutput>,
    /// Receiver for executor responses.
    executor_res_rx: mpsc::UnboundedReceiver<crate::install::ExecutorOutput>,
    /// Sender for post-summary computation requests.
    post_summary_req_tx: mpsc::UnboundedSender<(Vec<PackageItem>, Option<bool>)>,
    /// Receiver for post-summary computation requests.
    post_summary_req_rx: mpsc::UnboundedReceiver<(Vec<PackageItem>, Option<bool>)>,
    /// Sender for post-summary computation results.
    post_summary_res_tx: mpsc::UnboundedSender<crate::logic::summary::PostSummaryData>,
    /// Receiver for post-summary computation results.
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
    let (files_req_tx, files_req_rx) =
        mpsc::unbounded_channel::<(Vec<PackageItem>, crate::state::modal::PreflightAction)>();
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
    let (news_feed_tx, news_feed_rx) = mpsc::unbounded_channel::<Vec<NewsFeedItem>>();
    let (news_content_req_tx, news_content_req_rx) = mpsc::unbounded_channel::<String>();
    let (news_content_res_tx, news_content_res_rx) = mpsc::unbounded_channel::<(String, String)>();
    let (updates_tx, updates_rx) = mpsc::unbounded_channel::<(usize, Vec<String>)>();
    let (announcement_tx, announcement_rx) =
        mpsc::unbounded_channel::<crate::announcements::RemoteAnnouncement>();
    let (executor_req_tx, executor_req_rx) =
        mpsc::unbounded_channel::<crate::install::ExecutorRequest>();
    let (executor_res_tx, executor_res_rx) =
        mpsc::unbounded_channel::<crate::install::ExecutorOutput>();
    let (post_summary_req_tx, post_summary_req_rx) =
        mpsc::unbounded_channel::<(Vec<PackageItem>, Option<bool>)>();
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
        news_feed_tx,
        news_feed_rx,
        news_content_req_tx,
        news_content_req_rx,
        news_content_res_tx,
        news_content_res_rx,
        updates_tx,
        updates_rx,
        announcement_tx,
        announcement_rx,
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
        crate::app::runtime::workers::news_content::spawn_news_content_worker(
            utility_channels.news_content_req_rx,
            utility_channels.news_content_res_tx.clone(),
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
            news_feed_tx: utility_channels.news_feed_tx,
            news_feed_rx: utility_channels.news_feed_rx,
            news_content_req_tx: utility_channels.news_content_req_tx,
            news_content_res_rx: utility_channels.news_content_res_rx,
            updates_tx: utility_channels.updates_tx,
            updates_rx: utility_channels.updates_rx,
            announcement_tx: utility_channels.announcement_tx,
            announcement_rx: utility_channels.announcement_rx,
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
    mut req_rx: mpsc::UnboundedReceiver<(Vec<PackageItem>, Option<bool>)>,
    res_tx: mpsc::UnboundedSender<crate::logic::summary::PostSummaryData>,
) {
    tokio::spawn(async move {
        while let Some((items, success)) = req_rx.recv().await {
            let res_tx = res_tx.clone();
            tokio::task::spawn_blocking(move || {
                let data = crate::logic::compute_post_summary(&items, success);
                let _ = res_tx.send(data);
            });
        }
    });
}
