use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

const MODAL_MIN_WIDTH: u16 = 40;
const MODAL_MAX_WIDTH_RATIO: u16 = 3;
const MODAL_MAX_WIDTH_DIVISOR: u16 = 4;
const MODAL_MIN_HEIGHT: u16 = 6;
const MODAL_MAX_HEIGHT: u16 = 25;
const MODAL_HEIGHT_PADDING: u16 = 4;
const BORDER_WIDTH: u16 = 2;
const HEADER_LINES: u16 = 2;
const FOOTER_LINES: u16 = 1;
const TOTAL_HEADER_FOOTER_LINES: u16 = HEADER_LINES + FOOTER_LINES;
const CONTENT_PADDING: u16 = 2; // Left/right padding for content
const CONTENT_FOOTER_BUFFER: u16 = 2; // Buffer lines between content and footer

/// What: Calculate the number of display lines needed for content with wrapping.
///
/// Inputs:
/// - `content`: Markdown content string
/// - `available_width`: Width available for content (inside borders and padding)
///
/// Output:
/// - Number of display lines the content will take after wrapping
///
/// Details:
/// - Calculates how many lines each source line will wrap to
/// - Accounts for word wrapping at the available width
fn calculate_wrapped_lines(content: &str, available_width: u16) -> u16 {
    if content.trim().is_empty() {
        return 1;
    }

    let width = available_width.max(1) as usize;
    let mut total_lines: u16 = 0;

    for line in content.lines() {
        if line.is_empty() {
            total_lines = total_lines.saturating_add(1);
        } else {
            // Calculate wrapped line count for this source line using display width
            let line_width = line.width();
            #[allow(clippy::cast_possible_truncation)]
            let wrapped = line_width.div_ceil(width).max(1).min(u16::MAX as usize) as u16;
            total_lines = total_lines.saturating_add(wrapped);
        }
    }

    total_lines.max(1)
}

/// What: Calculate the maximum width needed for content lines.
///
/// Inputs:
/// - `content`: Markdown content string
/// - `max_width`: Maximum width to consider
///
/// Output:
/// - Maximum line width needed (capped at `max_width`)
///
/// Details:
/// - Finds the longest line in the content
/// - Accounts for markdown formatting that might add characters
fn calculate_content_width(content: &str, max_width: u16) -> u16 {
    let mut max_line_len = 0;
    for line in content.lines() {
        // Remove markdown formatting for width calculation
        let cleaned = line
            .replace("**", "")
            .replace("## ", "")
            .replace("### ", "")
            .replace("# ", "");
        // Use display width instead of byte length for multi-byte UTF-8 characters
        let line_width = cleaned.trim().width();
        #[allow(clippy::cast_possible_truncation)]
        let line_len = line_width.min(u16::MAX as usize) as u16;
        max_line_len = max_line_len.max(line_len);
    }
    max_line_len.min(max_width).max(MODAL_MIN_WIDTH)
}

/// What: Calculate the modal rectangle dimensions and position centered in the given area.
///
/// Inputs:
/// - `area`: Full screen area to center the modal within
/// - `content`: Content string to size the modal for
/// - `app`: Application state for i18n (to get footer text)
///
/// Output:
/// - `Rect` representing the modal's position and size
///
/// Details:
/// - Width: Based on max of content width and footer width, with min/max constraints
/// - Height: Based on content lines + header + footer, with min/max constraints
/// - Modal is centered both horizontally and vertically
fn calculate_modal_rect(area: Rect, content: &str, app: &crate::state::AppState) -> Rect {
    // Calculate max available width first
    let max_available_width = (area.width * MODAL_MAX_WIDTH_RATIO) / MODAL_MAX_WIDTH_DIVISOR;
    let content_width = calculate_content_width(content, max_available_width);

    // Calculate footer text width dynamically using display width
    let footer_text = crate::i18n::t(app, "app.modals.announcement.footer_hint");
    let footer_text_display_width = footer_text.width();
    #[allow(clippy::cast_possible_truncation)]
    let footer_text_width = footer_text_display_width.min(u16::MAX as usize) as u16;
    let footer_width = footer_text_width + CONTENT_PADDING * 2;

    // Calculate modal width (max of content width and footer width, with padding + borders)
    let required_width = content_width.max(footer_width) + CONTENT_PADDING * 2 + BORDER_WIDTH;
    let modal_width = required_width.min(max_available_width).max(MODAL_MIN_WIDTH);

    // Calculate content area width (inside borders and padding)
    let content_area_width = modal_width.saturating_sub(BORDER_WIDTH + CONTENT_PADDING * 2);

    // Calculate wrapped content lines based on available width
    let content_lines = calculate_wrapped_lines(content, content_area_width);

    // Calculate modal height (content + header + footer + buffer + borders)
    let modal_height =
        (content_lines + TOTAL_HEADER_FOOTER_LINES + CONTENT_FOOTER_BUFFER + BORDER_WIDTH)
            .min(area.height.saturating_sub(MODAL_HEIGHT_PADDING))
            .min(MODAL_MAX_HEIGHT)
            .clamp(MODAL_MIN_HEIGHT, MODAL_MAX_HEIGHT);

    // Center the modal
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;

    Rect {
        x,
        y,
        width: modal_width,
        height: modal_height,
    }
}

