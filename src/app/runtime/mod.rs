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
/// Background worker implementations shared with the crate-level production Pi adapter.
pub mod workers;

use background::{Channels, spawn_auxiliary_workers, spawn_event_thread};
use cleanup::cleanup_on_exit;
use event_loop::run_event_loop;
use init::{initialize_app_state, run_startup_config_preflight, trigger_initial_resolutions};

/// Result type alias for runtime operations.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Best-effort terminal restore guard for non-headless runtime sessions.
///
/// Inputs:
/// - `active`: Whether terminal restore should run on drop.
///
/// Output:
/// - On drop, attempts to restore terminal modes when active.
///
/// Details:
/// - Prevents leaked raw mode / mouse capture when `run()` returns early with `?`.
/// - Uses best-effort restore and logs failures without changing the original return path.
struct TerminalRestoreGuard {
    /// Whether drop should attempt terminal restoration.
    active: bool,
}

impl TerminalRestoreGuard {
    /// What: Create a new terminal restore guard.
    ///
    /// Inputs:
    /// - `active`: Enables or disables restore-on-drop behavior.
    ///
    /// Output:
    /// - New `TerminalRestoreGuard`.
    ///
    /// Details:
    /// - `active` should be `true` only after successful terminal setup in non-headless mode.
    const fn new(active: bool) -> Self {
        Self { active }
    }

    /// What: Disable restore-on-drop after explicit terminal restoration.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Guard no longer restores terminal in `Drop`.
    ///
    /// Details:
    /// - Prevents duplicate restoration attempts on normal exit path.
    const fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for TerminalRestoreGuard {
    /// What: Restore terminal modes when the guard goes out of scope.
    ///
    /// Inputs:
    /// - `self`: Guard state.
    ///
    /// Output:
    /// - No return value; logs on restore failure.
    ///
    /// Details:
    /// - This runs during unwinding and early-return paths.
    fn drop(&mut self) {
        if self.active
            && let Err(err) = restore_terminal()
        {
            tracing::warn!(error = %err, "failed to restore terminal from drop guard");
        }
    }
}

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

    // Migrate legacy configs, fill missing keys, and return the resolved settings snapshot.
    let prefs = run_startup_config_preflight();

    // Force theme resolution BEFORE terminal setup.
    // This is important because theme resolution may query terminal colors via OSC 10/11,
    // which must happen before mouse capture is enabled to avoid input conflicts.
    let _ = crate::theme::theme();

    if !headless {
        setup_terminal()?;
    }
    let mut terminal_restore_guard = TerminalRestoreGuard::new(!headless);
    let mut terminal = if headless {
        None
    } else {
        Some(Terminal::new(CrosstermBackend::new(std::io::stdout()))?)
    };

    let mut app = AppState::default();

    // Initialize application state (loads settings, caches, etc.)
    let init_flags = initialize_app_state(&mut app, dry_run_flag, headless, &prefs);

    // Create channels and spawn background workers. The optional scanner is enabled only
    // after settings validation; failures degrade that path without taking down Pacsea.
    let pi_scan_options = pi_scan_runtime_options(&prefs, app.dry_run);
    let mut channels =
        Channels::new_with_pi_scan(app.official_index_path.clone(), pi_scan_options)?;
    if channels.pi_scan_runtime_enabled {
        app.pi_scan.availability = crate::state::PiScanAvailability::RuntimeConnected;
    } else if prefs.pi_scan.enabled
        && !matches!(
            app.pi_scan.availability,
            crate::state::PiScanAvailability::MissingBinary
                | crate::state::PiScanAvailability::Unsupported
        )
    {
        app.pi_scan.availability = crate::state::PiScanAvailability::RuntimeDisconnected;
    }

    // Get updates refresh interval from settings (minimum 60s per requirement)
    let updates_refresh_interval = crate::theme::settings().updates_refresh_interval.max(60);

