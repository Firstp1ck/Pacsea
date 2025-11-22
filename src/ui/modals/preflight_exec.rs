use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::modal::PreflightHeaderChips;
use crate::state::{PackageItem, PreflightAction, PreflightTab};
use crate::theme::theme;
use crate::ui::helpers::{format_bytes, format_signed_bytes};

/// What: Calculate modal layout dimensions and split into sidebar and log columns.
///
/// Inputs:
/// - `area`: Full screen area used to center the modal
/// - `f`: Frame to render into (for clearing the modal area)
///
/// Output:
/// - Returns tuple of (`modal_rect`, `inner_rect`, `column_rects`) where `column_rects[0]` is sidebar and `column_rects[1]` is log panel.
///
/// Details:
/// - Centers modal with max width of 110, calculates inner area with 1px border, and splits into 30%/70% columns.
#[allow(clippy::many_single_char_names)]
fn calculate_modal_layout(area: Rect, f: &mut Frame) -> (Rect, Rect, Vec<Rect>) {
    let w = area.width.saturating_sub(4).min(110);
    let h = area.height.saturating_sub(4).min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let inner = Rect {
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

    (rect, inner, cols.to_vec())
}

/// What: Render tab header line showing available tabs with current tab highlighted.
///
/// Inputs:
/// - `tab`: Currently focused sidebar tab
///
/// Output:
/// - Returns a styled `Line` containing tab labels with the active tab in brackets.
///
/// Details:
/// - Displays all tabs (Summary, Deps, Files, Services, Sandbox) with the active tab wrapped in brackets.
fn render_tab_header(tab: PreflightTab) -> Line<'static> {
    let th = theme();
    let tab_labels = ["Summary", "Deps", "Files", "Services", "Sandbox"];
    let mut header = String::new();

    for (i, lbl) in tab_labels.iter().enumerate() {
        let is_active = matches!(
            (i, tab),
            (0, PreflightTab::Summary)
                | (1, PreflightTab::Deps)
                | (2, PreflightTab::Files)
                | (3, PreflightTab::Services)
                | (4, PreflightTab::Sandbox)
        );
        if i > 0 {
            header.push_str("  ");
        }
        if is_active {
            header.push('[');
            header.push_str(lbl);
            header.push(']');
        } else {
            header.push_str(lbl);
        }
    }

    Line::from(Span::styled(
        header,
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    ))
}

/// What: Format log panel footer with current state indicators.
///
/// Inputs:
/// - `verbose`: Whether verbose logging is enabled
/// - `abortable`: Whether abort is currently available
///
/// Output:
/// - Returns a formatted string describing current controls and their states.
///
/// Details:
/// - Shows verbose toggle state and abort availability status.
fn format_log_footer(verbose: bool, abortable: bool) -> String {
    format!(
        "l: verbose={}  •  x: abort{}  •  q/Esc/Enter: close",
        if verbose { "ON" } else { "OFF" },
        if abortable { " (available)" } else { "" }
    )
}

/// What: Render sidebar widget showing plan summary with header chips, tab header, and package list.
///
/// Inputs:
/// - `items`: Packages involved in the action
/// - `tab`: Currently focused sidebar tab
/// - `header_chips`: Header chip metrics to display
/// - `border_color`: Color for sidebar border
/// - `bg_color`: Background color for sidebar
///
/// Output:
/// - Returns a `Paragraph` widget ready to render.
///
/// Details:
/// - Displays header chips, tab navigation, and up to 10 package names in a bordered block.
fn render_sidebar(
    items: &[PackageItem],
    tab: PreflightTab,
    header_chips: &PreflightHeaderChips,
    border_color: ratatui::style::Color,
    bg_color: ratatui::style::Color,
) -> Paragraph<'static> {
    let th = theme();
    let mut s_lines = vec![
        render_header_chips(header_chips),
        Line::from(""),
        render_tab_header(tab),
        Line::from(""),
    ];

    // Package list
    for p in items.iter().take(10) {
        let p_name = &p.name;
        s_lines.push(Line::from(Span::styled(
            format!("- {p_name}"),
            Style::default().fg(th.text),
        )));
    }

    Paragraph::new(s_lines)
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
        )
}

