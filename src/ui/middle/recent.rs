use ratatui::{
    Frame,
    prelude::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem},
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

/// What: Render the Recent searches list in the left pane of the middle row.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (recent list, focus, pane find filter)
/// - `area`: Target rectangle for the recent pane
///
/// Output:
/// - Draws the recent searches list and records inner rect for mouse hit-testing.
///
/// Details:
/// - Shows filtered recent searches; title includes "/pattern" when pane find is active.
/// - Updates `app.recent_rect` with inner rectangle coordinates (excluding borders).
pub fn render_recent(f: &mut Frame, app: &mut AppState, area: Rect) {
    if !app.show_recent_pane || area.width == 0 {
        app.recent_rect = None;
        return;
    }

    let th = theme();
    let recent_focused = matches!(app.focus, crate::state::Focus::Recent);
    let rec_inds = crate::ui::helpers::filtered_recent_indices(app);
    let rec_items: Vec<ListItem> = rec_inds
        .iter()
        .filter_map(|&i| app.recent.get(i))
        .map(|s| {
            ListItem::new(Span::styled(
                s.clone(),
                Style::default().fg(if recent_focused { th.text } else { th.subtext0 }),
            ))
        })
        .collect();
    let recent_title = if recent_focused {
        i18n::t(app, "app.titles.recent_focused")
    } else {
        i18n::t(app, "app.titles.recent")
    };
    let mut recent_title_spans: Vec<Span> = vec![Span::styled(
        recent_title,
        Style::default().fg(if recent_focused {
            th.mauve
        } else {
            th.overlay1
        }),
    )];
    if recent_focused && let Some(pat) = &app.pane_find {
        recent_title_spans.push(Span::raw("  "));
        recent_title_spans.push(Span::styled(
            "/",
            Style::default()
                .fg(th.sapphire)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ));
        recent_title_spans.push(Span::styled(pat.clone(), Style::default().fg(th.text)));
    }
    let rec_block = Block::default()
        .title(Line::from(recent_title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if recent_focused {
            th.mauve
        } else {
            th.surface1
        }));
    let rec_list = List::new(rec_items)
        .style(
            Style::default()
                .fg(if recent_focused { th.text } else { th.subtext0 })
                .bg(th.base),
        )
        .block(rec_block)
        .highlight_style(Style::default().fg(th.text).bg(th.surface2))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(rec_list, area, &mut app.history_state);
    // Record inner Recent rect for mouse hit-testing (inside borders)
    app.recent_rect = Some((
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// What: Initialize minimal English translations for recent tests.
    ///
    /// Inputs:
    /// - `app`: `AppState` to populate with translations
    ///
    /// Output:
    /// - Populates `app.translations` and `app.translations_fallback` with recent-related translations
    ///
    /// Details:
    /// - Sets up only the translations needed for recent rendering tests.
    fn init_test_translations(app: &mut crate::state::AppState) {
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert("app.titles.recent".to_string(), "Recent".to_string());
        translations.insert(
            "app.titles.recent_focused".to_string(),
            "Recent".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    /// What: Verify recent pane renders and records rect when visible.
    ///
    /// Inputs:
    /// - Recent pane is visible with some recent searches
    ///
    /// Output:
    /// - Recent pane renders and `app.recent_rect` is set to inner rectangle.
    ///
    /// Details:
    /// - Tests that rect is recorded with borders excluded (x+1, y+1, width-2, height-2).
    #[test]
    fn recent_renders_and_records_rect_when_visible() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        app.show_recent_pane = true;
        app.recent.push("package1".to_string());
        app.recent.push("package2".to_string());

        term.draw(|f| {
            let area = f.area();
            render_recent(f, &mut app, area);
        })
        .expect("Failed to render recent pane");

        assert!(app.recent_rect.is_some());
        let (x, y, w, h) = app
            .recent_rect
            .expect("recent_rect should be Some after is_some() check");
        // Rect should exclude borders
        assert_eq!(x, 1);
        assert_eq!(y, 1);
        assert_eq!(w, 98); // 100 - 2
        assert_eq!(h, 28); // 30 - 2
    }

    /// What: Verify recent pane does not render and clears rect when hidden.
    ///
    /// Inputs:
    /// - Recent pane is hidden (`show_recent_pane = false`)
    ///
    /// Output:
    /// - `app.recent_rect` is set to `None`.
    ///
    /// Details:
    /// - Tests that hidden panes don't record rects.
    #[test]
    fn recent_clears_rect_when_hidden() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        app.show_recent_pane = false;
        app.recent_rect = Some((10, 10, 20, 20)); // Set initial value

        term.draw(|f| {
            let area = f.area();
            render_recent(f, &mut app, area);
        })
        .expect("Failed to render hidden recent pane");

        assert!(app.recent_rect.is_none());
    }

    /// What: Verify recent pane clears rect when area has zero width.
    ///
    /// Inputs:
    /// - Recent pane is visible but area has width 0
    ///
    /// Output:
    /// - `app.recent_rect` is set to `None`.
    ///
    /// Details:
    /// - Tests edge case where pane is visible but has no space to render.
    #[test]
    fn recent_clears_rect_when_area_zero_width() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        app.show_recent_pane = true;

        term.draw(|f| {
            let area = ratatui::prelude::Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 10,
            };
            render_recent(f, &mut app, area);
        })
        .expect("Failed to render recent pane with zero width");

        assert!(app.recent_rect.is_none());
    }

    /// What: Verify recent pane displays pane find filter when active and focused.
    ///
    /// Inputs:
    /// - Recent pane is focused with `pane_find` set to "test"
    ///
    /// Output:
    /// - Recent pane renders with "/test" in the title.
    ///
    /// Details:
    /// - Tests that pane find pattern appears in title when pane is focused.
    #[test]
    fn recent_displays_pane_find_when_focused() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        app.show_recent_pane = true;
        app.focus = crate::state::Focus::Recent;
        app.pane_find = Some("test".to_string());

        term.draw(|f| {
            let area = f.area();
            render_recent(f, &mut app, area);
        })
        .expect("Failed to render recent pane with pane find");

        // Should render without panic with pane find in title
        assert!(app.recent_rect.is_some());
    }

    /// What: Verify recent pane does not display pane find when unfocused.
    ///
    /// Inputs:
    /// - Recent pane is not focused but `pane_find` is set
    ///
    /// Output:
    /// - Recent pane renders without pane find in title.
    ///
    /// Details:
    /// - Tests that pane find only appears when pane is focused.
    #[test]
    fn recent_hides_pane_find_when_unfocused() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        app.show_recent_pane = true;
        app.focus = crate::state::Focus::Search;
        app.pane_find = Some("test".to_string());

        term.draw(|f| {
            let area = f.area();
            render_recent(f, &mut app, area);
        })
        .expect("Failed to render unfocused recent pane");

        // Should render without pane find in title
        assert!(app.recent_rect.is_some());
    }
}
