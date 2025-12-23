//! Command-line cache management functionality.

use crate::args::i18n;
use pacsea::theme;

/// What: Handle clear cache flag by removing all cache files and exiting.
///
/// Inputs:
/// - None (uses `theme::lists_dir()` to locate cache files).
///
/// Output:
/// - Exits the process after clearing cache files.
///
/// Details:
/// - Removes all cache files including dependency, file, service, sandbox, details, PKGBUILD parse,
///   news feed, news content, news seen updates/comments, and news/advisories article caches.
/// - Prints the number of cleared files to stdout.
/// - Exits immediately after clearing (doesn't launch TUI).
pub fn handle_clear_cache() -> ! {
    tracing::info!("Clear cache requested from CLI");
    let lists_dir = theme::lists_dir();
    let cache_files = [
        "install_deps_cache.json",
        "file_cache.json",
        "services_cache.json",
        "sandbox_cache.json",
        "details_cache.json",
        "pkgbuild_parse_cache.json",
        "news_content_cache.json",
        "news_feed.json",
        "news_seen_pkg_updates.json",
        "news_seen_aur_comments.json",
        "arch_news_cache.json",
        "advisories_cache.json",
        "news_article_cache.json",
    ];

    let mut cleared_count = 0;
    for cache_file in &cache_files {
        let cache_path = lists_dir.join(cache_file);
        match std::fs::remove_file(&cache_path) {
            Ok(()) => {
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
        println!("{}", i18n::t_fmt1("app.cli.cache.cleared", cleared_count));
    } else {
        tracing::info!("No cache files found to clear");
        println!("{}", i18n::t("app.cli.cache.none_found"));
    }
    std::process::exit(0);
}
