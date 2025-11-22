use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
};

use crate::state::{AppState, Focus};

mod install;
mod installed_only;
mod recent;
mod search;

/// What: Render the middle row: Recent (left), Search input (center), Install list (right).
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (focus, rects, lists, input)
/// - `area`: Target rectangle for the middle row
///
/// Output:
/// - Draws middle panes and updates rects for mouse hit-testing and cursor position.
///
/// Details:
/// - Titles and colors reflect focus; when installed-only mode is active the right column shows
///   Downgrade and Remove subpanes side-by-side.
/// - Records inner rects for Recent/Install/Downgrade and sets the caret position for the Search input.
pub fn render_middle(f: &mut Frame, app: &mut AppState, area: Rect) {
    // Middle row split: left Recent, middle Search input, right Install list
    // If a pane is hidden, reassign its percentage to the center pane.
    let left_pct = if app.show_recent_pane {
        app.layout_left_pct.min(100)
    } else {
        0
    };
    let mut right_pct = if app.show_install_pane {
        app.layout_right_pct.min(100)
    } else {
        0
    };
    // In installed-only mode, enlarge the right pane so Downgrade and Remove lists are each ~50% wider
    if app.installed_only_mode && right_pct > 0 {
        let max_right = 100u16.saturating_sub(left_pct);
        let widened = ((right_pct as u32 * 3) / 2) as u16; // 1.5x
        right_pct = widened.min(max_right);
    }
    let center_pct = 100u16
        .saturating_sub(left_pct)
        .saturating_sub(right_pct)
        .min(100);
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(center_pct),
            Constraint::Percentage(right_pct),
        ])
        .split(area);

    // Search input (center)
    search::render_search(f, app, middle[1]);

    // Recent searches (left) with filtering (render only if visible and has width)
    recent::render_recent(f, app, middle[0]);

    // Install/Remove List (right) with filtering (render only if visible and has width)
    if app.show_install_pane && middle[2].width > 0 {
        if app.installed_only_mode {
            installed_only::render_installed_only(f, app, middle[2]);
        } else {
            install::render_install(f, app, middle[2]);
        }
    } else {
        app.install_rect = None;
        // If Install pane is hidden and currently focused, move focus to Search
        if matches!(app.focus, Focus::Install) {
            app.focus = Focus::Search;
        }
    }
}

#[cfg(test)]
mod tests {
    /// What: Verify middle-pane rendering captures layout rectangles and realigns focus when the install pane hides.
    ///
    /// Inputs:
    /// - Initial render with recent and install panes visible, followed by a second pass hiding the install pane while focused there.
    ///
    /// Output:
    /// - Rectangles recorded for both panes initially, and focus reverts to `Search` once the install pane is hidden.
    ///
    /// Details:
    /// - Uses a `TestBackend` to drive rendering without interactive user input.
    #[test]
    fn middle_sets_rects_and_cursor_positions() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        app.show_recent_pane = true;
        app.show_install_pane = true;
        app.focus = crate::state::Focus::Search;
        app.input = "hello".into();

        term.draw(|f| {
            let area = f.area();
            super::render_middle(f, &mut app, area);
        })
        .expect("Failed to render middle pane");

        assert!(app.recent_rect.is_some());
        assert!(app.install_rect.is_some());
        // Move focus to Install and re-render; ensure focus fix-up when hidden
        app.focus = crate::state::Focus::Install;
        app.show_install_pane = false;
        term.draw(|f| {
            let area = f.area();
            super::render_middle(f, &mut app, area);
        })
        .expect("Failed to render middle pane");
        assert!(matches!(app.focus, crate::state::Focus::Search));
    }

    /// What: Verify layout calculation handles installed-only mode enlargement.
    ///
    /// Inputs:
    /// - Installed-only mode with right pane percentage set
    ///
    /// Output:
    /// - Right pane is enlarged to 1.5x its original size (up to maximum available).
    ///
    /// Details:
    /// - Tests that installed-only mode widens the right pane to accommodate two lists.
    #[test]
    fn middle_enlarges_right_pane_in_installed_only_mode() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        app.show_recent_pane = true;
        app.show_install_pane = true;
        app.installed_only_mode = true;
        app.layout_left_pct = 20;
        app.layout_right_pct = 30; // Should become 45 (30 * 1.5) if space allows

        term.draw(|f| {
            let area = f.area();
            super::render_middle(f, &mut app, area);
        })
        .expect("Failed to render middle pane");

        // Should render without panic with enlarged right pane
        assert!(app.install_rect.is_some());
        assert!(app.downgrade_rect.is_some());
    }

    /// What: Verify layout calculation reassigns space when panes are hidden.
    ///
    /// Inputs:
    /// - Recent pane hidden, install pane hidden
    ///
    /// Output:
    /// - Center pane (search) gets all available space.
    ///
    /// Details:
    /// - Tests that hidden panes don't take up space.
    #[test]
    fn middle_reassigns_space_when_panes_hidden() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        app.show_recent_pane = false;
        app.show_install_pane = false;
        app.focus = crate::state::Focus::Search;
        app.input = "test".into();

        term.draw(|f| {
            let area = f.area();
            super::render_middle(f, &mut app, area);
        })
        .expect("Failed to render middle pane");

        // Recent and install rects should be None
        assert!(app.recent_rect.is_none());
        assert!(app.install_rect.is_none());
    }

    /// What: Verify layout calculation handles zero-width areas gracefully.
    ///
    /// Inputs:
    /// - Area with zero width
    ///
    /// Output:
    /// - Rendering completes without panic.
    ///
    /// Details:
    /// - Tests edge case where terminal area is too small.
    #[test]
    fn middle_handles_zero_width_area() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(0, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        app.show_recent_pane = true;
        app.show_install_pane = true;

        term.draw(|f| {
            let area = f.area();
            super::render_middle(f, &mut app, area);
        })
        .expect("Failed to render middle pane");

        // Should handle zero width without panic
    }

    /// What: Verify focus switching when install pane is hidden while focused.
    ///
    /// Inputs:
    /// - Install pane is focused, then hidden
    ///
    /// Output:
    /// - Focus switches to Search.
    ///
    /// Details:
    /// - Tests that focus is automatically corrected when focused pane is hidden.
    #[test]
    fn middle_switches_focus_when_install_hidden() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(120, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        app.show_install_pane = true;
        app.focus = crate::state::Focus::Install;

        // Hide install pane
        app.show_install_pane = false;
        term.draw(|f| {
            let area = f.area();
            super::render_middle(f, &mut app, area);
        })
        .expect("Failed to render middle pane");

        assert!(matches!(app.focus, crate::state::Focus::Search));
    }
}
