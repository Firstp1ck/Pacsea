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
    app: &AppState,
    area: Rect,
    purpose: PasswordPurpose,
    items: &[PackageItem],
    input: &str,
    error: Option<&str>,
) {
    let th = theme();
    // Calculate required height based on content
    let base_height = 8u16; // heading + empty + password label + input + empty + error space + empty + instructions
    let package_lines = if items.is_empty() {
        0u16
    } else {
        // package label + packages (max 4 shown) + empty
        u16::try_from(items.len().min(4) + 2).unwrap_or(6)
    };
    let error_lines = if error.is_some() { 2u16 } else { 0u16 };
    let required_height = base_height + package_lines + error_lines;
    let h = area
        .height
        .saturating_sub(6)
        .min(required_height.clamp(8, 14));
    let w = area.width.saturating_sub(6).min(65);
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
        PasswordPurpose::Install => {
            crate::i18n::t(app, "app.modals.password_prompt.heading_install")
        }
        PasswordPurpose::Remove => crate::i18n::t(app, "app.modals.password_prompt.heading_remove"),
        PasswordPurpose::Update => crate::i18n::t(app, "app.modals.password_prompt.heading_update"),
        PasswordPurpose::Downgrade => {
            crate::i18n::t(app, "app.modals.password_prompt.heading_downgrade")
        }
        PasswordPurpose::FileSync => {
            crate::i18n::t(app, "app.modals.password_prompt.heading_file_sync")
        }
    };
    lines.push(Line::from(Span::styled(
        heading,
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(""));

    // Show package list if available
    if !items.is_empty() {
        let package_label = if items.len() == 1 {
            crate::i18n::t(app, "app.modals.password_prompt.package_label_singular")
        } else {
            crate::i18n::t(app, "app.modals.password_prompt.package_label_plural")
        };
        lines.push(Line::from(Span::styled(
            package_label,
            Style::default().fg(th.subtext1),
        )));
        // Show max 4 packages to keep dialog compact
        for p in items.iter().take(4) {
            let p_name = &p.name;
            lines.push(Line::from(Span::styled(
                format!("  • {p_name}"),
                Style::default().fg(th.text),
            )));
        }
        if items.len() > 4 {
            let remaining = items.len() - 4;
            lines.push(Line::from(Span::styled(
                crate::i18n::t_fmt1(app, "app.modals.password_prompt.and_more", remaining),
                Style::default().fg(th.subtext1),
            )));
        }
        lines.push(Line::from(""));
    }

    // Password input field (masked)
    let masked_input = "*".repeat(input.len());
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.password_prompt.password_label"),
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(Span::styled(
        format!("  {masked_input}_"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Error message if present
    if let Some(err) = error {
        lines.push(Line::from(Span::styled(
            format!("⚠ {err}"),
            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    // Instructions
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.common.close_hint"),
        Style::default().fg(th.overlay1),
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
