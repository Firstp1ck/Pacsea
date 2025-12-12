use std::time::{Duration, Instant};

use crate::state::AppState;
use crate::state::app_state::recent_capacity;

/// What: Debounced persistence of the current search input into the Recent list.
///
/// Inputs:
/// - `app`: Mutable application state providing the input text and timing markers
///
/// Output:
/// - Updates `recent` (deduped, clamped to 20), sets `recent_dirty`, and records last-saved value
///   when conditions are met (non-empty, past debounce window, changed since last save).
pub fn maybe_save_recent(app: &mut AppState) {
    let now = Instant::now();
    if app.input.trim().is_empty() {
        return;
    }
    if now.duration_since(app.last_input_change) < Duration::from_secs(2) {
        return;
    }
    if app.last_saved_value.as_deref() == Some(app.input.trim()) {
        return;
    }

    let value = app.input.trim().to_string();
    let key = value.to_ascii_lowercase();
    app.recent.resize(recent_capacity());
    app.recent.put(key, value.clone());
    app.last_saved_value = Some(value);
    app.recent_dirty = true;
}

/// What: Debounced persistence of the current news search input into the news Recent list.
///
/// Inputs:
/// - `app`: Mutable application state providing news search text and timing markers
///
/// Output:
/// - Updates `news_recent` (deduped, clamped to capacity), sets `news_recent_dirty`, and records
///   last-saved value when conditions are met (non-empty, past debounce window, changed since last save).
pub fn maybe_save_news_recent(app: &mut AppState) {
    if !matches!(app.app_mode, crate::state::types::AppMode::News) {
        return;
    }
    let now = Instant::now();
    let query = app.news_search_input.trim();
    if query.is_empty() {
        app.news_history_pending = None;
        app.news_history_pending_at = None;
        return;
    }

    // Track pending value and debounce start
    app.news_history_pending = Some(query.to_string());
    app.news_history_pending_at = Some(now);

    // Enforce 2s debounce from the last input change
    if now.duration_since(app.last_input_change) < Duration::from_secs(2) {
        return;
    }

    // Avoid duplicate save of the same value
    if app.news_history_last_saved.as_deref() == Some(query) {
        return;
    }

    let value = query.to_string();
    let key = value.to_ascii_lowercase();
    app.news_recent.resize(recent_capacity());
    app.news_recent.put(key, value.clone());
    app.news_history_last_saved = Some(value);
    app.news_recent_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_app() -> AppState {
        AppState::default()
    }

    fn recent_values(app: &AppState) -> Vec<String> {
        app.recent.iter().map(|(_, v)| v.clone()).collect()
    }

    #[test]
    /// What: Ensure recent-save logic respects emptiness, debounce timing, and change detection.
    ///
    /// Inputs:
    /// - Sequence of states: empty input, under-debounce input, unchanged value, and new value beyond debounce.
    ///
    /// Output:
    /// - First three scenarios avoid saving; final scenario inserts the new value at the front and marks the list dirty.
    ///
    /// Details:
    /// - Mimics user typing delays to guarantee the helper only persists meaningful changes.
    fn maybe_save_recent_rules() {
        let mut app = new_app();
        // 1) Empty input -> no save
        app.input.clear();
        maybe_save_recent(&mut app);
        assert!(app.recent.is_empty());

        // 2) Under debounce window -> no save
        app.input = "ripgrep".into();
        app.last_input_change = std::time::Instant::now();
        maybe_save_recent(&mut app);
        assert!(app.recent.is_empty());

        // 3) Same value as last_saved_value -> no save
        app.last_input_change = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(3))
            .unwrap_or_else(std::time::Instant::now);
        app.last_saved_value = Some("ripgrep".into());
        maybe_save_recent(&mut app);
        assert!(app.recent.is_empty());

        // 4) New value beyond debounce -> saved at front, deduped, clamped
        app.input = "fd".into();
        app.last_saved_value = Some("ripgrep".into());
        app.last_input_change = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(3))
            .unwrap_or_else(std::time::Instant::now);
        maybe_save_recent(&mut app);
        assert_eq!(recent_values(&app).first().map(String::as_str), Some("fd"));
        assert!(app.recent_dirty);
    }

    #[test]
    /// What: Confirm existing case-insensitive matches move to the front without duplication.
    ///
    /// Inputs:
    /// - Recent list containing `"RipGrep"` and input `"ripgrep"` beyond the debounce window.
    ///
    /// Output:
    /// - Recent list collapses to one entry `"ripgrep"`.
    ///
    /// Details:
    /// - Protects the dedupe branch that removes stale duplicates before inserting the new value.
    fn recent_dedup_case_insensitive() {
        let mut app = new_app();
        app.recent.put("ripgrep".into(), "RipGrep".into());
        app.input = "ripgrep".into();
        app.last_input_change = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(3))
            .unwrap_or_else(std::time::Instant::now);
        maybe_save_recent(&mut app);
        let recents = recent_values(&app);
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0], "ripgrep");
    }

    #[test]
    /// What: Confirm recent cache evicts least-recent entries while keeping newest first.
    ///
    /// Inputs:
    /// - Sequence of unique inputs exceeding the configured recent capacity.
    ///
    /// Output:
    /// - Cache length equals capacity; newest entry sits at the front; oldest entries are evicted.
    ///
    /// Details:
    /// - Advances the debounce timer for each iteration to permit saves.
    fn recent_eviction_respects_capacity() {
        let mut app = new_app();
        let cap = recent_capacity().get();
        for i in 0..(cap + 2) {
            app.input = format!("pkg{i}");
            app.last_input_change = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(3))
                .unwrap_or_else(std::time::Instant::now);
            maybe_save_recent(&mut app);
        }

        let recents = recent_values(&app);
        assert_eq!(recents.len(), cap);
        let newest = format!("pkg{}", cap + 1);
        assert_eq!(recents.first().map(String::as_str), Some(newest.as_str()));
        assert!(
            recents
                .iter()
                .all(|entry| entry != "pkg0" && entry != "pkg1")
        );
    }

    #[test]
    /// What: Ensure news recent save is debounced and uses news search input.
    fn news_recent_respects_debounce_and_changes() {
        let mut app = new_app();
        app.app_mode = crate::state::types::AppMode::News;
        app.news_recent.clear();

        // Under debounce: should not save
        app.news_search_input = "arch".into();
        app.last_input_change = Instant::now();
        maybe_save_news_recent(&mut app);
        assert!(app.news_recent.is_empty());

        // Beyond debounce: should save
        app.last_input_change = Instant::now()
            .checked_sub(Duration::from_secs(3))
            .unwrap_or_else(Instant::now);
        maybe_save_news_recent(&mut app);
        let recents: Vec<String> = app.news_recent.iter().map(|(_, v)| v.clone()).collect();
        assert_eq!(recents.first().map(String::as_str), Some("arch"));
        assert_eq!(app.news_history_last_saved.as_deref(), Some("arch"));
        assert!(app.news_recent_dirty);
    }
}
