//! Aggregated news feed fetcher (Arch news + security advisories).
use std::collections::HashSet;

use crate::state::types::{NewsFeedItem, NewsFeedSource, NewsSortMode};
use tracing::{info, warn};

type Result<T> = super::Result<T>;

/// What: Fetch combined news feed (Arch news + security advisories) and sort.
///
/// Inputs:
/// - `limit`: Maximum items per source.
/// - `include_arch_news`: Whether to fetch Arch news RSS.
/// - `include_advisories`: Whether to fetch security advisories.
/// - `installed_filter`: Optional set of installed package names for advisory filtering.
///
/// Output:
/// - `Ok(Vec<NewsFeedItem>)` combined and sorted by selected mode.
///
/// Details:
/// - Advisories are filtered to installed packages when `installed_filter` is provided and
///   `installed_only` is true.
///
/// # Errors
/// - Network failures fetching sources
/// - JSON parse errors from upstream feeds
pub async fn fetch_news_feed<S: std::hash::BuildHasher + Send + Sync + 'static>(
    limit: usize,
    include_arch_news: bool,
    include_advisories: bool,
    installed_filter: Option<&HashSet<String, S>>,
    installed_only: bool,
    sort_mode: NewsSortMode,
) -> Result<Vec<NewsFeedItem>> {
    info!(
        limit,
        include_arch_news,
        include_advisories,
        installed_only,
        installed_filter = installed_filter.is_some(),
        sort_mode = ?sort_mode,
        "fetch_news_feed start"
    );
    let mut items: Vec<NewsFeedItem> = Vec::new();
    if include_arch_news {
        match super::fetch_arch_news(limit).await {
            Ok(news) => items.extend(news.into_iter().map(|n| NewsFeedItem {
                id: n.url.clone(),
                date: n.date,
                title: n.title,
                summary: None,
                url: Some(n.url),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            })),
            Err(e) => {
                warn!(error = %e, "arch news fetch failed; continuing without Arch news");
            }
        }
    }
    if include_advisories {
        match super::fetch_security_advisories(limit).await {
            Ok(advisories) => {
                for adv in advisories {
                    if installed_only
                        && let Some(set) = installed_filter
                        && !adv.packages.iter().any(|p| set.contains(p))
                    {
                        continue;
                    }
                    items.push(adv);
                }
            }
            Err(e) => {
                warn!(error = %e, "security advisories fetch failed; continuing without advisories");
            }
        }
    }
    sort_news_items(&mut items, sort_mode);
    info!(
        total = items.len(),
        arch = items
            .iter()
            .filter(|i| matches!(i.source, NewsFeedSource::ArchNews))
            .count(),
        advisories = items
            .iter()
            .filter(|i| matches!(i.source, NewsFeedSource::SecurityAdvisory))
            .count(),
        "fetch_news_feed success"
    );
    Ok(items)
}

fn sort_news_items(items: &mut [NewsFeedItem], mode: NewsSortMode) {
    match mode {
        NewsSortMode::DateDesc => items.sort_by(|a, b| b.date.cmp(&a.date)),
        NewsSortMode::DateAsc => items.sort_by(|a, b| a.date.cmp(&b.date)),
        NewsSortMode::Title => {
            items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        }
        NewsSortMode::SourceThenTitle => items.sort_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::NewsFeedSource;

    #[test]
    fn sort_news_items_orders_by_date_desc() {
        let mut items = vec![
            NewsFeedItem {
                id: "1".into(),
                date: "2024-01-02".into(),
                title: "B".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
            NewsFeedItem {
                id: "2".into(),
                date: "2024-01-03".into(),
                title: "A".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
        ];
        sort_news_items(&mut items, NewsSortMode::DateDesc);
        assert_eq!(items.first().map(|i| &i.id), Some(&"2".to_string()));
    }
}
