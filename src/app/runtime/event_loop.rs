use ratatui::Terminal;
use tokio::select;

use crate::i18n;
use crate::state::types::NewsFeedPayload;
use crate::state::{AppState, PackageItem};
use crate::ui::ui;
use crate::util::parse_update_entry;
use tracing::info;

use super::background::Channels;
use super::handlers::{
    handle_add_to_install_list, handle_dependency_result, handle_details_update,
    handle_file_result, handle_preview, handle_sandbox_result, handle_search_results,
    handle_service_result,
};
use super::tick_handler::{
    handle_comments_result, handle_news, handle_pkgbuild_check_result, handle_pkgbuild_result,
    handle_status, handle_summary_result, handle_tick,
};

/// What: Parse updates entries from the `available_updates.txt` file.
///
/// Inputs:
/// - `updates_file`: Path to the updates file
///
/// Output:
/// - Vector of (name, `old_version`, `new_version`) tuples
///
/// Details:
/// - Parses format: "name - `old_version` -> name - `new_version`"
/// - Uses `parse_update_entry` helper function for parsing individual lines
fn parse_updates_file(updates_file: &std::path::Path) -> Vec<(String, String, String)> {
    if updates_file.exists() {
        std::fs::read_to_string(updates_file)
            .ok()
            .map(|content| {
                content
                    .lines()
                    .filter_map(parse_update_entry)
                    .collect::<Vec<(String, String, String)>>()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// What: Handle batch of items added to install list.
///
/// Inputs:
/// - `app`: Application state
/// - `channels`: Communication channels
/// - `first`: First item in the batch
///
/// Output: None (side effect: processes items)
///
/// Details:
/// - Batch-drains imported items arriving close together to avoid repeated redraws
fn handle_add_batch(app: &mut AppState, channels: &mut Channels, first: PackageItem) {
    let mut batch = vec![first];
    while let Ok(it) = channels.add_rx.try_recv() {
        batch.push(it);
    }
    for it in batch {
        handle_add_to_install_list(
            app,
            it,
            &channels.deps_req_tx,
            &channels.files_req_tx,
            &channels.services_req_tx,
            &channels.sandbox_req_tx,
        );
    }
}

/// What: Handle file result with logging.
///
/// Inputs:
/// - `app`: Application state
/// - `channels`: Communication channels
/// - `files`: File resolution results
///
/// Output: None (side effect: processes files)
fn handle_file_result_with_logging(
    app: &mut AppState,
    channels: &Channels,
    files: &[crate::state::modal::PackageFileInfo],
) {
    tracing::debug!(
        "[Runtime] Received file result: {} entries for packages: {:?}",
        files.len(),
        files.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    for file_info in files {
        tracing::debug!(
            "[Runtime] Package '{}' - total={}, new={}, changed={}, removed={}, config={}",
            file_info.name,
            file_info.total_count,
            file_info.new_count,
            file_info.changed_count,
            file_info.removed_count,
            file_info.config_count
        );
    }
    handle_file_result(app, files, &channels.tick_tx);
}

/// What: Handle remote announcement received from async fetch.
///
/// Inputs:
/// - `app`: Application state to update
/// - `announcement`: Remote announcement fetched from configured URL
///
/// Output: None (modifies app state in place)
///
/// Details:
fn handle_remote_announcement(
    app: &mut AppState,
    announcement: crate::announcements::RemoteAnnouncement,
) {
    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    // Check version range
    if !crate::announcements::version_matches(
        CURRENT_VERSION,
        announcement.min_version.as_deref(),
        announcement.max_version.as_deref(),
    ) {
        tracing::debug!(
            id = %announcement.id,
            current_version = CURRENT_VERSION,
            min_version = ?announcement.min_version,
            max_version = ?announcement.max_version,
            "announcement version range mismatch"
        );
        return;
    }

    // Check expiration
    if crate::announcements::is_expired(announcement.expires.as_deref()) {
        tracing::debug!(
            id = %announcement.id,
            expires = ?announcement.expires,
            "announcement expired"
        );
        return;
    }

    // Check if already read
    if app.announcements_read_ids.contains(&announcement.id) {
        tracing::info!(
            id = %announcement.id,
            "remote announcement already marked as read"
        );
        return;
    }

    // Only show if no modal is currently displayed
    if matches!(app.modal, crate::state::Modal::None) {
        app.modal = crate::state::Modal::Announcement {
            title: announcement.title,
            content: announcement.content,
            id: announcement.id,
            scroll: 0,
        };
        tracing::info!("showing remote announcement modal");
    } else {
        // Queue announcement to show after current modal closes
        let announcement_id = announcement.id.clone();
        app.pending_announcements.push(announcement);
        tracing::info!(
            id = %announcement_id,
            queue_size = app.pending_announcements.len(),
            "queued remote announcement (modal already open)"
        );
    }
}

/// What: Handle index notification message.
///
/// Inputs:
/// - `app`: Application state
/// - `channels`: Communication channels
///
/// Output: `false` (continue event loop)
///
/// Details:
/// - Marks index loading as complete and triggers a tick
fn handle_index_notification(app: &mut AppState, channels: &Channels) -> bool {
    app.loading_index = false;
    // Re-run query once the index is ready so first-launch results are populated.
    crate::logic::send_query(app, &channels.query_tx);
    let _ = channels.tick_tx.send(());
    false
}

/// What: Handle updates list received from background worker.
///
/// Inputs:
/// - `app`: Application state
/// - `payload`: Update check result from the background worker
///
/// Output: None (modifies app state in place)
///
/// Details:
/// - Updates app state with update count, list, and whether the official-repo probe was authoritative
/// - Shows a transient toast when the check ran in degraded mode (stale DB / sandbox issues)
/// - If pending updates modal is set, opens the updates modal
fn handle_updates_list(
    app: &mut AppState,
    payload: crate::app::runtime::workers::UpdateCheckPayload,
    pi_scan_tx: Option<
        &tokio::sync::mpsc::UnboundedSender<
            crate::app::runtime::workers::pi_scan::PiScanRequestMessage,
        >,
    >,
) {
    if let Some(pi_scan_tx) = pi_scan_tx {
        drop(pi_scan_tx.send(
            crate::app::runtime::workers::pi_scan::PiScanRequestMessage::UpdateCandidates(
                payload.candidates.clone(),
            ),
        ));
    }
    let count = payload.count;
    let list = payload.package_names;
    app.updates_last_check_authoritative = Some(payload.authoritative);
    app.updates_count = Some(count);
    app.updates_list = list;
    app.updates_loading = false;
    if !payload.authoritative {
        app.toast_message = Some(i18n::t(app, "app.toasts.update_check_degraded"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
        tracing::info!(
            authoritative = false,
            reasons = %payload.reason_codes.join(","),
            strategy = payload.official_strategy,
            "update check completed in degraded mode for official repositories"
        );
    }
    if app.pending_updates_modal {
        app.pending_updates_modal = false;
        let updates_file = crate::theme::lists_dir().join("available_updates.txt");
        let entries = parse_updates_file(&updates_file);
        let filtered_indices: Vec<usize> = (0..entries.len()).collect();
        app.modal = crate::state::Modal::Updates {
            entries,
            scroll: 0,
            selected: 0,
            filter_active: false,
            filter_query: String::new(),
            filter_caret: 0,
            last_selected_pkg_name: None,
            filtered_indices,
            selected_pkg_names: std::collections::HashSet::new(),
        };
    }
}

/// What: Handle AUR vote worker response and update UI feedback.
///
/// Inputs:
/// - `app`: Application state.
/// - `response`: Vote worker response with typed success/failure result.
///
/// Output:
/// - None (modifies app state in place).
///
/// Details:
/// - Success is surfaced as a short-lived toast.
/// - State-aligned failures (`AlreadyVoted`, `NotVoted`) sync local cache and show toast.
/// - Other actionable failures open `Modal::Alert` with guidance.
fn handle_aur_vote_response(
    app: &mut AppState,
    response: crate::app::runtime::workers::aur_vote::AurVoteResponse,
) {
    match response.result {
        Ok(outcome) => {
            let state = match outcome.action {
                crate::sources::VoteAction::Vote => crate::state::app_state::AurVoteStateUi::Voted,
                crate::sources::VoteAction::Unvote => {
                    crate::state::app_state::AurVoteStateUi::NotVoted
                }
            };
            if !outcome.dry_run {
                app.aur_vote_state_by_pkgbase
                    .insert(outcome.pkgbase.clone(), state);
                app.aur_vote_state_dirty = true;
            }
            app.toast_message = Some(outcome.message());
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
        }
        Err(error) => match error {
            crate::sources::AurVoteError::AlreadyVoted(pkgbase) => {
                app.aur_vote_state_by_pkgbase.insert(
                    pkgbase.clone(),
                    crate::state::app_state::AurVoteStateUi::Voted,
                );
                app.aur_vote_state_dirty = true;
                app.toast_message = Some(format!(
                    "Already voted for '{pkgbase}'. Local vote state synced."
                ));
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
            }
            crate::sources::AurVoteError::NotVoted(pkgbase) => {
                app.aur_vote_state_by_pkgbase.insert(
                    pkgbase.clone(),
                    crate::state::app_state::AurVoteStateUi::NotVoted,
                );
                app.aur_vote_state_dirty = true;
                app.toast_message = Some(format!(
                    "No vote exists for '{pkgbase}'. Local vote state synced."
                ));
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
            }
            other_error => {
                let guidance = match &other_error {
                    crate::sources::AurVoteError::NotFound(_) => {
                        "Verify the selected package base name."
                    }
                    crate::sources::AurVoteError::AuthFailed(_) => {
                        "Upload your SSH public key to https://aur.archlinux.org/account and retry."
                    }
                    crate::sources::AurVoteError::Maintenance => {
                        "Wait for AUR maintenance to end and retry later."
                    }
                    crate::sources::AurVoteError::Banned => {
                        "Your IP is blocked from the SSH interface. Contact AUR support."
                    }
                    crate::sources::AurVoteError::Timeout(_)
                    | crate::sources::AurVoteError::NetworkError(_) => {
                        "Check network connectivity and SSH reachability."
                    }
                    crate::sources::AurVoteError::SshNotFound(_) => {
                        "Install openssh or configure aur_vote_ssh_command in settings.conf."
                    }
                    crate::sources::AurVoteError::Unexpected(_) => {
                        "Retry once, then inspect logs if the issue persists."
                    }
                    crate::sources::AurVoteError::AlreadyVoted(_)
                    | crate::sources::AurVoteError::NotVoted(_) => {
                        "Use the opposite action or leave as-is."
                    }
                };
                app.modal = crate::state::Modal::Alert {
                    message: format!("AUR vote failed: {other_error}\n\nNext step: {guidance}"),
                };
            }
        },
    }
}

/// What: Handle AUR vote-state worker responses and update cached UI state.
///
/// Inputs:
/// - `app`: Application state.
/// - `response`: Vote-state worker response with pkgbase and typed result.
///
/// Output:
/// - None (modifies app state in place).
///
/// Details:
/// - Success updates package cache to `Voted`/`NotVoted`.
/// - Failure stores a short error marker for inline rendering in results/details.
fn handle_aur_vote_state_response(
    app: &mut AppState,
    response: crate::app::runtime::workers::aur_vote::AurVoteStateResponse,
) {
    let pkgbase = response.pkgbase;
    let next_state = match response.result {
        Ok(crate::sources::AurPackageVoteState::Voted) => {
            crate::state::app_state::AurVoteStateUi::Voted
        }
        Ok(crate::sources::AurPackageVoteState::NotVoted) => {
            crate::state::app_state::AurVoteStateUi::NotVoted
        }
        Err(error) => {
            if crate::sources::is_vote_state_unsupported_error(&error) {
                app.aur_vote_state_lookup_supported = false;
                match app.aur_vote_state_by_pkgbase.get(&pkgbase) {
                    Some(crate::state::app_state::AurVoteStateUi::Voted) => {
                        crate::state::app_state::AurVoteStateUi::Voted
                    }
                    Some(crate::state::app_state::AurVoteStateUi::NotVoted) => {
                        crate::state::app_state::AurVoteStateUi::NotVoted
                    }
                    _ => crate::state::app_state::AurVoteStateUi::Unknown,
                }
            } else {
                crate::state::app_state::AurVoteStateUi::Error(format!("{error}"))
            }
        }
    };
    let should_persist = matches!(
        next_state,
        crate::state::app_state::AurVoteStateUi::Voted
            | crate::state::app_state::AurVoteStateUi::NotVoted
    );
    app.aur_vote_state_by_pkgbase.insert(pkgbase, next_state);
    if should_persist {
        app.aur_vote_state_dirty = true;
    }
}

/// What: Apply filters and sorting to news feed items.
///
/// Inputs:
/// - `app`: Application state containing news feed data and filter flags.
/// - `payload`: News feed payload containing items and metadata.
///
/// Details:
/// - Does not clear `news_loading` flag here - it will be cleared when news modal is shown.
fn handle_news_feed_items(app: &mut AppState, payload: NewsFeedPayload) {
    tracing::info!(
        items_count = payload.items.len(),
        "received aggregated news feed payload in event loop"
    );
    app.news_items = payload.items;
    app.news_seen_pkg_versions = payload.seen_pkg_versions;
    app.news_seen_pkg_versions_dirty = true;
    app.news_seen_aur_comments = payload.seen_aur_comments;
    app.news_seen_aur_comments_dirty = true;
    match serde_json::to_string_pretty(&app.news_items) {
        Ok(serialized) => {
            if let Err(e) = std::fs::write(&app.news_feed_path, serialized) {
                tracing::warn!(error = %e, path = ?app.news_feed_path, "failed to persist news feed cache");
            }
        }
        Err(e) => tracing::warn!(error = %e, "failed to serialize news feed cache"),
    }
    app.refresh_news_results();

    // News feed is now loaded - clear loading flag and toast
    app.news_loading = false;
    app.toast_message = None;
    app.toast_expires_at = None;

    info!(
        fetched = app.news_items.len(),
        visible = app.news_results.len(),
        max_age_days = app.news_max_age_days.map(i64::from),
        installed_only = app.news_filter_installed_only,
        arch_on = app.news_filter_show_arch_news,
        advisories_on = app.news_filter_show_advisories,
        "news feed updated"
    );
    // Check for network errors and show a small toast
    if crate::sources::take_network_error() {
        app.toast_message = Some("Network error: some news sources unreachable".to_string());
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
    }
}

/// What: Handle a single incremental news item from background continuation.
///
/// Inputs:
/// - `app`: Application state
/// - `item`: The news feed item to add
///
/// Details:
/// - Appends the item to `news_items` if not already present (by id).
/// - Refreshes filtered/sorted results.
/// - Persists the updated feed cache to disk.
fn handle_incremental_news_item(app: &mut AppState, item: crate::state::types::NewsFeedItem) {
    // Check if item already exists (by id)
    if app.news_items.iter().any(|existing| existing.id == item.id) {
        tracing::debug!(
            item_id = %item.id,
            "incremental news item already exists, skipping"
        );
        return;
    }

    tracing::info!(
        item_id = %item.id,
        source = ?item.source,
        title = %item.title,
        "received incremental news item"
    );

    // Add the new item
    app.news_items.push(item);

    // Refresh filtered/sorted results
    app.refresh_news_results();

    // Persist to disk
    if let Ok(serialized) = serde_json::to_string_pretty(&app.news_items)
        && let Err(e) = std::fs::write(&app.news_feed_path, serialized)
    {
        tracing::warn!(error = %e, path = ?app.news_feed_path, "failed to persist incremental news feed cache");
    }
}

/// What: Handle news article content response.
///
/// Inputs:
/// - `app`: Application state
/// - `url`: The URL that was fetched
/// - `content`: The article content
fn handle_news_content(app: &mut AppState, url: &str, content: String) {
    // Only cache successful content, not error messages
    // Error messages start with "Failed to load content:" and should not be persisted
    let is_error = content.starts_with("Failed to load content:");
    if is_error {
        tracing::debug!(
            url,
            "news_content: not caching error response to allow retry"
        );
    } else {
        app.news_content_cache
            .insert(url.to_string(), content.clone());
        app.news_content_cache_dirty = true;
    }

    // Update displayed content if this is for the currently selected item
    if let Some(selected_url) = app
        .news_results
        .get(app.news_selected)
        .and_then(|selected| selected.url.as_deref())
        && selected_url == url
    {
        tracing::debug!(
            url,
            len = content.len(),
            selected = app.news_selected,
            "news_content: response matches selection"
        );
        app.news_content_loading = false;
        app.news_content = if content.is_empty() {
            None
        } else {
            Some(content)
        };
    } else {
        // Clear loading flag even if selection changed; a new request will be issued on next tick.
        tracing::debug!(
            url,
            len = content.len(),
            selected = app.news_selected,
            selected_url = ?app
                .news_results
                .get(app.news_selected)
                .and_then(|selected| selected.url.as_deref()),
            "news_content: response does not match current selection"
        );
        app.news_content_loading = false;
    }
    app.news_content_loading_since = None;
}

/// What: Process one iteration of channel message handling.
///
/// Inputs:
/// - `app`: Application state
/// - `channels`: Communication channels for background workers
///
/// Output: `true` if the event loop should exit, `false` to continue
///
/// Details:
/// - Waits for and processes a single message from any channel
/// - Returns `true` when an event handler indicates exit (e.g., quit command)
/// - Uses select! to wait on multiple channels concurrently
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
async fn process_channel_messages(app: &mut AppState, channels: &mut Channels) -> bool {
    select! {
        Some(ev) = channels.event_rx.recv() => {
            let should_exit = crate::events::handle_event_with_pkgbuild_checks(
                &ev,
                app,
                &channels.query_tx,
                &channels.details_req_tx,
                &channels.preview_tx,
                &channels.add_tx,
                &channels.pkgb_req_tx,
                &channels.comments_req_tx,
                &channels.pkgb_check_req_tx,
            );
            dispatch_pi_scan_ui_action(app, channels);
            should_exit
        }
        Some(()) = channels.index_notify_rx.recv() => {
            handle_index_notification(app, channels)
        }
        Some(new_results) = channels.results_rx.recv() => {
            handle_search_results(
                app,
                new_results,
                &channels.details_req_tx,
                &channels.index_notify_tx,
            );
            false
        }
        Some(details) = channels.details_res_rx.recv() => {
            handle_details_update(app, &details, &channels.tick_tx);
            false
        }
        Some(item) = channels.preview_rx.recv() => {
            handle_preview(app, item, &channels.details_req_tx);
            false
        }
        Some(first) = channels.add_rx.recv() => {
            handle_add_batch(app, channels, first);
            false
        }
        Some(deps) = channels.deps_res_rx.recv() => {
            handle_dependency_result(app, &deps, &channels.tick_tx);
            false
        }
        Some(files) = channels.files_res_rx.recv() => {
            handle_file_result_with_logging(app, channels, &files);
            false
        }
        Some(services) = channels.services_res_rx.recv() => {
            handle_service_result(app, &services, &channels.tick_tx);
            false
        }
        Some(sandbox_info) = channels.sandbox_res_rx.recv() => {
            handle_sandbox_result(app, &sandbox_info, &channels.tick_tx);
            false
        }
        Some(summary_outcome) = channels.summary_res_rx.recv() => {
            handle_summary_result(app, summary_outcome, &channels.tick_tx);
            false
        }
        Some((pkgname, text)) = channels.pkgb_res_rx.recv() => {
            handle_pkgbuild_result(app, pkgname, text, &channels.tick_tx);
            false
        }
        Some((pkgname, result)) = channels.comments_res_rx.recv() => {
            handle_comments_result(app, pkgname, result, &channels.tick_tx);
            false
        }
        Some(response) = channels.pkgb_check_res_rx.recv() => {
            handle_pkgbuild_check_result(app, response, &channels.tick_tx);
            false
        }
        Some(feed) = channels.news_feed_rx.recv() => {
            handle_news_feed_items(app, feed);
            false
        }
        Some(item) = channels.news_incremental_rx.recv() => {
            handle_incremental_news_item(app, item);
            false
        }
        Some((url, content)) = channels.news_content_res_rx.recv() => {
            handle_news_content(app, &url, content);
            false
        }
        Some(msg) = channels.net_err_rx.recv() => {
            tracing::warn!(error = %msg, "Network error received");
            #[cfg(not(windows))]
            {
                // Package-details-unavailable errors are expected when scrolling with flaky
                // network or circuit breaker; do not show a modal for each failed package.
                let is_details_unavailable = msg.starts_with("Official package details unavailable for")
                    || msg.starts_with("AUR package details unavailable for");
                if !is_details_unavailable {
                    app.modal = crate::state::Modal::Alert {
                        message: msg,
                    };
                }
            }
            // On Windows, only log (no popup)
            false
        }
        Some(()) = channels.tick_rx.recv() => {
            handle_tick(
                app,
                &channels.query_tx,
                &channels.details_req_tx,
                &channels.pkgb_req_tx,
                &channels.deps_req_tx,
                &channels.files_req_tx,
                &channels.services_req_tx,
                &channels.sandbox_req_tx,
                &channels.summary_req_tx,
                &channels.updates_tx,
                &channels.aur_vote_req_tx,
                &channels.aur_vote_state_req_tx,
                &channels.executor_req_tx,
                &channels.post_summary_req_tx,
                &channels.news_content_req_tx,
            );
            false
        }
        Some(items) = channels.news_rx.recv() => {
            tracing::info!(
                items_count = items.len(),
                news_loading_before = app.news_loading,
                "received news items from channel"
            );
            handle_news(app, &items);
            tracing::info!(
                news_loading_after = app.news_loading,
                modal = ?app.modal,
                "handle_news completed"
            );
            false
        }
        Some(announcement) = channels.announcement_rx.recv() => {
            handle_remote_announcement(app, announcement);
            false
        }
        Some((txt, color)) = channels.status_rx.recv() => {
            handle_status(app, &txt, color);
            false
        }
        Some(payload) = channels.updates_rx.recv() => {
            let pi_scan_tx = channels
                .pi_scan_runtime_enabled
                .then_some(&channels.pi_scan_request_tx);
            handle_updates_list(app, payload, pi_scan_tx);
            false
        }
        Some(aur_vote_response) = channels.aur_vote_res_rx.recv() => { handle_aur_vote_response(app, aur_vote_response); false }
        Some(aur_vote_state_response) = channels.aur_vote_state_res_rx.recv() => { handle_aur_vote_state_response(app, aur_vote_state_response); false }
        Some(executor_output) = channels.executor_res_rx.recv() => {
            handle_executor_output(app, executor_output);
            false
        }
        Some(post_summary_data) = channels.post_summary_res_rx.recv() => {
            handle_post_summary_result(app, post_summary_data);
            false
        }
        Some(event) = channels.pi_scan_setup_event_rx.recv() => {
            apply_pi_scan_setup_event(app, event);
            false
        }
        Some(timeout) = channels.pi_scan_setup_timeout_rx.recv() => {
            apply_pi_scan_setup_timeout(app, timeout);
            false
        }
        Some(transfer) = channels.pi_scan_setup_transfer_rx.recv() => {
            begin_pi_scan_runtime_transfer(app, channels, transfer);
            false
        }
        Some(completion) = channels.pi_scan_transfer_completion_rx.recv() => {
            complete_pi_scan_runtime_transfer_follow_up(app, channels, completion);
            false
        }
        Some(notice) = channels.pi_scan_notice_rx.recv() => {
            apply_pi_scan_runtime_notice(app, notice);
            false
        }
        Some(progress) = channels.pi_scan_progress_rx.recv() => {
            apply_pi_scan_progress(app, Some(&channels.pi_scan_request_tx), progress);
            false
        }
        Some(result) = channels.pi_scan_result_rx.recv() => {
            apply_pi_scan_result(app, channels, result);
            false
        }
        else => false
    }
}

/// Dispatch one isolated wizard action through the always-available setup controller.
fn dispatch_pi_scan_setup_action(app: &mut AppState, channels: &Channels) {
    use crate::state::pi_scan_setup::PiScanSetupDraftAction;

    let (action, candidate, consent, confirmations) = {
        let Some(wizard) = app.pi_scan.wizard.as_mut() else {
            return;
        };
        let Some(action) = wizard.pending_action.take() else {
            return;
        };
        (
            action,
            wizard.candidate.clone(),
            wizard.candidate_consent,
            wizard.confirmations,
        )
    };
    let (correlation_id, apply_action, request) = match action {
        PiScanSetupDraftAction::Probe {
            correlation_id,
            binary,
        } => (
            correlation_id,
            false,
            crate::app::runtime::workers::pi_scan_setup::PiScanSetupRequest::BeginSetupProbe {
                correlation_id,
                binary,
            },
        ),
        PiScanSetupDraftAction::Validate { correlation_id } => (
            correlation_id,
            false,
            crate::app::runtime::workers::pi_scan_setup::PiScanSetupRequest::ValidateSetupCandidate {
                correlation_id,
                candidate,
                consent,
                confirmations,
            },
        ),
        PiScanSetupDraftAction::Apply {
            correlation_id,
            validation_binding,
        } => (
            correlation_id,
            true,
            crate::app::runtime::workers::pi_scan_setup::PiScanSetupRequest::ApplySetupCandidate {
                correlation_id,
                candidate,
                consent,
                confirmations,
                validation_binding,
            },
        ),
    };
    app.pi_scan.last_setup_correlation = correlation_id;
    if let Err(error) = channels.pi_scan_setup_request_tx.send(request) {
        let reason = format!(
            "{}: {error}",
            crate::i18n::t(app, "app.pi_scan.wizard.failure.controller_unavailable")
        );
        let accepted = app.pi_scan.wizard.as_mut().is_some_and(|wizard| {
            wizard.accept_failure(correlation_id, apply_action, reason.clone())
        });
        if !accepted {
            app.pi_scan.set_foreground_notice(
                reason,
                crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
            );
        }
        if apply_action {
            app.pi_scan.finish_setup_transaction(correlation_id);
        }
    }
}

/// Project one correlated setup-controller event into wizard or workspace state.
fn apply_pi_scan_setup_event(
    app: &mut AppState,
    event: crate::app::runtime::workers::pi_scan_setup::PiScanSetupEvent,
) {
    use crate::app::runtime::workers::pi_scan_setup::{PiScanSetupEvent, PiScanSetupStage};
    match event {
        PiScanSetupEvent::CapabilitiesVerified {
            correlation_id,
            snapshot,
        } => {
            let accepted = app.pi_scan.wizard.as_mut().is_some_and(|wizard| {
                wizard.accept_verified_facts(correlation_id, wizard_facts(*snapshot))
            });
            log_stale_setup_event(accepted, correlation_id, "capabilities");
        }
        PiScanSetupEvent::CandidateValidated {
            correlation_id,
            validation_binding,
        } => {
            let accepted =
                app.pi_scan.wizard.as_mut().is_some_and(|wizard| {
                    wizard.accept_validation(correlation_id, validation_binding)
                });
            log_stale_setup_event(accepted, correlation_id, "validation");
        }
        PiScanSetupEvent::Applied { correlation_id, .. } => {
            let accepted = app.pi_scan.wizard.as_mut().is_some_and(|wizard| {
                wizard.accept_apply_status(
                    correlation_id,
                    crate::state::PiScanSetupApplyStatus::Persisting,
                )
            });
            log_stale_setup_event(
                accepted || app.pi_scan.setup_transaction_matches(correlation_id),
                correlation_id,
                "applied",
            );
        }
        PiScanSetupEvent::Failed {
            correlation_id,
            stage,
            reason,
        } => {
            let stage_key = setup_stage_key(stage);
            let message = format!(
                "{}: {reason}",
                crate::i18n::t(
                    app,
                    &format!("app.pi_scan.wizard.failure_stage.{stage_key}")
                )
            );
            let transaction_matches = app.pi_scan.setup_transaction_matches(correlation_id);
            let current_correlation =
                transaction_matches || app.pi_scan.last_setup_correlation == correlation_id;
            let apply_failure =
                transaction_matches
                    || matches!(
                        stage,
                        PiScanSetupStage::Activation | PiScanSetupStage::Persistence
                    )
                    || app.pi_scan.wizard.as_ref().is_some_and(|wizard| {
                        wizard.step == crate::state::PiScanSetupStep::Activate
                    });
            let accepted = app.pi_scan.wizard.as_mut().is_some_and(|wizard| {
                wizard.accept_failure(correlation_id, apply_failure, message.clone())
            });
            if transaction_matches {
                app.pi_scan.finish_setup_transaction(correlation_id);
            }
            if !accepted && current_correlation {
                app.pi_scan.set_foreground_notice(
                    message,
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
                );
            }
            log_stale_setup_event(accepted || current_correlation, correlation_id, stage_key);
        }
    }
}

/// Project one enforced setup timeout without accepting a stale correlation.
fn apply_pi_scan_setup_timeout(
    app: &mut AppState,
    timeout: crate::app::runtime::workers::pi_scan_setup::PiScanSetupTimeout,
) {
    let stage_key = setup_stage_key(timeout.stage);
    let message = format!(
        "{}: {}s",
        crate::i18n::t(
            app,
            &format!("app.pi_scan.wizard.failure_timeout.{stage_key}")
        ),
        timeout.deadline.as_secs()
    );
    let transaction_matches = app
        .pi_scan
        .setup_transaction_matches(timeout.correlation_id);
    let current_correlation =
        transaction_matches || app.pi_scan.last_setup_correlation == timeout.correlation_id;
    let accepted = app.pi_scan.wizard.as_mut().is_some_and(|wizard| {
        wizard.accept_failure(timeout.correlation_id, transaction_matches, message.clone())
    });
    if transaction_matches {
        app.pi_scan.finish_setup_transaction(timeout.correlation_id);
    }
    if !accepted && current_correlation {
        app.pi_scan.set_foreground_notice(
            message,
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
        );
    }
    log_stale_setup_event(
        accepted || current_correlation,
        timeout.correlation_id,
        "timeout",
    );
}

/// Return the localization suffix for one typed setup stage.
const fn setup_stage_key(
    stage: crate::app::runtime::workers::pi_scan_setup::PiScanSetupStage,
) -> &'static str {
    use crate::app::runtime::workers::pi_scan_setup::PiScanSetupStage;
    match stage {
        PiScanSetupStage::Probe => "probe",
        PiScanSetupStage::CandidateValidation => "validation",
        PiScanSetupStage::Activation => "activation",
        PiScanSetupStage::Persistence => "persistence",
    }
}

/// Debug-log one setup response rejected by the current correlation owner.
fn log_stale_setup_event(accepted: bool, correlation_id: u64, stage: &str) {
    if !accepted {
        tracing::debug!(
            correlation_id,
            stage,
            "ignored stale Pi Scan setup response"
        );
    }
}

/// Convert the runtime setup projection into credential-free wizard display facts.
fn wizard_facts(
    snapshot: crate::pi_scan_orchestrator::SetupSnapshot,
) -> crate::state::PiScanSetupVerifiedFacts {
    crate::state::PiScanSetupVerifiedFacts {
        pi_version: snapshot.pi_version,
        routes: snapshot.available_models,
        route_reservations: snapshot.route_reservations,
        reservation: snapshot.reservation,
        pricing_binding: snapshot.pricing_binding,
        pricing_observed_at_unix_seconds: snapshot.pricing_observed_at_unix_seconds,
        maximum_pricing_age_seconds: snapshot.maximum_pricing_age_seconds,
        pricing_summary: snapshot.pricing_summary,
    }
}

/// Begin one runtime transfer without awaiting its bounded shutdown on the redraw path.
fn begin_pi_scan_runtime_transfer(
    app: &mut AppState,
    channels: &Channels,
    transfer: crate::app::runtime::workers::pi_scan_setup::PiScanRuntimeTransfer,
) {
    let correlation_id = transfer.correlation_id();
    if !app.pi_scan.setup_transaction_matches(correlation_id) {
        tracing::debug!(
            correlation_id,
            "rolling back stale Pi Scan runtime transfer"
        );
        spawn_pi_scan_transfer_rollback(channels, transfer);
        return;
    }
    if setup_abandonment_requested(app, correlation_id) {
        spawn_pi_scan_transfer_rollback(channels, transfer);
        return;
    }
    if let Some(wizard) = app.pi_scan.wizard.as_mut() {
        let _ = wizard.accept_apply_status(
            correlation_id,
            crate::state::PiScanSetupApplyStatus::Persisting,
        );
    }
    let previous_options = channels.pi_scan_runtime_options.clone();
    let shutdown_tx = channels.pi_scan_shutdown_tx.clone();
    let completion_tx = channels.pi_scan_transfer_completion_tx.clone();
    tokio::spawn(async move {
        let result = shutdown_pi_scan_owner(shutdown_tx).await;
        let completion = super::channels::PiScanRuntimeTransferCompletion::OwnerShutdown {
            transfer: Box::new(transfer),
            previous_options: Box::new(previous_options),
            result,
        };
        if let Err(error) = completion_tx.send(completion)
            && let super::channels::PiScanRuntimeTransferCompletion::OwnerShutdown {
                transfer, ..
            } = error.0
        {
            drop(transfer.rollback_with_outcome());
        }
    });
}

/// Complete one typed transfer follow-up while retaining final owner swaps in the event loop.
fn complete_pi_scan_runtime_transfer_follow_up(
    app: &mut AppState,
    channels: &mut Channels,
    completion: super::channels::PiScanRuntimeTransferCompletion,
) {
    match completion {
        super::channels::PiScanRuntimeTransferCompletion::Rollback(report) => {
            project_pi_scan_rollback_report(app, report);
        }
        super::channels::PiScanRuntimeTransferCompletion::OwnerShutdown {
            transfer,
            previous_options,
            result,
        } => complete_pi_scan_owner_shutdown(
            app,
            channels,
            *transfer,
            previous_options.as_ref(),
            result,
        ),
    }
}

/// Finish candidate activation after the previous owner's bounded wait completes.
fn complete_pi_scan_owner_shutdown(
    app: &mut AppState,
    channels: &mut Channels,
    transfer: crate::app::runtime::workers::pi_scan_setup::PiScanRuntimeTransfer,
    previous_options: &crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions,
    result: super::channels::PiScanOwnerShutdownResult,
) {
    let correlation_id = transfer.correlation_id();
    if let Some(failure) = result.failure {
        let restore = failure
            .owner_stopped
            .then(|| restore_pi_scan_owner(channels, previous_options).err())
            .flatten();
        let runtime_connected = failure.owner_stopped && restore.is_none();
        let rollback = transfer.rollback_with_outcome();
        let reason = combine_setup_failures(
            app,
            combine_setup_failures(app, failure.reason, restore),
            Some(rollback_status(app, &rollback)),
        );
        fail_pi_scan_transfer(app, correlation_id, &reason, runtime_connected);
        return;
    }
    if setup_abandonment_requested(app, correlation_id) {
        let restore = restore_pi_scan_owner(channels, previous_options).err();
        let mut report = transfer.rollback_with_outcome();
        if let Some(reason) = restore {
            report.outcome =
                crate::app::runtime::workers::pi_scan_setup::PiScanRollbackOutcome::Failed {
                    reason: combine_setup_failures(
                        app,
                        crate::i18n::t(app, "app.pi_scan.notices.setup_rollback_complete"),
                        Some(reason),
                    ),
                };
        }
        project_pi_scan_rollback_report(app, report);
        return;
    }
    if let Some(wizard) = app.pi_scan.wizard.as_mut() {
        let _ = wizard.accept_apply_status(
            correlation_id,
            crate::state::PiScanSetupApplyStatus::Activating,
        );
    }
    let activated = match transfer.activate() {
        Ok(activated) => activated,
        Err(error) => {
            let restore = restore_pi_scan_owner(channels, previous_options).err();
            let runtime_connected = restore.is_none();
            fail_pi_scan_transfer(
                app,
                correlation_id,
                &combine_setup_failures(app, error.to_string(), restore),
                runtime_connected,
            );
            return;
        }
    };
    let effective = activated.effective().clone();
    let snapshot = activated.snapshot().clone();
    let new_options = super::pi_scan_runtime_options_for_settings(&effective, app.dry_run);
    let runtime = match activated.commit() {
        Ok(runtime) => runtime,
        Err(reason) => {
            let restore = restore_pi_scan_owner(channels, previous_options).err();
            let runtime_connected = restore.is_none();
            fail_pi_scan_transfer(
                app,
                correlation_id,
                &combine_setup_failures(app, reason, restore),
                runtime_connected,
            );
            return;
        }
    };
    install_pi_scan_owner(channels, runtime, new_options);
    complete_pi_scan_transfer(app, correlation_id, effective, snapshot);
}

/// Return whether one matching setup transaction has requested abandonment.
fn setup_abandonment_requested(app: &AppState, correlation_id: u64) -> bool {
    app.pi_scan.setup_transaction.is_some_and(|transaction| {
        transaction.correlation_id == correlation_id
            && transaction.abandonment
                == crate::state::pi_scan_ui::PiScanSetupAbandonment::AbandonRequested
    })
}

/// Explicitly roll back an unactivated transfer away from the redraw path.
fn spawn_pi_scan_transfer_rollback(
    channels: &Channels,
    transfer: crate::app::runtime::workers::pi_scan_setup::PiScanRuntimeTransfer,
) {
    let completion_tx = channels.pi_scan_transfer_completion_tx.clone();
    tokio::task::spawn_blocking(move || {
        let report = transfer.rollback_with_outcome();
        drop(
            completion_tx.send(super::channels::PiScanRuntimeTransferCompletion::Rollback(
                report,
            )),
        );
    });
}

/// Request bounded durability from a cloned runtime endpoint.
async fn shutdown_pi_scan_owner(
    shutdown_tx: tokio::sync::mpsc::UnboundedSender<
        crate::app::runtime::workers::pi_scan::PiScanShutdownMessage,
    >,
) -> super::channels::PiScanOwnerShutdownResult {
    let result = shutdown_pi_scan_owner_inner(shutdown_tx).await;
    super::channels::PiScanOwnerShutdownResult {
        failure: result.err(),
    }
}

/// Await one shutdown acknowledgement without borrowing central Channels ownership.
async fn shutdown_pi_scan_owner_inner(
    shutdown_tx: tokio::sync::mpsc::UnboundedSender<
        crate::app::runtime::workers::pi_scan::PiScanShutdownMessage,
    >,
) -> Result<(), super::channels::PiScanOwnerShutdownFailure> {
    let (acknowledge, receiver) = std::sync::mpsc::sync_channel(1);
    shutdown_tx
        .send(crate::app::runtime::workers::pi_scan::PiScanShutdownMessage { acknowledge })
        .map_err(|error| super::channels::PiScanOwnerShutdownFailure {
            reason: format!("could not stop the previous Pi Scan runtime: {error}"),
            owner_stopped: true,
        })?;
    let acknowledgement = tokio::task::spawn_blocking(move || {
        receiver.recv_timeout(std::time::Duration::from_secs(10))
    })
    .await
    .map_err(|error| super::channels::PiScanOwnerShutdownFailure {
        reason: format!("Pi Scan shutdown wait failed: {error}"),
        owner_stopped: false,
    })?
    .map_err(|error| super::channels::PiScanOwnerShutdownFailure {
        reason: format!("Pi Scan shutdown exceeded its bounded deadline: {error}"),
        owner_stopped: false,
    })?;
    if acknowledgement.persisted {
        Ok(())
    } else {
        Err(super::channels::PiScanOwnerShutdownFailure {
            reason: acknowledgement.warning.unwrap_or_else(|| {
                "previous Pi Scan runtime did not reach its durability boundary".to_string()
            }),
            owner_stopped: true,
        })
    }
}

/// Restore the exact previous runtime options after a failed candidate activation.
fn restore_pi_scan_owner(
    channels: &mut Channels,
    options: &crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions,
) -> Result<(), String> {
    let runtime = spawn_pi_scan_owner(options)?;
    install_pi_scan_owner(channels, runtime, options.clone());
    Ok(())
}

/// Spawn one worker owner from already validated current or rollback options.
fn spawn_pi_scan_owner(
    options: &crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions,
) -> Result<crate::app::runtime::workers::pi_scan::PiScanRuntimeChannels, String> {
    if options.effective_enabled() && options.production.is_some() {
        crate::pi_scan_production::spawn_production_pi_scan_worker(options)
    } else {
        crate::app::runtime::workers::pi_scan::spawn_pi_scan_worker(options.clone())
            .map_err(|error| error.to_string())
    }
}

/// Replace every runtime endpoint together so exactly one owner is addressable.
fn install_pi_scan_owner(
    channels: &mut Channels,
    runtime: crate::app::runtime::workers::pi_scan::PiScanRuntimeChannels,
    options: crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions,
) {
    channels.pi_scan_request_tx = runtime.request_tx;
    channels.pi_scan_cancel_tx = runtime.cancel_tx;
    channels.pi_scan_session_tx = runtime.session_tx;
    channels.pi_scan_shutdown_tx = runtime.shutdown_tx;
    channels.pi_scan_progress_rx = runtime.progress_rx;
    channels.pi_scan_result_rx = runtime.result_rx;
    channels.pi_scan_notice_rx = runtime.notice_rx;
    channels.pi_scan_runtime_enabled = options.effective_enabled();
    channels.pi_scan_runtime_options = options;
}

/// Project successful runtime ownership only after the non-fallible channel swap.
fn complete_pi_scan_transfer(
    app: &mut AppState,
    correlation_id: u64,
    effective: crate::theme::PiScanSettings,
    snapshot: crate::pi_scan_orchestrator::SetupSnapshot,
) {
    let (draft_consent, confirmations) = app.pi_scan.wizard.as_ref().map_or_else(
        || {
            (
                crate::state::pi_scan::PiScanConsentState::default(),
                crate::state::PiScanSetupConfirmations::default(),
            )
        },
        |wizard| (wizard.candidate_consent, wizard.confirmations),
    );
    app.pi_scan.settings = effective;
    app.pi_scan.runtime.consent = crate::state::pi_scan::PiScanConsentState {
        background_observation: draft_consent.background_observation,
        paid_execution: confirmations.foreground_paid_confirmed,
    };
    app.pi_scan.disclosure_confirmed = confirmations.disclosure_confirmed;
    app.pi_scan.fallback_confirmed = confirmations.fallback_confirmed;
    app.pi_scan.background_paid_execution_confirmed = draft_consent.paid_execution;
    app.pi_scan.readiness_warning_confirmed = confirmations.readiness_warning_confirmed;
    app.pi_scan.verified_pi_version = snapshot.pi_version;
    app.pi_scan.verified_provider = snapshot.selected_provider;
    app.pi_scan.verified_model = snapshot.selected_model;
    app.pi_scan.verified_available_models = snapshot
        .available_models
        .iter()
        .map(|(provider, model)| format!("{provider}/{model}"))
        .collect();
    app.pi_scan.verified_reservation = snapshot.reservation;
    app.pi_scan.verified_pricing_binding = snapshot.pricing_binding;
    app.pi_scan.verified_pricing_summary = snapshot.pricing_summary;
    app.pi_scan.setup_facts_verified = true;
    app.pi_scan.availability = crate::state::PiScanAvailability::RuntimeConnected;
    app.pi_scan.readiness = crate::state::PiScanReadiness::Confirmed;
    if let Some(wizard) = app.pi_scan.wizard.as_mut() {
        let _ = wizard.accept_apply_status(
            correlation_id,
            crate::state::PiScanSetupApplyStatus::Complete,
        );
    }
    app.pi_scan.finish_setup_transaction(correlation_id);
    app.pi_scan.set_foreground_notice(
        crate::i18n::t(app, "app.pi_scan.notices.setup_complete"),
        crate::state::pi_scan_ui::PiScanNoticeSeverity::Success,
    );
}

/// Keep the previous projection authoritative and expose actionable retry guidance.
fn fail_pi_scan_transfer(
    app: &mut AppState,
    correlation_id: u64,
    reason: &str,
    runtime_connected: bool,
) {
    app.pi_scan.availability = if runtime_connected {
        crate::state::PiScanAvailability::RuntimeConnected
    } else {
        crate::state::PiScanAvailability::RuntimeDisconnected
    };
    let accepted = app
        .pi_scan
        .wizard
        .as_mut()
        .is_some_and(|wizard| wizard.accept_failure(correlation_id, true, reason.to_string()));
    let transaction_matches = app.pi_scan.finish_setup_transaction(correlation_id);
    if !accepted && transaction_matches {
        app.pi_scan.set_foreground_notice(
            format!(
                "{}: {reason}",
                crate::i18n::t(app, "app.pi_scan.notices.setup_failed")
            ),
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
        );
    }
}

/// Project one explicit rollback outcome and terminalize its matching setup transaction.
fn project_pi_scan_rollback_report(
    app: &mut AppState,
    report: crate::app::runtime::workers::pi_scan_setup::PiScanRollbackReport,
) {
    let transaction_matches = app.pi_scan.finish_setup_transaction(report.correlation_id);
    match report.outcome {
        crate::app::runtime::workers::pi_scan_setup::PiScanRollbackOutcome::Succeeded => {
            if transaction_matches {
                app.pi_scan.set_foreground_notice(
                    crate::i18n::t(app, "app.pi_scan.notices.setup_rollback_complete"),
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Warning,
                );
            } else {
                tracing::debug!(
                    correlation_id = report.correlation_id,
                    "completed rollback for stale Pi Scan transfer"
                );
            }
        }
        crate::app::runtime::workers::pi_scan_setup::PiScanRollbackOutcome::Failed { reason } => {
            let message = format!(
                "{}: {reason}",
                crate::i18n::t(app, "app.pi_scan.notices.setup_rollback_failed")
            );
            if transaction_matches {
                app.pi_scan.set_foreground_notice(
                    message,
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
                );
            } else {
                app.pi_scan.set_background_notice(
                    message,
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
                );
            }
        }
    }
}

/// Return localized explicit rollback status for setup-failure composition.
fn rollback_status(
    app: &AppState,
    report: &crate::app::runtime::workers::pi_scan_setup::PiScanRollbackReport,
) -> String {
    match &report.outcome {
        crate::app::runtime::workers::pi_scan_setup::PiScanRollbackOutcome::Succeeded => {
            crate::i18n::t(app, "app.pi_scan.notices.setup_rollback_complete")
        }
        crate::app::runtime::workers::pi_scan_setup::PiScanRollbackOutcome::Failed { reason } => {
            format!(
                "{}: {reason}",
                crate::i18n::t(app, "app.pi_scan.notices.setup_rollback_failed")
            )
        }
    }
}

/// Append an independently localized rollback/restart outcome without hiding the primary error.
fn combine_setup_failures(app: &AppState, primary: String, secondary: Option<String>) -> String {
    match secondary {
        Some(secondary) => format!(
            "{primary}\n{}: {secondary}",
            crate::i18n::t(app, "app.pi_scan.notices.setup_secondary_outcome")
        ),
        None => primary,
    }
}

/// What: Dispatch one pending Pi Scan workspace action through the typed WS3 channels.
///
/// Inputs:
/// - `app`: Cohesive UI/runtime projection.
/// - `channels`: Runtime senders and effective scanner gate.
///
/// Output:
/// - The pending action is consumed after a send attempt; send failures become a visible notice.
///
/// Details:
/// - Consent is sent independently from paid/background execution. Queue requests carry only
///   validated package-base/full-OID identity and conservative worst-case reservations.
fn dispatch_pi_scan_ui_action(app: &mut AppState, channels: &Channels) {
    use crate::state::PiScanUiAction;

    if app.app_mode != crate::state::types::AppMode::PiScan {
        return;
    }
    dispatch_pi_scan_setup_action(app, channels);
    let Some(action) = app.pi_scan.pending_action.take() else {
        return;
    };
    if matches!(action, PiScanUiAction::Pause | PiScanUiAction::Resume) {
        let action_key = if action == PiScanUiAction::Pause {
            "pause"
        } else {
            "resume"
        };
        app.pi_scan.set_foreground_notice(
            crate::i18n::t(
                app,
                &format!("app.pi_scan.notices.policy.{action_key}.requesting"),
            ),
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Info,
        );
    }
    let sent = match action {
        PiScanUiAction::ProbeSetup => channels
            .pi_scan_request_tx
            .send(crate::app::runtime::workers::pi_scan::PiScanRequestMessage::ProbeSetup)
            .map_err(|error| error.to_string()),
        PiScanUiAction::UpdateConsent => channels
            .pi_scan_request_tx
            .send(
                crate::app::runtime::workers::pi_scan::PiScanRequestMessage::SetConsentDetails {
                    consent: app.pi_scan.runtime.consent,
                    disclosure_confirmed: app.pi_scan.disclosure_confirmed,
                    fallback_confirmed: app.pi_scan.fallback_confirmed,
                    background_paid_execution_confirmed: app
                        .pi_scan
                        .background_paid_execution_confirmed,
                    readiness_warning_confirmed: app.pi_scan.readiness_warning_confirmed,
                },
            )
            .map_err(|error| error.to_string()),
        PiScanUiAction::QueueSelected | PiScanUiAction::Retry => {
            send_selected_pi_scan_targets(app, channels)
        }
        PiScanUiAction::Pause => channels
            .pi_scan_request_tx
            .send(crate::app::runtime::workers::pi_scan::PiScanRequestMessage::SetUserPaused(true))
            .map_err(|error| error.to_string()),
        PiScanUiAction::Resume => channels
            .pi_scan_request_tx
            .send(crate::app::runtime::workers::pi_scan::PiScanRequestMessage::SetUserPaused(false))
            .map_err(|error| error.to_string()),
        PiScanUiAction::Cancel(correlation_id) => channels
            .pi_scan_cancel_tx
            .send(crate::app::runtime::workers::pi_scan::PiScanCancelMessage {
                correlation_id,
                requested_at_unix: pi_scan_unix_now(),
            })
            .map_err(|error| error.to_string()),
        PiScanUiAction::ContinueSelected => continue_selected_pi_scan_result(app, channels),
        PiScanUiAction::AcceptBaseline => accept_selected_pi_scan_baseline(app, channels),
    };
    if let Err(reason) = sent {
        app.pi_scan.set_foreground_notice(
            reason,
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
        );
    }
}

/// Send selected, immutable Pi Scan targets with conservative reservations.
fn send_selected_pi_scan_targets(app: &mut AppState, channels: &Channels) -> Result<(), String> {
    if !channels.pi_scan_runtime_enabled {
        return Err(crate::i18n::t(
            app,
            "app.pi_scan.notices.runtime_disconnected",
        ));
    }
    if app.pi_scan.pending_queue_intent.is_none() {
        app.pi_scan.snapshot_queue_intent();
    }
    let Some(intent) = app.pi_scan.pending_queue_intent.as_ref() else {
        return Err(crate::i18n::t(app, "app.pi_scan.notices.select_target"));
    };
    let unresolved = unresolved_queue_intent_members(app, &intent.package_names);
    if !unresolved.is_empty() {
        channels
            .pi_scan_request_tx
            .send(
                crate::app::runtime::workers::pi_scan::PiScanRequestMessage::ManualObservation {
                    package_names: unresolved.clone(),
                },
            )
            .map_err(|error| error.to_string())?;
        app.pi_scan.set_foreground_notice(
            format!(
                "{}: {}",
                crate::i18n::t(app, "app.pi_scan.notices.resolving_queue_intent"),
                unresolved.join(", ")
            ),
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Info,
        );
        return Ok(());
    }
    enqueue_resolved_queue_intent(app, &channels.pi_scan_request_tx)
}

/// Return exact intended package names whose immutable identities remain unresolved.
fn unresolved_queue_intent_members(app: &AppState, package_names: &[String]) -> Vec<String> {
    package_names
        .iter()
        .filter(|package_name| {
            !app.pi_scan.targets.iter().any(|target| {
                target.package_name == package_name.as_str() && target.commit_oid.is_some()
            })
        })
        .cloned()
        .collect()
}

/// Consume and enqueue one fully resolved exact queue-intent snapshot.
fn enqueue_resolved_queue_intent(
    app: &mut AppState,
    request_tx: &tokio::sync::mpsc::UnboundedSender<
        crate::app::runtime::workers::pi_scan::PiScanRequestMessage,
    >,
) -> Result<(), String> {
    let Some(intent) = app.pi_scan.pending_queue_intent.as_ref() else {
        return Ok(());
    };
    let reservation = crate::state::pi_scan::PiScanReservation {
        tokens: intent.reservation_tokens,
        cost_microusd: decimal_dollars_to_microusd(&intent.reservation_cost_cap)?,
    };
    let mut identities = Vec::with_capacity(intent.package_names.len());
    for package_name in &intent.package_names {
        let target = app
            .pi_scan
            .targets
            .iter()
            .find(|target| target.package_name == *package_name && target.commit_oid.is_some())
            .ok_or_else(|| {
                format!(
                    "{}: {package_name}",
                    crate::i18n::t(app, "app.pi_scan.notices.queue_intent_unresolved")
                )
            })?;
        identities.push((
            target.package_base.clone(),
            target.commit_oid.clone().unwrap_or_default(),
        ));
    }
    let base_request_id = pi_scan_unix_now();
    for (index, (package_base, commit_oid)) in identities.into_iter().enumerate() {
        let request = crate::state::pi_scan::PiScanJobRequest {
            request_id: base_request_id.saturating_add(index as u64),
            key: crate::state::pi_scan::PiScanQueueKey {
                package_base: crate::logic::pi_scan::identity::PackageBase::new(package_base)
                    .map_err(|error| error.to_string())?,
                commit_oid: crate::logic::pi_scan::identity::CommitOid::new(commit_oid)
                    .map_err(|error| error.to_string())?,
            },
            priority: crate::state::pi_scan::PiScanPriority::Foreground,
            reservation,
            manual_budget_override_confirmed: false,
        };
        request_tx
            .send(crate::app::runtime::workers::pi_scan::PiScanRequestMessage::Enqueue(request))
            .map_err(|error| error.to_string())?;
    }
    app.pi_scan.pending_queue_intent = None;
    app.pi_scan.set_foreground_notice(
        crate::i18n::t(app, "app.pi_scan.notices.queue_intent_submitted"),
        crate::state::pi_scan_ui::PiScanNoticeSeverity::Success,
    );
    Ok(())
}

/// Request a fresh AUR HEAD recheck for one fully acknowledged linked continuation.
fn continue_selected_pi_scan_result(app: &AppState, channels: &Channels) -> Result<(), String> {
    if !app.pi_scan.selected_result_acknowledged() {
        return Err(crate::i18n::t(app, "app.pi_scan.notices.confirm_required"));
    }
    let result = app
        .pi_scan
        .selected_result()
        .ok_or_else(|| crate::i18n::t(app, "app.pi_scan.notices.select_result_continue"))?;
    let package_base = crate::logic::pi_scan::identity::PackageBase::new(
        result.validated.identity.package_base.clone(),
    )
    .map_err(|error| error.to_string())?;
    let observed_head_oid =
        crate::logic::pi_scan::identity::CommitOid::new(result.observed_head_oid.clone())
            .map_err(|error| error.to_string())?;
    let result_binding = result.binding();
    channels
        .pi_scan_request_tx
        .send(
            crate::app::runtime::workers::pi_scan::PiScanRequestMessage::ValidateContinuation {
                package_base,
                observed_head_oid,
                mutable_sources: result.mutable_sources.clone(),
                result_binding,
            },
        )
        .map_err(|error| format!("could not request Pi continuation recheck: {error}"))
}

/// Request persistence of one explicit complete current-HEAD observation baseline.
fn accept_selected_pi_scan_baseline(app: &AppState, channels: &Channels) -> Result<(), String> {
    if !app.pi_scan.selected_result_acknowledged() {
        return Err(crate::i18n::t(app, "app.pi_scan.notices.confirm_required"));
    }
    let result = app
        .pi_scan
        .selected_result()
        .ok_or_else(|| crate::i18n::t(app, "app.pi_scan.notices.select_result_baseline"))?;
    if result.stale
        || result.validated.coverage != crate::logic::pi_scan::result::Coverage::Complete
        || result.validated.identity.commit_oid != result.observed_head_oid
    {
        return Err(
            "only a complete, current, exact-HEAD result can become the accepted baseline"
                .to_string(),
        );
    }
    let package_base = crate::logic::pi_scan::identity::PackageBase::new(
        result.validated.identity.package_base.clone(),
    )
    .map_err(|error| error.to_string())?;
    let commit_oid = crate::logic::pi_scan::identity::CommitOid::new(
        result.validated.identity.commit_oid.clone(),
    )
    .map_err(|error| error.to_string())?;
    channels
        .pi_scan_request_tx
        .send(
            crate::app::runtime::workers::pi_scan::PiScanRequestMessage::AcceptBaseline {
                package_base,
                commit_oid,
                scan_id: result.validated.identity.scan_id.clone(),
                result_binding: result.binding(),
            },
        )
        .map_err(|error| format!("could not request Pi baseline acceptance: {error}"))
}

/// Convert a validated non-negative decimal dollar amount to integer micro-USD.
fn decimal_dollars_to_microusd(value: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    let (whole, fraction) = trimmed.split_once('.').map_or((trimmed, ""), |parts| parts);
    let dollars = whole
        .parse::<u64>()
        .map_err(|_| "Pi scan cost cap is not a valid non-negative decimal".to_string())?;
    if fraction.len() > 6 || !fraction.chars().all(|character| character.is_ascii_digit()) {
        return Err("Pi scan cost cap supports at most six decimal places".to_string());
    }
    let padded = format!("{fraction:0<6}");
    let micros = if padded.is_empty() {
        0
    } else {
        padded
            .parse::<u64>()
            .map_err(|_| "Pi scan cost cap fraction is invalid".to_string())?
    };
    dollars
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(micros))
        .ok_or_else(|| "Pi scan cost cap is too large".to_string())
}

/// Project one provenance-bearing runtime policy acknowledgement into workspace state.
fn apply_pi_scan_runtime_notice(
    app: &mut AppState,
    notice: crate::app::runtime::workers::pi_scan::PiScanRuntimeNotice,
) {
    use crate::app::runtime::workers::pi_scan::{
        PiScanNoticeSource, PiScanPolicyAcknowledgement, PiScanRuntimeAction,
    };
    let action_key = match notice.provenance.action {
        Some(PiScanRuntimeAction::Pause) => "pause",
        Some(PiScanRuntimeAction::Resume) => "resume",
        None => "policy",
    };
    let (state_key, severity) = match &notice.acknowledgement {
        PiScanPolicyAcknowledgement::Queued => (
            "queued",
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Info,
        ),
        PiScanPolicyAcknowledgement::Persisted => {
            if notice.user_paused {
                app.pi_scan
                    .runtime
                    .pause_reasons
                    .insert(crate::state::pi_scan::PiScanPauseReason::User);
            } else {
                app.pi_scan
                    .runtime
                    .pause_reasons
                    .remove(&crate::state::pi_scan::PiScanPauseReason::User);
            }
            (
                "persisted",
                crate::state::pi_scan_ui::PiScanNoticeSeverity::Success,
            )
        }
        PiScanPolicyAcknowledgement::Failed { .. } => (
            "failed",
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
        ),
    };
    let mut text = crate::i18n::t(
        app,
        &format!("app.pi_scan.notices.policy.{action_key}.{state_key}"),
    );
    if let PiScanPolicyAcknowledgement::Failed { reason } = notice.acknowledgement {
        text = format!("{text}: {reason}");
    }
    match notice.provenance.source {
        PiScanNoticeSource::Foreground => app.pi_scan.set_foreground_notice(text, severity),
        PiScanNoticeSource::Background | PiScanNoticeSource::System => {
            app.pi_scan.set_background_notice(text, severity);
        }
    }
}

/// Project one typed worker progress message into the cohesive workspace state.
fn apply_pi_scan_progress(
    app: &mut AppState,
    request_tx: Option<
        &tokio::sync::mpsc::UnboundedSender<
            crate::app::runtime::workers::pi_scan::PiScanRequestMessage,
        >,
    >,
    progress: crate::app::runtime::workers::pi_scan::PiScanProgressMessage,
) {
    use crate::app::runtime::workers::pi_scan::PiScanProgressMessage;
    match progress {
        PiScanProgressMessage::SetupVerified(snapshot) => {
            app.pi_scan.verified_pi_version = snapshot.pi_version;
            app.pi_scan.verified_provider = snapshot.selected_provider;
            app.pi_scan.verified_model = snapshot.selected_model;
            app.pi_scan.verified_available_models = snapshot
                .available_models
                .into_iter()
                .map(|(provider, model)| format!("{provider}/{model}"))
                .collect();
            app.pi_scan.verified_reservation = snapshot.reservation;
            app.pi_scan.verified_pricing_binding = snapshot.pricing_binding;
            app.pi_scan.verified_pricing_summary = snapshot.pricing_summary;
            app.pi_scan.setup_facts_verified = true;
            app.pi_scan.readiness = crate::state::PiScanReadiness::Confirmed;
        }
        PiScanProgressMessage::RestoredRuntime(state) => {
            apply_restored_pi_scan_runtime(app, *state);
        }
        PiScanProgressMessage::RestoredConsent { consent, setup } => {
            app.pi_scan.runtime.consent = consent;
            app.pi_scan.disclosure_confirmed = setup.disclosure_confirmed;
            app.pi_scan.fallback_confirmed = setup.fallback_confirmed;
            app.pi_scan.background_paid_execution_confirmed = setup.background_paid_execution;
            app.pi_scan.readiness_warning_confirmed = setup.readiness_warning_confirmed;
            app.pi_scan.verified_pi_version = setup.confirmed_pi_version;
            app.pi_scan.verified_pricing_binding = setup.confirmed_pricing_binding;
            app.pi_scan.setup_facts_verified = !app.pi_scan.verified_pi_version.is_empty()
                && !app.pi_scan.verified_pricing_binding.is_empty();
        }
        PiScanProgressMessage::RestoredResults { documents } => {
            for document in documents {
                let stale = document.stale;
                let observed_head_oid = if document.observed_head_oid.is_empty() {
                    document.commit_oid.clone()
                } else {
                    document.observed_head_oid.clone()
                };
                let Ok(validated) = document.to_merged_result() else {
                    continue;
                };
                let restored = crate::state::PiScanDisplayResult {
                    validated,
                    observed_head_oid,
                    stale,
                    mutable_sources: document.mutable_sources,
                };
                let binding = restored.binding();
                if !app
                    .pi_scan
                    .results
                    .iter()
                    .any(|result| result.binding() == binding)
                {
                    app.pi_scan.results.push(restored);
                    app.pi_scan.record_result_inserted();
                }
            }
        }
        PiScanProgressMessage::Observed { targets } => {
            apply_pi_scan_observation(app, request_tx, targets);
        }
        PiScanProgressMessage::Queued { request, .. } => {
            set_pi_scan_target_key_status(
                app,
                &request.key,
                crate::state::PiScanTargetStatus::Queued,
            );
            if !app
                .pi_scan
                .runtime
                .queue
                .iter()
                .any(|queued| queued.request_id == request.request_id)
            {
                app.pi_scan.runtime.queue.push_back(request);
            }
        }
        PiScanProgressMessage::Started(active) => {
            set_pi_scan_target_key_status(
                app,
                &active.request.key,
                crate::state::PiScanTargetStatus::Running,
            );
            app.pi_scan
                .runtime
                .queue
                .retain(|queued| queued.request_id != active.request.request_id);
            app.pi_scan.runtime.active = Some(active);
        }
        PiScanProgressMessage::Paused(reason) => {
            if let crate::state::pi_scan::PiScanStartBlock::Paused(pause) = reason {
                app.pi_scan.runtime.pause_reasons.insert(pause);
            }
        }
        PiScanProgressMessage::Cancelling { correlation_id } => {
            if let Some(active) = app.pi_scan.runtime.active.as_mut()
                && active.correlation_id == correlation_id
            {
                active.cancellation_suppressed = true;
            }
        }
        PiScanProgressMessage::DryRunPreview(_)
        | PiScanProgressMessage::SessionRegistered { .. }
        | PiScanProgressMessage::Shutdown(_) => {}
    }
    app.pi_scan.clamp_selection();
}

/// Project exact observed identities and complete only their matching queue intent.
fn apply_pi_scan_observation(
    app: &mut AppState,
    request_tx: Option<
        &tokio::sync::mpsc::UnboundedSender<
            crate::app::runtime::workers::pi_scan::PiScanRequestMessage,
        >,
    >,
    targets: Vec<crate::pi_scan_orchestrator::FrozenScanIdentity>,
) {
    let observation_matches_intent =
        app.pi_scan
            .pending_queue_intent
            .as_ref()
            .is_some_and(|intent| {
                targets.iter().any(|observed| {
                    intent
                        .package_names
                        .iter()
                        .any(|name| name == &observed.package_name)
                })
            });
    for observed in targets {
        let package_base = observed.package_base.as_str().to_string();
        let commit_oid = observed.commit_oid.as_str().to_string();
        if let Some(target) = app.pi_scan.targets.iter_mut().find(|target| {
            target.package_name == observed.package_name && target.commit_oid.is_none()
        }) {
            target.package_base = package_base;
            target.commit_oid = Some(commit_oid);
            continue;
        }
        let exact_exists = app.pi_scan.targets.iter().any(|target| {
            target.package_base == package_base
                && target.commit_oid.as_deref() == Some(commit_oid.as_str())
        });
        if exact_exists {
            continue;
        }
        app.pi_scan.targets.push(crate::state::PiScanTarget {
            package_name: observed.package_name,
            package_base,
            commit_oid: Some(commit_oid),
            selected: false,
            status: crate::state::PiScanTargetStatus::Unbaselined,
        });
    }
    app.pi_scan.readiness = crate::state::PiScanReadiness::Confirmed;
    if !observation_matches_intent {
        return;
    }
    let Some(request_tx) = request_tx else {
        return;
    };
    let Some(intent) = app.pi_scan.pending_queue_intent.as_ref() else {
        return;
    };
    let unresolved = unresolved_queue_intent_members(app, &intent.package_names);
    if unresolved.is_empty() {
        if let Err(reason) = enqueue_resolved_queue_intent(app, request_tx) {
            app.pi_scan.set_foreground_notice(
                reason,
                crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
            );
        }
        return;
    }
    app.pi_scan.set_foreground_notice(
        format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.notices.queue_intent_unresolved"),
            unresolved.join(", ")
        ),
        crate::state::pi_scan_ui::PiScanNoticeSeverity::Info,
    );
}

/// Restore full target identities and queue/terminal state after a production restart.
fn apply_restored_pi_scan_runtime(
    app: &mut AppState,
    state: crate::pi_scan_orchestrator::OrchestrationState,
) {
    for (request_id, target) in &state.targets {
        let key_matches = |key: &crate::state::pi_scan::PiScanQueueKey| {
            key.package_base == target.package_base && key.commit_oid == target.commit_oid
        };
        let status = if state
            .runtime
            .active
            .as_ref()
            .is_some_and(|active| active.request.request_id == *request_id)
        {
            crate::state::PiScanTargetStatus::Running
        } else if state
            .runtime
            .queue
            .iter()
            .any(|request| request.request_id == *request_id || key_matches(&request.key))
        {
            crate::state::PiScanTargetStatus::Queued
        } else if let Some(record) = state.runtime.terminal.iter().rev().find(|record| {
            record.request.request_id == *request_id || key_matches(&record.request.key)
        }) {
            match record.status {
                crate::state::pi_scan::PiScanTerminalStatus::Completed => {
                    crate::state::PiScanTargetStatus::Completed
                }
                crate::state::pi_scan::PiScanTerminalStatus::Cancelled => {
                    crate::state::PiScanTargetStatus::Cancelled
                }
                crate::state::pi_scan::PiScanTerminalStatus::Interrupted => {
                    crate::state::PiScanTargetStatus::Interrupted
                }
                crate::state::pi_scan::PiScanTerminalStatus::Failed => {
                    crate::state::PiScanTargetStatus::Failed
                }
            }
        } else {
            crate::state::PiScanTargetStatus::Unbaselined
        };
        let package_base = target.package_base.as_str().to_string();
        let commit_oid = target.commit_oid.as_str().to_string();
        if let Some(existing) = app.pi_scan.targets.iter_mut().find(|candidate| {
            candidate.package_base == package_base
                && candidate.commit_oid.as_deref() == Some(commit_oid.as_str())
        }) {
            existing.status = status;
        } else {
            app.pi_scan.targets.push(crate::state::PiScanTarget {
                package_name: target.package_name.clone(),
                package_base,
                commit_oid: Some(commit_oid),
                selected: false,
                status,
            });
        }
    }
    app.pi_scan.runtime = state.runtime;
    if !app.pi_scan.targets.is_empty() {
        app.pi_scan.readiness = crate::state::PiScanReadiness::Confirmed;
    }
}

/// Project one terminal runtime message without treating it as a validated model result.
fn apply_pi_scan_result(
    app: &mut AppState,
    channels: &Channels,
    result: crate::app::runtime::workers::pi_scan::PiScanResultMessage,
) {
    use crate::app::runtime::workers::pi_scan::PiScanResultMessage;
    match result {
        PiScanResultMessage::DryRunAcquired {
            key,
            status,
            manifest_count,
            coverage_notes,
        } => {
            set_pi_scan_target_key_status(app, &key, crate::state::PiScanTargetStatus::Completed);
            app.pi_scan.set_foreground_notice(
                format!(
                    "{}: {status}; {manifest_count}; {}",
                    crate::i18n::t(app, "app.pi_scan.notices.dry_run_acquired"),
                    coverage_notes.len()
                ),
                crate::state::pi_scan_ui::PiScanNoticeSeverity::Success,
            );
        }
        PiScanResultMessage::Validated(receipt) => {
            let receipt = *receipt;
            let package_base = receipt.result.identity.package_base.clone();
            set_pi_scan_target_identity_status(
                app,
                package_base.as_str(),
                receipt.result.identity.commit_oid.as_str(),
                crate::state::PiScanTargetStatus::Completed,
            );
            app.pi_scan.runtime.active = None;
            let display = crate::state::PiScanDisplayResult {
                validated: receipt.result,
                observed_head_oid: receipt.observed_head_oid.as_str().to_string(),
                stale: receipt.stale,
                mutable_sources: receipt.mutable_sources,
            };
            let binding = display.binding();
            if !app
                .pi_scan
                .results
                .iter()
                .any(|result| result.binding() == binding)
            {
                app.pi_scan.results.push(display);
                app.pi_scan.record_result_inserted();
                app.pi_scan.set_foreground_notice(
                    format!(
                        "{}: {package_base}",
                        crate::i18n::t(app, "app.pi_scan.notices.validated_complete")
                    ),
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Success,
                );
            }
        }
        PiScanResultMessage::BaselineAccepted { result_binding } => {
            if app
                .pi_scan
                .results
                .iter()
                .any(|result| result.binding() == result_binding)
            {
                app.pi_scan.set_foreground_notice(
                    crate::i18n::t(app, "app.pi_scan.notices.baseline_persisted"),
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Success,
                );
            } else {
                app.pi_scan.set_foreground_notice(
                    crate::i18n::t(app, "app.pi_scan.notices.baseline_binding_changed"),
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Warning,
                );
            }
        }
        PiScanResultMessage::ContinuationValidated {
            package_base,
            result_binding,
            stale,
        } => {
            if let Err(reason) = finish_pi_scan_continuation(
                app,
                channels,
                package_base.as_str(),
                &result_binding,
                stale,
            ) {
                app.pi_scan.set_foreground_notice(
                    reason,
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
                );
            }
        }
        PiScanResultMessage::Completed(record) => {
            set_pi_scan_target_key_status(
                app,
                &record.request.key,
                crate::state::PiScanTargetStatus::Completed,
            );
            app.pi_scan.runtime.active = None;
            app.pi_scan.runtime.terminal.push(record);
        }
        PiScanResultMessage::Cancelled { record, warning } => {
            set_pi_scan_target_key_status(
                app,
                &record.request.key,
                crate::state::PiScanTargetStatus::Cancelled,
            );
            app.pi_scan.runtime.active = None;
            app.pi_scan.runtime.terminal.push(record);
            if let Some(warning) = warning {
                app.pi_scan.set_foreground_notice(
                    format!(
                        "{}: {warning}",
                        crate::i18n::t(app, "app.pi_scan.notices.cancelled")
                    ),
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Warning,
                );
            } else if app.pi_scan.notices.foreground.is_none() {
                app.pi_scan.set_foreground_notice(
                    crate::i18n::t(app, "app.pi_scan.notices.cancelled"),
                    crate::state::pi_scan_ui::PiScanNoticeSeverity::Info,
                );
            }
        }
        PiScanResultMessage::Rejected { reason } => {
            tracing::warn!(reason, "Pi scan runtime request rejected");
            if let Some(active) = app.pi_scan.runtime.active.take() {
                set_pi_scan_target_key_status(
                    app,
                    &active.request.key,
                    crate::state::PiScanTargetStatus::Failed,
                );
            }
            app.pi_scan.set_foreground_notice(
                format!(
                    "{}: {reason}",
                    crate::i18n::t(app, "app.pi_scan.notices.runtime_rejected")
                ),
                crate::state::pi_scan_ui::PiScanNoticeSeverity::Error,
            );
        }
    }
    app.pi_scan.clamp_selection();
}

/// Finish a linked continuation only after the exact result remains current and acknowledged.
fn finish_pi_scan_continuation(
    app: &mut AppState,
    channels: &Channels,
    package_base: &str,
    result_binding: &str,
    stale: bool,
) -> Result<(), String> {
    let result_index = app
        .pi_scan
        .results
        .iter()
        .position(|result| result.binding() == result_binding)
        .ok_or_else(|| {
            "the Pi scan result changed while continuation identity was rechecked".to_string()
        })?;
    if stale {
        app.pi_scan.results[result_index].stale = true;
        return Err(
            "AUR HEAD changed after the scan; the result is stale and must be re-acknowledged or rescanned"
                .to_string(),
        );
    }
    let result = &app.pi_scan.results[result_index];
    let binding = result.binding();
    let acknowledged = (!result.needs_finding_acknowledgement()
        || app.pi_scan.finding_acknowledgements.contains(&binding))
        && (!result.stale || app.pi_scan.stale_acknowledgements.contains(&binding));
    if !acknowledged {
        return Err(crate::i18n::t(app, "app.pi_scan.notices.confirm_required"));
    }
    let package_name = app
        .pi_scan
        .targets
        .iter()
        .find(|target| target.package_base == package_base)
        .map(|target| target.package_name.clone())
        .ok_or_else(|| {
            "the validated Pi scan result is no longer linked to a visible package target"
                .to_string()
        })?;
    let item = app
        .results
        .iter()
        .find(|item| item.name == package_name && matches!(item.source, crate::state::Source::Aur))
        .cloned()
        .ok_or_else(|| {
            "reselect the AUR package in Search before continuing to install/update".to_string()
        })?;
    channels
        .add_tx
        .send(item)
        .map_err(|error| format!("could not continue the acknowledged Pi scan result: {error}"))?;
    app.app_mode = crate::state::types::AppMode::Package;
    app.toast_message = Some(format!(
        "{}: {package_name}",
        crate::i18n::t(app, "app.pi_scan.notices.continuation_complete")
    ));
    app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(6));
    Ok(())
}

/// Update one visible target for an exact package-base and commit key.
fn set_pi_scan_target_key_status(
    app: &mut AppState,
    key: &crate::state::pi_scan::PiScanQueueKey,
    status: crate::state::PiScanTargetStatus,
) {
    set_pi_scan_target_identity_status(
        app,
        key.package_base.as_str(),
        key.commit_oid.as_str(),
        status,
    );
}

/// Update one visible target for exact validated identity text.
fn set_pi_scan_target_identity_status(
    app: &mut AppState,
    package_base: &str,
    commit_oid: &str,
    status: crate::state::PiScanTargetStatus,
) {
    for target in &mut app.pi_scan.targets {
        if target.package_base == package_base && target.commit_oid.as_deref() == Some(commit_oid) {
            target.status = status;
        }
    }
}

/// Return current Unix seconds for typed runtime messages.
fn pi_scan_unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// What: Handle post-summary computation result.
///
/// Inputs:
/// - `app`: Application state
/// - `data`: Computed post-summary data
///
/// Details:
/// - Transitions from Loading modal to `PostSummary` modal
fn handle_post_summary_result(app: &mut AppState, data: crate::logic::summary::PostSummaryData) {
    // Only transition if we're in Loading state
    if matches!(app.modal, crate::state::Modal::Loading { .. }) {
        tracing::debug!(
            success = data.success,
            changed_files = data.changed_files,
            pacnew_count = data.pacnew_count,
            pacsave_count = data.pacsave_count,
            services_pending = data.services_pending.len(),
            snapshot_label = ?data.snapshot_label,
            "[EventLoop] Transitioning modal: Loading -> PostSummary"
        );
        app.modal = crate::state::Modal::PostSummary {
            success: data.success,
            changed_files: data.changed_files,
            pacnew_count: data.pacnew_count,
            pacsave_count: data.pacsave_count,
            services_pending: data.services_pending,
            snapshot_label: data.snapshot_label,
        };
    }
}

/// What: Handle successful executor completion for Install action.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Package items that were installed
///
/// Output:
/// - None (modifies app state in place)
///
/// Details:
/// - Tracks installed packages and triggers refresh of installed packages pane
/// - Only tracks pending install names if items is non-empty (system updates use empty items)
fn handle_install_success(app: &mut AppState, items: &[crate::state::PackageItem]) {
    // Only track pending install names if items is non-empty.
    // System updates use empty items, and setting pending_install_names
    // to empty would cause install_list to be cleared in tick handler
    // due to vacuously true check (all elements of empty set satisfy any predicate).
    if !items.is_empty() {
        let installed_names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
        // Set pending install names to track installation completion
        app.pending_install_names = Some(installed_names);
    }

    // Trigger refresh of installed packages
    app.refresh_installed_until =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(8));

    // Refresh updates count after installation completes
    app.refresh_updates = true;

    tracing::info!(
        "Install operation completed: triggered refresh of installed packages and updates"
    );
}

/// What: Handle successful executor completion for Remove action.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Package items that were removed
///
/// Output:
/// - None (modifies app state in place)
///
/// Details:
/// - Clears remove list and triggers refresh of installed packages pane
fn handle_remove_success(app: &mut AppState, items: &[crate::state::PackageItem]) {
    let removed_names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();

    // Clear remove list
    app.remove_list.clear();
    app.remove_list_names.clear();
    app.remove_state.select(None);

    // Set pending remove names to track removal completion
    app.pending_remove_names = Some(removed_names);

    // Trigger refresh of installed packages
    app.refresh_installed_until =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(8));

    // Refresh updates count after removal completes
    app.refresh_updates = true;

    // Keep PreflightExec modal open so user can see completion message
    // User can close it with Esc/q, and refresh happens in background
    tracing::info!("Remove operation completed: cleared remove list and triggered refresh");
}

/// What: Handle successful executor completion for Downgrade action.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Package items that were downgraded
///
/// Output:
/// - None (modifies app state in place)
///
/// Details:
/// - Clears downgrade list and triggers refresh of installed packages pane
fn handle_downgrade_success(app: &mut AppState, items: &[crate::state::PackageItem]) {
    let downgraded_names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();

    // Clear downgrade list
    app.downgrade_list.clear();
    app.downgrade_list_names.clear();
    app.downgrade_state.select(None);

    // Set pending downgrade names to track downgrade completion
    app.pending_remove_names = Some(downgraded_names);

    // Trigger refresh of installed packages
    app.refresh_installed_until =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(8));

    // Refresh updates count after downgrade completes
    app.refresh_updates = true;

    // Keep PreflightExec modal open so user can see completion message
    // User can close it with Esc/q, and refresh happens in background
    tracing::info!("Downgrade operation completed: cleared downgrade list and triggered refresh");
}

