use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::AppState;
use crate::theme::theme;

pub fn render_alert(f: &mut Frame, app: &mut AppState, area: Rect, message: &str) {
    let th = theme();
    // Detect help messages and make them larger
    let is_help = message.contains("Help") || message.contains("Tab Help");
    let w = area
        .width
        .saturating_sub(10)
        .min(if is_help { 90 } else { 80 });
    let h = if is_help {
        area.height.saturating_sub(6).min(28)
    } else {
        7
    };
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    // Choose labels depending on error type (config vs network/other)
    let is_config = message.contains("Unknown key")
        || message.contains("Missing required keys")
        || message.contains("Missing '='")
        || message.contains("Missing key before '='")
        || message.contains("Duplicate key")
        || message.contains("Invalid color")
        || message.to_lowercase().contains("theme configuration");
    let clippy_block = {
        let ml = message.to_lowercase();
        ml.contains("clipboard")
            || ml.contains("wl-copy")
            || ml.contains("xclip")
            || ml.contains("wl-clipboard")
    };
    let header_text = if is_help {
        "Help"
    } else if is_config {
        "Configuration error"
    } else if clippy_block {
        "Clipboard Copy"
    } else {
        "Connection issue"
    };
    let is_clipboard = {
        let ml = message.to_lowercase();
        ml.contains("clipboard")
            || ml.contains("wl-copy")
            || ml.contains("xclip")
            || ml.contains("wl-clipboard")
    };
    let box_title = if is_help {
        " Help "
    } else if is_config {
        " Configuration Error "
    } else if is_clipboard {
        " Clipboard Copy "
    } else {
        " Connection issue "
    };
    let header_color = if is_help || is_config {
        th.mauve
    } else {
        th.red
    };

    // Parse message into lines for help messages
    let mut lines: Vec<Line<'static>> = Vec::new();
    if is_help {
        for line in message.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(th.text),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            header_text,
            Style::default()
                .fg(header_color)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            message.to_string(),
            Style::default().fg(th.text),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Enter or Esc to close",
        Style::default().fg(th.subtext1),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .scroll((if is_help { app.help_scroll } else { 0 }, 0))
        .block(
            Block::default()
                .title(Span::styled(
                    box_title,
                    Style::default()
                        .fg(header_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(header_color))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
