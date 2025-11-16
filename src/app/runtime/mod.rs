use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::select;

use crate::logic::send_query;
use crate::state::*;
use crate::ui::ui;

use super::persist::{
    maybe_flush_cache, maybe_flush_deps_cache, maybe_flush_files_cache, maybe_flush_install,
    maybe_flush_news_read, maybe_flush_recent, maybe_flush_sandbox_cache,
    maybe_flush_services_cache,
};
use super::terminal::{restore_terminal, setup_terminal};

mod background;
mod handlers;
mod init;
mod tick_handler;

use background::{Channels, spawn_auxiliary_workers, spawn_event_thread};
use handlers::*;
use init::{initialize_app_state, trigger_initial_resolutions};
use tick_handler::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Run the Pacsea TUI application end-to-end: initialize terminal and state, spawn
/// background workers (index, search, details, status/news), drive the event loop, persist
/// caches, and restore the terminal on exit.
///
/// Inputs:
/// - `dry_run_flag`: When `true`, install/remove/downgrade actions are displayed but not executed
///   (overrides the config default for the session).
///
/// Output:
/// - `Ok(())` when the UI exits cleanly; `Err` on unrecoverable terminal or runtime errors.
///
/// Details:
/// - Config/state: Migrates legacy configs, loads settings (layout, keymap, sort), and reads
///   persisted files (details cache, recent queries, install list, on-disk official index).
/// - Background tasks: Spawns channels and tasks for batched details fetch, AUR/official search,
///   PKGBUILD retrieval, official index refresh/enrichment, Arch status text, and Arch news.
/// - Event loop: Renders UI frames and handles keyboard, mouse, tick, and channel messages to
///   update results, details, ring-prefetch, PKGBUILD viewer, installed-only mode, and modals.
/// - Persistence: Debounces and periodically writes recent, details cache, and install list.
/// - Cleanup: Flushes pending writes and restores terminal modes before returning.
pub async fn run(dry_run_flag: bool) -> Result<()> {
    let headless = std::env::var("PACSEA_TEST_HEADLESS").ok().as_deref() == Some("1");
    if !headless {
        setup_terminal()?;
    }
    let mut terminal = if headless {
        None
    } else {
        Some(Terminal::new(CrosstermBackend::new(std::io::stdout()))?)
    };

    let mut app = AppState::default();

    // Initialize application state (loads settings, caches, etc.)
    let init_flags = initialize_app_state(&mut app, dry_run_flag, headless);

    // Create channels and spawn background workers
    let mut channels = Channels::new(app.official_index_path.clone());

    // Spawn auxiliary workers (status, news, tick, index updates)
    spawn_auxiliary_workers(
        headless,
        channels.status_tx.clone(),
        channels.news_tx.clone(),
        channels.tick_tx.clone(),
        app.news_read_urls.clone(),
        app.official_index_path.clone(),
        channels.net_err_tx.clone(),
        channels.index_notify_tx.clone(),
    );

    // Spawn event reading thread
    spawn_event_thread(
        headless,
        channels.event_tx.clone(),
        channels.event_thread_cancelled.clone(),
    );

    // Trigger initial background resolutions if caches were missing/invalid
    trigger_initial_resolutions(
        &mut app,
        &init_flags,
        &channels.deps_req_tx,
        &channels.files_req_tx,
        &channels.services_req_tx,
        &channels.sandbox_req_tx,
    );

    // Send initial query
    send_query(&mut app, &channels.query_tx);

    // Main event loop
    loop {
        if let Some(t) = terminal.as_mut() {
            let _ = t.draw(|f| ui(f, &mut app));
        }

        select! {
            Some(ev) = channels.event_rx.recv() => {
                if crate::events::handle_event(
                    ev,
                    &mut app,
                    &channels.query_tx,
                    &channels.details_req_tx,
                    &channels.preview_tx,
                    &channels.add_tx,
                    &channels.pkgb_req_tx,
                ) {
                    break;
                }
            }
            Some(_) = channels.index_notify_rx.recv() => {
                app.loading_index = false;
                let _ = channels.tick_tx.send(());
            }
            Some(new_results) = channels.results_rx.recv() => {
                handle_search_results(
                    &mut app,
                    new_results,
                    &channels.details_req_tx,
                    &channels.index_notify_tx,
                );
            }
            Some(details) = channels.details_res_rx.recv() => {
                handle_details_update(&mut app, details, &channels.tick_tx);
            }
            Some(item) = channels.preview_rx.recv() => {
                handle_preview(&mut app, item, &channels.details_req_tx);
            }
            Some(first) = channels.add_rx.recv() => {
                // Batch-drain imported items arriving close together to avoid
                // repeated redraws and disk writes. Limit batch window to ~50ms.
                let mut batch = vec![first];
                loop {
                    match channels.add_rx.try_recv() {
                        Ok(it) => batch.push(it),
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                    }
                }
                for it in batch.into_iter() {
                    handle_add_to_install_list(
                        &mut app,
                        it,
                        &channels.deps_req_tx,
                        &channels.files_req_tx,
                        &channels.services_req_tx,
                        &channels.sandbox_req_tx,
                    );
                }
            }
            Some(deps) = channels.deps_res_rx.recv() => {
                handle_dependency_result(&mut app, deps, &channels.tick_tx);
            }
            Some(files) = channels.files_res_rx.recv() => {
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
                handle_file_result(&mut app, files, &channels.tick_tx);
            }
            Some(services) = channels.services_res_rx.recv() => {
                handle_service_result(&mut app, services, &channels.tick_tx);
            }
            Some(sandbox_info) = channels.sandbox_res_rx.recv() => {
                tracing::debug!(
                    "[Runtime] Received sandbox result: {} entries for packages: {:?}",
                    sandbox_info.len(),
                    sandbox_info.iter().map(|s| &s.package_name).collect::<Vec<_>>()
                );
                handle_sandbox_result(&mut app, sandbox_info, &channels.tick_tx);
            }
            Some((pkgname, text)) = channels.pkgb_res_rx.recv() => {
                handle_pkgbuild_result(&mut app, pkgname, text, &channels.tick_tx);
            }
            Some(summary_outcome) = channels.summary_res_rx.recv() => {
                handle_summary_result(&mut app, summary_outcome, &channels.tick_tx);
            }
            Some(msg) = channels.net_err_rx.recv() => {
                app.modal = Modal::Alert { message: msg };
            }
            Some(_) = channels.tick_rx.recv() => {
                handle_tick(
                    &mut app,
                    &channels.query_tx,
                    &channels.details_req_tx,
                    &channels.pkgb_req_tx,
                    &channels.deps_req_tx,
                    &channels.files_req_tx,
                    &channels.services_req_tx,
                    &channels.sandbox_req_tx,
                    &channels.summary_req_tx,
                );
            }
            Some(todays) = channels.news_rx.recv() => {
                handle_news(&mut app, todays);
            }
            Some((txt, color)) = channels.status_rx.recv() => {
                handle_status(&mut app, txt, color);
            }
            else => {}
        }
    }

    // Reset resolution flags on exit to ensure clean shutdown
    // This prevents background tasks from blocking if they're still running
    tracing::debug!("[Runtime] Main loop exited, resetting resolution flags");
    app.deps_resolving = false;
    app.files_resolving = false;
    app.services_resolving = false;
    app.sandbox_resolving = false;

    // Signal event reading thread to exit immediately
    channels
        .event_thread_cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);

    maybe_flush_cache(&mut app);
    maybe_flush_recent(&mut app);
    maybe_flush_news_read(&mut app);
    maybe_flush_install(&mut app);
    maybe_flush_deps_cache(&mut app);
    maybe_flush_files_cache(&mut app);
    maybe_flush_services_cache(&mut app);
    maybe_flush_sandbox_cache(&mut app);

    if !headless {
        restore_terminal()?;
    }
    Ok(())
}