/// What: Handle executor output and update UI state accordingly.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `output`: Executor output to process
///
/// Output:
/// - None (modifies app state in place)
///
/// Details:
/// - Updates `PreflightExec` modal with log lines or completion status
/// - Processes `Line`, `ReplaceLastLine`, `Finished`, and `Error` outputs
/// - Handles success/failure cases for Install, Remove, and Downgrade actions
/// - Shows confirmation popup for AUR update when pacman fails
#[allow(clippy::too_many_lines)] // Function handles multiple executor output types and modal transitions (function has 187 lines)
fn handle_executor_output(app: &mut AppState, output: crate::install::ExecutorOutput) {
    // Log what we received (at trace level to avoid spam)
    match &output {
        crate::install::ExecutorOutput::Line(line) => {
            tracing::trace!(
                "[EventLoop] Received executor line: {}...",
                &line[..line.len().min(50)]
            );
        }
        crate::install::ExecutorOutput::ReplaceLastLine(line) => {
            tracing::trace!(
                "[EventLoop] Received executor replace line: {}...",
                &line[..line.len().min(50)]
            );
        }
        crate::install::ExecutorOutput::Finished {
            success,
            exit_code,
            failed_command: _,
        } => {
            tracing::debug!(
                "[EventLoop] Received executor Finished: success={}, exit_code={:?}",
                success,
                exit_code
            );
        }
        crate::install::ExecutorOutput::Error(err) => {
            tracing::warn!("[EventLoop] Received executor Error: {}", err);
        }
    }

    if let crate::state::Modal::PreflightExec {
        ref mut log_lines,
        ref mut abortable,
        ref mut success,
        ref items,
        ref action,
        ..
    } = app.modal
    {
        match output {
            crate::install::ExecutorOutput::Line(line) => {
                log_lines.push(line);
                // Keep only last 1000 lines to avoid memory issues
                if log_lines.len() > 1000 {
                    log_lines.remove(0);
                }
                tracing::debug!(
                    "[EventLoop] PreflightExec log_lines count: {}",
                    log_lines.len()
                );
            }
            crate::install::ExecutorOutput::ReplaceLastLine(line) => {
                // Replace the last line (for progress bar updates via \r)
                if log_lines.is_empty() {
                    log_lines.push(line);
                } else {
                    let last_idx = log_lines.len() - 1;
                    log_lines[last_idx] = line;
                }
            }
            crate::install::ExecutorOutput::Finished {
                success: exec_success,
                exit_code,
                failed_command: _,
            } => {
                tracing::info!(
                    "Received Finished: success={exec_success}, exit_code={exit_code:?}"
                );
                *abortable = false;
                if !exec_success {
                    app.pending_repo_apply_overlap_check = None;
                    app.pending_repositories_modal_resume = None;
                }
                // Store the execution result in the modal
                *success = Some(exec_success);
                log_lines.push(String::new()); // Empty line before completion message
                if exec_success {
                    let completion_msg = match action {
                        crate::state::PreflightAction::Install => {
                            "Installation successfully completed!".to_string()
                        }
                        crate::state::PreflightAction::Remove => {
                            "Removal successfully completed!".to_string()
                        }
                        crate::state::PreflightAction::Downgrade => {
                            "Downgrade successfully completed!".to_string()
                        }
                    };
                    log_lines.push(completion_msg);
                    tracing::info!(
                        "Added completion message, log_lines.len()={}",
                        log_lines.len()
                    );

                    // Clone items to avoid borrow checker issues when calling handlers
                    let items_clone = items.clone();
                    let action_clone = *action;

                    // Handle successful operations: refresh installed packages and update UI
                    match action_clone {
                        crate::state::PreflightAction::Install => {
                            handle_install_success(app, &items_clone);
                        }
                        crate::state::PreflightAction::Remove => {
                            handle_remove_success(app, &items_clone);
                        }
                        crate::state::PreflightAction::Downgrade => {
                            handle_downgrade_success(app, &items_clone);
                        }
                    }
                } else {
                    log_lines.push(format!("Execution failed (exit code: {exit_code:?})"));

                    // If this was a system update (empty items) and AUR update is pending, show confirmation
                    if items.is_empty() && app.pending_aur_update_command.is_some() {
                        tracing::info!(
                            "[EventLoop] System update failed (exit_code: {:?}), AUR update pending - showing confirmation popup",
                            exit_code
                        );
                        // Preserve password and header_chips for AUR update if user confirms
                        // (they're already stored in app state, so we just need to show the modal)

                        // Determine which command failed by checking the command list
                        let failed_command_name = app
                            .pending_update_commands
                            .as_ref()
                            .and_then(|cmds| {
                                // Extract command name from the first command (since commands are chained with &&,
                                // the first command that fails stops execution)
                                cmds.first().map(|cmd| {
                                    // Extract command name: "sudo pacman -Syu" -> "pacman", "paru -Sua" -> "paru"
                                    if cmd.contains("pacman") {
                                        "pacman"
                                    } else if cmd.contains("paru") {
                                        "paru"
                                    } else if cmd.contains("yay") {
                                        "yay"
                                    } else if cmd.contains("reflector") {
                                        "reflector"
                                    } else if cmd.contains("pacman-mirrors") {
                                        "pacman-mirrors"
                                    } else if cmd.contains("eos-rankmirrors") {
                                        "eos-rankmirrors"
                                    } else if cmd.contains("cachyos-rate-mirrors") {
                                        "cachyos-rate-mirrors"
                                    } else {
                                        "update command"
                                    }
                                })
                            })
                            .unwrap_or("update command");

                        // Close PreflightExec and show confirmation modal
                        let exit_code_str =
                            exit_code.map_or_else(|| "unknown".to_string(), |c| c.to_string());
                        app.modal = crate::state::Modal::ConfirmAurUpdate {
                            message: format!(
                                "{}\n\n{}\n{}\n\n{}",
                                i18n::t_fmt2(
                                    app,
                                    "app.modals.confirm_aur_update.command_failed",
                                    failed_command_name,
                                    &exit_code_str
                                ),
                                i18n::t(app, "app.modals.confirm_aur_update.continue_prompt"),
                                i18n::t(app, "app.modals.confirm_aur_update.warning"),
                                i18n::t(app, "app.modals.confirm_aur_update.hint")
                            ),
                        };
                    } else {
                        tracing::debug!(
                            "[EventLoop] System update failed but no confirmation popup - items.is_empty(): {}, pending_aur_update_command.is_some(): {}",
                            items.is_empty(),
                            app.pending_aur_update_command.is_some()
                        );
                    }
                }
            }
            crate::install::ExecutorOutput::Error(err) => {
                *abortable = false;
                log_lines.push(format!("Error: {err}"));
            }
        }
    } else {
        tracing::warn!(
            "[EventLoop] Received executor output but modal is not PreflightExec, modal={:?}",
            std::mem::discriminant(&app.modal)
        );
    }
}

