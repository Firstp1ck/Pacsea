//! Implementation methods for `AppState`.

use crate::state::app_state::{AppState, recent_capacity};
use crate::state::types::{
    NewsBookmark, NewsFeedItem, NewsReadFilter, NewsSortMode, severity_rank,
};
use chrono::{NaiveDate, Utc};

impl AppState {
    /// What: Return recent searches in most-recent-first order.
    ///
    /// Inputs:
    /// - `self`: Application state containing the recent LRU cache.
    ///
    /// Output:
    /// - Vector of recent search strings ordered from most to least recent.
    ///
    /// Details:
    /// - Clones stored values; limited to `RECENT_CAPACITY`.
    #[must_use]
    pub fn recent_values(&self) -> Vec<String> {
        self.recent.iter().map(|(_, v)| v.clone()).collect()
    }

    /// What: Fetch a recent search by positional index.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` when the index is valid; `None` otherwise.
    ///
    /// Details:
    /// - Uses the LRU iterator, so `index == 0` is the most recent entry.
    #[must_use]
    pub fn recent_value_at(&self, index: usize) -> Option<String> {
        self.recent.iter().nth(index).map(|(_, v)| v.clone())
    }

    /// What: Remove a recent search at the provided position.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` containing the removed value when found; `None` otherwise.
    ///
    /// Details:
    /// - Resolves the cache key via iteration, then pops it to maintain LRU invariants.
    pub fn remove_recent_at(&mut self, index: usize) -> Option<String> {
        let key = self.recent.iter().nth(index).map(|(k, _)| k.clone())?;
        self.recent.pop(&key)
    }

    /// What: Add or replace a news bookmark, marking state dirty.
    ///
    /// Inputs:
    /// - `bookmark`: Bookmark to insert (deduped by `item.id`).
    ///
    /// Output:
    /// - None (mutates bookmarks and dirty flag).
    pub fn add_news_bookmark(&mut self, bookmark: NewsBookmark) {
        if let Some(pos) = self
            .news_bookmarks
            .iter()
            .position(|b| b.item.id == bookmark.item.id)
        {
            self.news_bookmarks[pos] = bookmark;
        } else {
            self.news_bookmarks.push(bookmark);
        }
        self.news_bookmarks_dirty = true;
    }

    /// What: Remove a news bookmark at a position.
    ///
    /// Inputs:
    /// - `index`: Zero-based index into bookmarks vector.
    ///
    /// Output:
    /// - Removed bookmark if present.
    pub fn remove_news_bookmark_at(&mut self, index: usize) -> Option<NewsBookmark> {
        if index >= self.news_bookmarks.len() {
            return None;
        }
        let removed = self.news_bookmarks.remove(index);
        self.news_bookmarks_dirty = true;
        Some(removed)
    }

    /// What: Return recent news searches in most-recent-first order.
    ///
    /// Inputs:
    /// - `self`: Application state containing the news recent LRU cache.
    ///
    /// Output:
    /// - Vector of recent news search strings ordered from most to least recent.
    ///
    /// Details:
    /// - Clones stored values; limited by the configured recent capacity.
    #[must_use]
    pub fn news_recent_values(&self) -> Vec<String> {
        self.news_recent.iter().map(|(_, v)| v.clone()).collect()
    }

    /// What: Fetch a recent news search by positional index.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` when the index is valid; `None` otherwise.
    ///
    /// Details:
    /// - Uses the LRU iterator, so `index == 0` is the most recent entry.
    #[must_use]
    pub fn news_recent_value_at(&self, index: usize) -> Option<String> {
        self.news_recent.iter().nth(index).map(|(_, v)| v.clone())
    }

    /// What: Replace the news recent cache with the provided most-recent-first entries.
    ///
    /// Inputs:
    /// - `items`: Slice of recent news search strings ordered from most to least recent.
    ///
    /// Output:
    /// - None (mutates `self.news_recent`).
    ///
    /// Details:
    /// - Clears existing entries, enforces configured capacity, and preserves ordering by
    ///   inserting from least-recent to most-recent.
    pub fn load_news_recent_items(&mut self, items: &[String]) {
        self.news_recent.clear();
        self.news_recent.resize(recent_capacity());
        for value in items.iter().rev() {
            let stored = value.clone();
            let key = stored.to_ascii_lowercase();
            self.news_recent.put(key, stored);
        }
    }

    /// What: Remove a recent news search at the provided position.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` containing the removed value when found; `None` otherwise.
    ///
    /// Details:
    /// - Resolves the cache key via iteration, then pops it to maintain LRU invariants.
    pub fn remove_news_recent_at(&mut self, index: usize) -> Option<String> {
        let key = self.news_recent.iter().nth(index).map(|(k, _)| k.clone())?;
        self.news_recent.pop(&key)
    }

    /// What: Replace the recent cache with the provided most-recent-first entries.
    ///
    /// Inputs:
    /// - `items`: Slice of recent search strings ordered from most to least recent.
    ///
    /// Output:
    /// - None (mutates `self.recent`).
    ///
    /// Details:
    /// - Clears existing entries, enforces configured capacity, and preserves ordering by
    ///   inserting from least-recent to most-recent.
    pub fn load_recent_items(&mut self, items: &[String]) {
        self.recent.clear();
        self.recent.resize(recent_capacity());
        for value in items.iter().rev() {
            let stored = value.clone();
            let key = stored.to_ascii_lowercase();
            self.recent.put(key, stored);
        }
    }

