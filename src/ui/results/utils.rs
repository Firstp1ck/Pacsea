use ratatui::prelude::Rect;

use crate::state::{AppState, Source};

/// What: Detect availability of optional repos from all_results (unfiltered) to keep chips visible.
///
/// Returns: (has_eos, has_cachyos, has_manjaro)
pub fn detect_optional_repos(app: &AppState) -> (bool, bool, bool) {
    let mut eos = false;
    let mut cach = false;
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
        }
        // Treat presence by name prefix rather than repo value
        if !manj && crate::index::is_name_manjaro(&it.name) {
            manj = true;
        }
        if eos && cach && manj {
            break;
        }
    }
    (eos, cach, manj)
}

/// What: Keep selection centered within the visible results list when possible.
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

/// What: Record inner results rect for mouse hit-testing (inside borders).
///
/// This should be called after render_results to set up hit-testing.
pub fn record_results_rect(app: &mut AppState, area: Rect) {
    // Record inner results rect for mouse hit-testing (inside borders)
    app.results_rect = Some((
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    ));
}