/// What: Trigger startup news fetch using current startup news settings.
///
/// Inputs:
/// - `channels`: Communication channels for background workers
/// - `app`: Application state for read sets
///
/// Output: None
///
/// Details:
/// - Fetches news feed using startup news settings and sends to `news_tx` channel
/// - Called when `trigger_startup_news_fetch` flag is set after `NewsSetup` completion
/// - Sets `news_loading` flag to show loading modal
fn trigger_startup_news_fetch(channels: &Channels, app: &mut AppState) {
    use crate::sources;
    use crate::state::types::NewsSortMode;
    use std::collections::HashSet;

    let prefs = crate::theme::settings();
    if !prefs.startup_news_configured {
        return;
    }

    // Set loading flag to show loading modal
    app.news_loading = true;
    tracing::info!("news_loading set to true, triggering startup news fetch");

    let news_tx = channels.news_tx.clone();
    let read_urls = app.news_read_urls.clone();
    let read_ids = app.news_read_ids.clone();
    let installed: HashSet<String> = crate::index::explicit_names().into_iter().collect();
    // Create mutable copies for the fetch (won't be persisted, but needed for API)
    let mut seen_versions = app.news_seen_pkg_versions.clone();
    let mut seen_aur_comments = app.news_seen_aur_comments.clone();

    tokio::spawn(async move {
        tracing::info!("on-demand startup news fetch task started");
        let mut installed_set = installed;
        if installed_set.is_empty() {
            crate::index::refresh_installed_cache().await;
            crate::index::refresh_explicit_cache(crate::state::InstalledPackagesMode::AllExplicit)
                .await;
            let refreshed: HashSet<String> = crate::index::explicit_names().into_iter().collect();
            if !refreshed.is_empty() {
                installed_set = refreshed;
            }
        }
        let include_pkg_updates =
            prefs.startup_news_show_pkg_updates || prefs.startup_news_show_aur_updates;
        // Use lower limit for startup popup (20) vs main feed (50)
        // If both official and AUR updates are requested, double the limit so both types can be included
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
            max_age_days: prefs.startup_news_max_age_days,
        };
        tracing::info!(
            limit = updates_limit,
            include_arch_news = prefs.startup_news_show_arch_news,
            include_advisories = prefs.startup_news_show_advisories,
            include_pkg_updates,
            include_aur_comments = prefs.startup_news_show_aur_comments,
            max_age_days = ?prefs.startup_news_max_age_days,
            installed_count = installed_set.len(),
            "starting on-demand startup news fetch"
        );
        match sources::fetch_news_feed(ctx).await {
            Ok(feed) => {
                tracing::info!(
                    total_items = feed.len(),
                    "on-demand startup news fetch completed successfully"
                );
                // Filter by source type for package updates (AUR vs official are mixed in fetch_installed_updates)
                let source_filtered: Vec<crate::state::types::NewsFeedItem> = feed
                    .into_iter()
                    .filter(|item| match item.source {
                        crate::state::types::NewsFeedSource::ArchNews => {
                            prefs.startup_news_show_arch_news
                        }
                        crate::state::types::NewsFeedSource::SecurityAdvisory => {
                            prefs.startup_news_show_advisories
                        }
                        crate::state::types::NewsFeedSource::InstalledPackageUpdate => {
                            prefs.startup_news_show_pkg_updates
                        }
                        crate::state::types::NewsFeedSource::AurPackageUpdate => {
                            prefs.startup_news_show_aur_updates
                        }
                        crate::state::types::NewsFeedSource::AurComment => {
                            prefs.startup_news_show_aur_comments
                        }
                    })
                    .collect();
                // Filter by max age days
                let filtered: Vec<crate::state::types::NewsFeedItem> =
                    if let Some(max_days) = prefs.startup_news_max_age_days {
                        let cutoff_date = chrono::Utc::now()
                            .checked_sub_signed(chrono::Duration::days(i64::from(max_days)))
                            .map(|dt| dt.format("%Y-%m-%d").to_string());
                        #[allow(clippy::unnecessary_map_or)]
                        let filtered_items = source_filtered
                            .into_iter()
                            .filter(|item| {
                                cutoff_date
                                    .as_ref()
                                    .map_or(true, |cutoff| &item.date >= cutoff)
                            })
                            .collect();
                        filtered_items
                    } else {
                        source_filtered
                    };
                // Filter out already-read items
                #[allow(clippy::unnecessary_map_or)]
                let unread: Vec<crate::state::types::NewsFeedItem> = filtered
                    .into_iter()
                    .filter(|item| {
                        !read_ids.contains(&item.id)
                            && item.url.as_ref().is_none_or(|url| !read_urls.contains(url))
                    })
                    .collect();
                tracing::info!(
                    unread_count = unread.len(),
                    "sending on-demand startup news items to channel"
                );
                match news_tx.send(unread) {
                    Ok(()) => {
                        tracing::info!("on-demand startup news items sent to channel successfully");
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "failed to send on-demand startup news items to channel (receiver dropped?)"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "on-demand startup news fetch failed");
                tracing::info!("sending empty array to clear loading flag after fetch error");
                let _ = news_tx.send(Vec::new());
            }
        }
    });
}

