use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::theme::theme;

#[allow(clippy::too_many_arguments)]
pub fn render_post_summary(
    f: &mut Frame,
    area: Rect,
    success: bool,
    changed_files: usize,
    pacnew_count: usize,
    pacsave_count: usize,
    services_pending: &[String],
    snapshot_label: Option<&String>,
) {
    let th = theme();
    let w = area.width.saturating_sub(8).min(96);
    let h = area.height.saturating_sub(6).min(20);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let border_color = if success { th.green } else { th.red };
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        if success { "Success" } else { "Failed" },
        Style::default()
            .fg(border_color)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "Changed files: {} (pacnew: {}, pacsave: {})",
            changed_files, pacnew_count, pacsave_count
        ),
        Style::default().fg(th.text),
    )));
    if let Some(label) = snapshot_label {
        lines.push(Line::from(Span::styled(
            format!("Snapshot: {}", label),
            Style::default().fg(th.text),
        )));
    }
    if !services_pending.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Services pending restart:",
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )));
        for s in services_pending
            .iter()
            .take((h as usize).saturating_sub(10))
        {
            lines.push(Line::from(Span::styled(
                format!("- {}", s),
                Style::default().fg(th.text),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "r: rollback  •  s: restart services  •  Enter/Esc: close",
        Style::default().fg(th.subtext1),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " Post-Transaction Summary ",
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
