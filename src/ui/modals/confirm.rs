use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::PackageItem;
use crate::theme::theme;

pub fn render_confirm_install(f: &mut Frame, area: Rect, items: &[PackageItem]) {
    let th = theme();
    let w = area.width.saturating_sub(6).min(90);
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
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Confirm installation",
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nothing to install",
            Style::default().fg(th.subtext1),
        )));
    } else {
        for p in items.iter().take((h as usize).saturating_sub(6)) {
            lines.push(Line::from(Span::styled(
                format!("- {}", p.name),
                Style::default().fg(th.text),
            )));
        }
        if items.len() + 6 > h as usize {
            lines.push(Line::from(Span::styled(
                "…",
                Style::default().fg(th.subtext1),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Enter to confirm or Esc to cancel",
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(Span::styled(
        "Press S to scan AUR package(s) before install",
        Style::default().fg(th.overlay1),
    )));
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " Confirm Install ",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

pub fn render_confirm_remove(f: &mut Frame, area: Rect, items: &[PackageItem]) {
    let th = theme();
    let w = area.width.saturating_sub(6).min(90);
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
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Confirm removal",
        Style::default().fg(th.red).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    // Warn explicitly if any core packages are present
    let has_core = items.iter().any(|p| match &p.source {
        crate::state::Source::Official { repo, .. } => repo.eq_ignore_ascii_case("core"),
        _ => false,
    });
    if has_core {
        lines.push(Line::from(Span::styled(
            "WARNING: core packages selected. Removing core packages may break your system.",
            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nothing to remove",
            Style::default().fg(th.subtext1),
        )));
    } else {
        for p in items.iter().take((h as usize).saturating_sub(6)) {
            lines.push(Line::from(Span::styled(
                format!("- {}", p.name),
                Style::default().fg(th.text),
            )));
        }
        if items.len() + 6 > h as usize {
            lines.push(Line::from(Span::styled(
                "…",
                Style::default().fg(th.subtext1),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Enter to confirm or Esc to cancel",
        Style::default().fg(th.subtext1),
    )));
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " Confirm Remove ",
                    Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.red))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
