use std::fmt::Write;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::logic::{pkgbuild_check_response_matches_selection, send_query};
use crate::state::app_state::PkgbuildCheckStatus;
use crate::state::{AppState, ArchStatusColor, PackageItem, QueryInput};

use super::super::persist::{
    maybe_flush_announcement_read, maybe_flush_aur_vote_state, maybe_flush_cache,
    maybe_flush_deps_cache, maybe_flush_files_cache, maybe_flush_install,
    maybe_flush_news_bookmarks, maybe_flush_news_content_cache, maybe_flush_news_read,
    maybe_flush_news_read_ids, maybe_flush_news_recent, maybe_flush_news_seen_aur_comments,
    maybe_flush_news_seen_versions, maybe_flush_pkgbuild_parse_cache, maybe_flush_recent,
    maybe_flush_sandbox_cache, maybe_flush_services_cache,
};
use super::super::recent::{maybe_save_news_recent, maybe_save_recent};
use super::workers::UpdateCheckPayload;
use super::workers::aur_vote::{AurVoteRequest, AurVoteStateRequest};
use crate::install::ExecutorRequest;
use crate::sources::VoteAction;

/// What: Queue AUR update after system update success when pacman + AUR were both selected.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output: None
///
/// Details:
/// - When `PreflightExec` shows success, items are empty (system update), and pending AUR command
///   is set, queues the AUR command and updates the modal log. Called from tick to avoid borrow
///   conflict in `event_loop`.
fn maybe_queue_aur_after_system_update_success(app: &mut AppState) {
    let (success_is_true, items_empty) =
        if let crate::state::Modal::PreflightExec { success, items, .. } = &app.modal {
            (success == &Some(true), items.is_empty())
        } else {
            (false, false)
        };
    if !success_is_true || !items_empty || app.pending_aur_update_command.is_none() {
        return;
    }
    let aur_cmd = app.pending_aur_update_command.take();
    let password = app.pending_executor_password.take();
    if let (Some(aur_cmd), Some(password)) = (aur_cmd, password) {
        app.pending_executor_request = Some(ExecutorRequest::Update {
            commands: vec![aur_cmd],
            password: Some(password),
            dry_run: app.dry_run,
        });
        if let crate::state::Modal::PreflightExec {
            log_lines, success, ..
        } = &mut app.modal
        {
            log_lines.push(String::new());
            log_lines.push("Running AUR update...".to_string());
            *success = None;
        }
    }
}

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

/// What: Apply a completed PKGBUILD static-check worker response to the UI when it matches selection.
///
/// Inputs:
/// - `app`: Application state; receives findings, raw output, missing-tool hints, and metadata.
/// - `response`: Parsed check outcome from the details worker.
/// - `tick_tx`: Sender used to request a redraw.
///
/// Output:
/// - Updates check panels and toast, or returns early when the response targets another package.
///
/// Details:
/// - Missing tools are stored only in [`AppState::pkgb_check_missing_tools`]; the PKGBUILD pane
///   renders them under `[missing]` and must not duplicate them as synthetic `ShellCheck` findings.
pub fn handle_pkgbuild_check_result(
    app: &mut AppState,
    response: crate::state::PkgbuildCheckResponse,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    if !pkgbuild_check_response_matches_selection(app, response.package_name.as_str()) {
        return;
    }
    let warning_count = response
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.severity,
                crate::state::app_state::PkgbuildCheckSeverity::Warning
            )
        })
        .count();
    let error_count = response
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.severity,
                crate::state::app_state::PkgbuildCheckSeverity::Error
            )
        })
        .count();
    app.pkgb_check_status = PkgbuildCheckStatus::Complete;
    app.pkgb_check_last_package_name = Some(response.package_name);
    app.pkgb_check_findings = response.findings;
    app.pkgb_check_raw_results = response.raw_results;
    app.pkgb_check_missing_tools = response.missing_tools;
    app.pkgb_check_last_error = response.last_error;
    app.pkgb_check_last_run_at = Some(Instant::now());
    app.pkgb_check_scroll = 0;
    app.pkgb_check_raw_scroll = 0;
    app.pkgb_check_show_raw_output = true;
    app.toast_message = Some(format!(
        "PKGBUILD checks finished: {error_count} error(s), {warning_count} warning(s)"
    ));
    app.toast_expires_at = Some(Instant::now() + Duration::from_secs(4));
    let _ = tick_tx.send(());
}