/// What: Detect URLs in text and return vector of (`start_pos`, `end_pos`, `url_string`).
///
/// Inputs:
/// - `text`: Text to search for URLs.
///
/// Output:
/// - Vector of tuples: (`start_byte_pos`, `end_byte_pos`, `url_string`).
///
/// Details:
/// - Detects http://, https://, and www. URLs.
/// - Returns positions in byte offsets for string slicing.
fn detect_urls(text: &str) -> Vec<(usize, usize, String)> {
    let mut urls = Vec::new();
    let text_bytes = text.as_bytes();
    let mut i = 0;

    while i < text_bytes.len() {
        // Look for http:// or https://
        let is_http = i + 7 < text_bytes.len() && &text_bytes[i..i + 7] == b"http://";
        let is_https = i + 8 < text_bytes.len() && &text_bytes[i..i + 8] == b"https://";

        if is_http || is_https {
            let offset = if is_https { 8 } else { 7 };
            if let Some(end) = find_url_end(text, i + offset) {
                let url = text[i..end].to_string();
                urls.push((i, end, url));
                i = end;
                continue;
            }
        }

        // Look for www. (must be at word boundary)
        if i + 4 < text_bytes.len()
            && (i == 0 || text_bytes[i - 1].is_ascii_whitespace())
            && &text_bytes[i..i + 4] == b"www."
            && let Some(end) = find_url_end(text, i + 4)
        {
            let url = format!("https://{}", &text[i..end]);
            urls.push((i, end, url));
            i = end;
            continue;
        }
        i += 1;
    }

    urls
}

/// What: Find the end position of a URL in text.
///
/// Inputs:
/// - `text`: Text containing the URL.
/// - `start`: Starting byte position of the URL.
///
/// Output:
/// - `Some(usize)` with end byte position, or `None` if URL is invalid.
///
/// Details:
/// - URL ends at whitespace, closing parenthesis, or end of string.
/// - Removes trailing punctuation that's not part of the URL.
fn find_url_end(text: &str, start: usize) -> Option<usize> {
    let mut end = start;
    let text_bytes = text.as_bytes();

    // Find the end of the URL (stop at whitespace or closing paren)
    while end < text_bytes.len() {
        let byte = text_bytes[end];
        if byte.is_ascii_whitespace() || byte == b')' || byte == b']' || byte == b'>' {
            break;
        }
        end += 1;
    }

    // Remove trailing punctuation that's likely not part of the URL
    while end > start {
        let last_char = text_bytes[end - 1];
        if matches!(last_char, b'.' | b',' | b';' | b':' | b'!' | b'?') {
            end -= 1;
        } else {
            break;
        }
    }

    if end > start { Some(end) } else { None }
}

