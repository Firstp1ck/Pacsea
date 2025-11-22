use std::time::Instant;

use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::logic::send_query;
use crate::state::*;

use super::super::persist::{
    maybe_flush_cache, maybe_flush_deps_cache, maybe_flush_files_cache, maybe_flush_install,
    maybe_flush_news_read, maybe_flush_recent, maybe_flush_sandbox_cache,
    maybe_flush_services_cache,
};
use super::super::recent::maybe_save_recent;

/// What: Handle PKGBUILD result event.
///
/// Inputs:
/// - `app`: Application state
/// - `pkgname`: Package name
/// - `text`: PKGBUILD text
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates PKGBUILD text if still focused on the same package
/// - Clears pending reload request
pub fn handle_pkgbuild_result(
    app: &mut AppState,
    pkgname: String,
    text: String,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    if app.details_focus.as_deref() == Some(pkgname.as_str())
        || app.results.get(app.selected).map(|i| i.name.as_str()) == Some(pkgname.as_str())
    {
        app.pkgb_text = Some(text);
        app.pkgb_package_name = Some(pkgname);
        // Clear any pending debounce request since we've successfully loaded
        app.pkgb_reload_requested_at = None;
        app.pkgb_reload_requested_for = None;
    }
    let _ = tick_tx.send(());
}

/// What: Handle preflight summary result event.
///
/// Inputs:
/// - `app`: Application state
/// - `summary_outcome`: Preflight summary computation result
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates preflight modal with computed summary
/// - Respects cancellation flag
pub fn handle_summary_result(
    app: &mut AppState,
    summary_outcome: crate::logic::preflight::PreflightSummaryOutcome,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating modal
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    if cancelled {
        tracing::debug!("[Runtime] Ignoring summary result (preflight cancelled)");
    } else {
        // Update preflight modal with computed summary
        tracing::info!(
            stage = "summary",
            package_count = summary_outcome.summary.package_count,
            "[Runtime] Preflight summary computation worker completed"
        );
        if let crate::state::Modal::Preflight {
            summary,
            header_chips,
            ..
        } = &mut app.modal
        {
            *summary = Some(Box::new(summary_outcome.summary));
            *header_chips = summary_outcome.header;
        }
    }
    app.preflight_summary_resolving = false;
    // Clear preflight summary items
    app.preflight_summary_items = None;
    let _ = tick_tx.send(());
}

/// What: Check and trigger summary resolution if conditions are met.
fn check_and_trigger_summary_resolution(
    app: &mut AppState,
    summary_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
) {
    if let Some((ref items, ref action)) = app.preflight_summary_items
        && !app.preflight_summary_resolving
    {
        tracing::debug!(
            "[Runtime] Tick: Triggering summary computation for {} items, action={:?}",
            items.len(),
            action
        );
        app.preflight_summary_resolving = true;
        let _ = summary_req_tx.send((items.clone(), *action));
    } else if app.preflight_summary_items.is_some() {
        tracing::debug!(
            "[Runtime] Tick: NOT triggering summary - items={}, preflight_summary_resolving={}",
            app.preflight_summary_items
                .as_ref()
                .map(|(items, _)| items.len())
                .unwrap_or(0),
            app.preflight_summary_resolving
        );
    }
}

/// What: Check and trigger dependency resolution if conditions are met.
fn check_and_trigger_deps_resolution(
    app: &mut AppState,
    deps_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    if let Some(ref items) = app.preflight_deps_items
        && app.preflight_deps_resolving
        && !app.deps_resolving
    {
        tracing::debug!(
            "[Runtime] Tick: Triggering dependency resolution for {} preflight items (preflight_deps_resolving={}, deps_resolving={})",
            items.len(),
            app.preflight_deps_resolving,
            app.deps_resolving
        );
        app.deps_resolving = true;
        let _ = deps_req_tx.send(items.clone());
    } else if app.preflight_deps_items.is_some() {
        tracing::debug!(
            "[Runtime] Tick: NOT triggering deps - items={}, preflight_deps_resolving={}, deps_resolving={}",
            app.preflight_deps_items
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0),
            app.preflight_deps_resolving,
            app.deps_resolving
        );
    }
}

