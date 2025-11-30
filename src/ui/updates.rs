use ratatui::{
    Frame,
    prelude::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

/// What: Render the updates available button at the top of the window and lockout status on the right.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state containing updates count, loading state, and faillock status
/// - `area`: Target rectangle for the updates button (should be 1 line high)
///
/// Output:
/// - Draws the updates button and lockout status, records clickable rectangle in `app.updates_button_rect`
///
/// Details:
/// - Shows "Updates available (X)" if count > 0, "No updates available" if count is 0,
///   or "Checking updates..." if still loading
/// - Shows lockout status on the right if user is locked out
/// - Button is styled similar to other buttons in the UI
/// - Records clickable rectangle for mouse interaction
pub fn render_updates_button(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    // Determine button text based on state
    let button_text = if app.updates_loading {
        i18n::t(app, "app.updates_button.loading")
    } else if let Some(count) = app.updates_count {
        if count > 0 {
            i18n::t_fmt1(app, "app.updates_button.available", count)
        } else {
            i18n::t(app, "app.updates_button.none")
        }
    } else {
        i18n::t(app, "app.updates_button.none")
    };

    // Check if lockout status should be displayed
    let lockout_text = if app.faillock_locked {
        if let Some(remaining) = app.faillock_remaining_minutes {
            if remaining > 0 {
                Some(crate::i18n::t_fmt1(
                    app,
                    "app.updates_button.locked_with_time",
                    remaining,
                ))
            } else {
                Some(crate::i18n::t(app, "app.updates_button.locked"))
            }
        } else {
            Some(crate::i18n::t(app, "app.updates_button.locked"))
        }
    } else {
        None
    };

    // Split area if lockout status is shown
    if let Some(lockout) = &lockout_text {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(0),
                ratatui::layout::Constraint::Length(
                    u16::try_from(lockout.width())
                        .unwrap_or(20)
                        .min(area.width.saturating_sub(10)),
                ),
            ])
            .split(area);

        // Render updates button in left area
        render_updates_button_inner(f, app, chunks[0], &button_text, &th);

        // Render lockout status in right area
        let lockout_style = Style::default()
            .fg(th.red)
            .bg(th.base)
            .add_modifier(Modifier::BOLD);
        let lockout_line = Line::from(Span::styled(lockout.clone(), lockout_style));
        let lockout_paragraph = Paragraph::new(lockout_line)
            .alignment(Alignment::Right)
            .block(
                Block::default()
                    .borders(ratatui::widgets::Borders::NONE)
                    .style(Style::default().bg(th.base)),
            );
        f.render_widget(lockout_paragraph, chunks[1]);
    } else {
        // Render updates button only (centered)
        render_updates_button_inner(f, app, area, &button_text, &th);
    }
}

/// What: Render the updates button inner content.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Target rectangle
/// - `button_text`: Button text to display
/// - `th`: Theme
///
/// Output:
/// - Draws the updates button and records clickable rectangle
fn render_updates_button_inner(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    button_text: &str,
    th: &crate::theme::Theme,
) {
    // Style the button (similar to other buttons)
    let button_style = Style::default()
        .fg(th.mauve)
        .bg(th.surface2)
        .add_modifier(Modifier::BOLD);

    // Create button with underlined first character
    let mut spans = Vec::new();
    if let Some(first) = button_text.chars().next() {
        let rest = &button_text[first.len_utf8()..];
        spans.push(Span::styled(
            first.to_string(),
            button_style.add_modifier(Modifier::UNDERLINED),
        ));
        spans.push(Span::styled(rest.to_string(), button_style));
    } else {
        spans.push(Span::styled(button_text.to_string(), button_style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center).block(
        Block::default()
            .borders(ratatui::widgets::Borders::NONE)
            .style(Style::default().bg(th.base)),
    );

    // Render the button
    f.render_widget(paragraph, area);

    // Calculate clickable rectangle: only the button text width, centered
    // Use Unicode display width, not byte length, to handle wide characters
    let button_width = u16::try_from(button_text.width()).unwrap_or(u16::MAX);
    let button_x = area
        .x
        .saturating_add(area.width.saturating_sub(button_width) / 2);
    app.updates_button_rect = Some((button_x, area.y, button_width, area.height));
}
