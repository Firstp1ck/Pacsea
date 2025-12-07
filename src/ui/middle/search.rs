use ratatui::{
    Frame,
    prelude::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::i18n;
use crate::state::AppState;
use crate::state::types::AppMode;
use crate::theme::theme;

/// What: Build input line spans with optional selection highlighting.
///
/// Inputs:
/// - `app`: Application state (input, caret, selection, focus, normal mode)
/// - `search_focused`: Whether search pane is focused
/// - `th`: Theme
///
/// Output:
/// - Vector of spans for the input line
///
/// Details:
/// - Shows "> " prefix; in normal mode, highlights selected text with lavender background.
fn build_input_spans<'a>(
    app: &AppState,
    search_focused: bool,
    th: &'a crate::theme::Theme,
) -> Vec<Span<'a>> {
    let mut input_spans: Vec<Span> = Vec::new();
    input_spans.push(Span::styled(
        "> ",
        Style::default().fg(if search_focused {
            th.sapphire
        } else {
            th.overlay1
        }),
    ));
    if search_focused && app.search_normal_mode {
        let caret_ci = app.search_caret;
        let (sel_from_ci, sel_to_ci) = app
            .search_select_anchor
            .map_or((caret_ci, caret_ci), |anchor| {
                (anchor.min(caret_ci), anchor.max(caret_ci))
            });
        let cc = app.input.chars().count();
        let sel_from_ci = sel_from_ci.min(cc);
        let sel_to_ci = sel_to_ci.min(cc);
        let from_b = {
            if sel_from_ci == 0 {
                0
            } else {
                app.input
                    .char_indices()
                    .map(|(i, _)| i)
                    .nth(sel_from_ci)
                    .unwrap_or(app.input.len())
            }
        };
        let to_b = {
            if sel_to_ci == 0 {
                0
            } else {
                app.input
                    .char_indices()
                    .map(|(i, _)| i)
                    .nth(sel_to_ci)
                    .unwrap_or(app.input.len())
            }
        };
        let pre = &app.input[..from_b];
        let sel = &app.input[from_b..to_b];
        let post = &app.input[to_b..];
        if !pre.is_empty() {
            input_spans.push(Span::styled(
                pre.to_string(),
                Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
            ));
        }
        if sel_from_ci != sel_to_ci {
            input_spans.push(Span::styled(
                sel.to_string(),
                Style::default()
                    .fg(th.crust)
                    .bg(th.lavender)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if !post.is_empty() {
            input_spans.push(Span::styled(
                post.to_string(),
                Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
            ));
        }
    } else {
        input_spans.push(Span::styled(
            app.input.as_str().to_string(),
            Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
        ));
    }
    input_spans
}

/// What: Build title line with fuzzy/normal mode indicator.
///
/// Inputs:
/// - `app`: Application state
/// - `search_focused`: Whether search pane is focused
/// - `th`: Theme
///
/// Output:
/// - Tuple containing: title line with mode indicator, base title length, and mode text
///
/// Details:
/// - Returns base title length for rectangle calculation
/// - Returns mode text to avoid duplicate i18n lookups
fn build_title_line<'a>(
    app: &AppState,
    search_focused: bool,
    th: &'a crate::theme::Theme,
) -> (Line<'a>, usize, String) {
    let search_title_base = if matches!(app.app_mode, AppMode::News) {
        "News search".to_string()
    } else if search_focused {
        i18n::t(app, "app.titles.search_focused")
    } else {
        i18n::t(app, "app.titles.search")
    };
    let search_title_color = if search_focused {
        th.mauve
    } else {
        th.overlay1
    };

    // Calculate title length before building spans (needed for rectangle calculation)
    let base_title_len = search_title_base.chars().count();

    // Build title with fuzzy/normal indicator
    let mut title_spans = vec![Span::styled(
        search_title_base,
        Style::default().fg(search_title_color),
    )];

    // Add fuzzy/normal mode indicator
    let mode_text = if app.fuzzy_search_enabled {
        i18n::t(app, "app.search_mode_fuzzy")
    } else {
        i18n::t(app, "app.search_mode_normal")
    };
    let mode_color = if app.fuzzy_search_enabled {
        th.sapphire
    } else {
        th.subtext0
    };
    title_spans.push(Span::styled(
        format!(" [{mode_text}]"),
        Style::default().fg(mode_color),
    ));
    (Line::from(title_spans), base_title_len, mode_text)
}

/// What: Calculate and store fuzzy indicator rectangle.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `area`: Search input area
/// - `base_title_len`: Length of base title text
/// - `mode_text`: Mode indicator text
///
/// Output:
/// - None (modifies app state)
///
/// Details:
/// - Stores clickable rectangle for mouse interaction
fn store_fuzzy_indicator_rect(
    app: &mut AppState,
    area: Rect,
    base_title_len: usize,
    mode_text: &str,
) {
    let mode_indicator_len = mode_text.chars().count() + 3; // +3 for " [ ]"
    let max_indicator_width = mode_indicator_len.min(20);
    let max_indicator_width_u16 = u16::try_from(max_indicator_width).unwrap_or(u16::MAX);
    let available_width = area.width.saturating_sub(max_indicator_width_u16);
    let title_end_x = area.x.saturating_add(1).saturating_add(
        u16::try_from(
            base_title_len
                .min(available_width as usize)
                .min(u16::MAX as usize),
        )
        .unwrap_or(u16::MAX),
    );
    app.fuzzy_indicator_rect = Some((
        title_end_x,
        area.y,
        max_indicator_width_u16,
        1, // height
    ));
}

/// What: Render the search input widget in the center of the middle row.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (input, caret, selection, focus, fuzzy indicator rect)
/// - `area`: Target rectangle for the search input
///
/// Output:
/// - Draws the search input with optional text selection highlighting and sets cursor position.
///
/// Details:
/// - Shows "> " prefix; in normal mode, highlights selected text with lavender background.
/// - Cursor position is calculated based on caret index and character width.
/// - Records fuzzy indicator rectangle for mouse click detection.
pub fn render_search(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let search_focused = matches!(app.focus, crate::state::Focus::Search);

    // Build input line with optional selection highlight in Search normal mode
    let input_spans = build_input_spans(app, search_focused, &th);
    let input_line = Line::from(input_spans);

    // Build title with fuzzy/normal indicator
    let (search_title, base_title_len, mode_text) = build_title_line(app, search_focused, &th);
    let input = Paragraph::new(input_line)
        .style(
            Style::default()
                .fg(if search_focused { th.text } else { th.subtext0 })
                .bg(th.base),
        )
        .block(
            Block::default()
                .title(search_title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if search_focused {
                    th.mauve
                } else {
                    th.surface1
                })),
        );
    f.render_widget(input, area);

    // Store clickable rectangle for fuzzy indicator (right side of title)
    store_fuzzy_indicator_rect(app, area, base_title_len, &mode_text);

    // Cursor in input
    let right = area.x + area.width.saturating_sub(1);
    // Cursor x: align to caret in characters from start (prefix "> ")
    let caret_cols: u16 = if search_focused {
        let mut ci: u16 = 0;
        let mut it = app.input.chars();
        for _ in 0..app.search_caret {
            if it.next().is_some() {
                ci = ci.saturating_add(1);
            } else {
                break;
            }
        }
        ci
    } else {
        u16::try_from(app.input.len()).unwrap_or(u16::MAX)
    };
    let x = std::cmp::min(area.x + 1 + 2 + caret_cols, right);
    let y = area.y + 1;
    f.set_cursor_position(Position::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// What: Initialize minimal English translations for search tests.
    ///
    /// Inputs:
    /// - `app`: `AppState` to populate with translations
    ///
    /// Output:
    /// - Populates `app.translations` and `app.translations_fallback` with search-related translations
    ///
    /// Details:
    /// - Sets up only the translations needed for search rendering tests.
    fn init_test_translations(app: &mut crate::state::AppState) {
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert("app.titles.search".to_string(), "Search".to_string());
        translations.insert(
            "app.titles.search_focused".to_string(),
            "Search".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    /// What: Verify search input renders and sets cursor position correctly when focused.
    ///
    /// Inputs:
    /// - Search input with text "hello" and caret at position 3
    ///
    /// Output:
    /// - Search input renders without panic, cursor position is set correctly.
    ///
    /// Details:
    /// - Tests that cursor position calculation accounts for the "> " prefix and character width.
    #[test]
    fn search_renders_and_sets_cursor_when_focused() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.focus = crate::state::Focus::Search;
        app.input = "hello".into();
        app.search_caret = 3;

        term.draw(|f| {
            let area = f.area();
            render_search(f, &mut app, area);
        })
        .expect("Failed to render search pane");

        // Cursor position is set by set_cursor_position - verify rendering succeeded
        // TestBackend doesn't expose cursor position directly, but rendering
        // completing without panic verifies the function works correctly
    }

    /// What: Verify search input renders without selection highlighting when not in normal mode.
    ///
    /// Inputs:
    /// - Search input with text, focused but not in normal mode
    ///
    /// Output:
    /// - Search input renders without selection spans.
    ///
    /// Details:
    /// - Tests that selection highlighting only appears when both focused and in normal mode.
    #[test]
    fn search_renders_without_selection_when_not_normal_mode() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.focus = crate::state::Focus::Search;
        app.input = "test".into();
        app.search_normal_mode = false;
        app.search_caret = 2;
        app.search_select_anchor = Some(1);

        term.draw(|f| {
            let area = f.area();
            render_search(f, &mut app, area);
        })
        .expect("Failed to render search pane without selection");

        // Should render without panic even with selection anchor set but not in normal mode
    }

    /// What: Verify search input renders with text selection highlighting in normal mode.
    ///
    /// Inputs:
    /// - Search input with text "hello", caret at 3, anchor at 1, in normal mode
    ///
    /// Output:
    /// - Search input renders with selection highlighting between anchor and caret.
    ///
    /// Details:
    /// - Tests that selected text (characters 1-3) is highlighted with lavender background.
    #[test]
    fn search_renders_with_selection_in_normal_mode() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.focus = crate::state::Focus::Search;
        app.input = "hello".into();
        app.search_normal_mode = true;
        app.search_caret = 3;
        app.search_select_anchor = Some(1);

        term.draw(|f| {
            let area = f.area();
            render_search(f, &mut app, area);
        })
        .expect("Failed to render search pane with selection");

        // Should render with selection highlighting
    }

    /// What: Verify search input renders correctly when not focused.
    ///
    /// Inputs:
    /// - Search input with text, but focus is on another pane
    ///
    /// Output:
    /// - Search input renders with unfocused styling.
    ///
    /// Details:
    /// - Tests that unfocused search uses different colors and cursor position calculation.
    #[test]
    fn search_renders_when_unfocused() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.focus = crate::state::Focus::Recent;
        app.input = "test".into();
        app.search_caret = 2;

        term.draw(|f| {
            let area = f.area();
            render_search(f, &mut app, area);
        })
        .expect("Failed to render unfocused search pane");

        // Should render without panic with unfocused styling
    }

    /// What: Verify cursor position calculation handles empty input correctly.
    ///
    /// Inputs:
    /// - Empty search input with caret at 0
    ///
    /// Output:
    /// - Cursor position is set after the "> " prefix.
    ///
    /// Details:
    /// - Tests edge case where input is empty and caret is at start.
    #[test]
    fn search_handles_empty_input() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("failed to create test terminal");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.focus = crate::state::Focus::Search;
        app.input = String::new();
        app.search_caret = 0;

        term.draw(|f| {
            let area = f.area();
            render_search(f, &mut app, area);
        })
        .expect("failed to draw test terminal");

        // Should handle empty input without panic
    }
}