    /// What: Recompute news results applying filters, search, age cutoff, and sorting.
    ///
    /// Inputs:
    /// - `self`: Mutable application state containing news items and filter fields.
    ///
    /// Output:
    /// - Updates `news_results`, selection state, and recent news searches.
    pub fn refresh_news_results(&mut self) {
        let query = self.news_search_input.to_lowercase();
        if query.is_empty() {
            self.news_history_pending = None;
            self.news_history_pending_at = None;
        } else {
            self.news_history_pending = Some(self.news_search_input.clone());
            self.news_history_pending_at = Some(std::time::Instant::now());
        }
        let mut filtered: Vec<NewsFeedItem> = self
            .news_items
            .iter()
            .filter(|it| match it.source {
                crate::state::types::NewsFeedSource::ArchNews => self.news_filter_show_arch_news,
                crate::state::types::NewsFeedSource::SecurityAdvisory => {
                    self.news_filter_show_advisories
                }
                crate::state::types::NewsFeedSource::InstalledPackageUpdate => {
                    self.news_filter_show_pkg_updates
                }
                crate::state::types::NewsFeedSource::AurPackageUpdate => {
                    self.news_filter_show_aur_updates
                }
                crate::state::types::NewsFeedSource::AurComment => {
                    self.news_filter_show_aur_comments
                }
            })
            .cloned()
            .collect();

        // Apply installed-only filter for advisories when enabled.
        // When "[Advisories All]" is active (news_filter_show_advisories = true,
        // news_filter_installed_only = false), this block does not run, allowing
        // all advisories to be shown regardless of installed status.
        if self.news_filter_installed_only {
            let installed: std::collections::HashSet<String> =
                crate::index::explicit_names().into_iter().collect();
            filtered.retain(|it| {
                !matches!(
                    it.source,
                    crate::state::types::NewsFeedSource::SecurityAdvisory
                ) || it.packages.iter().any(|pkg| installed.contains(pkg))
            });
        }

        if !matches!(self.news_filter_read_status, NewsReadFilter::All) {
            filtered.retain(|it| {
                let is_read = self.news_read_ids.contains(&it.id)
                    || it
                        .url
                        .as_ref()
                        .is_some_and(|u| self.news_read_urls.contains(u));
                matches!(self.news_filter_read_status, NewsReadFilter::Read) && is_read
                    || matches!(self.news_filter_read_status, NewsReadFilter::Unread) && !is_read
            });
        }

        if !query.is_empty() {
            filtered.retain(|it| {
                let hay = format!(
                    "{} {} {}",
                    it.title,
                    it.summary.clone().unwrap_or_default(),
                    it.packages.join(" ")
                )
                .to_lowercase();
                hay.contains(&query)
            });
        }

        if let Some(max_days) = self.news_max_age_days
            && let Some(cutoff_date) = Utc::now()
                .date_naive()
                .checked_sub_days(chrono::Days::new(u64::from(max_days)))
        {
            filtered.retain(|it| {
                NaiveDate::parse_from_str(&it.date, "%Y-%m-%d").map_or(true, |d| d >= cutoff_date)
            });
        }

        let is_read = |it: &NewsFeedItem| {
            self.news_read_ids.contains(&it.id)
                || it
                    .url
                    .as_ref()
                    .is_some_and(|u| self.news_read_urls.contains(u))
        };

        match self.news_sort_mode {
            NewsSortMode::DateDesc => filtered.sort_by(|a, b| b.date.cmp(&a.date)),
            NewsSortMode::DateAsc => filtered.sort_by(|a, b| a.date.cmp(&b.date)),
            NewsSortMode::Title => {
                filtered.sort_by(|a, b| {
                    a.title
                        .to_lowercase()
                        .cmp(&b.title.to_lowercase())
                        .then(b.date.cmp(&a.date))
                });
            }
            NewsSortMode::SourceThenTitle => filtered.sort_by(|a, b| {
                a.source
                    .cmp(&b.source)
                    .then(b.date.cmp(&a.date))
                    .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }),
            NewsSortMode::SeverityThenDate => filtered.sort_by(|a, b| {
                let sa = severity_rank(a.severity);
                let sb = severity_rank(b.severity);
                sb.cmp(&sa)
                    .then(b.date.cmp(&a.date))
                    .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }),
            NewsSortMode::UnreadThenDate => filtered.sort_by(|a, b| {
                let ra = is_read(a);
                let rb = is_read(b);
                ra.cmp(&rb)
                    .then(b.date.cmp(&a.date))
                    .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }),
        }

        self.news_results = filtered;
        if self.news_results.is_empty() {
            self.news_selected = 0;
            self.news_list_state.select(None);
        } else {
            self.news_selected = self
                .news_selected
                .min(self.news_results.len().saturating_sub(1));
            self.news_list_state.select(Some(self.news_selected));
        }
    }
}
