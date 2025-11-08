use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::state::AppState;
use crate::theme::theme;

/// Render the Package Info pane with scroll support and clickable URL.
///
/// Updates geometry fields on [`AppState`] for mouse hit-testing:
/// - `details_rect`: Inner content area
/// - `url_button_rect`: Clickable URL area
/// - `pkgb_button_rect`: Show/Hide PKGBUILD button area
/// - `mouse_disabled_in_details`: Set to true to allow text selection
pub fn render_package_info(f: &mut Frame, app: &mut AppState, details_area: Rect) {
    let th = theme();

    let mut details_lines = crate::ui::helpers::format_details_lines(app, details_area.width, &th);
    // Record details inner rect for mouse hit-testing
    app.details_rect = Some((
        details_area.x + 1,
        details_area.y + 1,
        details_area.width.saturating_sub(2),
        details_area.height.saturating_sub(2),
    ));

    // Apply scroll offset by skipping lines from the top
    let scroll_offset = app.details_scroll as usize;
    let visible_lines: Vec<_> = details_lines.iter().skip(scroll_offset).cloned().collect();

    // Find the URL line, style it as a link, and record its rect; also compute PKGBUILD rect
    // Process original lines first to style URL and find buttons
    app.url_button_rect = None;
    app.pkgb_button_rect = None;
    let border_inset = 1u16;
    let content_x = details_area.x.saturating_add(border_inset);
    let content_y = details_area.y.saturating_add(border_inset);
    let inner_w: u16 = details_area.width.saturating_sub(2);

    // Process original lines to style URL
    for line in details_lines.iter_mut() {
        if line.spans.len() >= 2 {
            let key_txt = line.spans[0].content.to_string();
            if key_txt.starts_with("URL:") {
                let url_txt = app.details.url.clone();
                let mut style = Style::default().fg(th.text);
                if !url_txt.is_empty() {
                    style = Style::default()
                        .fg(th.mauve)
                        .add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
                }
                line.spans[1] = ratatui::text::Span::styled(url_txt.clone(), style);
            }
        }
    }

    // Calculate button positions based on visible lines only
    let mut cur_y: u16 = content_y;
    for (vis_idx, vis_line) in visible_lines.iter().enumerate() {
        let line_idx = vis_idx + scroll_offset;
        let original_line = &details_lines[line_idx];

        // Check for URL button
        if original_line.spans.len() >= 2 {
            let key_txt = original_line.spans[0].content.to_string();
            if key_txt.starts_with("URL:") {
                let url_txt = app.details.url.clone();
                if !url_txt.is_empty() {
                    let key_len = key_txt.len() as u16;
                    let x_start = content_x.saturating_add(key_len);
                    let max_w = inner_w.saturating_sub(key_len);
                    let w = url_txt.len().min(max_w as usize) as u16;
                    if w > 0 {
                        app.url_button_rect = Some((x_start, cur_y, w, 1));
                    }
                }
            }
        }

        // Check for PKGBUILD button
        if original_line.spans.len() == 1 {
            let txt = original_line.spans[0].content.to_string();
            let lowered = txt.to_lowercase();
            if lowered.contains("show pkgbuild") || lowered.contains("hide pkgbuild") {
                let x_start = content_x;
                let w = txt.len().min(inner_w as usize) as u16;
                if w > 0 {
                    app.pkgb_button_rect = Some((x_start, cur_y, w, 1));
                }
            }
        }

        // Advance y accounting for wrapping
        let line_len: usize = vis_line.spans.iter().map(|s| s.content.len()).sum();
        let rows = if inner_w == 0 {
            1
        } else {
            (line_len as u16).div_ceil(inner_w).max(1)
        };
        cur_y = cur_y.saturating_add(rows);
    }

    let details_block = Block::default()
        .title(ratatui::text::Span::styled(
            "Package Info",
            Style::default().fg(th.overlay1),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.surface2));
    // Render only visible lines (after scroll offset)
    let details = Paragraph::new(visible_lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: true })
        .block(details_block.clone());
    f.render_widget(details, details_area);

    // Allow terminal to mark/select text in details: ignore clicks within details by default
    app.mouse_disabled_in_details = true;
}