/// What: Parse markdown content into styled lines for display.
///
/// Inputs:
/// - `content`: Markdown content string
/// - `scroll`: Scroll offset in lines
/// - `max_lines`: Maximum number of lines to display
/// - `url_positions`: Mutable vector to track URL positions for click detection
/// - `content_rect`: Rectangle where content is rendered (for position tracking)
/// - `start_y`: Starting Y coordinate for content (after header)
///
/// Output:
/// - Vector of styled lines for rendering
///
/// Details:
/// What: Parse a header line and return styled line.
///
/// Inputs:
/// - `trimmed`: Trimmed line text
///
/// Output:
/// - `Some(Line)` if line is a header, `None` otherwise
///
/// Details:
/// - Handles #, ##, and ### headers with appropriate styling
fn parse_header_line(trimmed: &str) -> Option<Line<'static>> {
    let th = theme();
    if trimmed.starts_with("# ") {
        let text = trimmed.strip_prefix("# ").unwrap_or(trimmed).to_string();
        Some(Line::from(Span::styled(
            text,
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )))
    } else if trimmed.starts_with("## ") {
        let text = trimmed.strip_prefix("## ").unwrap_or(trimmed).to_string();
        Some(Line::from(Span::styled(
            text,
            Style::default().fg(th.mauve),
        )))
    } else if trimmed.starts_with("### ") {
        let text = trimmed.strip_prefix("### ").unwrap_or(trimmed).to_string();
        Some(Line::from(Span::styled(
            text,
            Style::default()
                .fg(th.subtext1)
                .add_modifier(Modifier::BOLD),
        )))
    } else {
        None
    }
}

/// What: Parse a code block line.
///
/// Inputs:
/// - `trimmed`: Trimmed line text
///
/// Output:
/// - `Some(Line)` if line is a code block marker, `None` otherwise
///
/// Details:
/// - Styles code block markers with subtext0 color
fn parse_code_block_line(trimmed: &str) -> Option<Line<'static>> {
    if trimmed.starts_with("```") {
        let th = theme();
        Some(Line::from(Span::styled(
            trimmed.to_string(),
            Style::default().fg(th.subtext0),
        )))
    } else {
        None
    }
}

/// What: Parse text into segments (URLs, bold, plain text).
///
/// Inputs:
/// - `trimmed`: Trimmed line text
///
/// Output:
/// - Vector of segments: (`text`, `style`, `is_url`, `url_string`)
///
/// Details:
/// - Detects URLs and bold markers (**text**)
/// - Returns styled segments for rendering
fn parse_text_segments(trimmed: &str) -> Vec<(String, Style, bool, Option<String>)> {
    let th = theme();
    let urls = detect_urls(trimmed);
    let mut segments: Vec<(String, Style, bool, Option<String>)> = Vec::new();
    let mut i = 0usize;
    let trimmed_bytes = trimmed.as_bytes();

    while i < trimmed_bytes.len() {
        // Check if we're at a URL position
        if let Some((url_start, url_end, url)) = urls.iter().find(|(s, _e, _)| *s == i) {
            // Add text before URL
            if *url_start > i {
                segments.push((
                    trimmed[i..*url_start].to_string(),
                    Style::default().fg(th.text),
                    false,
                    None,
                ));
            }
            // Add URL segment
            segments.push((
                trimmed[*url_start..*url_end].to_string(),
                Style::default()
                    .fg(th.mauve)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
                true,
                Some(url.clone()),
            ));
            i = *url_end;
            continue;
        }

        // Check for bold markers
        if let Some(pos) = trimmed[i..].find("**") {
            let pos = i + pos;
            if pos > i {
                segments.push((
                    trimmed[i..pos].to_string(),
                    Style::default().fg(th.text),
                    false,
                    None,
                ));
            }
            // Find closing **
            if let Some(end_pos) = trimmed[pos + 2..].find("**") {
                let end_pos = pos + 2 + end_pos;
                segments.push((
                    trimmed[pos + 2..end_pos].to_string(),
                    Style::default()
                        .fg(th.lavender)
                        .add_modifier(Modifier::BOLD),
                    false,
                    None,
                ));
                i = end_pos + 2;
            } else {
                // Unclosed bold, treat rest as bold
                segments.push((
                    trimmed[pos + 2..].to_string(),
                    Style::default()
                        .fg(th.lavender)
                        .add_modifier(Modifier::BOLD),
                    false,
                    None,
                ));
                break;
            }
            continue;
        }

        // No more special markers, add remaining text
        if i < trimmed.len() {
            segments.push((
                trimmed[i..].to_string(),
                Style::default().fg(th.text),
                false,
                None,
            ));
        }
        break;
    }

    if segments.is_empty() {
        segments.push((
            trimmed.to_string(),
            Style::default().fg(th.text),
            false,
            None,
        ));
    }

    segments
}

