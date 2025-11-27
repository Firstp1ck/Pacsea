//! AUR package comments viewer rendering.

use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

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

/// What: Detect markdown-style links in text: [text](url)
///
/// Inputs:
/// - `text`: Text to search for markdown links
///
/// Output:
/// - Vector of (`start_pos`, `end_pos`, `url_string`) tuples
fn detect_markdown_links(text: &str) -> Vec<(usize, usize, String)> {
    let mut links = Vec::new();
    let text_bytes = text.as_bytes();
    let mut i = 0;

    while i < text_bytes.len() {
        // Look for [text](url) pattern
        if text_bytes[i] == b'['
            && let Some(bracket_end) = text[i + 1..].find(']')
        {
            let bracket_end = i + 1 + bracket_end;
            if bracket_end + 1 < text_bytes.len()
                && text_bytes[bracket_end + 1] == b'('
                && let Some(paren_end) = text[bracket_end + 2..].find(')')
            {
                let paren_end = bracket_end + 2 + paren_end;
                let url = text[bracket_end + 2..paren_end].to_string();
                links.push((i, paren_end + 1, url));
                i = paren_end + 1;
                continue;
            }
        }
        i += 1;
    }

    links
}

/// What: Render content with markdown-like formatting (bold, italic, code, links).
///
/// Handles URL detection, markdown formatting, and word wrapping.
fn render_content_with_formatting<'a>(
    content: &'a str,
    urls: &[(usize, usize, String)],
    content_width: usize,
    th: &'a crate::theme::Theme,
    start_x: u16,
    start_y: u16,
    url_positions: &mut Vec<(u16, u16, u16, String)>,
) -> Vec<Line<'a>> {
    // Parse markdown-like formatting and create styled segments
    let segments = parse_markdown_segments(content, urls, th);

    // Build lines with word wrapping
    let mut lines = Vec::new();
    let mut current_line_spans: Vec<Span> = Vec::new();
    let mut current_line_width = 0;
    let mut current_y = start_y;

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
                current_y += 1;
            }

            // Track URL position if this is a URL
            if is_url && let Some(ref url) = url_string {
                let url_x = start_x
                    + u16::try_from(current_line_width).unwrap_or(u16::MAX)
                    + u16::from(current_line_width > 0);
                let url_width = u16::try_from(word_width).unwrap_or(u16::MAX);
                url_positions.push((url_x, current_y, url_width, url.clone()));
            }

            if current_line_width > 0 {
                current_line_spans.push(Span::raw(" "));
                current_line_width += 1;
            }

            current_line_spans.push(Span::styled(word.to_string(), style));
            current_line_width += word_width;
        }
    }

    // Add final line if not empty
    if !current_line_spans.is_empty() {
        lines.push(Line::from(current_line_spans));
    }

    lines
}

/// Parse markdown-like syntax and return segments with styling information.
/// Returns: (text, style, `is_url`, `url_string_opt`)
fn parse_markdown_segments<'a>(
    content: &'a str,
    urls: &[(usize, usize, String)],
    th: &'a crate::theme::Theme,
) -> Vec<(String, Style, bool, Option<String>)> {
    use ratatui::style::{Modifier, Style};
    let mut segments = Vec::new();
    let mut i = 0;
    let content_bytes = content.as_bytes();

    while i < content_bytes.len() {
        // Check if we're at a URL position
        if let Some((start, end, url)) = urls.iter().find(|(s, _e, _)| *s == i) {
            segments.push((
                content[*start..*end].to_string(),
                Style::default()
                    .fg(th.mauve)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
                true,
                Some(url.clone()),
            ));
            i = *end;
            continue;
        }

        // Check for code blocks: ```...```
        if i + 3 <= content_bytes.len()
            && &content_bytes[i..i + 3] == b"```"
            && let Some(end) = content[i + 3..].find("```")
        {
            let end = i + 3 + end + 3;
            let code = content[i + 3..end - 3].trim();
            segments.push((
                code.to_string(),
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
                false,
                None,
            ));
            i = end;
            continue;
        }

        // Check for inline code: `code`
        if content_bytes[i] == b'`'
            && let Some(end) = content[i + 1..].find('`')
        {
            let end = i + 1 + end + 1;
            let code = content[i + 1..end - 1].trim();
            segments.push((
                code.to_string(),
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
                false,
                None,
            ));
            i = end;
            continue;
        }

        // Check for bold: **text**
        if i + 2 <= content_bytes.len()
            && &content_bytes[i..i + 2] == b"**"
            && let Some(end) = content[i + 2..].find("**")
        {
            let end = i + 2 + end + 2;
            let text = content[i + 2..end - 2].trim();
            segments.push((
                text.to_string(),
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                false,
                None,
            ));
            i = end;
            continue;
        }

        // Check for italic: *text* (but not **text**)
        if content_bytes[i] == b'*'
            && (i + 1 >= content_bytes.len() || content_bytes[i + 1] != b'*')
            && let Some(end) = content[i + 1..].find('*')
        {
            let end = i + 1 + end + 1;
            let text = content[i + 1..end - 1].trim();
            segments.push((
                text.to_string(),
                Style::default().fg(th.text).add_modifier(Modifier::ITALIC),
                false,
                None,
            ));
            i = end;
            continue;
        }

        // Regular text - find next formatting marker
        let next_marker = find_next_marker(content, i);
        let end = next_marker.unwrap_or(content.len());
        if end > i {
            let text = content[i..end].trim();
            if !text.is_empty() {
                segments.push((text.to_string(), Style::default().fg(th.text), false, None));
            }
        }
        i = end.max(i + 1);
    }

    segments
}

