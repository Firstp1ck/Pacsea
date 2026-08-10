//! Native keyboard-first Pi Scan workspace rendering.

mod details;
mod overview;
mod progress;
mod results;
mod setup;
mod targets;
mod wizard;

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
    let wizard_active = app.pi_scan.wizard.is_some();
    let notice = if wizard_active {
        crate::i18n::t(app, "app.pi_scan.wizard.footer.notice")
    } else {
        app.pi_scan
            .notice
            .clone()
            .unwrap_or_else(|| crate::i18n::t(app, "app.pi_scan.footer.notice"))
    };
    let keys = crate::i18n::t(
        app,
        if wizard_active {
            "app.pi_scan.wizard.footer.keys"
        } else {
            "app.pi_scan.footer.keys"
        },
    );
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

/// What: Format a token count with grouped decimal digits.
///
/// Inputs:
/// - `value`: Exact non-negative token count.
///
/// Output:
/// - Locale-neutral grouped digits suitable for the compact terminal UI.
///
/// Details:
/// - Uses commas consistently because the surrounding translated label carries the locale context.
pub(super) fn format_token_count(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// What: Format an exact integer micro-USD amount as readable USD.
///
/// Inputs:
/// - `value`: Exact amount in millionths of one US dollar.
///
/// Output:
/// - Dollar text with two to six fractional digits and an explicit USD suffix.
///
/// Details:
/// - Integer arithmetic preserves micro-dollar precision and avoids floating-point rounding.
pub(super) fn format_microusd(value: u64) -> String {
    let dollars = value / 1_000_000;
    let mut fraction = format!("{:06}", value % 1_000_000);
    while fraction.len() > 2 && fraction.ends_with('0') {
        fraction.pop();
    }
    format!("${dollars}.{fraction} USD")
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

#[cfg(test)]
mod tests {
    use super::{format_microusd, format_token_count};

    /// Token counts remain readable at zero and across grouping boundaries.
    #[test]
    fn token_count_uses_decimal_grouping() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(999), "999");
        assert_eq!(format_token_count(1_000), "1,000");
        assert_eq!(format_token_count(500_000), "500,000");
        assert_eq!(format_token_count(u64::MAX), "18,446,744,073,709,551,615");
    }

    /// Micro-dollar formatting stays exact while omitting meaningless trailing zeros.
    #[test]
    fn microusd_is_displayed_as_exact_usd() {
        assert_eq!(format_microusd(0), "$0.00 USD");
        assert_eq!(format_microusd(125), "$0.000125 USD");
        assert_eq!(format_microusd(600_000), "$0.60 USD");
        assert_eq!(format_microusd(25_000_000), "$25.00 USD");
    }
}
