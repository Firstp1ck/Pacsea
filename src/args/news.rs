//! Command-line news functionality.

use crate::args::i18n;
use pacsea::theme;

/// What: Handle news flag by fetching Arch news and displaying to command line.
///
/// Inputs:
/// - `unread`: If true, list only unread news.
/// - `read`: If true, list only read news.
/// - `all_news`: If true, list all news (read and unread).
///
/// Output:
/// - Exits the process after displaying the news.
///
/// Details:
/// - Fetches Arch Linux news from RSS feed.
/// - Loads read news URLs from persisted file.
/// - Filters news based on the specified option (defaults to all if none specified).
/// - Prints news items with date, title, and URL.
/// - Outputs link to website at the end.
/// - Exits immediately after displaying (doesn't launch TUI).
pub fn handle_news(unread: bool, read: bool, all_news: bool) -> ! {
    use std::collections::HashSet;

    tracing::info!(
        unread = unread,
        read = read,
        all_news = all_news,
        "News mode requested from CLI"
    );

    // Default to all if no option is specified
    let show_all = if !unread && !read && !all_news {
        tracing::info!("No news option specified, defaulting to --all");
        true
    } else {
        all_news
    };

    // Load read news URLs from persisted file
    let news_read_path = theme::lists_dir().join("news_read_urls.json");
    let read_urls: HashSet<String> = if let Ok(s) = std::fs::read_to_string(&news_read_path)
        && let Ok(set) = serde_json::from_str::<HashSet<String>>(&s)
    {
        set
    } else {
        HashSet::new()
    };

    // Fetch news (using tokio runtime for async)
    // Spawn a separate thread with its own runtime to avoid nested runtime issues
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let res = match rt {
            Ok(rt) => rt.block_on(pacsea::sources::fetch_arch_news(100)),
            Err(e) => Err::<Vec<pacsea::state::NewsItem>, _>(format!("rt: {e}").into()),
        };
        let _ = tx.send(res);
    });
    let news_items = match rx.recv() {
        Ok(Ok(items)) => items,
        Ok(Err(e)) => {
            eprintln!("{}", i18n::t_fmt1("app.cli.news.fetch_error", &e));
            tracing::error!(error = %e, "Failed to fetch news");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("{}", i18n::t_fmt1("app.cli.news.runtime_error", e));
            tracing::error!(error = %e, "Failed to receive news from thread");
            std::process::exit(1);
        }
    };

    // Filter news based on option
    let filtered_items: Vec<&pacsea::state::NewsItem> = if show_all {
        news_items.iter().collect()
    } else if unread {
        news_items
            .iter()
            .filter(|item| !read_urls.contains(&item.url))
            .collect()
    } else if read {
        news_items
            .iter()
            .filter(|item| read_urls.contains(&item.url))
            .collect()
    } else {
        news_items.iter().collect()
    };

    // Print news items
    if filtered_items.is_empty() {
        println!("{}", i18n::t("app.cli.news.no_items"));
    } else {
        for item in &filtered_items {
            let status = if read_urls.contains(&item.url) {
                i18n::t("app.cli.news.status_read")
            } else {
                i18n::t("app.cli.news.status_unread")
            };
            println!("{} {} - {}", status, item.date, item.title);
            println!("{}", i18n::t_fmt1("app.cli.news.url_label", &item.url));
            println!();
        }
    }

    // Print link to website at the end
    println!("{}", i18n::t("app.cli.news.website_link"));

    tracing::info!(count = filtered_items.len(), "Displayed news items");
    std::process::exit(0);
}