/// Find the next markdown formatting marker position.
fn find_next_marker(content: &str, start: usize) -> Option<usize> {
    let markers = ["**", "`", "```", "["];
    let mut min_pos = None;

    for marker in &markers {
        if let Some(pos) = content[start..].find(marker) {
            let pos = start + pos;
            min_pos = Some(min_pos.map_or(pos, |m: usize| m.min(pos)));
        }
    }

    min_pos
}

/// What: Build a loading state list item.
///
/// Inputs:
/// - `app`: Application state for i18n.
///
/// Output:
/// - List item with loading message.
fn build_loading_item(app: &AppState) -> ListItem<'static> {
    ListItem::new(Line::from(i18n::t(app, "app.details.loading_comments")))
}

/// What: Build an error state list item.
///
/// Inputs:
/// - `error`: Error message to display.
/// - `th`: Theme for styling.
///
/// Output:
/// - List item with error message styled in red.
fn build_error_item(error: &str, th: &crate::theme::Theme) -> ListItem<'static> {
    ListItem::new(Line::from(Span::styled(
        error.to_string(),
        Style::default().fg(th.red),
    )))
}

/// What: Build an empty state list item.
///
/// Inputs:
/// - `app`: Application state for i18n.
///
/// Output:
/// - List item with empty state message.
fn build_empty_item(app: &AppState) -> ListItem<'static> {
    ListItem::new(Line::from(i18n::t(app, "app.details.no_comments")))
}

/// What: Build comment header line with author and date, tracking positions.
///
/// Inputs:
/// - `comment`: Comment to build header for.
/// - `th`: Theme for styling.
/// - `content_x`: X coordinate for position tracking.
/// - `comment_y`: Y coordinate for position tracking.
/// - `app`: Application state to track author/date positions.
///
/// Output:
/// - Tuple of (`header_line`, `pin_offset`) where `pin_offset` is the width of pin indicator.
///
/// Details:
/// - Tracks author position for click detection.
/// - Tracks date position if date has URL.
fn build_comment_header(
    comment: &crate::state::types::AurComment,
    th: &crate::theme::Theme,
    content_x: u16,
    comment_y: u16,
    app: &mut AppState,
) -> (Line<'static>, u16) {
    let author_style = Style::default()
        .fg(th.sapphire)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let date_style = if comment.date_url.is_some() {
        Style::default()
            .fg(th.mauve)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(th.overlay2)
    };

    let mut header_spans = Vec::new();
    let pin_offset = if comment.pinned {
        let pinned_style = Style::default().fg(th.yellow).add_modifier(Modifier::BOLD);
        let pin_text = "📌 ";
        header_spans.push(Span::styled(pin_text, pinned_style));
        u16::try_from(pin_text.width()).unwrap_or(3)
    } else {
        0
    };

    header_spans.push(Span::styled(comment.author.clone(), author_style));
    header_spans.push(Span::raw(" • "));
    header_spans.push(Span::styled(comment.date.clone(), date_style));

    let author_x = content_x + pin_offset;
    let author_width = u16::try_from(comment.author.width()).unwrap_or(u16::MAX);
    app.comments_authors
        .push((author_x, comment_y, author_width, comment.author.clone()));

    if let Some(ref date_url) = comment.date_url {
        let separator_width = 3;
        let date_x = author_x
            .saturating_add(author_width)
            .saturating_add(separator_width);
        let date_width = u16::try_from(comment.date.width()).unwrap_or(u16::MAX);
        app.comments_dates
            .push((date_x, comment_y, date_width, date_url.clone()));
    }

    (Line::from(header_spans), pin_offset)
}

