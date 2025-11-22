use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

/// What: Calculate content layout dimensions from a details area rect.
///
/// Inputs:
/// - `details_area`: Rect assigned to the Package Info pane content
///
/// Output:
/// - Tuple of (`content_x`, `content_y`, `inner_w`) representing layout dimensions.
///
/// Details:
/// - Accounts for border inset and calculates inner content dimensions.
fn calculate_content_layout(details_area: Rect) -> (u16, u16, u16) {
    let border_inset = 1u16;
    let content_x = details_area.x.saturating_add(border_inset);
    let content_y = details_area.y.saturating_add(border_inset);
    let inner_w = details_area.width.saturating_sub(2);
    (content_x, content_y, inner_w)
}

/// What: Style URL text in details lines as a clickable link.
///
/// Inputs:
/// - `details_lines`: Mutable reference to formatted details lines
/// - `url_label`: Localized label for the URL field
/// - `url_text`: The URL text to style
/// - `th`: Theme colors
///
/// Output:
/// - Modifies `details_lines` in place, styling the URL span.
///
/// Details:
/// - Finds the URL line and applies link styling (mauve, underlined, bold) if URL is non-empty.
fn style_url_in_lines(
    details_lines: &mut [Line],
    url_label: &str,
    url_text: &str,
    th: &crate::theme::Theme,
) {
    for line in details_lines.iter_mut() {
        if line.spans.len() >= 2 {
            let key_txt = line.spans[0].content.to_string();
            if key_txt.starts_with(url_label) {
                let style = if url_text.is_empty() {
                    Style::default().fg(th.text)
                } else {
                    Style::default()
                        .fg(th.mauve)
                        .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
                };
                line.spans[1] = ratatui::text::Span::styled(url_text.to_string(), style);
            }
        }
    }
}

/// What: Calculate the number of rows a line occupies when wrapped.
///
/// Inputs:
/// - `line_len`: Total character length of the line
/// - `inner_w`: Available width for wrapping
///
/// Output:
/// - Number of rows (at least 1) the line will occupy.
///
/// Details:
/// - Handles zero-width case and calculates wrapping using `div_ceil`.
fn calculate_wrapped_line_rows(line_len: usize, inner_w: u16) -> u16 {
    if inner_w == 0 {
        1
    } else {
        (line_len as u16).div_ceil(inner_w).max(1)
    }
}

/// What: Calculate the URL button rectangle position.
///
/// Inputs:
/// - `key_txt`: The label text before the URL
/// - `url_txt`: The URL text
/// - `content_x`: X coordinate of content area
/// - `cur_y`: Current Y coordinate
/// - `inner_w`: Available inner width
///
/// Output:
/// - Optional button rectangle (x, y, width, height) if URL is non-empty and fits.
///
/// Details:
/// - Positions button after the label text, constrained to available width.
fn calculate_url_button_rect(
    key_txt: &str,
    url_txt: &str,
    content_x: u16,
    cur_y: u16,
    inner_w: u16,
) -> Option<(u16, u16, u16, u16)> {
    if url_txt.is_empty() {
        return None;
    }
    // Use Unicode display width, not byte length, to handle wide characters
    let key_len = key_txt.width() as u16;
    let x_start = content_x.saturating_add(key_len);
    let max_w = inner_w.saturating_sub(key_len);
    let w = url_txt.width().min(max_w as usize) as u16;
    if w > 0 {
        Some((x_start, cur_y, w, 1))
    } else {
        None
    }
}

/// What: Calculate the PKGBUILD button rectangle position.
///
/// Inputs:
/// - `txt`: The button text
/// - `content_x`: X coordinate of content area
/// - `cur_y`: Current Y coordinate
/// - `inner_w`: Available inner width
///
/// Output:
/// - Optional button rectangle (x, y, width, height) if text fits.
///
/// Details:
/// - Positions button at content start, constrained to available width.
fn calculate_pkgbuild_button_rect(
    txt: &str,
    content_x: u16,
    cur_y: u16,
    inner_w: u16,
) -> Option<(u16, u16, u16, u16)> {
    // Use Unicode display width, not byte length, to handle wide characters
    let w = txt.width().min(inner_w as usize) as u16;
    if w > 0 {
        Some((content_x, cur_y, w, 1))
    } else {
        None
    }
}

/// What: Context for calculating button rectangles.
///
/// Details:
/// - Groups layout and URL parameters for button calculation.
struct ButtonContext<'a> {
    /// X coordinate of content area
    content_x: u16,
    /// Y coordinate of content area start
    content_y: u16,
    /// Available inner width
    inner_w: u16,
    /// Localized label for the URL field
    url_label: &'a str,
    /// The URL text
    url_text: &'a str,
}

