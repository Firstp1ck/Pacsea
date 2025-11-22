use ratatui::Terminal;
use tokio::select;

use crate::state::{AppState, Modal, PackageItem};
use crate::ui::ui;

use super::background::Channels;
use super::handlers::*;
use super::tick_handler::{
    handle_news, handle_pkgbuild_result, handle_status, handle_summary_result, handle_tick,
};

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
    channels: &mut Channels,
    files: Vec<crate::state::modal::PackageFileInfo>,
) {
    tracing::debug!(
        "[Runtime] Received file result: {} entries for packages: {:?}",
        files.len(),
        files.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    for file_info in &files {
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
#[allow(clippy::too_many_lines)]
async fn process_channel_messages(app: &mut AppState, channels: &mut Channels) -> bool {
    select! {
        Some(ev) = channels.event_rx.recv() => {
            crate::events::handle_event(
                ev,
                app,
                &channels.query_tx,
                &channels.details_req_tx,
                &channels.preview_tx,
                &channels.add_tx,
                &channels.pkgb_req_tx,
            )
        }
        Some(()) = channels.index_notify_rx.recv() => {
            app.loading_index = false;
            let _ = channels.tick_tx.send(());
            false
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
            handle_dependency_result(app, deps, &channels.tick_tx);
            false
        }
        Some(files) = channels.files_res_rx.recv() => {
            handle_file_result_with_logging(app, channels, files);
            false
        }
        Some(services) = channels.services_res_rx.recv() => {
            tracing::debug!(
                "[Runtime] Received service result: {} entries",
                services.len()
            );
            handle_service_result(app, services, &channels.tick_tx);
            false
        }
        Some(sandbox_info) = channels.sandbox_res_rx.recv() => {
            tracing::debug!(
                "[Runtime] Received sandbox result: {} entries for packages: {:?}",
                sandbox_info.len(),
                sandbox_info.iter().map(|s| &s.package_name).collect::<Vec<_>>()
            );
            handle_sandbox_result(app, sandbox_info, &channels.tick_tx);
            false
        }
        Some((pkgname, text)) = channels.pkgb_res_rx.recv() => {
            handle_pkgbuild_result(app, pkgname, text, &channels.tick_tx);
            false
        }
        Some(summary_outcome) = channels.summary_res_rx.recv() => {
            handle_summary_result(app, summary_outcome, &channels.tick_tx);
            false
        }
        Some(msg) = channels.net_err_rx.recv() => {
            app.modal = Modal::Alert { message: msg };
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
            );
            false
        }
        Some(todays) = channels.news_rx.recv() => {
            handle_news(app, &todays);
            false
        }
        Some((txt, color)) = channels.status_rx.recv() => {
            handle_status(app, txt, color);
            false
        }
        Some((count, list)) = channels.updates_rx.recv() => {
            app.updates_count = Some(count);
            app.updates_list = list;
            app.updates_loading = false;
            false
        }
        else => false
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
