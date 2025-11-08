use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::theme::theme;

/// Render a centered, simple list modal with a title and provided content lines.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `box_title`: Title shown in the modal border
/// - `lines`: Pre-built content lines
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