/// What: Build comment content lines with URL detection and formatting.
///
/// Inputs:
/// - `content`: Comment content text.
/// - `content_width`: Maximum width for wrapping.
/// - `th`: Theme for styling.
/// - `content_x`: X coordinate for URL position tracking.
/// - `content_y`: Y coordinate for URL position tracking.
/// - `app`: Application state to track URL positions.
///
/// Output:
/// - Vector of content lines (owned).
///
/// Details:
/// - Detects URLs and markdown links.
/// - Renders with markdown-like formatting.
/// - Returns empty comment placeholder if content is empty.
fn build_comment_content(
    content: &str,
    content_width: usize,
    th: &crate::theme::Theme,
    content_x: u16,
    content_y: u16,
    app: &mut AppState,
) -> Vec<Line<'static>> {
    if content.is_empty() {
        return vec![Line::from(Span::styled(
            "(empty comment)",
            Style::default().fg(th.overlay2),
        ))];
    }

    let urls = detect_urls(content);
    let markdown_urls = detect_markdown_links(content);
    let all_urls: Vec<_> = urls.into_iter().chain(markdown_urls).collect();

    // Convert to owned lines since we're creating owned data
    render_content_with_formatting(
        content,
        &all_urls,
        content_width,
        th,
        content_x,
        content_y,
        &mut app.comments_urls,
    )
    .into_iter()
    .map(|line| {
        // Convert line to owned by converting all spans to owned
        Line::from(
            line.spans
                .iter()
                .map(|span| {
                    Span::styled(
                        span.content.to_string(),
                        span.style,
                    )
                })
                .collect::<Vec<_>>(),
        )
    })
    .collect()
}

/// What: Render a single comment as a list item.
///
/// Inputs:
/// - `comment`: Comment to render.
/// - `th`: Theme for styling.
/// - `content_x`: X coordinate for position tracking.
/// - `content_width`: Maximum width for wrapping.
/// - `current_y`: Current Y coordinate (updated by this function).
/// - `app`: Application state to track positions.
///
/// Output:
/// - Tuple of (`list_item`, `new_y`) where `new_y` is the Y coordinate after rendering.
///
/// Details:
/// - Builds header, content, and separator lines.
/// - Tracks author, date, and URL positions.
fn render_single_comment(
    comment: &crate::state::types::AurComment,
    th: &crate::theme::Theme,
    content_x: u16,
    content_width: usize,
    current_y: u16,
    app: &mut AppState,
) -> (ListItem<'static>, u16) {
    let mut lines = Vec::new();
    let mut y = current_y;

    let (header_line, _pin_offset) = build_comment_header(comment, th, content_x, y, app);
    lines.push(header_line);
    y += 1;

    let content_lines =
        build_comment_content(&comment.content, content_width, th, content_x, y, app);
    y += u16::try_from(content_lines.len()).unwrap_or(u16::MAX);
    lines.extend(content_lines);

    lines.push(Line::from(Span::styled(
        "─".repeat(content_width.min(20)),
        Style::default().fg(th.surface2),
    )));
    y += 1;

    (ListItem::new(lines), y)
}

/// What: Build list items for all comments.
///
/// Inputs:
/// - `app`: Application state with comments and scroll offset.
/// - `th`: Theme for styling.
/// - `comments_area`: Rect assigned to the comments pane.
///
/// Output:
/// - Vector of list items for comments.
///
/// Details:
/// - Applies scroll offset by skipping items from top.
/// - Renders each comment with header, content, and separator.
fn build_comment_items(
    app: &mut AppState,
    th: &crate::theme::Theme,
    comments_area: Rect,
) -> Vec<ListItem<'static>> {
    let mut current_y = comments_area.y + 1;
    let content_x = comments_area.x + 1;
    let content_width = comments_area.width.saturating_sub(4) as usize;

    let scroll_offset = app.comments_scroll as usize;
    // Clone comments to avoid borrowing conflicts with mutable app access
    let comments_to_render: Vec<_> = app.comments[scroll_offset..].to_vec();
    let mut items = Vec::new();
    
    for comment in comments_to_render {
        let (item, new_y) =
            render_single_comment(&comment, th, content_x, content_width, current_y, app);
        current_y = new_y;
        items.push(item);
    }
    items
}

/// What: Render the comments viewer pane with scroll support.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (comments, scroll, cached rects)
/// - `comments_area`: Rect assigned to the comments pane
///
/// Output:
/// - Draws comments list and updates content rectangle for hit-testing.
///
/// Details:
/// - Applies scroll offset by skipping items from top
/// - Shows loading state, error message, or comments list
/// - Records content rect for mouse interactions (scrolling)
pub fn render_comments(f: &mut Frame, app: &mut AppState, comments_area: Rect) {
    let th = theme();

    app.comments_rect = Some((
        comments_area.x + 1,
        comments_area.y + 1,
        comments_area.width.saturating_sub(2),
        comments_area.height.saturating_sub(2),
    ));

    app.comments_urls.clear();
    app.comments_authors.clear();
    app.comments_dates.clear();

    let title_text = i18n::t(app, "app.titles.comments");
    let title_span = Span::styled(&title_text, Style::default().fg(th.overlay1));

    let items: Vec<ListItem<'static>> = if app.comments_loading {
        vec![build_loading_item(app)]
    } else if let Some(ref error) = app.comments_error {
        vec![build_error_item(error, &th)]
    } else if app.comments.is_empty() {
        vec![build_empty_item(app)]
    } else {
        build_comment_items(app, &th, comments_area)
    };

    let list = List::new(items)
        .style(Style::default().fg(th.text).bg(th.base))
        .block(
            Block::default()
                .title(Line::from(title_span))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        );

    f.render_widget(list, comments_area);
}
