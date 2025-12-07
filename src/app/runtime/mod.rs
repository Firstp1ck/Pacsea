use ratatui::{Terminal, backend::CrosstermBackend};

use crate::logic::send_query;
use crate::state::AppState;

use super::terminal::{restore_terminal, setup_terminal};

/// Background worker management and spawning.
mod background;
/// Channel definitions for runtime communication.
mod channels;
/// Cleanup operations on application exit.
mod cleanup;
/// Main event loop implementation.
mod event_loop;
/// Event handlers for different event types.
mod handlers;
/// Application state initialization module.
pub mod init;
/// Tick handler for periodic UI updates.
mod tick_handler;
/// Background worker implementations.
mod workers;

use background::{Channels, spawn_auxiliary_workers, spawn_event_thread};
use cleanup::cleanup_on_exit;
use event_loop::run_event_loop;
use init::{initialize_app_state, trigger_initial_resolutions};

/// Result type alias for runtime operations.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Run the Pacsea TUI application end-to-end.
///
/// This function initializes terminal and state, spawns background workers
/// (index, search, details, status/news), drives the event loop, persists
/// caches, and restores the terminal on exit.
///
/// Inputs:
/// - `dry_run_flag`: When `true`, install/remove/downgrade actions are displayed but not executed
///   (overrides the config default for the session).
///
/// Output:
/// - `Ok(())` when the UI exits cleanly; `Err` on unrecoverable terminal or runtime errors.
///
/// # Errors
/// - Returns `Err` when terminal setup fails (e.g., unable to initialize terminal backend)
/// - Returns `Err` when terminal restoration fails on exit
/// - Returns `Err` when critical runtime errors occur during initialization or event loop execution
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

    // Get updates refresh interval from settings
    let updates_refresh_interval = crate::theme::settings().updates_refresh_interval;

    // Spawn auxiliary workers (status, news, tick, index updates)
    spawn_auxiliary_workers(
        headless,
        &channels.status_tx,
        &channels.news_tx,
        &channels.news_feed_tx,
        &channels.announcement_tx,
        &channels.tick_tx,
        &app.news_read_urls,
        &app.official_index_path,
        &channels.net_err_tx,
        &channels.index_notify_tx,
        &channels.updates_tx,
        updates_refresh_interval,
        app.installed_packages_mode,
        crate::theme::settings().get_announcement,
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

    // Cleanup on exit - this resets flags and flushes caches
    cleanup_on_exit(&mut app, &channels);

    // Drop channels to close request channels and stop workers from accepting new work
    drop(channels);

    // Restore terminal so user sees prompt
    if !headless {
        restore_terminal()?;
    }

    // Force immediate process exit to avoid waiting for background blocking tasks
    // This is necessary because spawn_blocking tasks cannot be cancelled and would
    // otherwise keep the tokio runtime alive until they complete
    std::process::exit(0);
}