/// What: Check and trigger file resolution if conditions are met.
fn check_and_trigger_files_resolution(
    app: &mut AppState,
    files_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    if let Some(ref items) = app.preflight_files_items
        && app.preflight_files_resolving
        && !app.files_resolving
    {
        tracing::debug!(
            "[Runtime] Tick: Triggering file resolution for {} preflight items (preflight_files_resolving={}, files_resolving={})",
            items.len(),
            app.preflight_files_resolving,
            app.files_resolving
        );
        app.files_resolving = true;
        let _ = files_req_tx.send(items.clone());
    } else if app.preflight_files_items.is_some() {
        tracing::debug!(
            "[Runtime] Tick: NOT triggering files - items={}, preflight_files_resolving={}, files_resolving={}",
            app.preflight_files_items
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0),
            app.preflight_files_resolving,
            app.files_resolving
        );
    }
}

/// What: Check and trigger service resolution if conditions are met.
fn check_and_trigger_services_resolution(
    app: &mut AppState,
    services_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    if let Some(ref items) = app.preflight_services_items
        && app.preflight_services_resolving
        && !app.services_resolving
    {
        app.services_resolving = true;
        let _ = services_req_tx.send(items.clone());
    }
}

/// What: Check and trigger sandbox resolution if conditions are met.
fn check_and_trigger_sandbox_resolution(
    app: &mut AppState,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    if let Some(ref items) = app.preflight_sandbox_items
        && app.preflight_sandbox_resolving
        && !app.sandbox_resolving
    {
        tracing::debug!(
            "[Runtime] Tick: Triggering sandbox resolution for {} preflight items (preflight_sandbox_resolving={}, sandbox_resolving={})",
            items.len(),
            app.preflight_sandbox_resolving,
            app.sandbox_resolving
        );
        app.sandbox_resolving = true;
        let _ = sandbox_req_tx.send(items.clone());
    } else if app.preflight_sandbox_items.is_some() {
        tracing::debug!(
            "[Runtime] Tick: NOT triggering sandbox - items={}, preflight_sandbox_resolving={}, sandbox_resolving={}",
            app.preflight_sandbox_items
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0),
            app.preflight_sandbox_resolving,
            app.sandbox_resolving
        );
    }
}

/// What: Handle preflight resolution requests.
///
/// Inputs:
/// - `app`: Application state
/// - `deps_req_tx`: Channel sender for dependency resolution requests
/// - `files_req_tx`: Channel sender for file resolution requests
/// - `services_req_tx`: Channel sender for service resolution requests
/// - `sandbox_req_tx`: Channel sender for sandbox resolution requests
/// - `summary_req_tx`: Channel sender for summary computation requests
///
/// Output: None
///
/// Details:
/// - Clears queues if preflight is cancelled
/// - Otherwise triggers resolution requests for each preflight stage
fn handle_preflight_resolution(
    app: &mut AppState,
    deps_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    files_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    services_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    summary_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
) {
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    if cancelled {
        // Clear all queues if cancelled
        app.preflight_summary_items = None;
        app.preflight_deps_items = None;
        app.preflight_files_items = None;
        app.preflight_services_items = None;
        app.preflight_sandbox_items = None;
        return;
    }

    // Check for preflight resolution requests - each stage has its own queue
    check_and_trigger_summary_resolution(app, summary_req_tx);
    check_and_trigger_deps_resolution(app, deps_req_tx);
    check_and_trigger_files_resolution(app, files_req_tx);
    check_and_trigger_services_resolution(app, services_req_tx);
    check_and_trigger_sandbox_resolution(app, sandbox_req_tx);
}

/// What: Handle PKGBUILD reload debouncing.
///
/// Inputs:
/// - `app`: Application state
/// - `pkgb_req_tx`: Channel sender for PKGBUILD requests
///
/// Output: None
///
/// Details:
/// - Checks if debounce delay has elapsed
/// - Sends reload request if still on the same package
/// - Clears pending request after processing
fn handle_pkgbuild_reload_debounce(
    app: &mut AppState,
    pkgb_req_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    const PKGBUILD_DEBOUNCE_MS: u64 = 100; // Reduced from 250ms for faster preview loading
    let (Some(requested_at), Some(requested_for)) =
        (app.pkgb_reload_requested_at, &app.pkgb_reload_requested_for)
    else {
        return;
    };

    let elapsed = requested_at.elapsed();
    if elapsed.as_millis() < PKGBUILD_DEBOUNCE_MS as u128 {
        return;
    }

    // Check if the requested package is still the currently selected one
    if let Some(current_item) = app.results.get(app.selected)
        && current_item.name == *requested_for
    {
        // Still on the same package, actually send the request
        let _ = pkgb_req_tx.send(current_item.clone());
    }
    // Clear the pending request
    app.pkgb_reload_requested_at = None;
    app.pkgb_reload_requested_for = None;
}

