use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::{AppState, PackageItem, modal::PasswordPurpose};
use crate::theme::theme;

/// What: Render the password prompt modal for sudo authentication.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state for translations
/// - `area`: Full screen area used to center the modal
/// - `purpose`: Purpose of the password prompt
/// - `items`: Packages involved in the operation
/// - `input`: Current password input (masked)
/// - `error`: Optional error message
///
/// Output:
/// - Draws the password prompt dialog with masked input field.
///
/// Details:
/// - Shows purpose-specific message, package list, and masked password input.
/// - Displays error message if password was incorrect.
#[allow(clippy::many_single_char_names)]
pub fn render_password_prompt(
    f: &mut Frame,
    _app: &AppState,
    area: Rect,
    purpose: PasswordPurpose,
    items: &[PackageItem],
    input: &str,
    error: Option<&str>,
) {
    let th = theme();
    let w = area.width.saturating_sub(6).min(90);
    let h = area.height.saturating_sub(6).min(20);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Purpose-specific heading
    let heading = match purpose {
        PasswordPurpose::Install => "Enter sudo password to install packages",
        PasswordPurpose::Remove => "Enter sudo password to remove packages",
        PasswordPurpose::Update => "Enter sudo password to update system",
        PasswordPurpose::Downgrade => "Enter sudo password to downgrade packages",
    };
    lines.push(Line::from(Span::styled(
        heading,
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Show package list if available
    if !items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Packages:",
            Style::default().fg(th.subtext1),
        )));
        for p in items.iter().take((h as usize).saturating_sub(10)) {
            let p_name = &p.name;
            lines.push(Line::from(Span::styled(
                format!("  - {p_name}"),
                Style::default().fg(th.text),
            )));
        }
        if items.len() + 10 > h as usize {
            lines.push(Line::from(Span::styled(
                "  ...",
                Style::default().fg(th.subtext1),
            )));
        }
        lines.push(Line::from(""));
    }

    // Password input field (masked)
    let masked_input = "*".repeat(input.len());
    lines.push(Line::from(Span::styled(
        "Password:",
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(Span::styled(
        format!("  {masked_input}_"),
        Style::default().fg(th.text),
    )));
    lines.push(Line::from(""));

    // Error message if present
    if let Some(err) = error {
        lines.push(Line::from(Span::styled(
            format!("Error: {err}"),
            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    // Instructions
    lines.push(Line::from(Span::styled(
        "Press Enter to confirm, Esc to cancel",
        Style::default().fg(th.subtext1),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    "Password Required",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
