use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, SortMode};
use crate::theme::theme;

/// What: Render the sort dropdown overlay near the Sort button.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (uses `sort_menu_open`, updates rect)
/// - `area`: Target rectangle for the results block
/// - `btn_x`: X coordinate of the Sort button
///
/// Output:
/// - Draws the dropdown when open and records its inner hit-test rectangle.
///
/// Details:
/// - Highlights the active sort mode, clamps placement within `area`, and clears the region before
///   drawing to avoid overlapping artifacts.
pub fn render_sort_menu(f: &mut Frame, app: &mut AppState, area: Rect, btn_x: u16) {
    let th = theme();

    app.sort_menu_rect = None;
    if app.sort_menu_open {
        let opts: Vec<String> = vec![
            i18n::t(app, "app.results.sort_menu.options.alphabetical"),
            i18n::t(app, "app.results.sort_menu.options.aur_popularity"),
            i18n::t(app, "app.results.sort_menu.options.best_matches"),
        ];
        let widest = opts
            .iter()
            .map(|s| u16::try_from(s.len()).map_or(u16::MAX, |x| x))
            .max()
            .unwrap_or(0);
        let w = widest.saturating_add(2).min(area.width.saturating_sub(2));
        // Place menu just under the title, aligned to button if possible
        let rect_w = w.saturating_add(2);
        let max_x = area.x + area.width.saturating_sub(rect_w);
        let menu_x = btn_x.min(max_x);
        let menu_y = area.y.saturating_add(1); // just below top border
        let h = u16::try_from(opts.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2); // borders
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.sort_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        // Build lines with current mode highlighted
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let is_selected = matches!(
                (i, app.sort_mode),
                (0, SortMode::RepoThenName)
                    | (1, SortMode::AurPopularityThenOfficial)
                    | (2, SortMode::BestMatches)
            );
            let mark = if is_selected { "✔ " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(th.crust)
                    .bg(th.lavender)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(th.text)
            };
            lines.push(Line::from(vec![
                Span::styled(mark.to_string(), Style::default().fg(th.overlay1)),
                Span::styled(text.clone(), style),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(" {} ", i18n::t(app, "app.results.sort_menu.title")),
                        Style::default().fg(th.overlay1),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.surface2)),
            );
        f.render_widget(Clear, rect);
        f.render_widget(menu, rect);
    }
}