    // Spawn auxiliary workers (status, news, tick, index updates)
    spawn_auxiliary_workers(
        headless,
        channels.toolkit.clone(),
        &channels.status_tx,
        &channels.news_tx,
        &channels.news_feed_tx,
        &channels.news_incremental_tx,
        &channels.announcement_tx,
        &channels.tick_tx,
        &app.news_read_ids,
        &app.news_read_urls,
        &app.news_seen_pkg_versions,
        &app.news_seen_aur_comments,
        &app.official_index_path,
        &channels.net_err_tx,
        &channels.index_notify_tx,
        &channels.updates_tx,
        updates_refresh_interval,
        app.installed_packages_mode,
        crate::theme::settings().get_announcement,
        app.last_startup_timestamp.as_deref(),
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
    cleanup_on_exit(&mut app, &channels).await;

    // Drop channels to close request channels and stop workers from accepting new work
    drop(channels);

    // Restore terminal so user sees prompt
    if !headless {
        restore_terminal()?;
        terminal_restore_guard.disarm();
    }

    // Force immediate process exit to avoid waiting for background blocking tasks
    // This is necessary because spawn_blocking tasks cannot be cancelled and would
    // otherwise keep the tokio runtime alive until they complete
    std::process::exit(0);
}

/// What: Build validated private runtime options for the optional Pi scanner.
///
/// Inputs:
/// - `prefs`: Resolved settings snapshot.
/// - `dry_run`: Effective session dry-run flag.
///
/// Output:
/// - Default-off options when scanner settings are invalid; otherwise the explicit gate
///   and private config-relative state/quarantine paths.
///
/// Details:
/// - Validation never clamps raised security limits. The runtime worker separately applies
///   the Linux platform gate and refuses corrupt/newer durable state.
fn pi_scan_runtime_options(
    prefs: &crate::theme::Settings,
    dry_run: bool,
) -> crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions {
    pi_scan_runtime_options_for_settings(&prefs.pi_scan, dry_run)
}

/// What: Build runtime options from one effective Pi Scan settings snapshot.
///
/// Inputs:
/// - `settings`: Effective scanner settings.
/// - `dry_run`: Effective session dry-run flag.
///
/// Output:
/// - Validated runtime options suitable for initial spawn or rollback restoration.
///
/// Details:
/// - Central setup integration uses this after an in-process Apply and when restoring
///   the previous owner after activation failure.
fn pi_scan_runtime_options_for_settings(
    settings: &crate::theme::PiScanSettings,
    dry_run: bool,
) -> crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions {
    let root = crate::theme::config_dir().join("pi_scan");
    let settings_valid = settings.validation_issues().is_empty();
    let enabled = settings.enabled && settings_valid;
    let production = settings_valid.then(|| {
        let mut models = Vec::new();
        if !settings.provider.trim().is_empty() && !settings.model.trim().is_empty() {
            models.push(crate::pi_agent::session::ModelChoice {
                provider: settings.provider.trim().to_string(),
                model: settings.model.trim().to_string(),
            });
        }
        for fallback in settings
            .fallback_models
            .split(',')
            .map(str::trim)
            .filter(|model| !model.is_empty())
        {
            let (provider, model) = fallback.split_once('/').map_or_else(
                || (settings.provider.trim(), fallback),
                |(provider, model)| (provider, model),
            );
            models.push(crate::pi_agent::session::ModelChoice {
                provider: provider.to_string(),
                model: model.to_string(),
            });
        }
        crate::pi_scan_production::ProductionRuntimeSettings {
            binary: settings.binary.clone(),
            models,
            background_execution: settings.background_enabled,
            thinking: settings.thinking.clone(),
            observation_interval_seconds: settings.observation_interval_seconds,
            model_attempt_timeout: std::time::Duration::from_secs(
                settings.model_attempt_timeout_seconds,
            ),
            logical_timeout: std::time::Duration::from_secs(settings.logical_timeout_seconds),
            head_query_timeout: std::time::Duration::from_secs(settings.head_query_timeout_seconds),
            observation_deadline: std::time::Duration::from_secs(
                settings.observation_deadline_seconds,
            ),
            result_retention_days: settings.result_retention_days,
            reservation: crate::state::pi_scan::PiScanReservation {
                tokens: crate::pi_agent::setup_probe::SETUP_PROBE_RESERVATION_TOKENS,
                cost_microusd: u64::MAX,
            },
            budget_limits: crate::state::pi_scan::PiScanBudgetLimits {
                starts_per_hour: settings.background_starts_per_hour,
                tokens_per_24h: settings.background_token_cap_24h,
                cost_microusd_per_24h: pi_scan_cost_cap_microusd(&settings.background_cost_cap_24h)
                    .unwrap_or(0),
            },
            https_proxy: settings.https_proxy.clone(),
        }
    });
    crate::app::runtime::workers::pi_scan::PiScanRuntimeOptions {
        enabled,
        dry_run,
        state_path: root.join("backlog-v1.json"),
        quarantine_dir: root.join("quarantine"),
        production,
    }
}

/// Convert a validated decimal dollar cap to integer micro-USD without floating-point drift.
fn pi_scan_cost_cap_microusd(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    let (whole, fraction) = trimmed.split_once('.').map_or((trimmed, ""), |parts| parts);
    if whole.is_empty()
        || !whole.chars().all(|character| character.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let dollars = whole.parse::<u64>().ok()?;
    let micros = format!("{fraction:0<6}").parse::<u64>().ok()?;
    dollars.checked_mul(1_000_000)?.checked_add(micros)
}
