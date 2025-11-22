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

/// What: Render the updates available button at the top of the window.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state containing updates count and loading state
/// - `area`: Target rectangle for the updates button (should be 1 line high)
///
/// Output:
/// - Draws the updates button and records clickable rectangle in `app.updates_button_rect`
///
/// Details:
/// - Shows "Updates available (X)" if count > 0, "No updates available" if count is 0,
///   or "Checking updates..." if still loading
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
        spans.push(Span::styled(button_text.clone(), button_style));
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
    let button_width = button_text.width() as u16;
    let button_x = area
        .x
        .saturating_add(area.width.saturating_sub(button_width) / 2);
    app.updates_button_rect = Some((button_x, area.y, button_width, area.height));
}