/// What: Handle comments result event.
///
/// Inputs:
/// - `app`: Application state
/// - `pkgname`: Package name
/// - `result`: Comments result (Ok with comments or Err with error message)
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates comments if still focused on the same package
/// - Sets loading state to false and error state if applicable
pub fn handle_comments_result(
    app: &mut AppState,
    pkgname: String,
    result: Result<Vec<crate::state::types::AurComment>, String>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    if app.details_focus.as_deref() == Some(pkgname.as_str())
        || app.results.get(app.selected).map(|i| i.name.as_str()) == Some(pkgname.as_str())
    {
        app.comments_loading = false;
        match result {
            Ok(comments) => {
                app.comments = comments;
                app.comments_package_name = Some(pkgname);
                app.comments_fetched_at = Some(Instant::now());
                app.comments_error = None;
            }
            Err(error) => {
                app.comments.clear();
                app.comments_package_name = None;
                app.comments_fetched_at = None;
                app.comments_error = Some(error);
            }
        }
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
            cached_reverse_deps_report,
            ..
        } = &mut app.modal
        {
            *summary = Some(Box::new(summary_outcome.summary));
            *header_chips = summary_outcome.header;
            *cached_reverse_deps_report = summary_outcome.reverse_deps_report;
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
    summary_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
) {
    if let Some((items, action)) = app.preflight_summary_items.take()
        && !app.preflight_summary_resolving
    {
        tracing::debug!(
            "[Runtime] Tick: Triggering summary computation for {} items, action={:?}",
            items.len(),
            action
        );
        app.preflight_summary_resolving = true;
        let _ = summary_req_tx.send((items, action));
    } else if app.preflight_summary_items.is_some() {
        tracing::debug!(
            "[Runtime] Tick: NOT triggering summary - items={}, preflight_summary_resolving={}",
            app.preflight_summary_items
                .as_ref()
                .map_or(0, |(items, _)| items.len()),
            app.preflight_summary_resolving
        );
    }
}

/// What: Check and trigger dependency resolution if conditions are met.
fn check_and_trigger_deps_resolution(
    app: &mut AppState,
    deps_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
) {
    let preflight_items_len = app
        .preflight_deps_items
        .as_ref()
        .map_or(0, |(items, _)| items.len());
    let should_log_state =
        (app.preflight_deps_items.is_some() || app.preflight_deps_resolving || app.deps_resolving)
            && (app.last_logged_preflight_deps_state
                != Some((
                    preflight_items_len,
                    app.preflight_deps_resolving,
                    app.deps_resolving,
                )));

    if should_log_state {
        tracing::info!(
            "[Runtime] check_and_trigger_deps_resolution: preflight_deps_items={}, preflight_deps_resolving={}, deps_resolving={}",
            preflight_items_len,
            app.preflight_deps_resolving,
            app.deps_resolving
        );
        app.last_logged_preflight_deps_state = Some((
            preflight_items_len,
            app.preflight_deps_resolving,
            app.deps_resolving,
        ));
    } else if app.preflight_deps_items.is_none()
        && !app.preflight_deps_resolving
        && !app.deps_resolving
    {
        // Reset snapshot once idle so future state transitions log again.
        app.last_logged_preflight_deps_state = None;
    }

    if let Some((items, action)) = app.preflight_deps_items.take()
        && app.preflight_deps_resolving
        && !app.deps_resolving
    {
        tracing::info!(
            "[Runtime] Tick: Triggering dependency resolution for {} preflight items (action={:?}, preflight_deps_resolving={}, deps_resolving={})",
            items.len(),
            action,
            app.preflight_deps_resolving,
            app.deps_resolving
        );
        app.deps_resolving = true;
        let send_result = deps_req_tx.send((items, action));
        tracing::info!(
            "[Runtime] Tick: deps_req_tx.send result: {:?}",
            send_result.is_ok()
        );
    } else if app.preflight_deps_items.is_some() {
        tracing::warn!(
            "[Runtime] Tick: NOT triggering deps - items={}, preflight_deps_resolving={}, deps_resolving={}",
            app.preflight_deps_items
                .as_ref()
                .map_or(0, |(items, _)| items.len()),
            app.preflight_deps_resolving,
            app.deps_resolving
        );
    }
}

/// What: Check and trigger file resolution if conditions are met.
fn check_and_trigger_files_resolution(
    app: &mut AppState,
    files_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
) {
    if let Some(items) = app.preflight_files_items.take()
        && app.preflight_files_resolving
        && !app.files_resolving
    {
        // Get action from preflight modal state
        let action = if let crate::state::Modal::Preflight { action, .. } = &app.modal {
            *action
        } else {
            // Fallback to Install if modal state is unavailable
            crate::state::modal::PreflightAction::Install
        };
        tracing::debug!(
            "[Runtime] Tick: Triggering file resolution for {} preflight items with action={:?} (preflight_files_resolving={}, files_resolving={})",
            items.len(),
            action,
            app.preflight_files_resolving,
            app.files_resolving
        );
        app.files_resolving = true;
        let _ = files_req_tx.send((items, action));
    } else if app.preflight_files_items.is_some() {
        tracing::debug!(
            "[Runtime] Tick: NOT triggering files - items={}, preflight_files_resolving={}, files_resolving={}",
            app.preflight_files_items.as_ref().map_or(0, Vec::len),
            app.preflight_files_resolving,
            app.files_resolving
        );
    }
}

/// What: Check and trigger service resolution if conditions are met.
fn check_and_trigger_services_resolution(
    app: &mut AppState,
    services_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
) {
    if let Some(ref items) = app.preflight_services_items
        && app.preflight_services_resolving
        && !app.services_resolving
    {
        // Get action from preflight modal state
        let action = if let crate::state::Modal::Preflight { action, .. } = &app.modal {
            *action
        } else {
            // Fallback to Install if modal state is unavailable
            crate::state::modal::PreflightAction::Install
        };
        app.services_resolving = true;
        let _ = services_req_tx.send((items.clone(), action));
    }
}

/// What: Check and trigger sandbox resolution if conditions are met.
fn check_and_trigger_sandbox_resolution(
    app: &mut AppState,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    if let Some(items) = app.preflight_sandbox_items.take()
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
        let _ = sandbox_req_tx.send(items);
    } else if app.preflight_sandbox_items.is_some() {
        tracing::debug!(
            "[Runtime] Tick: NOT triggering sandbox - items={}, preflight_sandbox_resolving={}, sandbox_resolving={}",
            app.preflight_sandbox_items.as_ref().map_or(0, Vec::len),
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
    deps_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    files_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    services_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
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
    if elapsed.as_millis() < u128::from(PKGBUILD_DEBOUNCE_MS) {
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

    let should_poll = app.next_installed_refresh_at.is_none_or(|t| now >= t);
    if !should_poll {
        return;
    }

    let maybe_pending_installs = app.pending_install_names.clone();
    let maybe_pending_removes = app.pending_remove_names.clone();
    let installed_mode = app.installed_packages_mode;
    tokio::spawn(async move {
        // Refresh caches in background; ignore errors
        crate::index::refresh_installed_cache().await;
        crate::index::refresh_explicit_cache(installed_mode).await;
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
            app.install_list_names.clear();
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
                        let _ = writeln!(message, "  {pkg}: {}", dir.display());
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

/// What: Queue a file database sync command directly in executor flow.
///
/// Inputs:
/// - `app`: Mutable application state.
/// - `command`: Privileged command string to execute.
///
/// Output:
/// - Sets `PreflightExec` modal and enqueues a `CustomCommand` executor request.
///
/// Details:
/// - Used when auth flow does not require the in-app password modal
///   (e.g., interactive handoff or passwordless-only mode).
fn queue_file_sync_command_without_password(app: &mut AppState, command: String) {
    let header_chips = crate::state::modal::PreflightHeaderChips::default();
    app.modal = crate::state::Modal::PreflightExec {
        items: Vec::new(),
        action: crate::state::PreflightAction::Install,
        tab: crate::state::PreflightTab::Summary,
        verbose: false,
        log_lines: Vec::new(),
        abortable: false,
        header_chips,
        success: None,
    };
    app.pending_executor_request = Some(ExecutorRequest::CustomCommand {
        command,
        password: None,
        dry_run: app.dry_run,
    });
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
// Function is 151 lines, just 1 line over the threshold. Refactoring would require
// significant restructuring of the tick handling logic which would reduce readability.
#[allow(clippy::too_many_lines)]
// Function has 205 lines - handles periodic tasks (cache flushing, faillock checks, news content timeouts, preflight resolution, executor requests) that require sequential processing
#[allow(clippy::cognitive_complexity)] // Tick processing is intentionally centralized to preserve deterministic update ordering across state transitions.
pub fn handle_tick(
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_req_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_req_tx: &mpsc::UnboundedSender<PackageItem>,
    deps_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    files_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    services_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    summary_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
    updates_tx: &mpsc::UnboundedSender<UpdateCheckPayload>,
    aur_vote_req_tx: &mpsc::UnboundedSender<AurVoteRequest>,
    aur_vote_state_req_tx: &mpsc::UnboundedSender<AurVoteStateRequest>,
    executor_req_tx: &mpsc::UnboundedSender<crate::install::ExecutorRequest>,
    post_summary_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, Option<bool>)>,
    news_content_req_tx: &mpsc::UnboundedSender<String>,
) {
    // Check faillock status periodically (every minute via worker, but also check here)
    // We check every tick but only update if enough time has passed
    static LAST_FAILLOCK_CHECK: std::sync::OnceLock<std::sync::Mutex<Instant>> =
        std::sync::OnceLock::new();
    maybe_save_recent(app);
    maybe_flush_aur_vote_state(app);
    maybe_save_news_recent(app);
    maybe_flush_cache(app);
    maybe_flush_recent(app);
    maybe_flush_news_recent(app);
    maybe_flush_news_bookmarks(app);
    maybe_flush_news_content_cache(app);
    maybe_flush_news_read(app);
    maybe_flush_news_read_ids(app);
    maybe_flush_news_seen_versions(app);
    maybe_flush_news_seen_aur_comments(app);
    maybe_flush_announcement_read(app);
    maybe_flush_install(app);
    maybe_flush_deps_cache(app);
    maybe_flush_files_cache(app);
    maybe_flush_services_cache(app);
    maybe_flush_sandbox_cache(app);
    maybe_flush_pkgbuild_parse_cache();
    let last_check = LAST_FAILLOCK_CHECK.get_or_init(|| std::sync::Mutex::new(Instant::now()));
    if let Ok(mut last_check_guard) = last_check.lock()
        && last_check_guard.elapsed().as_secs() >= 60
    {
        *last_check_guard = Instant::now();
        drop(last_check_guard);
        // Check faillock status
        let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        let (is_locked, lockout_until, remaining_minutes) =
            crate::logic::faillock::get_lockout_info(&username);

        // If user was locked but is now unlocked, close any lockout alert modal
        if app.faillock_locked && !is_locked {
            // User is no longer locked - close lockout alert if it's showing
            if let crate::state::Modal::Alert { message } = &app.modal
                && (message.contains("locked") || message.contains("lockout"))
            {
                app.modal = crate::state::Modal::None;
            }
        }

        app.faillock_locked = is_locked;
        app.faillock_lockout_until = lockout_until;
        app.faillock_remaining_minutes = remaining_minutes;
    }

    // Timeout guard for news content fetches to avoid stuck "Loading content..."
    // Only check timeout if main news feed is not loading (to avoid showing timeout toast during initial load)
    if app.news_content_loading && !app.news_loading {
        if let Some(started) = app.news_content_loading_since {
            if started.elapsed() > std::time::Duration::from_secs(10) {
                let url = app
                    .news_results
                    .get(app.news_selected)
                    .and_then(|it| it.url.clone());
                tracing::warn!(
                    selected = app.news_selected,
                    url = ?url,
                    elapsed_ms = started.elapsed().as_millis(),
                    "news_content: timed out waiting for response"
                );
                app.news_content_loading = false;
                app.news_content_loading_since = None;
                app.news_content = Some("Failed to load content: timed out after 10s".to_string());
                app.toast_message = Some("News content timed out".to_string());
                app.toast_expires_at = Some(Instant::now() + std::time::Duration::from_secs(3));
            } else {
                tracing::trace!(
                    selected = app.news_selected,
                    elapsed_ms = started.elapsed().as_millis(),
                    "news_content: still loading"
                );
            }
        } else {
            // Ensure we set a start time if missing for safety
            app.news_content_loading_since = Some(Instant::now());
        }
    }

    // Refresh updates list if flag is set (manual refresh via button click)
    if app.refresh_updates {
        app.refresh_updates = false;
        app.updates_loading = true;
        crate::app::runtime::workers::updates::spawn_updates_worker(updates_tx.clone());
    }

    // Request news content if in news mode and content not cached
    crate::events::utils::maybe_request_news_content(app, news_content_req_tx);

    handle_preflight_resolution(
        app,
        deps_req_tx,
        files_req_tx,
        services_req_tx,
        sandbox_req_tx,
        summary_req_tx,
    );

    maybe_queue_aur_after_system_update_success(app);

    // Send pending AUR vote request.
    if let Some((pkgbase, action)) = app.pending_aur_vote_request.take() {
        let settings = crate::theme::settings();
        let request = AurVoteRequest {
            pkgbase,
            action,
            dry_run: app.dry_run,
            ssh_timeout_secs: settings.aur_vote_ssh_timeout_seconds,
            ssh_command: settings.aur_vote_ssh_command,
        };
        if let Err(err) = aur_vote_req_tx.send(request) {
            let action_label = match action {
                VoteAction::Vote => "vote",
                VoteAction::Unvote => "unvote",
            };
            app.modal = crate::state::Modal::Alert {
                message: format!("Failed to queue AUR {action_label} request: {err}"),
            };
        }
    }

    // Send pending AUR vote-state check request.
    if let Some(pkgbase) = app.pending_aur_vote_state_request.take() {
        let settings = crate::theme::settings();
        let request = AurVoteStateRequest {
            pkgbase: pkgbase.clone(),
            ssh_timeout_secs: settings.aur_vote_ssh_timeout_seconds,
            ssh_command: settings.aur_vote_ssh_command,
        };
        if let Err(err) = aur_vote_state_req_tx.send(request) {
            app.modal = crate::state::Modal::Alert {
                message: format!("Failed to queue AUR vote-state request for '{pkgbase}': {err}"),
            };
        }
    }

    // Send pending executor request if PreflightExec modal is active
    if let Some(request) = app.pending_executor_request.take()
        && matches!(app.modal, crate::state::Modal::PreflightExec { .. })
        && let Err(e) = executor_req_tx.send(request)
    {
        tracing::error!("Failed to send executor request: {:?}", e);
    }

    // Send pending post-summary request if Loading modal is active
    if let Some((items, success)) = app.pending_post_summary_items.take()
        && matches!(app.modal, crate::state::Modal::Loading { .. })
        && let Err(e) = post_summary_req_tx.send((items, success))
    {
        tracing::error!("Failed to send post-summary request: {:?}", e);
    }

    // Check file database sync result from background thread
    if let Some(sync_result_arc) = app.pending_file_sync_result.take()
        && let Ok(mut sync_result) = sync_result_arc.lock()
        && let Some(result) = sync_result.take()
    {
        match result {
            Ok(synced) => {
                // Sync succeeded
                if synced {
                    app.toast_message =
                        Some("File database sync completed successfully".to_string());
                } else {
                    app.toast_message = Some("File database is already fresh".to_string());
                }
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            }
            Err(_e) => {
                let command = match crate::logic::privilege::active_tool() {
                    Ok(tool) => Some(crate::logic::privilege::build_privilege_command(
                        tool,
                        "pacman -Fy",
                    )),
                    Err(msg) => {
                        app.toast_message = Some(msg);
                        app.toast_expires_at =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
                        None
                    }
                };
                if let Some(command) = command {
                    let settings = crate::theme::settings();
                    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
                        match crate::events::try_interactive_auth_handoff() {
                            Ok(true) => {
                                queue_file_sync_command_without_password(app, command);
                            }
                            Ok(false) => {
                                app.modal = crate::state::Modal::Alert {
                                    message: crate::i18n::t(
                                        app,
                                        "app.errors.authentication_failed",
                                    ),
                                };
                            }
                            Err(err) => {
                                app.modal = crate::state::Modal::Alert { message: err };
                            }
                        }
                    } else if crate::logic::password::should_use_passwordless_sudo(&settings) {
                        queue_file_sync_command_without_password(app, command);
                    } else {
                        // Sync failed, show password prompt
                        app.modal = crate::state::Modal::PasswordPrompt {
                            purpose: crate::state::modal::PasswordPurpose::FileSync,
                            items: Vec::new(), // No packages involved in file sync
                            input: String::new(),
                            cursor: 0,
                            error: None,
                        };
                        // Store the command to execute after password is provided
                        app.pending_custom_command = Some(command);
                        app.pending_exec_header_chips =
                            Some(crate::state::modal::PreflightHeaderChips::default());
                    }
                }
            }
        }
    }

    // Check background AUR SSH validation result and refresh OptionalDeps row if open.
    if let Some(check_result_arc) = app.pending_aur_ssh_help_check_result.take() {
        if let Ok(mut check_result) = check_result_arc.lock()
            && let Some(is_ready) = check_result.take()
        {
            app.aur_ssh_help_ready = Some(is_ready);
            if let crate::state::Modal::OptionalDeps { rows, .. } = &mut app.modal
                && let Some(row) = rows.iter_mut().find(|row| row.package == "aur-ssh-setup")
            {
                row.installed = is_ready;
                row.selectable = !is_ready;
                row.note = Some(if is_ready {
                    "Validated".to_string()
                } else {
                    "Setup".to_string()
                });
            }
            if let crate::state::Modal::StartupSetupSelector { selected, .. } = &mut app.modal
                && is_ready
            {
                selected.remove(&crate::state::modal::StartupSetupTask::SshAurSetup);
            }
        } else {
            app.pending_aur_ssh_help_check_result = Some(check_result_arc);
        }
    }

    handle_pkgbuild_reload_debounce(app, pkgb_req_tx);

    handle_installed_cache_polling(app, query_tx);

    if app.need_ring_prefetch
        && app
            .ring_resume_at
            .is_some_and(|t| std::time::Instant::now() >= t)
    {
        crate::logic::set_allowed_ring(app, 30);
        crate::logic::ring_prefetch_from_selected(app, details_req_tx);
        app.need_ring_prefetch = false;
        app.scroll_moves = 0;
        app.ring_resume_at = None;
    }

    // Clear expired toast, but don't clear news loading toast while news are still loading
    if let Some(deadline) = app.toast_expires_at
        && std::time::Instant::now() >= deadline
    {
        // Only prevent clearing if it's the actual news loading toast and news are still loading
        let is_news_loading_toast = app.toast_message.as_ref().is_some_and(|msg| {
            let loading_msg = crate::i18n::t(app, "app.news_button.loading");
            msg == &loading_msg
        });
        if !is_news_loading_toast || !app.news_loading {
            app.toast_message = None;
            app.toast_expires_at = None;
        }
    }
}

/// What: Handle news update event.
///
/// Inputs:
/// - `app`: Application state
/// - `items`: List of news feed items
///
/// Details:
/// - Shows toast if no new news
/// - Opens news modal if there are unread items
/// - Clears `news_loading` flag only when news modal is actually shown
pub fn handle_news(app: &mut AppState, items: &[crate::state::types::NewsFeedItem]) {
    tracing::info!(
        items_count = items.len(),
        current_modal = ?app.modal,
        news_loading = app.news_loading,
        "handle_news called"
    );
    // Don't clear news_loading or toast here - the main news feed pane may still be loading.
    // The loading toast and flag will be cleared when handle_news_feed_items receives the aggregated feed.

    if items.is_empty() {
        // No news available - set ready flag to false
        tracing::info!("no news items, marking as not ready");
        app.news_ready = false;
    } else {
        // News are ready - set flag and store items for button click
        tracing::info!("news items available, marking as ready");
        app.news_ready = true;
        // Store news items for later display when button is clicked
        // Convert NewsFeedItem to NewsItem for pending_news (legacy format)
        let legacy_items: Vec<crate::state::NewsItem> = items
            .iter()
            .filter_map(|item| {
                item.url.as_ref().map(|url| crate::state::NewsItem {
                    date: item.date.clone(),
                    title: item.title.clone(),
                    url: url.clone(),
                })
            })
            .collect();
        app.pending_news = Some(legacy_items);
    }
}

/// What: Handle status update event.
///
/// Inputs:
/// - `app`: Application state
/// - `txt`: Status text (in English, will be translated)
/// - `color`: Status color
///
/// Details:
/// - Translates status text to current locale
/// - Updates Arch status text and color
pub fn handle_status(app: &mut AppState, txt: &str, color: ArchStatusColor) {
    use crate::sources::status::translate;
    app.arch_status_text = translate::translate_status_text(app, txt);
    app.arch_status_color = color;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::ExecutorRequest;
    use crate::state::Modal;
    use crate::state::modal::{PreflightAction, PreflightTab};

    /// What: Provide a baseline `AppState` for tick handler tests.
    ///
    /// Inputs: None
    /// Output: Fresh `AppState` with default values
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Verify that `handle_tick` flushes caches when called.
    ///
    /// Inputs:
    /// - `AppState` with `cache_dirty` = true
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
        let (updates_tx, _updates_rx) = mpsc::unbounded_channel();
        let (aur_vote_req_tx, _aur_vote_req_rx) = mpsc::unbounded_channel();
        let (aur_vote_state_req_tx, _aur_vote_state_req_rx) = mpsc::unbounded_channel();
        let (executor_req_tx, _executor_req_rx) = mpsc::unbounded_channel();
        let (post_summary_req_tx, _post_summary_req_rx) = mpsc::unbounded_channel();
        let (news_content_req_tx, _news_content_req_rx) = mpsc::unbounded_channel();

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
            &updates_tx,
            &aur_vote_req_tx,
            &aur_vote_state_req_tx,
            &executor_req_tx,
            &post_summary_req_tx,
            &news_content_req_tx,
        );
    }

    #[test]
    /// What: Verify that `handle_tick` clears queues when cancelled.
    ///
    /// Inputs:
    /// - `AppState` with cancellation flag set
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
                out_of_date: None,
                orphaned: false,
            }],
            crate::state::modal::PreflightAction::Install,
        ));
        app.preflight_deps_items = Some((vec![], crate::state::modal::PreflightAction::Install));
        app.preflight_files_items = Some(vec![]);
        app.preflight_services_items = Some(vec![]);
        app.preflight_sandbox_items = Some(vec![]);

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel::<(
            Vec<crate::state::PackageItem>,
            crate::state::modal::PreflightAction,
        )>();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();
        let (updates_tx, _updates_rx) = mpsc::unbounded_channel();
        let (aur_vote_req_tx, _aur_vote_req_rx) = mpsc::unbounded_channel();
        let (aur_vote_state_req_tx, _aur_vote_state_req_rx) = mpsc::unbounded_channel();
        let (executor_req_tx, _executor_req_rx) = mpsc::unbounded_channel();
        let (post_summary_req_tx, _post_summary_req_rx) = mpsc::unbounded_channel();
        let (news_content_req_tx, _news_content_req_rx) = mpsc::unbounded_channel();

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
            &updates_tx,
            &aur_vote_req_tx,
            &aur_vote_state_req_tx,
            &executor_req_tx,
            &post_summary_req_tx,
            &news_content_req_tx,
        );

        // Queues should be cleared
        assert!(app.preflight_summary_items.is_none());
        assert!(app.preflight_deps_items.is_none());
        assert!(app.preflight_files_items.is_none());
        assert!(app.preflight_services_items.is_none());
        assert!(app.preflight_sandbox_items.is_none());
    }

    #[test]
    /// What: Verify AUR update is queued after system update success when pacman + AUR were selected.
    ///
    /// Inputs:
    /// - `AppState` with `PreflightExec` (success, empty items), `pending_aur_update_command`,
    ///   and `pending_executor_password` set; tick handler called.
    ///
    /// Output:
    /// - `pending_executor_request` is `Some(Update { commands: [aur_cmd], ... })`,
    ///   `pending_aur_update_command` is `None`, modal log contains "Running AUR update...".
    ///
    /// Details:
    /// - Ensures "Update system" with all selections (including AUR) runs AUR after pacman succeeds.
    fn handle_tick_queues_aur_after_system_update_success() {
        let mut app = new_app();
        let aur_cmd = "paru -Sua --noconfirm".to_string();
        app.modal = Modal::PreflightExec {
            items: vec![],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec!["Done.".to_string()],
            abortable: false,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            success: Some(true),
        };
        app.pending_aur_update_command = Some(aur_cmd.clone());
        app.pending_executor_password = Some("pass".to_string());

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();
        let (updates_tx, _updates_rx) = mpsc::unbounded_channel();
        let (aur_vote_req_tx, _aur_vote_req_rx) = mpsc::unbounded_channel();
        let (aur_vote_state_req_tx, _aur_vote_state_req_rx) = mpsc::unbounded_channel();
        let (executor_req_tx, mut executor_req_rx) = mpsc::unbounded_channel();
        let (post_summary_tx, _post_summary_rx) = mpsc::unbounded_channel();
        let (news_content_tx, _news_content_rx) = mpsc::unbounded_channel();

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
            &updates_tx,
            &aur_vote_req_tx,
            &aur_vote_state_req_tx,
            &executor_req_tx,
            &post_summary_tx,
            &news_content_tx,
        );

        assert!(
            app.pending_aur_update_command.is_none(),
            "pending_aur_update_command should be taken when AUR is queued"
        );
        let request = executor_req_rx
            .try_recv()
            .expect("executor should receive Update request with AUR command");
        match &request {
            ExecutorRequest::Update {
                commands, password, ..
            } => {
                assert_eq!(commands.len(), 1);
                assert_eq!(commands[0], aur_cmd);
                assert_eq!(password.as_deref(), Some("pass"));
            }
            _ => panic!("expected ExecutorRequest::Update, got {request:?}"),
        }
        if let Modal::PreflightExec {
            log_lines, success, ..
        } = &app.modal
        {
            assert!(
                log_lines.iter().any(|l| l == "Running AUR update..."),
                "modal log should contain 'Running AUR update...'"
            );
            assert!(success.is_none(), "success should be cleared for AUR run");
        } else {
            panic!("modal should still be PreflightExec");
        }
    }

    #[test]
    /// What: Verify that `handle_tick` processes `PKGBUILD` reload debouncing.
    ///
    /// Inputs:
    /// - `AppState` with pending `PKGBUILD` reload request
    /// - Time elapsed beyond debounce threshold
    ///
    /// Output:
    /// - `PKGBUILD` request is sent if still on same package
    ///
    /// Details:
    /// - Tests that debouncing works correctly
    fn handle_tick_processes_pkgbuild_debounce() {
        let mut app = new_app();
        app.pkgb_reload_requested_at = Some(
            Instant::now()
                .checked_sub(Duration::from_millis(300))
                .unwrap_or_else(Instant::now),
        );
        app.pkgb_reload_requested_for = Some("test-package".to_string());
        app.results = vec![crate::state::PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
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
        let (updates_tx, _updates_rx) = mpsc::unbounded_channel();
        let (aur_vote_req_tx, _aur_vote_req_rx) = mpsc::unbounded_channel();
        let (aur_vote_state_req_tx, _aur_vote_state_req_rx) = mpsc::unbounded_channel();
        let (executor_req_tx, _executor_req_rx) = mpsc::unbounded_channel();
        let (post_summary_req_tx, _post_summary_req_rx) = mpsc::unbounded_channel();
        let (news_content_req_tx, _news_content_req_rx) = mpsc::unbounded_channel();

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
            &updates_tx,
            &aur_vote_req_tx,
            &aur_vote_state_req_tx,
            &executor_req_tx,
            &post_summary_req_tx,
            &news_content_req_tx,
        );

        // Request should be sent
        assert!(pkgb_rx.try_recv().is_ok());
        // Pending request should be cleared
        assert!(app.pkgb_reload_requested_at.is_none());
        assert!(app.pkgb_reload_requested_for.is_none());
    }

    #[test]
    /// What: Verify that `handle_tick` dispatches pending AUR vote-state requests.
    ///
    /// Inputs:
    /// - `AppState` with `pending_aur_vote_state_request` set.
    ///
    /// Output:
    /// - Vote-state request is sent to worker channel and pending request is cleared.
    ///
    /// Details:
    /// - Ensures tick-based runtime dispatch for live vote-state checks.
    fn handle_tick_dispatches_pending_aur_vote_state_request() {
        let mut app = new_app();
        app.pending_aur_vote_state_request = Some("pacsea-bin".to_string());

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();
        let (updates_tx, _updates_rx) = mpsc::unbounded_channel();
        let (aur_vote_req_tx, _aur_vote_req_rx) = mpsc::unbounded_channel();
        let (aur_vote_state_req_tx, mut aur_vote_state_req_rx) = mpsc::unbounded_channel();
        let (executor_req_tx, _executor_req_rx) = mpsc::unbounded_channel();
        let (post_summary_req_tx, _post_summary_req_rx) = mpsc::unbounded_channel();
        let (news_content_req_tx, _news_content_req_rx) = mpsc::unbounded_channel();

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
            &updates_tx,
            &aur_vote_req_tx,
            &aur_vote_state_req_tx,
            &executor_req_tx,
            &post_summary_req_tx,
            &news_content_req_tx,
        );

        assert!(app.pending_aur_vote_state_request.is_none());
        let request = aur_vote_state_req_rx
            .try_recv()
            .expect("vote-state request should be sent");
        assert_eq!(request.pkgbase, "pacsea-bin");
    }

    #[test]
    /// What: Verify that `handle_news` shows toast when no new news.
    ///
    /// Inputs:
    /// - `AppState`
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
        let news: Vec<crate::state::types::NewsFeedItem> = vec![];

        handle_news(&mut app, &news);

        // News should not be ready
        assert!(!app.news_ready);
        // Toast should be cleared
        assert!(app.toast_message.is_none());
        assert!(app.toast_expires_at.is_none());
    }

    #[test]
    /// What: Verify that `handle_news` sets `news_ready` and stores news for button click.
    ///
    /// Inputs:
    /// - `AppState`
    /// - Non-empty news list
    ///
    /// Output:
    /// - `news_ready` is true
    /// - `pending_news` is set with news items
    /// - Modal is NOT automatically opened (waiting for button click)
    ///
    /// Details:
    /// - Tests that news are marked as ready and stored for later display
    fn handle_news_opens_modal_when_available() {
        let mut app = new_app();
        let news = vec![crate::state::types::NewsFeedItem {
            id: "https://example.com/news".to_string(),
            date: String::new(),
            title: "Test News".to_string(),
            summary: None,
            url: Some("https://example.com/news".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        }];

        handle_news(&mut app, &news);

        // News should be ready
        assert!(app.news_ready);
        // Pending news should be set
        assert!(app.pending_news.is_some());
        if let Some(pending) = &app.pending_news {
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].title, "Test News");
        }
        // Modal should NOT be automatically opened (waiting for button click)
        assert!(matches!(app.modal, crate::state::Modal::None));
    }

    #[test]
    /// What: Verify file-sync auth failure uses interactive handoff path when configured.
    ///
    /// Inputs:
    /// - `AppState` containing a failed `pending_file_sync_result`.
    /// - Integration test env with `PACSEA_TEST_AUTH_MODE=interactive`,
    ///   `PACSEA_TEST_PRIVILEGE_AVAILABLE=sudo`, and `PACSEA_TEST_HEADLESS=1`.
    ///
    /// Output:
    /// - Sets `PreflightExec` modal.
    /// - Queues `ExecutorRequest::CustomCommand` with `password: None`.
    ///
    /// Details:
    /// - Regression test for file sync: interactive mode must not force `PasswordPrompt`.
    fn handle_tick_file_sync_failure_interactive_queues_passwordless_custom_command() {
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_AUTH_MODE", "interactive");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::set_var("PACSEA_TEST_HEADLESS", "1");
        }

        let mut app = new_app();
        app.dry_run = true;
        app.pending_file_sync_result = Some(std::sync::Arc::new(std::sync::Mutex::new(Some(Err(
            "permission denied".to_string(),
        )))));

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();
        let (updates_tx, _updates_rx) = mpsc::unbounded_channel();
        let (aur_vote_req_tx, _aur_vote_req_rx) = mpsc::unbounded_channel();
        let (aur_vote_state_req_tx, _aur_vote_state_req_rx) = mpsc::unbounded_channel();
        let (executor_req_tx, _executor_req_rx) = mpsc::unbounded_channel();
        let (post_summary_req_tx, _post_summary_req_rx) = mpsc::unbounded_channel();
        let (news_content_req_tx, _news_content_req_rx) = mpsc::unbounded_channel();

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
            &updates_tx,
            &aur_vote_req_tx,
            &aur_vote_state_req_tx,
            &executor_req_tx,
            &post_summary_req_tx,
            &news_content_req_tx,
        );

        assert!(
            matches!(app.modal, Modal::PreflightExec { .. }),
            "Interactive mode should queue direct execution for file sync failures"
        );
        match &app.pending_executor_request {
            Some(ExecutorRequest::CustomCommand {
                command, password, ..
            }) => {
                assert!(command.contains("pacman -Fy"));
                assert!(
                    password.is_none(),
                    "Interactive file sync command must not carry a password"
                );
            }
            other => {
                panic!("Expected CustomCommand executor request, got {other:?}");
            }
        }

        unsafe {
            std::env::remove_var("PACSEA_TEST_HEADLESS");
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }

    #[test]
    /// What: Verify file-sync auth failure bypasses in-app prompt for doas in prompt mode.
    ///
    /// Inputs:
    /// - `AppState` containing a failed `pending_file_sync_result`.
    /// - Integration test env with `PACSEA_TEST_AUTH_MODE=prompt`,
    ///   `PACSEA_TEST_PRIVILEGE_AVAILABLE=doas`, and `PACSEA_TEST_HEADLESS=1`.
    ///
    /// Output:
    /// - Sets `PreflightExec` modal.
    /// - Queues `ExecutorRequest::CustomCommand` with `password: None`.
    ///
    /// Details:
    /// - Regression test: doas cannot validate stdin passwords, so prompt mode must not
    ///   route through `PasswordPrompt`.
    fn handle_tick_file_sync_failure_prompt_doas_queues_tool_managed_auth_command() {
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_AUTH_MODE", "prompt");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "doas");
            std::env::set_var("PACSEA_TEST_HEADLESS", "1");
        }

        let mut app = new_app();
        app.dry_run = true;
        app.pending_file_sync_result = Some(std::sync::Arc::new(std::sync::Mutex::new(Some(Err(
            "permission denied".to_string(),
        )))));

        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        let (deps_tx, _deps_rx) = mpsc::unbounded_channel();
        let (files_tx, _files_rx) = mpsc::unbounded_channel();
        let (services_tx, _services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, _sandbox_rx) = mpsc::unbounded_channel();
        let (summary_tx, _summary_rx) = mpsc::unbounded_channel();
        let (updates_tx, _updates_rx) = mpsc::unbounded_channel();
        let (aur_vote_req_tx, _aur_vote_req_rx) = mpsc::unbounded_channel();
        let (aur_vote_state_req_tx, _aur_vote_state_req_rx) = mpsc::unbounded_channel();
        let (executor_req_tx, _executor_req_rx) = mpsc::unbounded_channel();
        let (post_summary_req_tx, _post_summary_req_rx) = mpsc::unbounded_channel();
        let (news_content_req_tx, _news_content_req_rx) = mpsc::unbounded_channel();

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
            &updates_tx,
            &aur_vote_req_tx,
            &aur_vote_state_req_tx,
            &executor_req_tx,
            &post_summary_req_tx,
            &news_content_req_tx,
        );

        assert!(
            matches!(app.modal, Modal::PreflightExec { .. }),
            "Prompt mode with doas should not open PasswordPrompt"
        );
        match &app.pending_executor_request {
            Some(ExecutorRequest::CustomCommand {
                command, password, ..
            }) => {
                assert!(command.contains("doas pacman -Fy"));
                assert!(
                    password.is_none(),
                    "doas file sync command must not carry a password"
                );
            }
            other => {
                panic!("Expected CustomCommand executor request, got {other:?}");
            }
        }

        unsafe {
            std::env::remove_var("PACSEA_TEST_HEADLESS");
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }

    #[test]
    /// What: Verify that `handle_status` updates status text and color.
    ///
    /// Inputs:
    /// - `AppState`
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

        handle_status(&mut app, &txt, color);

        assert_eq!(app.arch_status_text, txt);
        assert_eq!(app.arch_status_color, color);
    }

    #[test]
    /// What: Verify that `handle_pkgbuild_result` updates text when focused.
    ///
    /// Inputs:
    /// - `AppState` with `details_focus` set
    /// - `PKGBUILD` text
    ///
    /// Output:
    /// - `PKGBUILD` text is updated
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
    /// What: Verify PKGBUILD check results are ignored when the user selected another package.
    ///
    /// Inputs:
    /// - `AppState` with results list and selection pointing at `other`, plus prior idle check state.
    /// - A `PkgbuildCheckResponse` whose `package_name` is `stale-pkg`.
    ///
    /// Output:
    /// - Check state stays idle and findings stay empty.
    ///
    /// Details:
    /// - Prevents a late worker completion from repopulating the panel after navigation.
    fn handle_pkgbuild_check_result_ignores_wrong_package() {
        let mut app = new_app();
        app.results = vec![crate::state::PackageItem {
            name: "other".to_string(),
            version: "1".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];
        app.selected = 0;
        app.details_focus = Some("other".to_string());

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        handle_pkgbuild_check_result(
            &mut app,
            crate::state::PkgbuildCheckResponse {
                package_name: "stale-pkg".to_string(),
                findings: vec![crate::state::app_state::PkgbuildCheckFinding {
                    tool: crate::state::app_state::PkgbuildCheckTool::Shellcheck,
                    severity: crate::state::app_state::PkgbuildCheckSeverity::Warning,
                    line: Some(2),
                    message: "should not apply".to_string(),
                }],
                raw_results: vec![],
                missing_tools: vec![],
                last_error: None,
            },
            &tick_tx,
        );

        assert_eq!(app.pkgb_check_status, PkgbuildCheckStatus::Idle);
        assert!(app.pkgb_check_findings.is_empty());
    }

    #[test]
    /// What: Missing checker hints stay out of `pkgb_check_findings` so the pane lists them once.
    ///
    /// Inputs:
    /// - `AppState` with selection matching the response package, empty findings and raw output, and
    ///   non-empty `missing_tools`.
    ///
    /// Output:
    /// - `pkgb_check_findings` remains empty; `pkgb_check_missing_tools` holds the worker strings.
    ///
    /// Details:
    /// - Avoids duplicating `[missing]` lines under the `ShellCheck` findings list with the wrong tool tag.
    fn handle_pkgbuild_check_result_keeps_missing_tools_out_of_findings() {
        let mut app = new_app();
        app.results = vec![crate::state::PackageItem {
            name: "demo".to_string(),
            version: "1".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];
        app.selected = 0;
        app.details_focus = Some("demo".to_string());

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();
        let missing = "shellcheck missing: install package `shellcheck`".to_string();
        handle_pkgbuild_check_result(
            &mut app,
            crate::state::PkgbuildCheckResponse {
                package_name: "demo".to_string(),
                findings: vec![],
                raw_results: vec![],
                missing_tools: vec![missing.clone()],
                last_error: None,
            },
            &tick_tx,
        );

        assert_eq!(app.pkgb_check_status, PkgbuildCheckStatus::Complete);
        assert!(app.pkgb_check_findings.is_empty());
        assert_eq!(app.pkgb_check_missing_tools, vec![missing]);
    }

    #[test]
    /// What: Verify that `handle_summary_result` updates modal when not cancelled.
    ///
    /// Inputs:
    /// - `AppState` with preflight modal open
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
            cached_reverse_deps_report: None,
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
            reverse_deps_report: None,
        };

        handle_summary_result(&mut app, summary_outcome, &tick_tx);

        // Summary should be updated
        if let crate::state::Modal::Preflight { summary, .. } = &app.modal {
            assert!(summary.is_some());
            assert_eq!(
                summary
                    .as_ref()
                    .expect("summary should be Some after is_some() check")
                    .package_count,
                1
            );
        } else {
            panic!("Expected Preflight modal");
        }
        // Flags should be reset
        assert!(!app.preflight_summary_resolving);
        assert!(app.preflight_summary_items.is_none());
    }
}
