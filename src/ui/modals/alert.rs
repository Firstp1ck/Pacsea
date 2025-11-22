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

/// What: Render the alert modal with contextual styling for help/config/network messages.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (help scroll used for large help dialogs)
/// - `area`: Full screen area used to center the modal
/// - `message`: Alert message text to display
///
/// Output:
/// - Draws a centered alert box and adjusts styling/size based on the message content.
///
/// Details:
/// - Detects help/configuration/clipboard keywords to pick header titles, resizes large help
///   dialogs, and instructs users on dismissal while respecting the current theme.
#[allow(clippy::many_single_char_names)]
pub fn render_alert(f: &mut Frame, app: &AppState, area: Rect, message: &str) {
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
    let is_clipboard = {
        let ml = message.to_lowercase();
        ml.contains("clipboard")
            || ml.contains("wl-copy")
            || ml.contains("xclip")
            || ml.contains("wl-clipboard")
    };
    let is_refresh = {
        let ml = message.to_lowercase();
        ml.contains("package database refresh")
    };
    let is_refresh_success = is_refresh && message.contains("✓");
    let header_text = if is_help {
        i18n::t(app, "app.modals.help.heading")
    } else if is_config {
        i18n::t(app, "app.modals.alert.header_configuration_error")
    } else if is_clipboard {
        i18n::t(app, "app.modals.alert.header_clipboard_copy")
    } else if is_refresh {
        i18n::t(app, "app.modals.refresh.header")
    } else {
        i18n::t(app, "app.modals.alert.header_connection_issue")
    };
    let box_title = if is_help {
        format!(" {} ", i18n::t(app, "app.modals.help.title"))
    } else if is_config {
        i18n::t(app, "app.modals.alert.title_configuration_error")
    } else if is_clipboard {
        i18n::t(app, "app.modals.alert.title_clipboard_copy")
    } else if is_refresh {
        i18n::t(app, "app.modals.refresh.title")
    } else {
        i18n::t(app, "app.modals.alert.title_connection_issue")
    };
    let header_color = if is_help || is_config {
        th.mauve
    } else if is_refresh_success {
        th.green
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
        i18n::t(app, "app.modals.common.close_hint"),
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