#[cfg(test)]
mod startup_news_tests {
    use crate::state::types::{NewsFeedItem, NewsFeedSource};
    use std::collections::HashSet;

    #[test]
    /// What: Test filtering logic for already-read news items.
    ///
    /// Inputs:
    /// - News items with some marked as read (by ID and URL).
    ///
    /// Output:
    /// - Only unread items returned.
    ///
    /// Details:
    /// - Verifies read filtering excludes items by both ID and URL.
    fn test_filter_already_read_items() {
        let read_ids: HashSet<String> = HashSet::from(["id-1".to_string()]);

        let read_urls: HashSet<String> = HashSet::from(["https://example.com/news/2".to_string()]);

        let items = vec![
            NewsFeedItem {
                id: "id-1".to_string(),
                date: "2025-01-01".to_string(),
                title: "Item 1".to_string(),
                summary: None,
                url: Some("https://example.com/news/1".to_string()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
            NewsFeedItem {
                id: "id-2".to_string(),
                date: "2025-01-02".to_string(),
                title: "Item 2".to_string(),
                summary: None,
                url: Some("https://example.com/news/2".to_string()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
            NewsFeedItem {
                id: "id-3".to_string(),
                date: "2025-01-03".to_string(),
                title: "Item 3".to_string(),
                summary: None,
                url: Some("https://example.com/news/3".to_string()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
        ];

        let unread: Vec<NewsFeedItem> = items
            .into_iter()
            .filter(|item| {
                !read_ids.contains(&item.id)
                    && item.url.as_ref().is_none_or(|url| !read_urls.contains(url))
            })
            .collect();

        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].id, "id-3");
    }
}

/// What: Run the main event loop, processing all channel messages and rendering the UI.
///
/// Inputs:
/// - `terminal`: Optional terminal for rendering (None in headless mode)
/// - `app`: Application state
/// - `channels`: Communication channels for background workers
///
/// Output: None (runs until exit condition is met)
///
/// Details:
/// - Renders UI frames and handles all channel messages (events, search results, details,
///   preflight data, PKGBUILD, news, status, etc.)
/// - Exits when event handler returns true (e.g., quit command)
/// - Checks for `trigger_startup_news_fetch` flag and triggers fetch if set
pub async fn run_event_loop(
    terminal: &mut Option<Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>>,
    app: &mut AppState,
    channels: &mut Channels,
) {
    loop {
        // Check if we need to trigger startup news fetch
        if app.trigger_startup_news_fetch {
            app.trigger_startup_news_fetch = false;
            trigger_startup_news_fetch(channels, &mut *app);
        }

        if let Some(t) = terminal.as_mut() {
            let _ = t.draw(|f| ui(f, app));
        }

        if process_channel_messages(app, channels).await {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_pi_scan_progress;
    use super::apply_pi_scan_result;
    use super::apply_pi_scan_runtime_notice;
    use super::handle_aur_vote_response;
    use super::handle_aur_vote_state_response;
    use super::handle_index_notification;
    use super::handle_news_content;
    use super::handle_updates_list;
    use super::install_pi_scan_owner;
    use super::project_pi_scan_rollback_report;
    use crate::app::runtime::background::Channels;
    use crate::app::runtime::workers::UpdateCheckPayload;
    use crate::state::AppState;
    use crate::state::types::{NewsFeedItem, NewsFeedSource};

    /// What: Build a minimal `NewsFeedItem` for news content tests.
    ///
    /// Inputs:
    /// - `id`: Stable identifier for the item.
    /// - `url`: URL to associate with the item.
    ///
    /// Output:
    /// - `NewsFeedItem` with Arch news source and empty optional fields.
    ///
    /// Details:
    /// - Uses a fixed date to keep assertions deterministic.
    fn make_news_item(id: &str, url: &str) -> NewsFeedItem {
        NewsFeedItem {
            id: id.to_string(),
            date: "2024-01-01".to_string(),
            title: format!("Title {id}"),
            summary: None,
            url: Some(url.to_string()),
            source: NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        }
    }

    /// Build one deterministic Pi Scan request for projection regressions.
    fn pi_scan_request(
        package_base: &str,
        oid_character: char,
    ) -> crate::state::pi_scan::PiScanJobRequest {
        crate::state::pi_scan::PiScanJobRequest {
            request_id: 7,
            key: crate::state::pi_scan::PiScanQueueKey {
                package_base: crate::logic::pi_scan::identity::PackageBase::new(package_base)
                    .expect("package base"),
                commit_oid: crate::logic::pi_scan::identity::CommitOid::new(
                    oid_character.to_string().repeat(40),
                )
                .expect("commit oid"),
            },
            priority: crate::state::pi_scan::PiScanPriority::Foreground,
            reservation: crate::state::pi_scan::PiScanReservation {
                tokens: 100,
                cost_microusd: 20,
            },
            manual_budget_override_confirmed: false,
        }
    }

    /// Build one deterministic validated execution receipt for projection regressions.
    fn pi_scan_receipt(package_base: &str) -> crate::pi_scan_orchestrator::ExecutionReceipt {
        let request = pi_scan_request(package_base, 'a');
        crate::pi_scan_orchestrator::ExecutionReceipt {
            result: crate::logic::pi_scan::result::MergedScanResult {
                identity: crate::logic::pi_scan::result::ExpectedIdentity {
                    scan_id: "scan-validated".to_string(),
                    package_base: package_base.to_string(),
                    commit_oid: request.key.commit_oid.as_str().to_string(),
                },
                coverage: crate::logic::pi_scan::result::Coverage::Complete,
                limitations: Vec::new(),
                findings: Vec::new(),
            },
            observed_head_oid: request.key.commit_oid,
            provenance: crate::logic::pi_scan::result::ScanProvenance {
                pi_version: "0.84.0".to_string(),
                extension_sha256: "b".repeat(64),
                prompt_version: "pacsea-scan-prompt-1".to_string(),
                schema_version: "pacsea-scan-result-1".to_string(),
                tool_contract_version: "pacsea-scan-tools-1".to_string(),
                attempts: Vec::new(),
            },
            manifests: vec![crate::logic::pi_scan::manifest::CanonicalManifest::new(
                Vec::new(),
            )],
            usage: crate::state::pi_scan::PiScanActualUsage {
                tokens: 10,
                cost_microusd: 2,
            },
            stale: false,
            mutable_sources: Vec::new(),
        }
    }

    #[tokio::test]
    /// Replacing the runtime owner must retain its dedicated typed notice receiver.
    async fn pi_scan_owner_replacement_retains_notice_receiver() {
        use crate::app::runtime::workers::pi_scan::{
            PiScanNoticeProvenance, PiScanNoticeSource, PiScanPolicyAcknowledgement,
            PiScanRuntimeAction, PiScanRuntimeChannels, PiScanRuntimeNotice,
        };
        let mut channels =
            Channels::new(std::path::PathBuf::from("/tmp")).expect("channels should construct");
        let (request_tx, _request_rx) = tokio::sync::mpsc::unbounded_channel();
        let (cancel_tx, _cancel_rx) = tokio::sync::mpsc::unbounded_channel();
        let (session_tx, _session_rx) = tokio::sync::mpsc::unbounded_channel();
        let (shutdown_tx, _shutdown_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_progress_tx, progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_result_tx, result_rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let notice = PiScanRuntimeNotice {
            provenance: PiScanNoticeProvenance {
                source: PiScanNoticeSource::Foreground,
                action: Some(PiScanRuntimeAction::Pause),
                correlation_id: None,
            },
            user_paused: true,
            acknowledgement: PiScanPolicyAcknowledgement::Queued,
        };
        notice_tx.send(notice.clone()).expect("notice send");

        install_pi_scan_owner(
            &mut channels,
            PiScanRuntimeChannels {
                request_tx,
                cancel_tx,
                session_tx,
                shutdown_tx,
                progress_rx,
                result_rx,
                notice_rx,
            },
            crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions::default(),
        );

        assert_eq!(channels.pi_scan_notice_rx.try_recv(), Ok(notice));
    }

    #[test]
    /// Setup failures after wizard closure must surface and terminalize workspace ownership.
    fn pi_scan_closed_wizard_setup_failure_reaches_workspace_notice() {
        let mut app = AppState::default();
        app.pi_scan.setup_transaction = Some(crate::state::pi_scan_ui::PiScanSetupTransaction {
            correlation_id: 55,
            abandonment: crate::state::pi_scan_ui::PiScanSetupAbandonment::Active,
        });

        super::apply_pi_scan_setup_event(
            &mut app,
            crate::app::runtime::workers::pi_scan_setup::PiScanSetupEvent::Failed {
                correlation_id: 55,
                stage: crate::app::runtime::workers::pi_scan_setup::PiScanSetupStage::Activation,
                reason: "candidate failed".to_string(),
            },
        );

        assert!(app.pi_scan.setup_transaction.is_none());
        assert!(app.pi_scan.notices.foreground.is_some());

        let mut timed_out = AppState::default();
        timed_out.pi_scan.last_setup_correlation = 56;
        super::apply_pi_scan_setup_timeout(
            &mut timed_out,
            crate::app::runtime::workers::pi_scan_setup::PiScanSetupTimeout {
                correlation_id: 56,
                stage: crate::app::runtime::workers::pi_scan_setup::PiScanSetupStage::Probe,
                deadline: std::time::Duration::from_secs(30),
            },
        );
        assert!(timed_out.pi_scan.notices.foreground.is_some());
    }

    #[test]
    /// Typed policy acknowledgements distinguish queued, persisted, and failed durability.
    fn pi_scan_policy_notice_projects_without_false_persistence() {
        use crate::app::runtime::workers::pi_scan::{
            PiScanNoticeProvenance, PiScanNoticeSource, PiScanPolicyAcknowledgement,
            PiScanRuntimeAction, PiScanRuntimeNotice,
        };
        let mut app = AppState::default();
        let provenance = PiScanNoticeProvenance {
            source: PiScanNoticeSource::Foreground,
            action: Some(PiScanRuntimeAction::Pause),
            correlation_id: Some(8),
        };

        apply_pi_scan_runtime_notice(
            &mut app,
            PiScanRuntimeNotice {
                provenance,
                user_paused: true,
                acknowledgement: PiScanPolicyAcknowledgement::Queued,
            },
        );
        assert!(
            !app.pi_scan
                .runtime
                .pause_reasons
                .contains(&crate::state::pi_scan::PiScanPauseReason::User)
        );

        apply_pi_scan_runtime_notice(
            &mut app,
            PiScanRuntimeNotice {
                provenance,
                user_paused: true,
                acknowledgement: PiScanPolicyAcknowledgement::Persisted,
            },
        );
        assert!(
            app.pi_scan
                .runtime
                .pause_reasons
                .contains(&crate::state::pi_scan::PiScanPauseReason::User)
        );

        apply_pi_scan_runtime_notice(
            &mut app,
            PiScanRuntimeNotice {
                provenance: PiScanNoticeProvenance {
                    source: PiScanNoticeSource::Background,
                    action: Some(PiScanRuntimeAction::Resume),
                    correlation_id: Some(8),
                },
                user_paused: false,
                acknowledgement: PiScanPolicyAcknowledgement::Failed {
                    reason: "durability unavailable".to_string(),
                },
            },
        );
        assert!(
            app.pi_scan
                .runtime
                .pause_reasons
                .contains(&crate::state::pi_scan::PiScanPauseReason::User)
        );
        assert!(app.pi_scan.notices.background.is_some());
    }

    #[tokio::test]
    /// Cancellation without a teardown warning must retain existing foreground feedback.
    async fn pi_scan_cancelled_without_warning_preserves_foreground_feedback() {
        let mut app = AppState::default();
        app.pi_scan.set_foreground_notice(
            "cancel requested",
            crate::state::pi_scan_ui::PiScanNoticeSeverity::Info,
        );
        let request = pi_scan_request("cancel-demo", 'c');
        let record = crate::state::pi_scan::PiScanTerminalRecord {
            request,
            correlation_id: 9,
            status: crate::state::pi_scan::PiScanTerminalStatus::Cancelled,
            finished_at_unix: 10,
        };
        let channels =
            Channels::new(std::path::PathBuf::from("/tmp")).expect("channels should construct");

        apply_pi_scan_result(
            &mut app,
            &channels,
            crate::app::runtime::workers::pi_scan::PiScanResultMessage::Cancelled {
                record,
                warning: None,
            },
        );

        assert_eq!(
            app.pi_scan.notices.foreground_text(),
            Some("cancel requested")
        );

        app.pi_scan.notices.foreground = None;
        let request = pi_scan_request("cancel-demo-empty", 'd');
        apply_pi_scan_result(
            &mut app,
            &channels,
            crate::app::runtime::workers::pi_scan::PiScanResultMessage::Cancelled {
                record: crate::state::pi_scan::PiScanTerminalRecord {
                    request,
                    correlation_id: 10,
                    status: crate::state::pi_scan::PiScanTerminalStatus::Cancelled,
                    finished_at_unix: 11,
                },
                warning: None,
            },
        );
        assert!(app.pi_scan.notices.foreground.is_some());
    }

    #[tokio::test]
    /// A unique validated result must announce completion and increment unseen state once.
    async fn pi_scan_validated_inserts_announces_and_increments_unseen_once() {
        let mut app = AppState::default();
        app.pi_scan.view = crate::state::PiScanView::Progress;
        let channels =
            Channels::new(std::path::PathBuf::from("/tmp")).expect("channels should construct");
        let message = crate::app::runtime::workers::pi_scan::PiScanResultMessage::Validated(
            Box::new(pi_scan_receipt("validated-demo")),
        );

        apply_pi_scan_result(&mut app, &channels, message.clone());
        apply_pi_scan_result(&mut app, &channels, message);

        assert_eq!(app.pi_scan.results.len(), 1);
        assert_eq!(app.pi_scan.unseen_result_count, 1);
        assert!(app.pi_scan.notices.foreground.is_some());
    }

    #[test]
    /// An abandoned transfer must terminalize through an explicit correlated rollback report.
    fn pi_scan_abandoned_transfer_explicitly_reports_rollback() {
        let mut app = AppState::default();
        app.pi_scan.setup_transaction = Some(crate::state::pi_scan_ui::PiScanSetupTransaction {
            correlation_id: 44,
            abandonment: crate::state::pi_scan_ui::PiScanSetupAbandonment::AbandonRequested,
        });

        project_pi_scan_rollback_report(
            &mut app,
            crate::app::runtime::workers::pi_scan_setup::PiScanRollbackReport {
                correlation_id: 44,
                outcome:
                    crate::app::runtime::workers::pi_scan_setup::PiScanRollbackOutcome::Succeeded,
            },
        );

        assert!(app.pi_scan.setup_transaction.is_none());
        assert!(app.pi_scan.notices.foreground.is_some());
    }

    #[test]
    /// Exact queue intent waits for every intended identity and ignores unrelated observations.
    fn pi_scan_exact_queue_intent_waits_for_all_intended_identities() {
        let mut app = AppState::default();
        app.pi_scan.targets.extend([
            crate::state::PiScanTarget {
                package_name: "alpha-bin".to_string(),
                package_base: "alpha-bin".to_string(),
                commit_oid: None,
                selected: true,
                status: crate::state::PiScanTargetStatus::Unbaselined,
            },
            crate::state::PiScanTarget {
                package_name: "beta-bin".to_string(),
                package_base: "beta-bin".to_string(),
                commit_oid: None,
                selected: true,
                status: crate::state::PiScanTargetStatus::Unbaselined,
            },
        ]);
        app.pi_scan.snapshot_queue_intent();
        let (request_tx, mut request_rx) = tokio::sync::mpsc::unbounded_channel();
        let observed = |package_name: &str, package_base: &str, character: char| {
            crate::pi_scan_orchestrator::FrozenScanIdentity {
                scan_id: format!("scan-{package_name}"),
                package_name: package_name.to_string(),
                package_base: crate::logic::pi_scan::identity::PackageBase::new(package_base)
                    .expect("package base"),
                installed_names: vec![package_name.to_string()],
                installed_version: "1.0-1".to_string(),
                candidate_version: None,
                commit_oid: crate::logic::pi_scan::identity::CommitOid::new(
                    character.to_string().repeat(40),
                )
                .expect("commit oid"),
                observed_head_oid: crate::logic::pi_scan::identity::CommitOid::new(
                    character.to_string().repeat(40),
                )
                .expect("observed oid"),
                cycle_id: "manual".to_string(),
                provider: "provider".to_string(),
                model: "model".to_string(),
                priority: crate::state::pi_scan::PiScanPriority::Foreground,
                reservation: crate::state::pi_scan::PiScanReservation {
                    tokens: 100,
                    cost_microusd: 20,
                },
            }
        };

        apply_pi_scan_progress(
            &mut app,
            Some(&request_tx),
            crate::app::runtime::workers::pi_scan::PiScanProgressMessage::Observed {
                targets: vec![observed("unrelated-bin", "unrelated", 'c')],
            },
        );
        assert!(app.pi_scan.pending_queue_intent.is_some());
        assert!(request_rx.try_recv().is_err());
        apply_pi_scan_progress(
            &mut app,
            Some(&request_tx),
            crate::app::runtime::workers::pi_scan::PiScanProgressMessage::Observed {
                targets: vec![observed("alpha-bin", "alpha", 'a')],
            },
        );
        assert!(app.pi_scan.pending_queue_intent.is_some());
        assert!(request_rx.try_recv().is_err());
        apply_pi_scan_progress(
            &mut app,
            Some(&request_tx),
            crate::app::runtime::workers::pi_scan::PiScanProgressMessage::Observed {
                targets: vec![observed("beta-bin", "beta", 'b')],
            },
        );
        assert!(app.pi_scan.pending_queue_intent.is_none());
        let mut queued = Vec::new();
        while let Ok(message) = request_rx.try_recv() {
            if let crate::app::runtime::workers::pi_scan::PiScanRequestMessage::Enqueue(request) =
                message
            {
                queued.push(request.key.package_base.as_str().to_string());
            }
        }
        queued.sort();
        assert_eq!(queued, ["alpha", "beta"]);
    }

    #[test]
    /// What: Ensure stale news content responses do not clear loading for the active selection.
    ///
    /// Inputs:
    /// - App with selection on item `b` and loading flagged true.
    /// - Content response for outdated item `a`.
    ///
    /// Output:
    /// - `news_content_loading` remains true and displayed content stays `None`.
    ///
    /// Details:
    /// - Prevents stale responses from cancelling the fetch for the current item.
    fn handle_news_content_keeps_loading_for_mismatched_url() {
        let mut app = AppState {
            news_results: vec![
                make_news_item("a", "https://example.com/a"),
                make_news_item("b", "https://example.com/b"),
            ],
            news_selected: 1,
            news_content_loading: true,
            ..AppState::default()
        };

        handle_news_content(&mut app, "https://example.com/a", "old".to_string());

        assert!(!app.news_content_loading);
        assert!(app.news_content.is_none());
        assert!(app.news_content_cache.contains_key("https://example.com/a"));
    }

    #[test]
    /// What: Ensure news content responses for the selected item clear loading and set content.
    ///
    /// Inputs:
    /// - App with selection on item `a` and loading flagged true.
    /// - Content response for the same item.
    ///
    /// Output:
    /// - Loading flag clears and content is stored.
    ///
    /// Details:
    /// - Confirms the happy path still updates UI state correctly.
    fn handle_news_content_updates_current_selection() {
        let mut app = AppState {
            news_results: vec![make_news_item("a", "https://example.com/a")],
            news_content_loading: true,
            ..AppState::default()
        };

        handle_news_content(&mut app, "https://example.com/a", "payload".to_string());

        assert!(!app.news_content_loading);
        assert_eq!(app.news_content, Some("payload".to_string()));
        assert!(app.news_content_cache.contains_key("https://example.com/a"));
    }

    #[test]
    /// What: Verify degraded update-check payloads set authoritative flag and user toast.
    ///
    /// Inputs:
    /// - Default `AppState` and a synthetic [`UpdateCheckPayload`] with `authoritative` false.
    ///
    /// Output:
    /// - `updates_last_check_authoritative` is `Some(false)` and a toast is scheduled.
    ///
    /// Details:
    /// - Ensures silent failure modes surface guidance instead of looking like a clean zero-update state.
    fn handle_updates_list_degraded_surfaces_toast() {
        let mut app = AppState::default();
        let payload = UpdateCheckPayload {
            count: 0,
            package_names: Vec::new(),
            candidates: Vec::new(),
            authoritative: false,
            reason_codes: vec!["stale_db_fallback".to_string()],
            official_strategy: "stale_pacman_qu",
        };
        handle_updates_list(&mut app, payload, None);
        assert_eq!(app.updates_last_check_authoritative, Some(false));
        assert!(app.toast_message.is_some());
        assert!(app.toast_expires_at.is_some());
    }

    #[test]
    /// What: Verify authoritative update-check payloads do not set a degraded toast.
    ///
    /// Inputs:
    /// - Default `AppState` and a synthetic authoritative [`UpdateCheckPayload`].
    ///
    /// Output:
    /// - `updates_last_check_authoritative` is `Some(true)`; no toast from this handler.
    ///
    /// Details:
    /// - Guards against noisy toasts on the happy path.
    fn handle_updates_list_authoritative_skips_degraded_toast() {
        let mut app = AppState::default();
        let payload = UpdateCheckPayload {
            count: 2,
            package_names: vec!["a".to_string(), "b".to_string()],
            candidates: Vec::new(),
            authoritative: true,
            reason_codes: Vec::new(),
            official_strategy: "checkupdates_db",
        };
        handle_updates_list(&mut app, payload, None);
        assert_eq!(app.updates_last_check_authoritative, Some(true));
        assert!(app.toast_message.is_none());
    }

    #[test]
    /// What: Verify durable material-bound consent restores into the Pi workspace projection.
    fn pi_scan_restored_consent_updates_runtime_and_setup_projection() {
        let mut app = AppState::default();
        apply_pi_scan_progress(
            &mut app,
            None,
            crate::app::runtime::workers::pi_scan::PiScanProgressMessage::RestoredConsent {
                consent: crate::state::pi_scan::PiScanConsentState {
                    background_observation: true,
                    paid_execution: true,
                },
                setup: crate::pi_scan_orchestrator::PiScanSetupConsentState {
                    configuration_binding: "binding".to_string(),
                    disclosure_confirmed: true,
                    fallback_confirmed: true,
                    background_paid_execution: true,
                    readiness_warning_confirmed: true,
                    confirmed_pi_version: "0.84.0".to_string(),
                    confirmed_pricing_binding: "pricing".to_string(),
                },
            },
        );

        assert!(app.pi_scan.runtime.consent.background_observation);
        assert!(app.pi_scan.runtime.consent.paid_execution);
        assert!(app.pi_scan.disclosure_confirmed);
        assert!(app.pi_scan.fallback_confirmed);
        assert!(app.pi_scan.background_paid_execution_confirmed);
        assert!(app.pi_scan.readiness_warning_confirmed);
        assert_eq!(app.pi_scan.verified_pi_version, "0.84.0");
        assert_eq!(app.pi_scan.verified_pricing_binding, "pricing");
        assert!(app.pi_scan.setup_facts_verified);
    }

    #[test]
    /// What: Verify exact setup facts become visible before material consent can be granted.
    fn pi_scan_setup_verification_projects_exact_pricing_facts() {
        let mut app = AppState::default();
        apply_pi_scan_progress(
            &mut app,
            None,
            crate::app::runtime::workers::pi_scan::PiScanProgressMessage::SetupVerified(
                crate::pi_scan_orchestrator::SetupSnapshot {
                    pi_version: "0.84.0".to_string(),
                    available_models: vec![("provider".to_string(), "model".to_string())],
                    selected_provider: "provider".to_string(),
                    selected_model: "model".to_string(),
                    reservation: crate::state::pi_scan::PiScanReservation {
                        tokens: 10_000,
                        cost_microusd: 50,
                    },
                    route_reservations: vec![(
                        "provider".to_string(),
                        "model".to_string(),
                        crate::state::pi_scan::PiScanReservation {
                            tokens: 10_000,
                            cost_microusd: 50,
                        },
                    )],
                    pricing_binding: "pricing-binding".to_string(),
                    pricing_observed_at_unix_seconds: 1_000,
                    maximum_pricing_age_seconds: 900,
                    pricing_summary: vec![
                        "provider/model · Pi native model metadata · cost={input:1,output:2}"
                            .to_string(),
                    ],
                },
            ),
        );

        assert!(app.pi_scan.setup_facts_verified);
        assert_eq!(app.pi_scan.verified_pi_version, "0.84.0");
        assert_eq!(app.pi_scan.verified_provider, "provider");
        assert_eq!(app.pi_scan.verified_model, "model");
        assert_eq!(app.pi_scan.verified_available_models, ["provider/model"]);
        assert_eq!(app.pi_scan.verified_reservation.tokens, 10_000);
        assert_eq!(app.pi_scan.verified_reservation.cost_microusd, 50);
        assert_eq!(app.pi_scan.verified_pricing_binding, "pricing-binding");
        assert_eq!(app.pi_scan.verified_pricing_summary.len(), 1);
        assert!(matches!(
            app.pi_scan.readiness,
            crate::state::PiScanReadiness::Confirmed
        ));
    }

    #[test]
    /// What: Verify validated persisted Pi results reopen once after restart projection.
    fn pi_scan_restored_results_reopen_without_duplicates() {
        let mut app = AppState::default();
        let merged = crate::logic::pi_scan::result::MergedScanResult {
            identity: crate::logic::pi_scan::result::ExpectedIdentity {
                scan_id: "scan-restored".to_string(),
                package_base: "demo".to_string(),
                commit_oid: "a".repeat(40),
            },
            coverage: crate::logic::pi_scan::result::Coverage::Complete,
            limitations: Vec::new(),
            findings: Vec::new(),
        };
        let document =
            crate::logic::pi_scan::result_store::StoredScanResult::from_validated_with_staleness(
                "scan-restored",
                &merged,
                &crate::logic::pi_scan::result::ScanProvenance {
                    pi_version: "0.84.0".to_string(),
                    extension_sha256: "b".repeat(64),
                    prompt_version: "pacsea-scan-prompt-1".to_string(),
                    schema_version: "pacsea-scan-result-1".to_string(),
                    tool_contract_version: "pacsea-scan-tools-1".to_string(),
                    attempts: Vec::new(),
                },
                &[crate::logic::pi_scan::manifest::CanonicalManifest::new(
                    Vec::new(),
                )],
                1,
                false,
                &"c".repeat(40),
                true,
            )
            .expect("stored result");
        let progress =
            crate::app::runtime::workers::pi_scan::PiScanProgressMessage::RestoredResults {
                documents: vec![document.clone()],
            };
        apply_pi_scan_progress(&mut app, None, progress);
        apply_pi_scan_progress(
            &mut app,
            None,
            crate::app::runtime::workers::pi_scan::PiScanProgressMessage::RestoredResults {
                documents: vec![document],
            },
        );

        assert_eq!(app.pi_scan.results.len(), 1);
        assert_eq!(app.pi_scan.results[0].observed_head_oid, "c".repeat(40));
        assert!(app.pi_scan.results[0].stale);
    }

    #[tokio::test]
    /// What: Ensure index-ready notification re-runs current query.
    ///
    /// Inputs:
    /// - `AppState` with `loading_index=true` and default query counters.
    /// - Runtime channels instance.
    ///
    /// Output:
    /// - `loading_index` is cleared.
    /// - `latest_query_id` advances, confirming a query dispatch was triggered.
    ///
    /// Details:
    /// - Prevents first-launch empty result list after async index refresh finishes.
    async fn handle_index_notification_retriggers_query() {
        let mut app = AppState {
            loading_index: true,
            ..AppState::default()
        };
        let channels =
            Channels::new(std::path::PathBuf::from("/tmp")).expect("channels should construct");
        let latest_before = app.latest_query_id;

        let should_exit = handle_index_notification(&mut app, &channels);

        assert!(!should_exit);
        assert!(!app.loading_index);
        assert!(app.latest_query_id > latest_before);
    }

    #[test]
    /// What: Ensure successful AUR vote responses are surfaced as toasts.
    ///
    /// Inputs:
    /// - `AppState` default and a synthetic success vote response.
    ///
    /// Output:
    /// - Toast message and expiration are set.
    ///
    /// Details:
    /// - Confirms UI-safe success feedback path from runtime worker results.
    fn handle_aur_vote_response_success_sets_toast() {
        let mut app = AppState::default();
        let response = crate::app::runtime::workers::aur_vote::AurVoteResponse {
            result: Ok(crate::sources::AurVoteOutcome {
                action: crate::sources::VoteAction::Vote,
                pkgbase: "pacsea-bin".to_string(),
                dry_run: false,
            }),
        };

        handle_aur_vote_response(&mut app, response);

        let toast = app
            .toast_message
            .as_ref()
            .expect("success vote should set a toast");
        assert!(toast.contains("Voted for"));
        assert!(app.toast_expires_at.is_some());
    }

    #[test]
    /// What: Ensure dry-run vote responses do not persist local vote state.
    ///
    /// Inputs:
    /// - `AppState` default and a synthetic dry-run success vote response.
    ///
    /// Output:
    /// - Vote-state cache remains unchanged and dirty flag stays false.
    ///
    /// Details:
    /// - Dry-run must not mark package votes as changed because no remote mutation occurred.
    fn handle_aur_vote_response_dry_run_does_not_mark_cache_dirty() {
        let mut app = AppState::default();
        let before_vote_state = app.aur_vote_state_by_pkgbase.clone();
        let before_dirty = app.aur_vote_state_dirty;
        let response = crate::app::runtime::workers::aur_vote::AurVoteResponse {
            result: Ok(crate::sources::AurVoteOutcome {
                action: crate::sources::VoteAction::Vote,
                pkgbase: "pacsea-bin".to_string(),
                dry_run: true,
            }),
        };

        handle_aur_vote_response(&mut app, response);

        assert_eq!(app.aur_vote_state_by_pkgbase, before_vote_state);
        assert_eq!(app.aur_vote_state_dirty, before_dirty);
        let toast = app
            .toast_message
            .as_ref()
            .expect("dry-run vote should set a toast");
        assert!(toast.contains("[dry-run]"));
        assert!(app.toast_expires_at.is_some());
    }

    #[test]
    /// What: Ensure failed AUR vote responses are surfaced as actionable alerts.
    ///
    /// Inputs:
    /// - `AppState` default and synthetic auth failure response.
    ///
    /// Output:
    /// - Modal transitions to `Modal::Alert` with actionable guidance.
    ///
    /// Details:
    /// - Verifies runtime failure mapping reaches user-visible guidance text.
    fn handle_aur_vote_response_auth_failure_sets_alert() {
        let mut app = AppState::default();
        let response = crate::app::runtime::workers::aur_vote::AurVoteResponse {
            result: Err(crate::sources::AurVoteError::AuthFailed(
                "Permission denied".to_string(),
            )),
        };

        handle_aur_vote_response(&mut app, response);

        match app.modal {
            crate::state::Modal::Alert { message } => {
                assert!(message.contains("AUR vote failed"));
                assert!(message.contains("Upload your SSH public key"));
            }
            other => panic!("expected alert modal, got {other:?}"),
        }
    }

    #[test]
    /// What: Ensure `AlreadyVoted` syncs local vote cache without blocking alert.
    ///
    /// Inputs:
    /// - `AppState` default and synthetic `AlreadyVoted` response for one pkgbase.
    ///
    /// Output:
    /// - Vote-state cache is set to `Voted`, persistence dirty flag is set, and toast is shown.
    ///
    /// Details:
    /// - Keeps local state aligned with AUR when duplicate vote attempts occur.
    fn handle_aur_vote_response_already_voted_syncs_cache() {
        let mut app = AppState::default();
        let response = crate::app::runtime::workers::aur_vote::AurVoteResponse {
            result: Err(crate::sources::AurVoteError::AlreadyVoted(
                "pacsea-bin".to_string(),
            )),
        };

        handle_aur_vote_response(&mut app, response);

        assert!(matches!(
            app.aur_vote_state_by_pkgbase.get("pacsea-bin"),
            Some(crate::state::app_state::AurVoteStateUi::Voted)
        ));
        assert!(app.aur_vote_state_dirty);
        assert!(matches!(app.modal, crate::state::Modal::None));
        assert!(
            app.toast_message
                .as_ref()
                .is_some_and(|msg| msg.contains("Already voted"))
        );
    }

    #[test]
    /// What: Ensure `NotVoted` syncs local vote cache without blocking alert.
    ///
    /// Inputs:
    /// - `AppState` default and synthetic `NotVoted` response for one pkgbase.
    ///
    /// Output:
    /// - Vote-state cache is set to `NotVoted`, persistence dirty flag is set, and toast is shown.
    ///
    /// Details:
    /// - Keeps local state aligned with AUR when duplicate unvote attempts occur.
    fn handle_aur_vote_response_not_voted_syncs_cache() {
        let mut app = AppState::default();
        let response = crate::app::runtime::workers::aur_vote::AurVoteResponse {
            result: Err(crate::sources::AurVoteError::NotVoted(
                "pacsea-bin".to_string(),
            )),
        };

        handle_aur_vote_response(&mut app, response);

        assert!(matches!(
            app.aur_vote_state_by_pkgbase.get("pacsea-bin"),
            Some(crate::state::app_state::AurVoteStateUi::NotVoted)
        ));
        assert!(app.aur_vote_state_dirty);
        assert!(matches!(app.modal, crate::state::Modal::None));
        assert!(
            app.toast_message
                .as_ref()
                .is_some_and(|msg| msg.contains("No vote exists"))
        );
    }

    #[test]
    /// What: Ensure vote-state worker responses update the app vote-state cache.
    ///
    /// Inputs:
    /// - `AppState` default and a synthetic `Voted` vote-state response.
    ///
    /// Output:
    /// - Vote-state cache stores `AurVoteStateUi::Voted` for pkgbase.
    ///
    /// Details:
    /// - Verifies event-loop mapping for live check responses.
    fn handle_aur_vote_state_response_updates_cache() {
        let mut app = AppState::default();
        let response = crate::app::runtime::workers::aur_vote::AurVoteStateResponse {
            pkgbase: "pacsea-bin".to_string(),
            result: Ok(crate::sources::AurPackageVoteState::Voted),
        };

        handle_aur_vote_state_response(&mut app, response);

        assert!(matches!(
            app.aur_vote_state_by_pkgbase.get("pacsea-bin"),
            Some(crate::state::app_state::AurVoteStateUi::Voted)
        ));
    }

    #[test]
    /// What: Ensure unsupported vote-state command errors degrade to `Unknown`.
    ///
    /// Inputs:
    /// - `AppState` default and synthetic unsupported-command vote-state response.
    ///
    /// Output:
    /// - Vote-state cache stores `AurVoteStateUi::Unknown` for pkgbase.
    ///
    /// Details:
    /// - Prevents noisy inline error rendering when upstream SSH endpoint
    ///   does not expose `list-votes`.
    fn handle_aur_vote_state_response_unsupported_maps_to_unknown() {
        let mut app = AppState::default();
        let pkgbase = "pkg-unsupported-unknown-test";
        let response = crate::app::runtime::workers::aur_vote::AurVoteStateResponse {
            pkgbase: pkgbase.to_string(),
            result: Err(crate::sources::AurVoteError::Unexpected(
                "AUR SSH server does not support vote-state lookup.".to_string(),
            )),
        };

        handle_aur_vote_state_response(&mut app, response);

        assert!(matches!(
            app.aur_vote_state_by_pkgbase.get(pkgbase),
            Some(crate::state::app_state::AurVoteStateUi::Unknown)
        ));
        assert!(!app.aur_vote_state_lookup_supported);
    }

    #[test]
    /// What: Ensure unsupported live lookup does not override stable persisted vote-state.
    ///
    /// Inputs:
    /// - Existing `Voted` cache entry and unsupported-command vote-state response.
    ///
    /// Output:
    /// - Cache remains `Voted` and live lookup is disabled for the runtime session.
    ///
    /// Details:
    /// - Prevents "Loading..." followed by losing the visible vote-state when `list-votes`
    ///   is unavailable upstream.
    fn handle_aur_vote_state_response_unsupported_keeps_stable_cache() {
        let mut app = AppState::default();
        app.aur_vote_state_by_pkgbase.insert(
            "pacsea-bin".to_string(),
            crate::state::app_state::AurVoteStateUi::Voted,
        );
        let response = crate::app::runtime::workers::aur_vote::AurVoteStateResponse {
            pkgbase: "pacsea-bin".to_string(),
            result: Err(crate::sources::AurVoteError::Unexpected(
                "AUR SSH server does not support vote-state lookup.".to_string(),
            )),
        };

        handle_aur_vote_state_response(&mut app, response);

        assert!(matches!(
            app.aur_vote_state_by_pkgbase.get("pacsea-bin"),
            Some(crate::state::app_state::AurVoteStateUi::Voted)
        ));
        assert!(!app.aur_vote_state_lookup_supported);
    }
}