/// What: Build wrapped lines from text segments with URL position tracking.
///
/// Inputs:
/// - `segments`: Text segments with styles
/// - `content_width`: Available width for wrapping
/// - `content_rect`: Content rectangle for URL position calculation
/// - `start_y`: Starting Y position
/// - `url_positions`: Mutable vector to track URL positions
///
/// Output:
/// - Tuple of (wrapped lines, final Y position)
///
/// Details:
/// - Wraps text at word boundaries
/// - Tracks URL positions for click detection
fn build_wrapped_lines_from_segments(
    segments: Vec<(String, Style, bool, Option<String>)>,
    content_width: usize,
    content_rect: Rect,
    start_y: u16,
    url_positions: &mut Vec<(u16, u16, u16, String)>,
) -> (Vec<Line<'static>>, u16) {
    let mut lines = Vec::new();
    let mut current_line_spans: Vec<Span<'static>> = Vec::new();
    let mut current_line_width = 0usize;
    let mut line_y = start_y;

    for (text, style, is_url, url_string) in segments {
        let words: Vec<&str> = text.split_whitespace().collect();

        for word in words {
            let word_width = word.width();
            let separator_width = usize::from(current_line_width > 0);
            let test_width = current_line_width + separator_width + word_width;

            if test_width > content_width && !current_line_spans.is_empty() {
                // Wrap to new line
                lines.push(Line::from(current_line_spans.clone()));
                current_line_spans.clear();
                current_line_width = 0;
                line_y += 1;
            }

            // Track URL position if this is a URL
            if is_url && let Some(ref url) = url_string {
                let url_x = content_rect.x
                    + u16::try_from(current_line_width + separator_width).unwrap_or(u16::MAX);
                let url_width = u16::try_from(word_width).unwrap_or(u16::MAX);
                url_positions.push((url_x, line_y, url_width, url.clone()));
            }

            if current_line_width > 0 {
                current_line_spans.push(Span::raw(" "));
                current_line_width += 1;
            }

            current_line_spans.push(Span::styled(word.to_string(), style));
            current_line_width += word_width;
        }
    }

    if !current_line_spans.is_empty() {
        lines.push(Line::from(current_line_spans));
    }

    (lines, line_y + 1)
}

/// What: Parse markdown content into styled lines with wrapping and URL detection.
///
/// Inputs:
/// - `content`: Markdown content string
/// - `scroll`: Scroll offset (lines)
/// - `max_lines`: Maximum number of lines to render
/// - `url_positions`: Mutable vector to track URL positions for click detection
/// - `content_rect`: Content rectangle for width calculation and URL positioning
/// - `start_y`: Starting Y position for rendering
///
/// Output:
/// - Vector of styled lines for rendering
///
/// Details:
/// - Basic markdown parsing: headers (#), bold (**text**), code blocks (triple backticks)
/// - Detects and styles URLs with mauve color, underlined and bold
/// - Tracks URL positions for click detection
fn parse_markdown(
    content: &str,
    scroll: u16,
    max_lines: usize,
    url_positions: &mut Vec<(u16, u16, u16, String)>,
    content_rect: Rect,
    start_y: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let content_lines: Vec<&str> = content.lines().collect();
    let scroll_usize = scroll as usize;
    let start_idx = scroll_usize.min(content_lines.len());
    // Take up to max_lines starting from start_idx
    let lines_to_take = max_lines.min(content_lines.len().saturating_sub(start_idx));
    let mut current_y = start_y;
    let content_width = content_rect.width as usize;

    for line in content_lines.iter().skip(start_idx).take(lines_to_take) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            current_y += 1;
            continue;
        }

        // Check for headers
        if let Some(header_line) = parse_header_line(trimmed) {
            lines.push(header_line);
            current_y += 1;
            continue;
        }

        // Check for code blocks
        if let Some(code_line) = parse_code_block_line(trimmed) {
            lines.push(code_line);
            current_y += 1;
            continue;
        }

        // Regular text - handle URLs and bold markers
        let segments = parse_text_segments(trimmed);
        let (wrapped_lines, final_y) = build_wrapped_lines_from_segments(
            segments,
            content_width,
            content_rect,
            current_y,
            url_positions,
        );
        lines.extend(wrapped_lines);
        current_y = final_y;
    }

    lines
}

