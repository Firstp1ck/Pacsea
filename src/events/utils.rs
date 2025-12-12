use crossterm::event::KeyEvent;
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};
use std::time::Instant;

/// What: Check if a key event matches any chord in a list, handling Shift+char edge cases.
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `list`: List of configured key chords to match against
///
/// Output:
/// - `true` if the key event matches any chord in the list, `false` otherwise
///
/// Details:
/// - Treats Shift+<char> from config as equivalent to uppercase char without Shift from terminal.
/// - Handles cases where terminals report Shift inconsistently.
#[must_use]
pub fn matches_any(ke: &KeyEvent, list: &[crate::theme::KeyChord]) -> bool {
    list.iter().any(|c| {
        if (c.code, c.mods) == (ke.code, ke.modifiers) {
            return true;
        }
        match (c.code, ke.code) {
            (crossterm::event::KeyCode::Char(cfg_ch), crossterm::event::KeyCode::Char(ev_ch)) => {
                let cfg_has_shift = c.mods.contains(crossterm::event::KeyModifiers::SHIFT);
                if !cfg_has_shift {
                    return false;
                }
                // Accept uppercase event regardless of SHIFT flag
                if ev_ch == cfg_ch.to_ascii_uppercase() {
                    return true;
                }
                // Accept lowercase char if terminal reports SHIFT in modifiers
                if ke.modifiers.contains(crossterm::event::KeyModifiers::SHIFT)
                    && ev_ch.to_ascii_lowercase() == cfg_ch
                {
                    return true;
                }
                false
            }
            _ => false,
        }
    })
}

/// What: Return the number of Unicode scalar values (characters) in the input.
///
/// Input: `s` string to measure
/// Output: Character count as `usize`
///
/// Details: Counts Unicode scalar values using `s.chars().count()`.
#[must_use]
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// What: Convert a character index to a byte index for slicing.
///
/// Input: `s` source string; `ci` character index
/// Output: Byte index into `s` corresponding to `ci`
///
/// Details: Returns 0 for `ci==0`; returns `s.len()` when `ci>=char_count(s)`; otherwise maps
/// the character index to a byte offset via `char_indices()`.
#[must_use]
pub fn byte_index_for_char(s: &str, ci: usize) -> usize {
    let cc = char_count(s);
    if ci == 0 {
        return 0;
    }
    if ci >= cc {
        return s.len();
    }
    s.char_indices()
        .map(|(i, _)| i)
        .nth(ci)
        .map_or(s.len(), |i| i)
}

/// What: Advance selection in the Recent pane to the next/previous match of the pane-find pattern.
///
/// Input: `app` mutable application state; `forward` when true searches downward, else upward
/// Output: No return value; updates `history_state` selection when a match is found
///
/// Details: Searches within the filtered Recent indices and wraps around the list; matching is
/// case-insensitive against the current pane-find pattern.
pub fn find_in_recent(app: &mut AppState, forward: bool) {
    let Some(pattern) = app.pane_find.clone() else {
        return;
    };
    let inds = crate::ui::helpers::filtered_recent_indices(app);
    if inds.is_empty() {
        return;
    }
    let start = app.history_state.selected().unwrap_or(0);
    let mut vi = start;
    let n = inds.len();
    for _ in 0..n {
        vi = if forward {
            (vi + 1) % n
        } else if vi == 0 {
            n - 1
        } else {
            vi - 1
        };
        let i = inds[vi];
        if let Some(s) = app.recent_value_at(i)
            && s.to_lowercase().contains(&pattern.to_lowercase())
        {
            app.history_state.select(Some(vi));
            break;
        }
    }
}

/// What: Advance selection in the Install pane to the next/previous item matching the pane-find pattern.
///
/// Input: `app` mutable application state; `forward` when true searches downward, else upward
/// Output: No return value; updates `install_state` selection when a match is found
///
/// Details: Operates on visible indices and tests case-insensitive matches against package name
/// or description; wraps around the list.
pub fn find_in_install(app: &mut AppState, forward: bool) {
    let Some(pattern) = app.pane_find.clone() else {
        return;
    };
    let inds = crate::ui::helpers::filtered_install_indices(app);
    if inds.is_empty() {
        return;
    }
    let start = app.install_state.selected().unwrap_or(0);
    let mut vi = start;
    let n = inds.len();
    for _ in 0..n {
        vi = if forward {
            (vi + 1) % n
        } else if vi == 0 {
            n - 1
        } else {
            vi - 1
        };
        let i = inds[vi];
        if let Some(p) = app.install_list.get(i)
            && (p.name.to_lowercase().contains(&pattern.to_lowercase())
                || p.description
                    .to_lowercase()
                    .contains(&pattern.to_lowercase()))
        {
            app.install_state.select(Some(vi));
            break;
        }
    }
}

