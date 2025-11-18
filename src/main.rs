//! Pacsea binary entrypoint kept minimal. The full runtime lives in `app`.

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

/// Pacsea - A fast, friendly TUI for browsing and installing Arch and AUR packages
#[derive(Parser, Debug)]
#[command(name = "pacsea")]
#[command(version)]
#[command(about = "A fast, friendly TUI for browsing and installing Arch and AUR packages", long_about = None)]
struct Args {
    /// Perform a dry run without making actual changes
    #[arg(long)]
    dry_run: bool,

    /// Set the logging level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Enable verbose output (equivalent to --log-level debug)
    #[arg(short, long)]
    verbose: bool,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,

    /// [Not yet implemented] Specify the configuration directory (default: ~/.config/pacsea)
    #[arg(long)]
    config_dir: Option<String>,

    /// [Not yet implemented] Search for packages from command line (opens TUI with search results)
    #[arg(short, long)]
    search: Option<String>,

    /// [Not yet implemented] Install packages from command line (comma-separated or space-separated)
    #[arg(short, long, num_args = 1..)]
    install: Vec<String>,

    /// [Not yet implemented] Install packages from file (e.g., pacsea -I FILENAME.txt)
    #[arg(short = 'I')]
    install_from_file: Option<String>,

    /// [Not yet implemented] Remove packages from command line (e.g., pacsea -r PACKAGE1 PACKAGE2 or pacsea --remove PACKAGE)
    #[arg(short = 'r', long, num_args = 1..)]
    remove: Vec<String>,

    /// [Not yet implemented] Remove packages from file (e.g., pacsea -R FILENAME.txt)
    #[arg(short = 'R')]
    remove_from_file: Option<String>,

    /// [Not yet implemented] System update (sync + update, e.g., pacsea --update)
    #[arg(short = 'u', long)]
    update: bool,

    /// [Not yet implemented] Show news dialog on startup
    #[arg(short = 'n', long)]
    news: bool,

    /// [Not yet implemented] Update package database before starting
    #[arg(short = 'y', long)]
    refresh: bool,

    /// Clear all cache files (dependencies, files, services, sandbox) and exit
    #[arg(long)]
    clear_cache: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Determine log level (verbose flag overrides log_level)
    // PACSEA_PREFLIGHT_TRACE=1 enables TRACE level for detailed preflight timing
    let log_level = if args.verbose {
        "debug"
    } else if std::env::var("PACSEA_PREFLIGHT_TRACE").ok().as_deref() == Some("1") {
        "trace"
    } else {
        &args.log_level
    };

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
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));
                // File logger: always disable ANSI codes for clean log files
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .with_ansi(false) // Always disable ANSI for file output
                    .with_writer(non_blocking)
                    .with_timer(PacseaTimer)
                    .init();
                let _ = LOG_GUARD.set(guard);
                tracing::info!(path = %log_path.display(), "logging initialized");
            }
            Err(e) => {
                // Fallback: init stderr logger to avoid blocking startup
                let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));
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

    // Handle command-line install mode
    if !args.install.is_empty() {
        tracing::info!(packages = ?args.install, "Install mode requested from CLI");
        // TODO: Implement CLI install mode (mentioned in roadmap)
        tracing::warn!("CLI install mode not yet implemented, falling back to TUI");
    }

    // Handle install from file (-I)
    if let Some(file_path) = &args.install_from_file {
        tracing::info!(file = %file_path, "Install from file requested from CLI");
        // TODO: Implement install from file (mentioned in roadmap)
        tracing::warn!("Install from file not yet implemented, falling back to TUI");
    }

    // Handle remove packages from command line (-r / --remove)
    if !args.remove.is_empty() {
        tracing::info!(packages = ?args.remove, "Remove packages requested from CLI");
        // TODO: Implement remove packages (mentioned in roadmap)
        tracing::warn!("Remove packages not yet implemented, falling back to TUI");
    }

    // Handle remove packages from file (-R)
    if let Some(file_path) = &args.remove_from_file {
        tracing::info!(file = %file_path, "Remove from file requested from CLI");
        // TODO: Implement remove from file (mentioned in roadmap)
        tracing::warn!("Remove from file not yet implemented, falling back to TUI");
    }

    // Handle system update (--update / -u)
    if args.update {
        tracing::info!("System update requested from CLI");
        // TODO: Implement system update (mentioned in roadmap)
        tracing::warn!("System update not yet implemented");
    }

    // Handle command-line search mode
    if let Some(search_query) = &args.search {
        tracing::info!(query = %search_query, "Search mode requested from CLI");
        // TODO: Implement CLI search mode with initial query
        tracing::warn!("CLI search mode not yet implemented, falling back to TUI");
    }

    // Handle refresh flag
    if args.refresh {
        tracing::info!("Refresh mode requested from CLI");
        // TODO: Implement package database refresh
        tracing::warn!("Refresh mode not yet implemented");
    }

    // Handle news flag
    if args.news {
        tracing::info!("News dialog requested from CLI");
        // TODO: Implement showing news dialog on startup
        tracing::warn!("News flag not yet implemented");
    }

    // Handle clear cache flag
    if args.clear_cache {
        tracing::info!("Clear cache requested from CLI");
        let lists_dir = theme::lists_dir();
        let cache_files = [
            "install_deps_cache.json",
            "file_cache.json",
            "services_cache.json",
            "sandbox_cache.json",
            "details_cache.json",
        ];

        let mut cleared_count = 0;
        for cache_file in &cache_files {
            let cache_path = lists_dir.join(cache_file);
            match std::fs::remove_file(&cache_path) {
                Ok(_) => {
                    tracing::info!(path = %cache_path.display(), "cleared cache file");
                    cleared_count += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    tracing::debug!(path = %cache_path.display(), "cache file does not exist, skipping");
                }
                Err(e) => {
                    tracing::warn!(path = %cache_path.display(), error = %e, "failed to clear cache file");
                }
            }
        }

        if cleared_count > 0 {
            tracing::info!(cleared_count = cleared_count, "cleared cache files");
            println!("Cleared {} cache file(s)", cleared_count);
        } else {
            tracing::info!("No cache files found to clear");
            println!("No cache files found to clear");
        }
        return;
    }

    tracing::info!(dry_run = args.dry_run, "Pacsea starting");
    if let Err(err) = app::run(args.dry_run).await {
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