/// What: Calculate button rectangles for URL and PKGBUILD based on visible lines.
///
/// Inputs:
/// - `details_lines`: All formatted details lines
/// - `visible_lines`: Only the visible lines after scroll offset
/// - `scroll_offset`: Number of lines skipped from top
/// - `ctx`: Button calculation context (layout and URL info)
/// - `app`: Application state to update button rects
///
/// Output:
/// - Updates `app.url_button_rect` and `app.pkgb_button_rect` with calculated positions.
///
/// Details:
/// - Iterates through visible lines, calculates button positions, and accounts for text wrapping.
fn calculate_button_rects(
    details_lines: &[Line],
    visible_lines: &[Line],
    scroll_offset: usize,
    ctx: ButtonContext<'_>,
    app: &mut AppState,
) {
    app.url_button_rect = None;
    app.pkgb_button_rect = None;

    let show_pkgb = crate::i18n::t(app, "app.details.show_pkgbuild").to_lowercase();
    let hide_pkgb = crate::i18n::t(app, "app.details.hide_pkgbuild").to_lowercase();

    let mut cur_y = ctx.content_y;
    for (vis_idx, vis_line) in visible_lines.iter().enumerate() {
        let line_idx = vis_idx + scroll_offset;
        let original_line = &details_lines[line_idx];

        // Check for URL button
        if original_line.spans.len() >= 2 {
            let key_txt = original_line.spans[0].content.to_string();
            if key_txt.starts_with(ctx.url_label)
                && let Some(rect) = calculate_url_button_rect(
                    &key_txt,
                    ctx.url_text,
                    ctx.content_x,
                    cur_y,
                    ctx.inner_w,
                )
            {
                app.url_button_rect = Some(rect);
            }
        }

        // Check for PKGBUILD button
        if original_line.spans.len() == 1 {
            let txt = original_line.spans[0].content.to_string();
            let lowered = txt.to_lowercase();
            if (lowered.contains(&show_pkgb) || lowered.contains(&hide_pkgb))
                && let Some(rect) =
                    calculate_pkgbuild_button_rect(&txt, ctx.content_x, cur_y, ctx.inner_w)
            {
                app.pkgb_button_rect = Some(rect);
            }
        }

        // Advance y accounting for wrapping
        let line_len: usize = vis_line.spans.iter().map(|s| s.content.len()).sum();
        let rows = calculate_wrapped_line_rows(line_len, ctx.inner_w);
        cur_y = cur_y.saturating_add(rows);
    }
}

/// What: Render the Package Info pane with scroll support and interactive buttons.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (details, scroll offsets, cached rects)
/// - `details_area`: Rect assigned to the Package Info pane content
///
/// Output:
/// - Draws package details and updates mouse hit-test rects for URL/PKGBUILD elements.
///
/// Details:
/// - Applies scroll offsets, styles the URL as a link when present, records button rectangles, and
///   enables text selection by marking `mouse_disabled_in_details`.
pub fn render_package_info(f: &mut Frame, app: &mut AppState, details_area: Rect) {
    let th = theme();
    let url_label = crate::i18n::t(app, "app.details.url_label");
    let url_text = app.details.url.clone();

    let mut details_lines = crate::ui::helpers::format_details_lines(app, details_area.width, &th);

    // Record details inner rect for mouse hit-testing
    app.details_rect = Some((
        details_area.x + 1,
        details_area.y + 1,
        details_area.width.saturating_sub(2),
        details_area.height.saturating_sub(2),
    ));

    // Style URL in lines
    style_url_in_lines(&mut details_lines, &url_label, &url_text, &th);

    // Apply scroll offset by skipping lines from the top
    let scroll_offset = app.details_scroll as usize;
    let visible_lines: Vec<_> = details_lines.iter().skip(scroll_offset).cloned().collect();

    // Calculate layout dimensions
    let (content_x, content_y, inner_w) = calculate_content_layout(details_area);

    // Calculate button positions based on visible lines
    let button_ctx = ButtonContext {
        content_x,
        content_y,
        inner_w,
        url_label: &url_label,
        url_text: &url_text,
    };
    calculate_button_rects(
        &details_lines,
        &visible_lines,
        scroll_offset,
        button_ctx,
        app,
    );

    // Render the widget
    let package_info_title = i18n::t(app, "app.headings.package_info");
    let details_block = Block::default()
        .title(ratatui::text::Span::styled(
            &package_info_title,
            Style::default().fg(th.overlay1),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.surface2));
    let details = Paragraph::new(visible_lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: true })
        .block(details_block);
    f.render_widget(details, details_area);

    // Allow terminal to mark/select text in details: ignore clicks within details by default
    app.mouse_disabled_in_details = true;
}