/// What: Ensure details reflect the currently selected result.
///
/// Input: `app` mutable application state; `details_tx` channel for details requests
/// Output: No return value; uses cache or sends a details request
///
/// Details: If details for the selected item exist in the cache, they are applied immediately;
/// otherwise, the item is sent over `details_tx` to be fetched asynchronously.
pub fn refresh_selected_details(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    if let Some(item) = app.results.get(app.selected).cloned() {
        // Reset scroll when package changes
        app.details_scroll = 0;
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

/// Move news selection by delta, keeping it in view.
pub fn move_news_selection(app: &mut AppState, delta: isize) {
    if app.news_results.is_empty() {
        app.news_selected = 0;
        app.news_list_state.select(None);
        app.details.url.clear();
        return;
    }
    let len = app.news_results.len();
    if app.news_selected >= len {
        app.news_selected = len.saturating_sub(1);
    }
    app.news_list_state.select(Some(app.news_selected));
    let steps = delta.unsigned_abs();
    for _ in 0..steps {
        if delta.is_negative() {
            app.news_list_state.select_previous();
        } else {
            app.news_list_state.select_next();
        }
    }
    let sel = app.news_list_state.selected().unwrap_or(0);
    app.news_selected = std::cmp::min(sel, len.saturating_sub(1));
    app.news_list_state.select(Some(app.news_selected));
    update_news_url(app);
}

/// Synchronize details URL and content with currently selected news item.
/// Also triggers content fetching if channel is provided and content is not cached.
pub fn update_news_url(app: &mut AppState) {
    if let Some(item) = app.news_results.get(app.news_selected)
        && let Some(url) = &item.url
    {
        app.details.url.clone_from(url);
        // Check if content is cached
        let mut cached = app.news_content_cache.get(url).cloned();
        if let Some(ref c) = cached
            && url.contains("://archlinux.org/packages/")
            && !c.starts_with("Package Info:")
        {
            // Cached pre-metadata version: force refresh
            cached = None;
            tracing::debug!(
                url,
                "news content cache missing package metadata; will refetch"
            );
        }
        app.news_content = cached;
        if app.news_content.is_some() {
            tracing::debug!(url, "news content served from cache");
        }
        app.news_content_scroll = 0;
    } else {
        app.details.url.clear();
        app.news_content = None;
    }
    app.news_content_loading = false;
}

/// Request news content fetch if not cached or loading.
pub fn maybe_request_news_content(
    app: &mut AppState,
    news_content_req_tx: &mpsc::UnboundedSender<String>,
) {
    // Only request if in news mode with a selected item that has a URL
    if !matches!(app.app_mode, crate::state::types::AppMode::News) {
        tracing::trace!("news_content: skip request, not in news mode");
        return;
    }
    if app.news_content_loading {
        tracing::debug!(
            selected = app.news_selected,
            "news_content: skip request, already loading"
        );
        return;
    }
    if let Some(item) = app.news_results.get(app.news_selected)
        && let Some(url) = &item.url
        && app.news_content.is_none()
        && !app.news_content_cache.contains_key(url)
    {
        app.news_content_loading = true;
        app.news_content_loading_since = Some(Instant::now());
        tracing::debug!(
            selected = app.news_selected,
            title = item.title,
            url,
            "news_content: requesting article content"
        );
        if let Err(e) = news_content_req_tx.send(url.clone()) {
            tracing::warn!(
                error = %e,
                selected = app.news_selected,
                title = item.title,
                url,
                "news_content: failed to enqueue content request"
            );
            app.news_content_loading = false;
            app.news_content_loading_since = None;
            app.news_content = Some(format!("Failed to load content: {e}"));
            app.toast_message = Some("News content request failed".to_string());
            app.toast_expires_at = Some(Instant::now() + std::time::Duration::from_secs(3));
        }
    } else {
        tracing::trace!(
            selected = app.news_selected,
            has_item = app.news_results.get(app.news_selected).is_some(),
            has_url = app
                .news_results
                .get(app.news_selected)
                .and_then(|it| it.url.as_ref())
                .is_some(),
            content_cached = app
                .news_results
                .get(app.news_selected)
                .and_then(|it| it.url.as_ref())
                .is_some_and(|u| app.news_content_cache.contains_key(u)),
            has_content = app.news_content.is_some(),
            "news_content: skip request (cached/absent URL/already loaded)"
        );
    }
}

/// What: Ensure details reflect the selected item in the Install pane.
///
/// Input: `app` mutable application state; `details_tx` channel for details requests
/// Output: No return value; focuses details on the selected Install item and uses cache or requests fetch
///
/// Details: Sets `details_focus`, populates a placeholder from the selected item, then uses the
/// cache when present; otherwise sends a request over `details_tx`.
pub fn refresh_install_details(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    let Some(vsel) = app.install_state.selected() else {
        return;
    };
    let inds = crate::ui::helpers::filtered_install_indices(app);
    if inds.is_empty() || vsel >= inds.len() {
        return;
    }
    let i = inds[vsel];
    if let Some(item) = app.install_list.get(i).cloned() {
        // Reset scroll when package changes
        app.details_scroll = 0;
        // Focus details on the install selection
        app.details_focus = Some(item.name.clone());

        // Provide an immediate placeholder reflecting the selection
        app.details.name.clone_from(&item.name);
        app.details.version.clone_from(&item.version);
        app.details.description.clear();
        match &item.source {
            crate::state::Source::Official { repo, arch } => {
                app.details.repository.clone_from(repo);
                app.details.architecture.clone_from(arch);
            }
            crate::state::Source::Aur => {
                app.details.repository = "AUR".to_string();
                app.details.architecture = "any".to_string();
            }
        }

        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

/// What: Ensure details reflect the selected item in the Remove pane.
///
/// Input: `app` mutable application state; `details_tx` channel for details requests
/// Output: No return value; focuses details on the selected Remove item and uses cache or requests fetch
///
/// Details: Sets `details_focus`, populates a placeholder from the selected item, then uses the
/// cache when present; otherwise sends a request over `details_tx`.
pub fn refresh_remove_details(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    let Some(vsel) = app.remove_state.selected() else {
        return;
    };
    if app.remove_list.is_empty() || vsel >= app.remove_list.len() {
        return;
    }
    if let Some(item) = app.remove_list.get(vsel).cloned() {
        // Reset scroll when package changes
        app.details_scroll = 0;
        app.details_focus = Some(item.name.clone());
        app.details.name.clone_from(&item.name);
        app.details.version.clone_from(&item.version);
        app.details.description.clear();
        match &item.source {
            crate::state::Source::Official { repo, arch } => {
                app.details.repository.clone_from(repo);
                app.details.architecture.clone_from(arch);
            }
            crate::state::Source::Aur => {
                app.details.repository = "AUR".to_string();
                app.details.architecture = "any".to_string();
            }
        }
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

/// What: Ensure details reflect the selected item in the Downgrade pane.
///
/// Input: `app` mutable application state; `details_tx` channel for details requests
/// Output: No return value; focuses details on the selected Downgrade item and uses cache or requests fetch
///
/// Details: Sets `details_focus`, populates a placeholder from the selected item, then uses the
/// cache when present; otherwise sends a request over `details_tx`.
pub fn refresh_downgrade_details(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    let Some(vsel) = app.downgrade_state.selected() else {
        return;
    };
    if app.downgrade_list.is_empty() || vsel >= app.downgrade_list.len() {
        return;
    }
    if let Some(item) = app.downgrade_list.get(vsel).cloned() {
        // Reset scroll when package changes
        app.details_scroll = 0;
        app.details_focus = Some(item.name.clone());
        app.details.name.clone_from(&item.name);
        app.details.version.clone_from(&item.version);
        app.details.description.clear();
        match &item.source {
            crate::state::Source::Official { repo, arch } => {
                app.details.repository.clone_from(repo);
                app.details.architecture.clone_from(arch);
            }
            crate::state::Source::Aur => {
                app.details.repository = "AUR".to_string();
                app.details.architecture = "any".to_string();
            }
        }
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Produce a baseline `AppState` tailored for utils tests.
    ///
    /// Inputs:
    /// - None; relies on `Default::default()` for deterministic state.
    ///
    /// Output:
    /// - Fresh `AppState` instance for individual unit tests.
    ///
    /// Details:
    /// - Centralizes setup so each test starts from a clean copy without repeated boilerplate.
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Ensure `char_count` returns the number of Unicode scalar values.
    ///
    /// Inputs:
    /// - Strings `"abc"`, `"π"`, and `"aπb"`.
    ///
    /// Output:
    /// - Counts `3`, `1`, and `3` respectively.
    ///
    /// Details:
    /// - Demonstrates correct handling of multi-byte characters.
    fn char_count_basic() {
        assert_eq!(char_count("abc"), 3);
        assert_eq!(char_count("π"), 1);
        assert_eq!(char_count("aπb"), 3);
    }

    #[test]
    /// What: Verify `byte_index_for_char` translates character indices to UTF-8 byte offsets.
    ///
    /// Inputs:
    /// - String `"aπb"` with char indices 0 through 3.
    ///
    /// Output:
    /// - Returns byte offsets `0`, `1`, `3`, and `len`.
    ///
    /// Details:
    /// - Confirms the function respects variable-width encoding.
    fn byte_index_for_char_basic() {
        let s = "aπb";
        assert_eq!(byte_index_for_char(s, 0), 0);
        assert_eq!(byte_index_for_char(s, 1), 1);
        assert_eq!(byte_index_for_char(s, 2), 1 + "π".len());
        assert_eq!(byte_index_for_char(s, 3), s.len());
    }

    #[test]
    /// What: Ensure `find_in_recent` cycles through entries matching the pane filter.
    ///
    /// Inputs:
    /// - Recent list `alpha`, `beta`, `gamma` with filter `"a"`.
    ///
    /// Output:
    /// - Selection rotates among matching entries without panicking.
    ///
    /// Details:
    /// - Provides smoke coverage for the wrap-around logic inside the helper.
    fn find_in_recent_basic() {
        let mut app = new_app();
        app.load_recent_items(&["alpha".to_string(), "beta".to_string(), "gamma".to_string()]);
        app.pane_find = Some("a".into());
        app.history_state.select(Some(0));
        find_in_recent(&mut app, true);
        assert!(app.history_state.selected().is_some());
    }

    #[test]
    /// What: Check `find_in_install` advances selection to the next matching entry by name or description.
    ///
    /// Inputs:
    /// - Install list with `ripgrep` and `fd`, filter term `"rip"` while selection starts on the second item.
    ///
    /// Output:
    /// - Selection wraps to the first item containing the filter term.
    ///
    /// Details:
    /// - Protects against regressions in forward search and wrap-around behaviour.
    fn find_in_install_basic() {
        let mut app = new_app();
        app.install_list = vec![
            crate::state::PackageItem {
                name: "ripgrep".into(),
                version: "1".into(),
                description: "fast search".into(),
                source: crate::state::Source::Aur,
                popularity: None,
                out_of_date: None,
                orphaned: false,
            },
            crate::state::PackageItem {
                name: "fd".into(),
                version: "1".into(),
                description: "find".into(),
                source: crate::state::Source::Aur,
                popularity: None,
                out_of_date: None,
                orphaned: false,
            },
        ];
        app.pane_find = Some("rip".into());
        // Start from visible selection 1 so advancing wraps to 0 matching "ripgrep"
        app.install_state.select(Some(1));
        find_in_install(&mut app, true);
        assert_eq!(app.install_state.selected(), Some(0));
    }

    #[test]
    /// What: Ensure `refresh_selected_details` dispatches a fetch when cache misses occur.
    ///
    /// Inputs:
    /// - Results list with a single entry and an empty details cache.
    ///
    /// Output:
    /// - Sends the selected item through `details_tx`, confirming a fetch request.
    ///
    /// Details:
    /// - Uses an unbounded channel to observe the request without performing actual I/O.
    fn refresh_selected_details_requests_when_missing() {
        let mut app = new_app();
        app.results = vec![crate::state::PackageItem {
            name: "rg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];
        app.selected = 0;
        let (tx, mut rx) = mpsc::unbounded_channel();
        refresh_selected_details(&mut app, &tx);
        let got = rx.try_recv().ok();
        assert!(got.is_some());
    }
}