/// What: Handle installed cache polling logic.
///
/// Inputs:
/// - `app`: Application state
/// - `query_tx`: Channel sender for query input
///
/// Output: None
///
/// Details:
/// - Polls installed/explicit caches if within deadline
/// - Checks if pending installs/removals are complete
/// - Clears tracking when operations complete
fn handle_installed_cache_polling(
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
) {
    let Some(deadline) = app.refresh_installed_until else {
        return;
    };

    let now = Instant::now();
    if now >= deadline {
        app.refresh_installed_until = None;
        app.next_installed_refresh_at = None;
        app.pending_install_names = None;
        return;
    }

    let should_poll = app
        .next_installed_refresh_at
        .map(|t| now >= t)
        .unwrap_or(true);
    if !should_poll {
        return;
    }

    let maybe_pending_installs = app.pending_install_names.clone();
    let maybe_pending_removes = app.pending_remove_names.clone();
    tokio::spawn(async move {
        // Refresh caches in background; ignore errors
        crate::index::refresh_installed_cache().await;
        crate::index::refresh_explicit_cache().await;
    });
    // Schedule next poll ~1s later
    app.next_installed_refresh_at = Some(now + Duration::from_millis(1000));
    // If installed-only mode, results depend on explicit set; re-run query soon
    send_query(app, query_tx);

    // If we are tracking pending installs, check if all are installed now
    if let Some(pending) = maybe_pending_installs {
        let all_installed = pending.iter().all(|n| crate::index::is_installed(n));
        if all_installed {
            // Clear install list and stop tracking
            app.install_list.clear();
            app.install_dirty = true;
            app.pending_install_names = None;
            // Clear dependency cache when install list is cleared
            app.install_list_deps.clear();
            app.install_list_files.clear();
            app.deps_resolving = false;
            app.files_resolving = false;
            // End polling soon to avoid extra work
            app.refresh_installed_until = Some(now + Duration::from_secs(1));
        }
    }

    // If tracking pending removals, log once all are uninstalled
    if let Some(pending_rm) = maybe_pending_removes {
        let all_removed = pending_rm.iter().all(|n| !crate::index::is_installed(n));
        if all_removed {
            if let Err(e) = crate::install::log_removed(&pending_rm) {
                let _ = e; // ignore logging errors
            }

            // Check for config directories after successful removal
            if let Ok(home) = std::env::var("HOME") {
                let mut found_configs = Vec::new();
                for pkg in &pending_rm {
                    let config_dirs = crate::install::check_config_directories(pkg, &home);
                    for dir in config_dirs {
                        found_configs.push((pkg.clone(), dir));
                    }
                }

                if !found_configs.is_empty() {
                    let mut message = String::from(
                        "Configuration directories were found in your home directory:\n\n",
                    );
                    for (pkg, dir) in &found_configs {
                        message.push_str(&format!("  {}: {}\n", pkg, dir.display()));
                    }
                    message.push_str("\nYou may want to manually remove these directories if they are no longer needed.");
                    app.modal = crate::state::Modal::Alert { message };
                }
            }

            app.pending_remove_names = None;
            // End polling soon to avoid extra work
            app.refresh_installed_until = Some(now + Duration::from_secs(1));
        }
    }
}

