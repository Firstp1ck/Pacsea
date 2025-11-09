use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::theme::theme;

/// What: Render a centered list modal with a styled title and supplied lines.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `box_title`: Border title to display
/// - `lines`: Fully prepared line content
///
/// Output:
/// - Draws the modal box with the provided lines; does not mutate shared state.
///
/// Details:
/// - Applies consistent theming (double border, mantle background) and ensures the modal fits by
///   clamping width/height within the supplied area.
pub fn render_simple_list_modal(
    f: &mut Frame,
    area: Rect,
    box_title: &str,
    lines: Vec<Line<'static>>,
) {
    let th = theme();
    let w = area.width.saturating_sub(8).min(80);
    let h = area.height.saturating_sub(8).min(20);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(ratatui::text::Span::styled(
                    format!(" {} ", box_title),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
