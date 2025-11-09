use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::{PackageItem, PreflightAction, PreflightTab};
use crate::theme::theme;

#[allow(clippy::too_many_arguments)]
/// What: Render the preflight execution modal showing plan summary and live logs.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `items`: Packages involved in the action
/// - `action`: Install or remove action being executed
/// - `tab`: Currently focused sidebar tab
/// - `verbose`: Whether verbose logging is enabled
/// - `log_lines`: Buffered log output
/// - `abortable`: Whether abort is currently available
///
/// Output:
/// - Draws sidebar summary plus log panel, reflecting controls for verbosity and aborting.
///
/// Details:
/// - Splits the modal into sidebar/log columns, caps displayed log lines to viewport, and appends
///   footer instructions with dynamic state indicators.
pub fn render_preflight_exec(
    f: &mut Frame,
    area: Rect,
    items: &[PackageItem],
    action: PreflightAction,
    tab: PreflightTab,
    verbose: bool,
    log_lines: &[String],
    abortable: bool,
) {
    let th = theme();
    let w = area.width.saturating_sub(4).min(110);
    let h = area.height.saturating_sub(4).min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let border_color = th.lavender;
    let bg_color = th.crust;
    let title = match action {
        PreflightAction::Install => " Execute: Install ",
        PreflightAction::Remove => " Execute: Remove ",
    };

    // Split inner content: left (sidebar) 30%, right (log) 70%
    let inner = ratatui::prelude::Rect {
        x: rect.x + 1,
        y: rect.y + 1,
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    };
    let cols = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Percentage(30),
            ratatui::layout::Constraint::Percentage(70),
        ])
        .split(inner);

    // Sidebar: show selected tab header and items
    let mut s_lines: Vec<Line<'static>> = Vec::new();
    let tab_labels = ["Summary", "Deps", "Files", "Services", "Sandbox"];
    let mut header = String::new();
    let current_tab = tab;
    for (i, lbl) in tab_labels.iter().enumerate() {
        let is = matches!(
            (i, current_tab),
            (0, PreflightTab::Summary)
                | (1, PreflightTab::Deps)
                | (2, PreflightTab::Files)
                | (3, PreflightTab::Services)
                | (4, PreflightTab::Sandbox)
        );
        if i > 0 {
            header.push_str("  ");
        }
        if is {
            header.push('[');
            header.push_str(lbl);
            header.push(']');
        } else {
            header.push_str(lbl);
        }
    }
    s_lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    s_lines.push(Line::from(""));
    for p in items.iter().take(12) {
        s_lines.push(Line::from(Span::styled(
            format!("- {}", p.name),
            Style::default().fg(th.text),
        )));
    }
    let sidebar = Paragraph::new(s_lines)
        .style(Style::default().fg(th.text).bg(bg_color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " Plan ",
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(bg_color)),
        );
    f.render_widget(sidebar, cols[0]);

    // Log panel
    let mut log_text: Vec<Line<'static>> = Vec::new();
    if log_lines.is_empty() {
        log_text.push(Line::from(Span::styled(
            "Starting… (placeholder; real logs will stream here)",
            Style::default().fg(th.subtext1),
        )));
    } else {
        for l in log_lines
            .iter()
            .rev()
            .take(cols[1].height as usize - 2)
            .rev()
        {
            log_text.push(Line::from(Span::styled(
                l.clone(),
                Style::default().fg(th.text),
            )));
        }
    }
    log_text.push(Line::from(""));
    let footer = format!(
        "l: verbose={}  •  x: abort{}  •  q/Esc/Enter: close",
        if verbose { "ON" } else { "OFF" },
        if abortable { " (available)" } else { "" }
    );
    log_text.push(Line::from(Span::styled(
        footer,
        Style::default().fg(th.subtext1),
    )));

    let logw = Paragraph::new(log_text)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(th.base)),
        );
    f.render_widget(logw, cols[1]);
}