/// What: Handle tick event (periodic updates).
///
/// Inputs:
/// - `app`: Application state
/// - `query_tx`: Channel sender for query input
/// - `details_req_tx`: Channel sender for detail requests
/// - `pkgb_req_tx`: Channel sender for PKGBUILD requests
/// - `deps_req_tx`: Channel sender for dependency resolution requests
/// - `files_req_tx`: Channel sender for file resolution requests
/// - `services_req_tx`: Channel sender for service resolution requests
/// - `sandbox_req_tx`: Channel sender for sandbox resolution requests
/// - `summary_req_tx`: Channel sender for summary computation requests
///
/// Details:
/// - Flushes caches and persists data
/// - Handles preflight resolution requests
/// - Handles PKGBUILD reload debouncing
/// - Polls installed/explicit caches if needed
/// - Handles ring prefetch, sort menu auto-close, and toast expiration
#[allow(clippy::too_many_arguments)]
pub fn handle_tick(
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_req_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_req_tx: &mpsc::UnboundedSender<PackageItem>,
    deps_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    files_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    services_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    summary_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
) {
    maybe_save_recent(app);
    maybe_flush_cache(app);
    maybe_flush_recent(app);
    maybe_flush_news_read(app);
    maybe_flush_install(app);
    maybe_flush_deps_cache(app);
    maybe_flush_files_cache(app);
    maybe_flush_services_cache(app);
    maybe_flush_sandbox_cache(app);

    handle_preflight_resolution(
        app,
        deps_req_tx,
        files_req_tx,
        services_req_tx,
        sandbox_req_tx,
        summary_req_tx,
    );

    handle_pkgbuild_reload_debounce(app, pkgb_req_tx);

    handle_installed_cache_polling(app, query_tx);

    if app.need_ring_prefetch
        && app
            .ring_resume_at
            .map(|t| std::time::Instant::now() >= t)
            .unwrap_or(false)
    {
        crate::logic::set_allowed_ring(app, 30);
        crate::logic::ring_prefetch_from_selected(app, details_req_tx);
        app.need_ring_prefetch = false;
        app.scroll_moves = 0;
        app.ring_resume_at = None;
    }

    if app.sort_menu_open
        && let Some(deadline) = app.sort_menu_auto_close_at
        && std::time::Instant::now() >= deadline
    {
        app.sort_menu_open = false;
        app.sort_menu_auto_close_at = None;
    }

    if let Some(deadline) = app.toast_expires_at
        && std::time::Instant::now() >= deadline
    {
        app.toast_message = None;
        app.toast_expires_at = None;
    }
}

/// What: Handle news update event.
///
/// Inputs:
/// - `app`: Application state
/// - `todays`: List of news items
///
/// Details:
/// - Shows toast if no new news
/// - Opens news modal if there are unread items
pub fn handle_news(app: &mut AppState, todays: &[NewsItem]) {
    if todays.is_empty() {
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.no_new_news"));
        app.toast_expires_at = Some(Instant::now() + Duration::from_secs(10));
    } else {
        // Show unread news items; default to first selected
        app.modal = Modal::News {
            items: todays.to_vec(),
            selected: 0,
        };
    }
}

