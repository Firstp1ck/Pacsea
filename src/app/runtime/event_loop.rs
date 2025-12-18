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
    handle_comments_result, handle_news, handle_pkgbuild_result, handle_status,
    handle_summary_result, handle_tick,
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
    let _ = channels.tick_tx.send(());
    false
}

/// What: Handle updates list received from background worker.
///
/// Inputs:
/// - `app`: Application state
/// - `count`: Number of available updates
/// - `list`: List of update package names
///
/// Output: None (modifies app state in place)
///
/// Details:
/// - Updates app state with update count and list
/// - If pending updates modal is set, opens the updates modal
fn handle_updates_list(app: &mut AppState, count: usize, list: Vec<String>) {
    app.updates_count = Some(count);
    app.updates_list = list;
    app.updates_loading = false;
    if app.pending_updates_modal {
        app.pending_updates_modal = false;
        let updates_file = crate::theme::lists_dir().join("available_updates.txt");
        let entries = parse_updates_file(&updates_file);
        app.modal = crate::state::Modal::Updates {
            entries,
            scroll: 0,
            selected: 0,
        };
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
#[allow(clippy::cognitive_complexity)]
async fn process_channel_messages(app: &mut AppState, channels: &mut Channels) -> bool {
    select! {
        Some(ev) = channels.event_rx.recv() => {
            crate::events::handle_event(
                &ev,
                app,
                &channels.query_tx,
                &channels.details_req_tx,
                &channels.preview_tx,
                &channels.add_tx,
                &channels.pkgb_req_tx,
                &channels.comments_req_tx,
            )
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
        Some(feed) = channels.news_feed_rx.recv() => {
            handle_news_feed_items(app, feed);
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
                // On Linux, show error to user via Alert modal
                app.modal = crate::state::Modal::Alert {
                    message: msg,
                };
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
        Some((count, list)) = channels.updates_rx.recv() => {
            handle_updates_list(app, count, list);
            false
        }
        Some(executor_output) = channels.executor_res_rx.recv() => {
            handle_executor_output(app, executor_output);
            false
        }
        Some(post_summary_data) = channels.post_summary_res_rx.recv() => {
            handle_post_summary_result(app, post_summary_data);
            false
        }
        else => false
    }
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
#[allow(clippy::too_many_lines)] // Function handles multiple executor output types and modal transitions
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
    use super::handle_news_content;
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
}
