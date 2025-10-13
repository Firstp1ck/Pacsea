//! Pacsea binary entrypoint kept minimal. The full runtime lives in `app`.

mod app;
mod events;
mod index;
mod install;
mod logic;
mod sources;
mod state;
mod theme;
mod ui;
mod util;

use std::sync::OnceLock;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

#[tokio::main]
async fn main() {
    // Initialize tracing logger writing to ~/.config/pacsea/pacsea.log
    {
        let mut log_path = crate::theme::config_dir();
        log_path.push("pacsea.log");
        // Ensure directory exists (theme::config_dir already ensures it)
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(non_blocking)
                    .init();
                let _ = LOG_GUARD.set(guard);
                tracing::info!(path = %log_path.display(), "logging initialized");
            }
            Err(e) => {
                // Fallback: init stderr logger to avoid blocking startup
                let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .with_ansi(true)
                    .init();
                tracing::warn!(error = %e, "failed to open log file; using stderr");
            }
        }
    }

    let dry_run_flag = std::env::args().any(|a| a == "--dry-run");
    tracing::info!(dry_run = dry_run_flag, "Pacsea starting");
    if let Err(err) = app::run(dry_run_flag).await {
        tracing::error!(error = ?err, "Application error");
    }
    tracing::info!("Pacsea exited");
}