/// What: Build footer line with keybindings hint.
///
/// Inputs:
/// - `app`: Application state for i18n
///
/// Output:
/// - `Line<'static>` containing the formatted footer hint
///
/// Details:
/// - Shows keybindings for marking as read and dismissing
fn build_footer(app: &AppState) -> Line<'static> {
    let th = theme();
    let footer_text = i18n::t(app, "app.modals.announcement.footer_hint");
    Line::from(Span::styled(footer_text, Style::default().fg(th.overlay1)))
}

/// What: Render the announcement modal with markdown content.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (records rect)
/// - `area`: Full screen area used to center the modal
/// - `title`: Title to display in the modal header
/// - `content`: Markdown content to display
/// - `scroll`: Scroll offset in lines
///
/// Output:
/// - Draws the announcement modal and updates rect for hit-testing
///
/// Details:
/// - Centers modal, renders markdown content with basic formatting
pub fn render_announcement(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    title: &str,
    content: &str,
    scroll: u16,
) {
    let rect = calculate_modal_rect(area, content, app);
    app.announcement_rect = Some((rect.x, rect.y, rect.width, rect.height));

    let th = theme();

    // Calculate areas: footer needs 1 line for text, buffer is positional gap above
    let footer_height = FOOTER_LINES; // Footer text only
    let footer_total_height = footer_height + CONTENT_FOOTER_BUFFER; // Footer + buffer space
    let inner_height = rect.height.saturating_sub(BORDER_WIDTH); // Height inside borders (top + bottom)
    // Content area should fit: header (2 lines) + content + buffer + footer (2 lines)
    // So content area = inner_height - footer_total_height (header + content share the remaining space)
    let content_area_height = inner_height.saturating_sub(footer_total_height); // Area for header + content

    // Content rect (inside borders, for header + content, excluding footer area)
    // Ensure minimum height for header + at least 1 line of content
    let min_content_height = HEADER_LINES + 1;
    let content_rect = Rect {
        x: rect.x + 1,                                       // Inside left border
        y: rect.y + 1,                                       // Inside top border
        width: rect.width.saturating_sub(2),                 // Account for left + right borders
        height: content_area_height.max(min_content_height), // At least header + 1 line for content
    };

    // Footer rect at the bottom (inside borders, footer text only)
    // Position footer after content area + buffer gap, ensuring it's inside the modal
    let footer_y = rect.y + 1 + content_area_height + CONTENT_FOOTER_BUFFER;
    // Ensure footer fits within the modal bounds
    let footer_available_height =
        inner_height.saturating_sub(content_area_height + CONTENT_FOOTER_BUFFER);
    let footer_rect = Rect {
        x: rect.x + 1, // Inside left border
        y: footer_y.min(rect.y + rect.height.saturating_sub(footer_height + 1)), // Ensure it fits
        width: rect.width.saturating_sub(2), // Account for left + right borders
        height: footer_height.min(footer_available_height),
    };

    // Clear URL positions at start of rendering
    app.announcement_urls.clear();

    // Build content lines (header + content, no footer)
    let mut content_lines = Vec::new();
    content_lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    content_lines.push(Line::from(""));

    // Available height for actual content (after header)
    let available_height = content_area_height.saturating_sub(HEADER_LINES);
    let max_content_lines = available_height.max(1) as usize;
    let start_y = content_rect.y + HEADER_LINES;
    let parsed_content = parse_markdown(
        content,
        scroll,
        max_content_lines,
        &mut app.announcement_urls,
        content_rect,
        start_y,
    );
    content_lines.extend(parsed_content);

    // Render modal border first
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.subtext0));
    // Render border block (empty content, just borders)
    let empty_paragraph = Paragraph::new(vec![]).block(block);
    f.render_widget(empty_paragraph, rect);

    // Render content area (no borders, borders already drawn)
    let content_paragraph = Paragraph::new(content_lines).wrap(Wrap { trim: true });
    f.render_widget(content_paragraph, content_rect);

    // Render footer separately at fixed bottom position (always visible)
    // Buffer space is provided by the positional gap between content_rect and footer_rect
    let footer_lines = vec![build_footer(app)];
    let footer_paragraph = Paragraph::new(footer_lines);
    f.render_widget(footer_paragraph, footer_rect);
}
