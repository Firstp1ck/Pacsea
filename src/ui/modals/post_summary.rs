use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

#[allow(clippy::too_many_arguments)]
/// What: Render the post-transaction summary modal summarizing results and follow-up actions.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `success`: Whether the transaction succeeded
/// - `changed_files`, `pacnew_count`, `pacsave_count`: File change metrics
/// - `services_pending`: Services requiring restart
/// - `snapshot_label`: Optional snapshot identifier
///
/// Output:
/// - Draws the summary dialog highlighting status, file counts, and optional services list.
///
/// Details:
/// - Colors border based on success, truncates service lines to fit, and advertises rollback/service
///   restart shortcuts.
#[allow(clippy::many_single_char_names)]
pub fn render_post_summary(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    success: bool,
    changed_files: usize,
    pacnew_count: usize,
    pacsave_count: usize,
    services_pending: &[String],
    snapshot_label: Option<&String>,
) {
    let th = theme();
    // Calculate required height based on content
    // Structure: success/failed + empty + changed_files + snapshot(optional) + empty + services(optional) + empty + footer_hint
    // Base: success(1) + empty(1) + changed_files(1) + empty(1) + footer_hint(1) = 5
    let base_lines = 5u16;
    let snapshot_lines = u16::from(snapshot_label.is_some());
    let services_lines = if services_pending.is_empty() {
        0u16
    } else {
        // empty + services label + services (max 4 shown) + potentially "and more" line
        let service_count = services_pending.len().min(4);
        u16::try_from(service_count + 2 + usize::from(services_pending.len() > 4)).unwrap_or(7)
    };
    // Total content lines needed (borders take 2 lines, so add 2)
    let required_height = base_lines + snapshot_lines + services_lines + 2;
    let h = area
        .height
        .saturating_sub(4)
        .min(required_height.clamp(9, 18));
    let w = area.width.saturating_sub(8).min(70);
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
        if success {
            i18n::t(app, "app.modals.post_summary.success")
        } else {
            i18n::t(app, "app.modals.post_summary.failed")
        },
        Style::default()
            .fg(border_color)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t_fmt(
            app,
            "app.modals.post_summary.changed_files",
            &[
                &changed_files.to_string(),
                &pacnew_count.to_string(),
                &pacsave_count.to_string(),
            ],
        ),
        Style::default().fg(th.text),
    )));
    if let Some(label) = snapshot_label {
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(app, "app.modals.post_summary.snapshot", label),
            Style::default().fg(th.text),
        )));
    }
    if !services_pending.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.post_summary.services_pending"),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )));
        // Show max 4 services to keep dialog compact
        for s in services_pending.iter().take(4) {
            lines.push(Line::from(Span::styled(
                format!("  • {s}"),
                Style::default().fg(th.text),
            )));
        }
        if services_pending.len() > 4 {
            let remaining = services_pending.len() - 4;
            lines.push(Line::from(Span::styled(
                format!("  ... and {remaining} more"),
                Style::default().fg(th.subtext1),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.post_summary.footer_hint"),
        Style::default()
            .fg(th.subtext1)
            .add_modifier(Modifier::BOLD),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" {} ", i18n::t(app, "app.modals.post_summary.title")),
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