/// What: Handle status update event.
///
/// Inputs:
/// - `app`: Application state
/// - `txt`: Status text
/// - `color`: Status color
///
/// Details:
/// - Updates Arch status text and color
pub fn handle_status(app: &mut AppState, txt: String, color: ArchStatusColor) {
    app.arch_status_text = txt;
    app.arch_status_color = color;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Provide a baseline `AppState` for tick handler tests.
    ///
    /// Inputs: None
    /// Output: Fresh `AppState` with default values
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Verify that handle_tick flushes caches when called.
    ///
    /// Inputs:
    /// - App state with cache_dirty = true
    /// - Channel senders
    ///
    /// Output:
    /// - Cache dirty flags may be checked (actual flushing depends on debounce logic)
    ///
    /// Details:
    /// - Tests that tick handler processes cache flushing
    fn handle_tick_processes_cache_flushing() {
        let mut app = new_app();
        app.cache_dirty = true;
        app.deps_cache_dirty = true;
        app.files_cache_dirty = true;
        app.services_cache_dirty = true;
        app.sandbox_cache_dirty = true;

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();

        // Should not panic
        handle_tick(
            &mut app,
            &query_tx,
            &details_tx,
            &pkgb_tx,
            &deps_tx,
            &files_tx,
            &services_tx,
            &sandbox_tx,
            &summary_tx,
        );
    }

    #[test]
    /// What: Verify that handle_tick clears queues when cancelled.
    ///
    /// Inputs:
    /// - App state with cancellation flag set
    /// - Preflight items queued
    ///
    /// Output:
    /// - Queued items are cleared
    ///
    /// Details:
    /// - Tests that cancellation properly clears pending work
    fn handle_tick_clears_queues_when_cancelled() {
        let mut app = new_app();
        app.preflight_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        app.preflight_summary_items = Some((
            vec![crate::state::PackageItem {
                name: "test".to_string(),
                version: "1.0".to_string(),
                description: String::new(),
                source: crate::state::Source::Aur,
                popularity: None,
            }],
            crate::state::modal::PreflightAction::Install,
        ));
        app.preflight_deps_items = Some(vec![]);
        app.preflight_files_items = Some(vec![]);
        app.preflight_services_items = Some(vec![]);
        app.preflight_sandbox_items = Some(vec![]);

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();

        handle_tick(
            &mut app,
            &query_tx,
            &details_tx,
            &pkgb_tx,
            &deps_tx,
            &files_tx,
            &services_tx,
            &sandbox_tx,
            &summary_tx,
        );

        // Queues should be cleared
        assert!(app.preflight_summary_items.is_none());
        assert!(app.preflight_deps_items.is_none());
        assert!(app.preflight_files_items.is_none());
        assert!(app.preflight_services_items.is_none());
        assert!(app.preflight_sandbox_items.is_none());
    }

    #[test]
    /// What: Verify that handle_tick processes PKGBUILD reload debouncing.
    ///
    /// Inputs:
    /// - App state with pending PKGBUILD reload request
    /// - Time elapsed beyond debounce threshold
    ///
    /// Output:
    /// - PKGBUILD request is sent if still on same package
    ///
    /// Details:
    /// - Tests that debouncing works correctly
    fn handle_tick_processes_pkgbuild_debounce() {
        let mut app = new_app();
        app.pkgb_reload_requested_at = Some(Instant::now() - Duration::from_millis(300));
        app.pkgb_reload_requested_for = Some("test-package".to_string());
        app.results = vec![crate::state::PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.selected = 0;

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, mut pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();

        handle_tick(
            &mut app,
            &query_tx,
            &details_tx,
            &pkgb_tx,
            &deps_tx,
            &files_tx,
            &services_tx,
            &sandbox_tx,
            &summary_tx,
        );

        // Request should be sent
        assert!(pkgb_rx.try_recv().is_ok());
        // Pending request should be cleared
        assert!(app.pkgb_reload_requested_at.is_none());
        assert!(app.pkgb_reload_requested_for.is_none());
    }

    #[test]
    /// What: Verify that handle_news shows toast when no new news.
    ///
    /// Inputs:
    /// - App state
    /// - Empty news list
    ///
    /// Output:
    /// - Toast message is set
    /// - Toast expiration is set
    ///
    /// Details:
    /// - Tests that empty news list shows appropriate message
    fn handle_news_shows_toast_when_empty() {
        let mut app = new_app();
        let news: Vec<NewsItem> = vec![];

        handle_news(&mut app, &news);

        // Toast should be set
        assert!(app.toast_message.is_some());
        assert!(app.toast_expires_at.is_some());
    }

    #[test]
    /// What: Verify that handle_news opens modal when news available.
    ///
    /// Inputs:
    /// - App state
    /// - Non-empty news list
    ///
    /// Output:
    /// - News modal is opened
    /// - First item is selected
    ///
    /// Details:
    /// - Tests that news modal is properly opened
    fn handle_news_opens_modal_when_available() {
        let mut app = new_app();
        let news = vec![NewsItem {
            title: "Test News".to_string(),
            url: "https://example.com/news".to_string(),
            date: String::new(),
        }];

        handle_news(&mut app, &news);

        // Modal should be opened
        if let crate::state::Modal::News { items, selected } = &app.modal {
            assert_eq!(items.len(), 1);
            assert_eq!(selected, &0);
        } else {
            panic!("Expected News modal");
        }
    }

    #[test]
    /// What: Verify that handle_status updates status text and color.
    ///
    /// Inputs:
    /// - App state
    /// - Status text and color
    ///
    /// Output:
    /// - Status text is updated
    /// - Status color is updated
    ///
    /// Details:
    /// - Tests that status updates are properly applied
    fn handle_status_updates_text_and_color() {
        let mut app = new_app();
        let txt = "System is up to date".to_string();
        let color = ArchStatusColor::Operational;

        handle_status(&mut app, txt.clone(), color);

        assert_eq!(app.arch_status_text, txt);
        assert_eq!(app.arch_status_color, color);
    }

    #[test]
    /// What: Verify that handle_pkgbuild_result updates text when focused.
    ///
    /// Inputs:
    /// - App state with details_focus set
    /// - PKGBUILD text
    ///
    /// Output:
    /// - PKGBUILD text is updated
    /// - Pending reload request is cleared
    ///
    /// Details:
    /// - Tests that PKGBUILD results are properly handled
    fn handle_pkgbuild_result_updates_when_focused() {
        let mut app = new_app();
        app.details_focus = Some("test-package".to_string());
        app.pkgb_reload_requested_at = Some(Instant::now());
        app.pkgb_reload_requested_for = Some("test-package".to_string());

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        handle_pkgbuild_result(
            &mut app,
            "test-package".to_string(),
            "pkgbuild content".to_string(),
            &tick_tx,
        );

        // PKGBUILD text should be updated
        assert_eq!(app.pkgb_text, Some("pkgbuild content".to_string()));
        assert_eq!(app.pkgb_package_name, Some("test-package".to_string()));
        // Pending request should be cleared
        assert!(app.pkgb_reload_requested_at.is_none());
        assert!(app.pkgb_reload_requested_for.is_none());
    }

    #[test]
    /// What: Verify that handle_summary_result updates modal when not cancelled.
    ///
    /// Inputs:
    /// - App state with preflight modal open
    /// - Summary outcome
    /// - Cancellation flag not set
    ///
    /// Output:
    /// - Summary is updated in modal
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that summary results are properly processed
    fn handle_summary_result_updates_modal() {
        let mut app = new_app();
        app.modal = crate::state::Modal::Preflight {
            items: vec![],
            action: crate::state::modal::PreflightAction::Install,
            tab: crate::state::modal::PreflightTab::Summary,
            summary: None,
            summary_scroll: 0,
            header_chips: crate::state::modal::PreflightHeaderChips {
                package_count: 0,
                download_bytes: 0,
                install_delta_bytes: 0,
                aur_count: 0,
                risk_score: 0,
                risk_level: crate::state::modal::RiskLevel::Low,
            },
            dependency_info: vec![],
            dep_selected: 0,
            dep_tree_expanded: std::collections::HashSet::new(),
            deps_error: None,
            file_info: vec![],
            file_selected: 0,
            file_tree_expanded: std::collections::HashSet::new(),
            files_error: None,
            service_info: vec![],
            service_selected: 0,
            services_loaded: false,
            services_error: None,
            sandbox_info: vec![],
            sandbox_selected: 0,
            sandbox_tree_expanded: std::collections::HashSet::new(),
            sandbox_loaded: false,
            sandbox_error: None,
            selected_optdepends: std::collections::HashMap::new(),
            cascade_mode: crate::state::modal::CascadeMode::Basic,
        };
        app.preflight_summary_resolving = true;
        app.preflight_cancelled
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let summary_outcome = crate::logic::preflight::PreflightSummaryOutcome {
            summary: crate::state::modal::PreflightSummaryData {
                packages: vec![],
                package_count: 1,
                aur_count: 0,
                download_bytes: 1000,
                install_delta_bytes: 500,
                risk_score: 10,
                risk_level: crate::state::modal::RiskLevel::Low,
                risk_reasons: vec![],
                major_bump_packages: vec![],
                core_system_updates: vec![],
                pacnew_candidates: 0,
                pacsave_candidates: 0,
                config_warning_packages: vec![],
                service_restart_units: vec![],
                summary_warnings: vec![],
                summary_notes: vec![],
            },
            header: crate::state::modal::PreflightHeaderChips {
                package_count: 1,
                download_bytes: 1000,
                install_delta_bytes: 500,
                aur_count: 0,
                risk_score: 10,
                risk_level: crate::state::modal::RiskLevel::Low,
            },
        };

        handle_summary_result(&mut app, summary_outcome, &tick_tx);

        // Summary should be updated
        if let crate::state::Modal::Preflight { summary, .. } = &app.modal {
            assert!(summary.is_some());
            assert_eq!(summary.as_ref().unwrap().package_count, 1);
        } else {
            panic!("Expected Preflight modal");
        }
        // Flags should be reset
        assert!(!app.preflight_summary_resolving);
        assert!(app.preflight_summary_items.is_none());
    }
}
