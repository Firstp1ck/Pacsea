//! Pacsea binary entrypoint kept minimal. The full runtime lives in `app`.

mod args;

use clap::Parser;
use pacsea::{app, theme, util};
use std::sync::OnceLock;
use std::{fmt, time::SystemTime};

struct PacseaTimer;

impl tracing_subscriber::fmt::time::FormatTime for PacseaTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> fmt::Result {
        let secs = match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(_) => 0,
        };
        let s = util::ts_to_date(Some(secs)); // "YYYY-MM-DD HH:MM:SS"
        let ts = s.replacen(' ', "-T", 1); // "YYYY-MM-DD-T HH:MM:SS"
        w.write_str(&ts)
    }
}

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

#[tokio::main]
async fn main() {
    let args = args::Args::parse();

    // Determine log level (verbose flag overrides log_level)
    // PACSEA_PREFLIGHT_TRACE=1 enables TRACE level for detailed preflight timing
    let log_level = args::determine_log_level(&args);

    // Initialize tracing logger writing to ~/.config/pacsea/logs/pacsea.log
    {
        let mut log_path = theme::logs_dir();
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
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level));
                // File logger: always disable ANSI codes for clean log files
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .with_ansi(false) // Always disable ANSI for file output
                    .with_timer(PacseaTimer)
                    .with_writer(non_blocking)
                    .init();
                let _ = LOG_GUARD.set(guard);
                tracing::info!(path = %log_path.display(), "logging initialized");
            }
            Err(e) => {
                // Fallback: init stderr logger to avoid blocking startup
                let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level));
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .with_ansi(!args.no_color)
                    .with_timer(PacseaTimer)
                    .init();
                tracing::warn!(error = %e, "failed to open log file; using stderr");
            }
        }
    }

    // Process command-line arguments (may exit early for search/clear-cache)
    let refresh_result = args::process_args(&args);

    tracing::info!(dry_run = args.dry_run, "Pacsea starting");
    if let Err(err) = app::run(args.dry_run, refresh_result).await {
        tracing::error!(error = ?err, "Application error");
    }
    tracing::info!("Pacsea exited");
}

#[cfg(test)]
mod tests {
    /// What: Validate that the `PacseaTimer` formatter writes a timestamp without panicking.
    ///
    /// Inputs:
    /// - `writer`: In-memory tracing writer capturing formatted output
    ///
    /// Output:
    /// - Buffer contains at least one character after formatting succeeds.
    ///
    /// Details:
    /// - Calls [`PacseaTimer::format_time`] directly on an empty buffer to ensure it neither panics
    ///   nor leaves the buffer untouched.
    #[test]
    fn pacsea_timer_formats_time_without_panic() {
        use tracing_subscriber::fmt::time::FormatTime;
        // Smoke test FormatTime impl doesn't panic
        let mut buf = String::new();
        let mut writer = tracing_subscriber::fmt::format::Writer::new(&mut buf);
        let t = super::PacseaTimer;
        let _ = t.format_time(&mut writer);
        // Ensure something was written
        assert!(!buf.is_empty());
    }
}
