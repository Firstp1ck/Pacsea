use ratatui::prelude::Rect;

use crate::state::{AppState, Source};

/// What: Detect availability of optional repos from the unfiltered results set.
///
/// Inputs:
/// - `app`: Application state providing `all_results`
///
/// Output:
/// - Tuple `(has_eos, has_cachyos, has_artix, has_manjaro)` indicating which repo chips to show.
///
/// Details:
/// - Scans official result sources and package names to infer EOS/CachyOS/Artix/Manjaro presence, short
///   circuiting once all four are detected.
pub fn detect_optional_repos(app: &AppState) -> (bool, bool, bool, bool) {
    let mut eos = false;
    let mut cach = false;
    let mut artix = false;
    let mut manj = false;
    for it in app.all_results.iter() {
        if let Source::Official { repo, .. } = &it.source {
            let r = repo.to_lowercase();
            if !eos && crate::index::is_eos_repo(&r) {
                eos = true;
            }
            if !cach && crate::index::is_cachyos_repo(&r) {
                cach = true;
            }
            if !artix && crate::index::is_artix_repo(&r) {
                artix = true;
            }
        }
        // Treat presence by name prefix rather than repo value
        if !manj && crate::index::is_name_manjaro(&it.name) {
            manj = true;
        }
        if eos && cach && artix && manj {
            break;
        }
    }
    (eos, cach, artix, manj)
}

/// What: Keep the results selection centered within the visible viewport when possible.
///
/// Inputs:
/// - `app`: Mutable application state containing the list state and results
/// - `area`: Rect describing the results block (used to derive viewport rows)
///
/// Output:
/// - Adjusts `app.list_state` offset/selection to keep the highlight in view.
///
/// Details:
/// - Recenters around the selected index for long lists, resets offset for short lists, and ensures
///   the selection is applied even when the filtered list shrinks.
pub fn center_selection(app: &mut AppState, area: Rect) {
    let viewport_rows = area.height.saturating_sub(2) as usize; // account for borders
    let len = app.results.len();
    let selected_idx = if app.results.is_empty() {
        None
    } else {
        Some(app.selected.min(len - 1))
    };

    if viewport_rows > 0 && len > viewport_rows {
        let selected = selected_idx.unwrap_or(0);
        let max_offset = len.saturating_sub(viewport_rows);
        let desired = selected.saturating_sub(viewport_rows / 2).min(max_offset);
        if app.list_state.offset() != desired {
            let mut st = ratatui::widgets::ListState::default().with_offset(desired);
            st.select(selected_idx);
            app.list_state = st;
        } else {
            // ensure selection is set
            app.list_state.select(selected_idx);
        }
    } else {
        // Small lists: ensure offset is 0 and selection is applied
        if app.list_state.offset() != 0 {
            let mut st = ratatui::widgets::ListState::default().with_offset(0);
            st.select(selected_idx);
            app.list_state = st;
        } else {
            app.list_state.select(selected_idx);
        }
    }
}

/// What: Record the inner results rect for mouse hit-testing (inside borders).
///
/// Inputs:
/// - `app`: Mutable application state receiving the results rect
/// - `area`: Rect of the overall results block
///
/// Output:
/// - Updates `app.results_rect` with the inner content rectangle.
///
/// Details:
/// - Offsets by one cell to exclude borders and reduces width/height accordingly for accurate
///   click detection.
pub fn record_results_rect(app: &mut AppState, area: Rect) {
    // Record inner results rect for mouse hit-testing (inside borders)
    app.results_rect = Some((
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    ));
}
