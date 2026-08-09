//! Native keyboard-first Pi Scan workspace rendering.

mod details;
mod overview;
mod progress;
mod results;
mod setup;
mod targets;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::state::{AppState, PiScanAvailability, PiScanView};
use crate::theme::theme;

/// Render the complete Pi Scan workspace at any terminal size.
pub fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);
    render_tabs(f, app, chunks[0]);
    match app.pi_scan.view {
        PiScanView::Setup => setup::render(f, app, chunks[1]),
        PiScanView::Overview => overview::render(f, app, chunks[1]),
        PiScanView::Targets => targets::render(f, app, chunks[1]),
        PiScanView::Progress => progress::render(f, app, chunks[1]),
        PiScanView::Results => results::render(f, app, chunks[1]),
        PiScanView::Details => details::render(f, app, chunks[1]),
    }
    render_footer(f, app, chunks[2]);
}

/// Draw tabs and record mouse hit rectangles.
fn render_tabs(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let mut spans = vec![Span::styled(
        format!(" {}  ", crate::i18n::t(app, "app.pi_scan.title")),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )];
    let labels = [
        "setup", "overview", "targets", "progress", "results", "details",
    ];
    let mut x = area
        .x
        .saturating_add(u16::try_from(spans[0].content.chars().count()).unwrap_or(0));
    for (index, label) in labels.iter().enumerate() {
        let text = format!(
            " {} ",
            crate::i18n::t(app, &format!("app.pi_scan.tabs.{label}"))
        );
        let width = u16::try_from(text.chars().count()).unwrap_or(0);
        app.pi_scan.tab_rects[index] =
            (x < area.x.saturating_add(area.width)).then_some((x, area.y, width, 1));
        let style = if app.pi_scan.view.index() == index {
            Style::default()
                .fg(th.base)
                .bg(th.sapphire)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.subtext1)
        };
        spans.push(Span::styled(text, style));
        x = x.saturating_add(width);
    }
    let status = match app.pi_scan.availability {
        PiScanAvailability::Disabled => crate::i18n::t(app, "app.pi_scan.status.disabled"),
        PiScanAvailability::Unsupported => crate::i18n::t(app, "app.pi_scan.status.unsupported"),
        PiScanAvailability::MissingBinary => crate::i18n::t(app, "app.pi_scan.status.missing_pi"),
        PiScanAvailability::RuntimeDisconnected => {
            crate::i18n::t(app, "app.pi_scan.status.disconnected")
        }
        PiScanAvailability::RuntimeConnected => crate::i18n::t(app, "app.pi_scan.status.connected"),
    };
    spans.push(Span::styled(
        format!("  {status}"),
        Style::default().fg(th.yellow),
    ));
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

/// Draw persistent notices and the workspace key footer.
fn render_footer(f: &mut Frame, app: &AppState, area: Rect) {
    let th = theme();
    let notice = app
        .pi_scan
        .notice
        .clone()
        .unwrap_or_else(|| crate::i18n::t(app, "app.pi_scan.footer.notice"));
    let keys = crate::i18n::t(app, "app.pi_scan.footer.keys");
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(notice, Style::default().fg(th.yellow))),
            Line::from(keys),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP)
                .title(crate::i18n::t(app, "app.pi_scan.footer.title")),
        ),
        area,
    );
}

/// Render a titled body paragraph shared by workspace pages.
pub(super) fn body(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    title_key: &str,
    lines: Vec<Line<'static>>,
) {
    let th = theme();
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                crate::i18n::t(app, title_key),
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            ))),
        area,
    );
}
