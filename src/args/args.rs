//! Command-line argument parsing and handling.

use clap::Parser;

/// Pacsea - A fast, friendly TUI for browsing and installing Arch and AUR packages
#[derive(Parser, Debug)]
#[command(name = "pacsea")]
#[command(version)]
#[command(about = "A fast, friendly TUI for browsing and installing Arch and AUR packages", long_about = None)]
pub struct Args {
    /// Perform a dry run without making actual changes
    #[arg(long)]
    pub dry_run: bool,

    /// Set the logging level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Enable verbose output (equivalent to --log-level debug)
    #[arg(short, long)]
    pub verbose: bool,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// [Not yet implemented] Specify the configuration directory (default: ~/.config/pacsea)
    #[arg(long)]
    pub config_dir: Option<String>,

    /// Search for packages from command line
    #[arg(short, long)]
    pub search: Option<String>,

    /// Install packages from command line (comma-separated or space-separated)
    #[arg(short, long, num_args = 1..)]
    pub install: Vec<String>,

    /// Install packages from file (e.g., pacsea -I FILENAME.txt)
    #[arg(short = 'I')]
    pub install_from_file: Option<String>,

    /// Remove packages from command line (e.g., pacsea -r PACKAGE1 PACKAGE2 or pacsea --remove PACKAGE)
    #[arg(short = 'r', long, num_args = 1..)]
    pub remove: Vec<String>,

    /// [Not yet implemented] Remove packages from file (e.g., pacsea -R FILENAME.txt)
    #[arg(short = 'R')]
    pub remove_from_file: Option<String>,

    /// System update (sync + update, e.g., pacsea --update)
    #[arg(short = 'u', long)]
    pub update: bool,

    /// Output news dialog to commandline with link to website at the end
    #[arg(short = 'n', long)]
    pub news: bool,

    /// List unread news (use with --news)
    #[arg(long)]
    pub unread: bool,

    /// List read news (use with --news)
    #[arg(long)]
    pub read: bool,

    /// List all news (read and unread) (use with --news)
    #[arg(long, short = 'a')]
    pub all_news: bool,

    /// [Not yet implemented] Update package database before starting
    #[arg(short = 'y', long)]
    pub refresh: bool,

    /// Clear all cache files (dependencies, files, services, sandbox) and exit
    #[arg(long)]
    pub clear_cache: bool,

    /// List installed packages (use with --exp, --imp, or --all)
    #[arg(short = 'l', long)]
    pub list: bool,

    /// List explicitly installed packages (use with --list)
    #[arg(long)]
    pub exp: bool,

    /// List implicitly installed packages (use with --list)
    #[arg(long)]
    pub imp: bool,

    /// List all installed packages (use with --list)
    #[arg(long)]
    pub all: bool,
}

/// What: Process all command-line arguments and handle early-exit flags.
///
/// Inputs:
/// - `args`: Parsed command-line arguments.
///
/// Output:
/// - Returns `true` if the program should continue to TUI, `false` if it should exit early.
///
/// Details:
/// - Handles search mode (exits immediately).
/// - Handles clear cache flag (exits immediately).
/// - Logs warnings for unimplemented flags (install, remove, update, refresh, news).
/// - Returns `true` if no early-exit flags were triggered.
pub fn process_args(args: &Args) -> bool {
    use crate::args::{cache, install, list, news, remove, search, update};

    // Handle command-line search mode
    if let Some(search_query) = &args.search {
        search::handle_search(search_query);
    }

    // Handle clear cache flag
    if args.clear_cache {
        cache::handle_clear_cache();
    }

    // Handle list installed packages flag
    if args.list {
        list::handle_list(args.exp, args.imp, args.all);
    }

    // Handle command-line install mode
    if !args.install.is_empty() {
        install::handle_install(&args.install);
    }

    // Handle install from file (-I)
    if let Some(file_path) = &args.install_from_file {
        install::handle_install_from_file(file_path);
    }

    // Handle remove packages from command line (-r / --remove)
    if !args.remove.is_empty() {
        remove::handle_remove(&args.remove);
    }

    // Handle remove packages from file (-R)
    if let Some(file_path) = &args.remove_from_file {
        tracing::info!(file = %file_path, "Remove from file requested from CLI");
        // TODO: Implement remove from file (mentioned in roadmap)
        tracing::warn!("Remove from file not yet implemented, falling back to TUI");
    }

    // Handle system update (--update / -u)
    if args.update {
        update::handle_update();
    }

    // Handle refresh flag
    if args.refresh {
        tracing::info!("Refresh mode requested from CLI");
        // TODO: Implement package database refresh
        tracing::warn!("Refresh mode not yet implemented");
    }

    // Handle news flag
    if args.news {
        news::handle_news(args.unread, args.read, args.all_news);
    }

    true
}