/// What: Render log panel widget showing execution logs with footer.
///
/// Inputs:
/// - `log_lines`: Buffered log output
/// - `verbose`: Whether verbose logging is enabled
/// - `abortable`: Whether abort is currently available
/// - `title`: Title for the log panel block
/// - `border_color`: Color for log panel border
/// - `log_area_height`: Height of the log area in characters
///
/// Output:
/// - Returns a `Paragraph` widget ready to render.
///
/// Details:
/// - Shows placeholder message if no logs, otherwise displays recent log lines capped to viewport height, plus footer.
fn render_log_panel(
    log_lines: &[String],
    verbose: bool,
    abortable: bool,
    title: String,
    border_color: ratatui::style::Color,
    log_area_height: u16,
) -> Paragraph<'static> {
    let th = theme();
    let mut log_text = if log_lines.is_empty() {
        vec![Line::from(Span::styled(
            "Starting… (placeholder; real logs will stream here)",
            Style::default().fg(th.subtext1),
        ))]
    } else {
        log_lines
            .iter()
            .rev()
            .take(log_area_height as usize - 2)
            .rev()
            .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(th.text))))
            .collect()
    };

    let footer = format_log_footer(verbose, abortable);
    log_text.push(Line::from(""));
    log_text.push(Line::from(Span::styled(
        footer,
        Style::default().fg(th.subtext1),
    )));

    Paragraph::new(log_text)
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
        )
}

/// What: Render header chips as a compact horizontal line of metrics.
///
/// Inputs:
/// - `chips`: Header chip data containing counts and sizes.
///
/// Output:
/// - Returns a `Line` containing styled chip spans separated by spaces.
fn render_header_chips(chips: &PreflightHeaderChips) -> Line<'static> {
    let th = theme();
    let mut spans = Vec::new();

    // Package count chip
    let package_count = chips.package_count;
    let aur_count = chips.aur_count;
    let pkg_text = if aur_count > 0 {
        format!("{package_count} ({aur_count} AUR)")
    } else {
        format!("{package_count}")
    };
    spans.push(Span::styled(
        format!("[{pkg_text}]"),
        Style::default()
            .fg(th.sapphire)
            .add_modifier(Modifier::BOLD),
    ));

    // Download size chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    spans.push(Span::styled(
        format!("[DL: {}]", format_bytes(chips.download_bytes)),
        Style::default().fg(th.sapphire),
    ));

    // Install delta chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    let delta_color = if chips.install_delta_bytes > 0 {
        th.green
    } else if chips.install_delta_bytes < 0 {
        th.red
    } else {
        th.overlay1 // Neutral color for zero
    };
    spans.push(Span::styled(
        format!("[Size: {}]", format_signed_bytes(chips.install_delta_bytes)),
        Style::default().fg(delta_color),
    ));

    // Risk score chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    let risk_label = match chips.risk_level {
        crate::state::modal::RiskLevel::Low => "Low",
        crate::state::modal::RiskLevel::Medium => "Medium",
        crate::state::modal::RiskLevel::High => "High",
    };
    let risk_color = match chips.risk_level {
        crate::state::modal::RiskLevel::Low => th.green,
        crate::state::modal::RiskLevel::Medium => th.yellow,
        crate::state::modal::RiskLevel::High => th.red,
    };
    spans.push(Span::styled(
        format!("[Risk: {} ({})]", risk_label, chips.risk_score),
        Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
    ));

    Line::from(spans)
}

#[allow(clippy::too_many_arguments)]
/// What: Render the preflight execution modal showing plan summary and live logs.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full screen area used to center the modal
/// - `items`: Packages involved in the action
/// - `action`: Install or remove action being executed
/// - `tab`: Currently focused sidebar tab
/// - `verbose`: Whether verbose logging is enabled
/// - `log_lines`: Buffered log output
/// - `abortable`: Whether abort is currently available
/// - `header_chips`: Header chip metrics to display in sidebar
///
/// Output:
/// - Draws sidebar summary plus log panel, reflecting controls for verbosity and aborting.
///
/// Details:
/// - Splits the modal into sidebar/log columns, caps displayed log lines to viewport, and appends
///   footer instructions with dynamic state indicators.
pub fn render_preflight_exec(
    f: &mut Frame,
    app: &crate::state::AppState,
    area: Rect,
    items: &[PackageItem],
    action: PreflightAction,
    tab: PreflightTab,
    verbose: bool,
    log_lines: &[String],
    abortable: bool,
    header_chips: &PreflightHeaderChips,
) {
    let th = theme();
    let (_rect, _inner, cols) = calculate_modal_layout(area, f);

    let border_color = th.lavender;
    let bg_color = th.crust;
    let title = match action {
        PreflightAction::Install => crate::i18n::t(app, "app.modals.preflight_exec.title_install"),
        PreflightAction::Remove => crate::i18n::t(app, "app.modals.preflight_exec.title_remove"),
    };

    let sidebar = render_sidebar(items, tab, header_chips, border_color, bg_color);
    f.render_widget(sidebar, cols[0]);

    let log_panel = render_log_panel(
        log_lines,
        verbose,
        abortable,
        title,
        border_color,
        cols[1].height,
    );
    f.render_widget(log_panel, cols[1]);
}
