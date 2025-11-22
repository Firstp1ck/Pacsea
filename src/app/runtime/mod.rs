use ratatui::{Terminal, backend::CrosstermBackend};

use crate::logic::send_query;
use crate::state::AppState;

use super::terminal::{restore_terminal, setup_terminal};

mod background;
mod channels;
mod cleanup;
mod event_loop;
mod handlers;
mod init;
mod tick_handler;
mod workers;

use background::{Channels, spawn_auxiliary_workers, spawn_event_thread};
use cleanup::cleanup_on_exit;
use event_loop::run_event_loop;
use init::{initialize_app_state, trigger_initial_resolutions};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Run the Pacsea TUI application end-to-end: initialize terminal and state, spawn
/// background workers (index, search, details, status/news), drive the event loop, persist
/// caches, and restore the terminal on exit.
///
/// Inputs:
/// - `dry_run_flag`: When `true`, install/remove/downgrade actions are displayed but not executed
///   (overrides the config default for the session).
/// - `refresh_result`: `Some(true)` if package database refresh succeeded, `Some(false)` if it failed,
///   `None` if refresh was not run. Used to display a popup notification when starting the TUI.
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
/// - If refresh was run, displays an Alert modal with the success/failure status.
pub async fn run(dry_run_flag: bool, refresh_result: Option<bool>) -> Result<()> {
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

    // Show refresh result popup if refresh was run
    if let Some(success) = refresh_result {
        use crate::i18n;
        let message = if success {
            i18n::t(&app, "app.modals.refresh.success")
        } else {
            i18n::t(&app, "app.modals.refresh.failure")
        };
        app.modal = crate::state::Modal::Alert { message };
    }

    // Create channels and spawn background workers
    let mut channels = Channels::new(app.official_index_path.clone());

    // Spawn auxiliary workers (status, news, tick, index updates)
    spawn_auxiliary_workers(
        headless,
        &channels.status_tx,
        &channels.news_tx,
        &channels.tick_tx,
        &app.news_read_urls,
        &app.official_index_path,
        &channels.net_err_tx,
        &channels.index_notify_tx,
        &channels.updates_tx,
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
    run_event_loop(&mut terminal, &mut app, &mut channels).await;

    // Cleanup on exit
    cleanup_on_exit(&mut app, &channels);

    if !headless {
        restore_terminal()?;
    }
    Ok(())
}
