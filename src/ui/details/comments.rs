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

/// What: Render content text with URL detection and styling, with word wrapping.
///
/// Inputs:
/// - `content`: Comment content text.
/// - `urls`: Vector of detected URLs (`start_pos`, `end_pos`, `url_string`).
/// - `content_width`: Maximum width for wrapping.
/// - `th`: Theme for styling.
/// - `start_x`: Starting X coordinate for URL position tracking.
/// - `start_y`: Starting Y coordinate for URL position tracking.
/// - `url_positions`: Mutable vector to store URL screen positions.
///
/// Output:
/// - Vector of `Line` objects with styled spans, including URL styling.
///
/// Details:
/// - Wraps text to fit within `content_width`.
/// - Styles URLs with underline and mauve color.
/// - Preserves word boundaries when wrapping.
/// - Tracks URL screen positions for click detection.
fn render_content_with_urls<'a>(
    content: &'a str,
    urls: &[(usize, usize, String)],
    content_width: usize,
    th: &'a crate::theme::Theme,
    start_x: u16,
    start_y: u16,
    url_positions: &mut Vec<(u16, u16, u16, String)>,
) -> Vec<Line<'a>> {
    #[allow(clippy::needless_pass_by_ref_mut)]
    let mut lines = Vec::new();

    // If no URLs, use simple word wrapping
    if urls.is_empty() {
        let words: Vec<&str> = content.split_whitespace().collect();
        let mut current_line = String::new();
        for word in words {
            let test_line = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{current_line} {word}")
            };
            if test_line.width() <= content_width {
                current_line = test_line;
            } else {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                }
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(Line::from(current_line));
        }
        return lines;
    }

    // Build spans with URL styling
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, end, _url) in urls {
        // Add text before URL
        if *start > last_end {
            let before_text = &content[last_end..*start];
            if !before_text.is_empty() {
                spans.push((last_end, *start, before_text.to_string(), false));
            }
        }

        // Add URL with special styling
        let url_text = &content[*start..*end];
        spans.push((*start, *end, url_text.to_string(), true));

        last_end = *end;
    }

    // Add remaining text after last URL
    if last_end < content.len() {
        let after_text = &content[last_end..];
        if !after_text.is_empty() {
            spans.push((last_end, content.len(), after_text.to_string(), false));
        }
    }

    // If no spans were created (shouldn't happen), fall back to simple rendering
    if spans.is_empty() {
        return vec![Line::from(content)];
    }

    // Build lines with word wrapping, preserving URL spans and tracking positions
    let mut current_line_spans: Vec<Span> = Vec::new();
    let mut current_line_width = 0;
    let mut current_y = start_y;

    for (start, end, text, is_url) in spans {
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

            // Add word to current line
            let style = if is_url {
                Style::default()
                    .fg(th.mauve)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
            } else {
                Style::default().fg(th.text)
            };

            // Track URL position if this is a URL word
            if is_url {
                // Find the corresponding URL from the urls vector
                if let Some((_, _, url_string)) =
                    urls.iter().find(|(s, e, _)| *s == start && *e == end)
                {
                    let url_x = start_x
                        + u16::try_from(current_line_width).unwrap_or(u16::MAX)
                        + u16::from(current_line_width > 0);
                    let url_width = u16::try_from(word_width).unwrap_or(u16::MAX);
                    url_positions.push((url_x, current_y, url_width, url_string.clone()));
                }
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

    // Remember comments rect for mouse interactions (scrolling)
    app.comments_rect = Some((
        comments_area.x + 1,
        comments_area.y + 1,
        comments_area.width.saturating_sub(2),
        comments_area.height.saturating_sub(2),
    ));

    // Clear previous URL, author, and date positions
    app.comments_urls.clear();
    app.comments_authors.clear();
    app.comments_dates.clear();

    let title_text = i18n::t(app, "app.titles.comments");
    let title_span = Span::styled(&title_text, Style::default().fg(th.overlay1));

    // Build list items from comments
    let items: Vec<ListItem> = if app.comments_loading {
        // Show loading state
        vec![ListItem::new(Line::from(i18n::t(
            app,
            "app.details.loading_comments",
        )))]
    } else if let Some(ref error) = app.comments_error {
        // Show error message
        vec![ListItem::new(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(th.red),
        )))]
    } else if app.comments.is_empty() {
        // Show empty state
        vec![ListItem::new(Line::from(i18n::t(
            app,
            "app.details.no_comments",
        )))]
    } else {
        // Render comments and track URL positions
        let mut current_y = comments_area.y + 1; // Start after border
        let content_x = comments_area.x + 1;
        let content_width = comments_area.width.saturating_sub(4) as usize; // Account for borders and padding

        // Separate pinned and regular comments for display
        // Pinned comments are already at the top from parsing, but we'll add a visual indicator
        let items: Vec<ListItem> = app
            .comments
            .iter()
            .skip(app.comments_scroll as usize)
            .map(|comment| {
                // Format each comment: author (styled) + date (styled) + content
                let mut lines = Vec::new();

                // Author and date line
                let author_style = Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                // Make date clickable if it has a URL (styled like URLs/authors)
                let date_style = if comment.date_url.is_some() {
                    Style::default()
                        .fg(th.mauve)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(th.overlay2)
                };

                // Build header line with optional pinned indicator
                let mut header_spans = Vec::new();
                let pin_offset = if comment.pinned {
                    // Add pinned indicator (📌)
                    let pinned_style = Style::default().fg(th.yellow).add_modifier(Modifier::BOLD);
                    let pin_text = "📌 ";
                    header_spans.push(Span::styled(pin_text, pinned_style));
                    // Pin emoji takes 2 character width, plus space
                    u16::try_from(pin_text.width()).unwrap_or(3)
                } else {
                    0
                };
                header_spans.push(Span::styled(comment.author.clone(), author_style));
                header_spans.push(Span::raw(" • "));
                header_spans.push(Span::styled(comment.date.clone(), date_style));

                // Track author position for click detection (account for pin emoji)
                let author_x = content_x + pin_offset;
                let author_width = u16::try_from(comment.author.width()).unwrap_or(u16::MAX);
                let comment_start_y = current_y;
                app.comments_authors.push((
                    author_x,
                    comment_start_y,
                    author_width,
                    comment.author.clone(),
                ));

                // Track date position for click detection if it has a URL
                if let Some(ref date_url) = comment.date_url {
                    let separator_width = 3; // " • "
                    let date_x = author_x
                        .saturating_add(author_width)
                        .saturating_add(separator_width);
                    let date_width = u16::try_from(comment.date.width()).unwrap_or(u16::MAX);
                    app.comments_dates.push((
                        date_x,
                        comment_start_y,
                        date_width,
                        date_url.clone(),
                    ));
                }

                let header_line = Line::from(header_spans);
                lines.push(header_line);
                current_y += 1; // Header line

                // Content line(s) - wrap if needed and detect URLs
                let content = &comment.content;
                if content.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "(empty comment)",
                        Style::default().fg(th.overlay2),
                    )));
                    current_y += 1;
                } else {
                    // Detect URLs in content
                    let urls = detect_urls(content);

                    // Render content with URL styling and track positions
                    let content_lines = render_content_with_urls(
                        content,
                        &urls,
                        content_width,
                        &th,
                        content_x,
                        comment_start_y + 1, // After header line
                        &mut app.comments_urls,
                    );
                    current_y += u16::try_from(content_lines.len()).unwrap_or(u16::MAX);
                    lines.extend(content_lines);
                }

                // Add separator line between comments
                lines.push(Line::from(Span::styled(
                    "─".repeat(content_width.min(20)),
                    Style::default().fg(th.surface2),
                )));
                current_y += 1; // Separator line

                ListItem::new(lines)
            })
            .collect();

        items
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
