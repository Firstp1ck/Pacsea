use std::time::{Duration, Instant};

use crate::state::AppState;

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
    if let Some(pos) = app
        .recent
        .iter()
        .position(|s| s.eq_ignore_ascii_case(&value))
    {
        app.recent.remove(pos);
    }
    app.recent.insert(0, value.clone());
    if app.recent.len() > 20 {
        app.recent.truncate(20);
    }
    app.last_saved_value = Some(value);
    app.recent_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_app() -> AppState {
        AppState {
            ..Default::default()
        }
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
        app.last_input_change = std::time::Instant::now() - std::time::Duration::from_secs(3);
        app.last_saved_value = Some("ripgrep".into());
        maybe_save_recent(&mut app);
        assert!(app.recent.is_empty());

        // 4) New value beyond debounce -> saved at front, deduped, clamped
        app.input = "fd".into();
        app.last_saved_value = Some("ripgrep".into());
        app.last_input_change = std::time::Instant::now() - std::time::Duration::from_secs(3);
        maybe_save_recent(&mut app);
        assert_eq!(app.recent.first().map(|s| s.as_str()), Some("fd"));
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
        app.recent = vec!["RipGrep".into()];
        app.input = "ripgrep".into();
        app.last_input_change = std::time::Instant::now() - std::time::Duration::from_secs(3);
        maybe_save_recent(&mut app);
        assert_eq!(app.recent.len(), 1);
        assert_eq!(app.recent[0], "ripgrep");
    }
}
