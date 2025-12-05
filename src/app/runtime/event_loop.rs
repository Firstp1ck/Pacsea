use ratatui::Terminal;
use tokio::select;

use crate::state::{AppState, PackageItem};
use crate::ui::ui;
use crate::util::parse_update_entry;

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
        Some(msg) = channels.net_err_rx.recv() => {
            tracing::warn!(error = %msg, "Network error received");
            #[cfg(not(windows))]
            {
                // On Linux, show error to user via Alert modal
                app.modal = crate::state::Modal::Alert {
                    message: msg.clone(),
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
            );
            false
        }
        Some(todays) = channels.news_rx.recv() => {
            handle_news(app, &todays);
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

/// What: Handle executor output messages.
///
/// Inputs:
/// - `app`: Application state
/// - `output`: Executor output message
///
/// Details:
/// - Updates `PreflightExec` modal with log lines or completion status
/// - For successful install operations, tracks installed packages and refreshes installed packages pane
/// - For successful remove operations, closes modal, clears remove list, and refreshes installed packages pane
fn handle_executor_output(app: &mut AppState, output: crate::install::ExecutorOutput) {
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

                    // Handle successful operations: refresh installed packages and update UI
                    match action {
                        crate::state::PreflightAction::Install => {
                            let installed_names: Vec<String> =
                                items.iter().map(|p| p.name.clone()).collect();

                            // Set pending install names to track installation completion
                            app.pending_install_names = Some(installed_names);

                            // Trigger refresh of installed packages
                            app.refresh_installed_until =
                                Some(std::time::Instant::now() + std::time::Duration::from_secs(8));

                            tracing::info!(
                                "Install operation completed: triggered refresh of installed packages"
                            );
                        }
                        crate::state::PreflightAction::Remove => {
                            let removed_names: Vec<String> =
                                items.iter().map(|p| p.name.clone()).collect();

                            // Clear remove list
                            app.remove_list.clear();
                            app.remove_list_names.clear();
                            app.remove_state.select(None);

                            // Set pending remove names to track removal completion
                            app.pending_remove_names = Some(removed_names);

                            // Trigger refresh of installed packages
                            app.refresh_installed_until =
                                Some(std::time::Instant::now() + std::time::Duration::from_secs(8));

                            // Keep PreflightExec modal open so user can see completion message
                            // User can close it with Esc/q, and refresh happens in background
                            tracing::info!(
                                "Remove operation completed: cleared remove list and triggered refresh"
                            );
                        }
                        crate::state::PreflightAction::Downgrade => {
                            let downgraded_names: Vec<String> =
                                items.iter().map(|p| p.name.clone()).collect();

                            // Clear downgrade list
                            app.downgrade_list.clear();
                            app.downgrade_list_names.clear();
                            app.downgrade_state.select(None);

                            // Set pending downgrade names to track downgrade completion
                            app.pending_remove_names = Some(downgraded_names);

                            // Trigger refresh of installed packages
                            app.refresh_installed_until =
                                Some(std::time::Instant::now() + std::time::Duration::from_secs(8));

                            // Keep PreflightExec modal open so user can see completion message
                            // User can close it with Esc/q, and refresh happens in background
                            tracing::info!(
                                "Downgrade operation completed: cleared downgrade list and triggered refresh"
                            );
                        }
                    }
                } else {
                    log_lines.push(format!("Execution failed (exit code: {exit_code:?})"));
                }
            }
            crate::install::ExecutorOutput::Error(err) => {
                *abortable = false;
                log_lines.push(format!("Error: {err}"));
            }
        }
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
pub async fn run_event_loop(
    terminal: &mut Option<Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>>,
    app: &mut AppState,
    channels: &mut Channels,
) {
    loop {
        if let Some(t) = terminal.as_mut() {
            let _ = t.draw(|f| ui(f, app));
        }

        if process_channel_messages(app, channels).await {
            break;
        }
    }
}
