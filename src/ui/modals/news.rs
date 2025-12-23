use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, types::NewsFeedItem, types::NewsFeedSource};
use crate::theme::{KeyChord, theme};

/// Width ratio for news modal.
const MODAL_WIDTH_RATIO: u16 = 2;
/// Width divisor for news modal.
const MODAL_WIDTH_DIVISOR: u16 = 3;
/// Height padding for news modal.
const MODAL_HEIGHT_PADDING: u16 = 8;
/// Maximum height for news modal.
const MODAL_MAX_HEIGHT: u16 = 20;
/// Border width for news modal.
const BORDER_WIDTH: u16 = 1;
/// Number of header lines.
const HEADER_LINES: u16 = 2;
/// Height of the keybinds pane at the bottom.
const KEYBINDS_PANE_HEIGHT: u16 = 3;

/// What: Determine if a news item title indicates a critical announcement.
///
/// Inputs:
/// - `title`: The news item title to check
///
/// Output:
/// - `true` if the title contains critical keywords, `false` otherwise
///
/// Details:
/// - Checks for "critical", "require manual intervention", "requires manual intervention", or "corrupting"
///   in the lowercase title text.
fn is_critical_news(title: &str) -> bool {
    let title_lower = title.to_lowercase();
    title_lower.contains("critical")
        || title_lower.contains("require manual intervention")
        || title_lower.contains("requires manual intervention")
        || title_lower.contains("corrupting")
}

/// What: Compute foreground and background colors for a news item based on selection and criticality.
///
/// Inputs:
/// - `is_selected`: Whether this item is currently selected
/// - `is_critical`: Whether this item is marked as critical
///
/// Output:
/// - Tuple of `(foreground_color, background_color)` from the theme
///
/// Details:
/// - Critical items use red foreground regardless of selection state.
/// - Selected items have a background color applied.
fn compute_item_colors(
    is_selected: bool,
    is_critical: bool,
) -> (ratatui::style::Color, Option<ratatui::style::Color>) {
    let th = theme();
    let fg = if is_critical { th.red } else { th.text };
    let bg = if is_selected { Some(th.surface1) } else { None };
    (fg, bg)
}

/// What: Calculate the modal rectangle dimensions and position centered in the given area.
///
/// Inputs:
/// - `area`: Full screen area to center the modal within
///
/// Output:
/// - `Rect` representing the modal's position and size
///
/// Details:
/// - Modal width is 2/3 of the area width.
/// - Modal height is area height minus padding, capped at maximum height.
/// - Modal is centered both horizontally and vertically.
fn calculate_modal_rect(area: Rect) -> Rect {
    let w = (area.width * MODAL_WIDTH_RATIO) / MODAL_WIDTH_DIVISOR;
    let h = area
        .height
        .saturating_sub(MODAL_HEIGHT_PADDING)
        .min(MODAL_MAX_HEIGHT);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// What: Highlight text with red/green/yellow keywords for AUR comments and Arch News.
///
/// Inputs:
/// - `text`: The text to highlight
/// - `th`: Theme for colors
///
/// Output:
/// - Vector of styled spans with keyword highlighting
///
/// Details:
/// - Red for negative keywords (crash, bug, fail, etc.)
/// - Green for positive keywords (fix, patch, solve, etc.)
/// - Yellow (bold) for default text
fn highlight_keywords(text: &str, th: &crate::theme::Theme) -> Vec<Span<'static>> {
    let normal = Style::default().fg(th.yellow).add_modifier(Modifier::BOLD);
    let neg = Style::default().fg(th.red).add_modifier(Modifier::BOLD);
    let pos = Style::default().fg(th.green).add_modifier(Modifier::BOLD);

    let negative_words = [
        "crash",
        "crashed",
        "crashes",
        "critical",
        "bug",
        "bugs",
        "fail",
        "fails",
        "failed",
        "failure",
        "failures",
        "issue",
        "issues",
        "trouble",
        "troubles",
        "panic",
        "segfault",
        "broken",
        "regression",
        "hang",
        "freeze",
        "unstable",
        "error",
        "errors",
        "require manual intervention",
        "requires manual intervention",
        "corrupting",
    ];
    let positive_words = [
        "fix",
        "fixed",
        "fixes",
        "patch",
        "patched",
        "solve",
        "solved",
        "solves",
        "solution",
        "resolve",
        "resolved",
        "resolves",
        "workaround",
    ];
    let neg_set: std::collections::HashSet<&str> = negative_words.into_iter().collect();
    let pos_set: std::collections::HashSet<&str> = positive_words.into_iter().collect();

    let mut spans = Vec::new();
    for token in text.split_inclusive(' ') {
        let cleaned = token
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .to_ascii_lowercase();
        let style = if pos_set.contains(cleaned.as_str()) {
            pos
        } else if neg_set.contains(cleaned.as_str()) {
            neg
        } else {
            normal
        };
        spans.push(Span::styled(token.to_string(), style));
    }
    spans
}

/// What: Format a single news feed item into a styled line for display.
///
/// Inputs:
/// - `item`: The news feed item to format
/// - `is_selected`: Whether this item is currently selected
/// - `is_read`: Whether this item has been marked as read
/// - `is_critical`: Whether this item is critical
///
/// Output:
/// - `Line<'static>` containing the formatted news item with appropriate styling
///
/// Details:
/// - Uses read/unread symbols from theme settings.
/// - Applies color styling based on selection and criticality.
/// - Shows source type indicator with color coding.
/// - For AUR comments, shows actual comment text (summary) instead of title.
/// - Applies keyword highlighting for AUR comments and Arch News.
fn format_news_item(
    item: &NewsFeedItem,
    is_selected: bool,
    is_read: bool,
    is_critical: bool,
) -> Line<'static> {
    let th = theme();
    let prefs = crate::theme::settings();
    let symbol = if is_read {
        &prefs.news_read_symbol
    } else {
        &prefs.news_unread_symbol
    };

    // Get source label and color
    let (source_label, source_color) = match item.source {
        NewsFeedSource::ArchNews => ("Arch", th.sapphire),
        NewsFeedSource::SecurityAdvisory => ("Advisory", th.yellow),
        NewsFeedSource::InstalledPackageUpdate => ("Update", th.green),
        NewsFeedSource::AurPackageUpdate => ("AUR Upd", th.mauve),
        NewsFeedSource::AurComment => ("AUR Cmt", th.yellow),
    };

    // Build the line with source indicator
    let source_span = Span::styled(
        format!("[{source_label}] "),
        Style::default().fg(source_color),
    );
    let symbol_span = Span::raw(format!("{symbol} "));
    let date_span = Span::raw(format!("{}  ", item.date));

    // Determine what text to display and how to style it
    let (display_text, should_highlight) = match item.source {
        NewsFeedSource::AurComment => {
            // For AUR comments, show the actual comment text (summary) instead of title
            let text = item
                .summary
                .as_ref()
                .map_or_else(|| item.title.as_str(), String::as_str);
            (text.to_string(), true)
        }
        NewsFeedSource::ArchNews => {
            // For Arch News, show title with keyword highlighting
            (item.title.clone(), true)
        }
        _ => {
            // For other sources, show title without keyword highlighting
            (item.title.clone(), false)
        }
    };

    let (fg, bg) = compute_item_colors(is_selected, is_critical);
    let base_style = bg.map_or_else(
        || Style::default().fg(fg),
        |bg_color| Style::default().fg(fg).bg(bg_color),
    );

    // Build content spans with or without keyword highlighting
    let mut content_spans = if should_highlight {
        // Apply keyword highlighting
        let highlighted = highlight_keywords(&display_text, &th);
        // Apply base style (selection background) to each span
        highlighted
            .into_iter()
            .map(|mut span| {
                // Merge styles: preserve keyword color, add selection background if needed
                if let Some(bg_color) = bg {
                    span.style = span.style.bg(bg_color);
                }
                span
            })
            .collect()
    } else {
        // No keyword highlighting, just apply base style
        vec![Span::styled(display_text, base_style)]
    };

    // Combine all spans
    let mut all_spans = vec![source_span, symbol_span, date_span];
    all_spans.append(&mut content_spans);

    Line::from(all_spans)
}

/// What: Build the keybinds pane lines for the news modal footer.
///
/// Inputs:
/// - `app`: Application state containing keymap and i18n context
///
/// Output:
/// - `Vec<Line<'static>>` containing the formatted keybinds hint
///
/// Details:
/// - Extracts key labels from keymap, falling back to defaults if unavailable.
/// - Replaces placeholders in the i18n template with actual key labels.
/// - Returns multiple lines for the keybinds pane.
fn build_keybinds_lines(app: &AppState) -> Vec<Line<'static>> {
    let th = theme();
    let mark_read_key = app
        .keymap
        .news_mark_read
        .first()
        .map_or_else(|| "R".to_string(), KeyChord::label);
    let mark_all_read_key = app
        .keymap
        .news_mark_all_read
        .first()
        .map_or_else(|| "Ctrl+R".to_string(), KeyChord::label);

    let footer_template = i18n::t(app, "app.modals.news.keybinds_hint");
    // Replace placeholders one at a time to avoid replacing all {} with the first value
    let footer_text =
        footer_template
            .replacen("{}", &mark_read_key, 1)
            .replacen("{}", &mark_all_read_key, 1);

    vec![
        Line::from(""), // Empty line for spacing
        Line::from(Span::styled(footer_text, Style::default().fg(th.subtext1))),
    ]
}

/// What: Build all content lines for the news modal including header and items.
///
/// Inputs:
/// - `app`: Application state for i18n and read status tracking
/// - `items`: News entries to display
/// - `selected`: Index of the currently highlighted news item
///
/// Output:
/// - `Vec<Line<'static>>` containing all formatted lines for the modal content
///
/// Details:
/// - Includes heading, empty line, and news items (or "none" message).
/// - Applies critical styling and read markers to items.
/// - Footer/keybinds are rendered separately in a bottom pane.
fn build_news_lines(app: &AppState, items: &[NewsFeedItem], selected: usize) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.news.heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.news.none"),
            Style::default().fg(th.subtext1),
        )));
    } else {
        for (i, item) in items.iter().enumerate() {
            let is_critical = is_critical_news(&item.title);
            let is_selected = selected == i;
            // Check read status using id (for NewsFeedItem) or url if available
            let is_read = app.news_read_ids.contains(&item.id)
                || item
                    .url
                    .as_ref()
                    .is_some_and(|url| app.news_read_urls.contains(url));
            lines.push(format_news_item(item, is_selected, is_read, is_critical));
        }
    }

    lines
}

/// What: Calculate the inner list rectangle for mouse hit-testing.
///
/// Inputs:
/// - `rect`: The outer modal rectangle
///
/// Output:
/// - Tuple of `(x, y, width, height)` representing the inner list area
///
/// Details:
/// - Accounts for borders, header lines, and keybinds pane.
/// - Used for mouse click detection on news items.
#[allow(clippy::missing_const_for_fn)] // Cannot be const due to saturating_sub
fn calculate_list_rect(rect: Rect) -> (u16, u16, u16, u16) {
    let list_inner_x = rect.x + BORDER_WIDTH;
    let list_inner_y = rect.y + BORDER_WIDTH + HEADER_LINES;
    let list_inner_w = rect.width.saturating_sub(BORDER_WIDTH * 2);
    let inner_h = rect.height.saturating_sub(BORDER_WIDTH * 2);
    // Subtract keybinds pane height from available height
    let list_rows = inner_h.saturating_sub(HEADER_LINES + KEYBINDS_PANE_HEIGHT);
    (list_inner_x, list_inner_y, list_inner_w, list_rows)
}

/// What: Build a styled Paragraph widget for the news modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `lines`: Content lines to display in the paragraph
///
/// Output:
/// - Configured `Paragraph` widget ready for rendering
///
/// Details:
/// - Applies theme colors, wrapping, borders, and title styling.
fn build_news_paragraph(app: &AppState, lines: Vec<Line<'static>>) -> Paragraph<'static> {
    let th = theme();
    Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    i18n::t(app, "app.modals.news.title"),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        )
}

/// What: Prepare all data needed for rendering the news modal.
///
/// Inputs:
/// - `app`: Mutable application state (will be updated with rect information)
/// - `area`: Full screen area used to center the modal
/// - `items`: News feed entries to display
/// - `selected`: Index of the currently highlighted news item
///
/// Output:
/// - Tuple of `(Rect, Vec<Line<'static>>, Rect)` containing the modal rect, content lines, and keybinds rect
///
/// Details:
/// - Calculates modal dimensions and position.
/// - Builds content lines including header and items.
/// - Calculates keybinds pane rectangle.
/// - Updates app state with rect information for mouse hit-testing.
fn prepare_news_modal(
    app: &mut AppState,
    area: Rect,
    items: &[NewsFeedItem],
    selected: usize,
) -> (Rect, Vec<Line<'static>>, Rect) {
    let rect = calculate_modal_rect(area);
    app.news_rect = Some((rect.x, rect.y, rect.width, rect.height));
    let (list_x, list_y, list_w, list_h) = calculate_list_rect(rect);
    app.news_list_rect = Some((list_x, list_y, list_w, list_h));
    let lines = build_news_lines(app, items, selected);

    // Calculate keybinds pane rect (will be adjusted in render function)
    let keybinds_rect = Rect {
        x: rect.x + BORDER_WIDTH,
        y: rect.y + rect.height - KEYBINDS_PANE_HEIGHT - BORDER_WIDTH,
        width: rect.width.saturating_sub(BORDER_WIDTH * 2),
        height: KEYBINDS_PANE_HEIGHT,
    };

    (rect, lines, keybinds_rect)
}

/// What: Render the prepared news modal content and keybinds pane to the frame.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `rect`: Modal rectangle position and dimensions
/// - `content_lines`: Content lines to display
/// - `keybinds_rect`: Rectangle for the keybinds pane
/// - `app`: Application state for i18n (used in paragraph building)
/// - `scroll`: Scroll offset (lines) for the news list
///
/// Output:
/// - Draws the modal widget and keybinds pane to the frame
///
/// Details:
/// - Clears the area first, then renders the styled paragraph with scroll offset.
/// - Renders keybinds pane at the bottom with borders.
fn render_news_modal(
    f: &mut Frame,
    rect: Rect,
    content_lines: Vec<Line<'static>>,
    _keybinds_rect: Rect,
    app: &AppState,
    scroll: u16,
) {
    let th = theme();
    f.render_widget(Clear, rect);

    // Split rect into content and keybinds areas
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),                       // Content area
            Constraint::Length(KEYBINDS_PANE_HEIGHT), // Keybinds pane
        ])
        .split(rect);

    // Render content area
    let mut paragraph = build_news_paragraph(app, content_lines);
    paragraph = paragraph.scroll((scroll, 0));
    f.render_widget(paragraph, chunks[0]);

    // Render keybinds pane
    let keybinds_lines = build_keybinds_lines(app);
    let keybinds_widget = Paragraph::new(keybinds_lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(keybinds_widget, chunks[1]);
}

/// What: Render the Arch news modal with selectable entries and read markers.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (records list rects, read state)
/// - `area`: Full screen area used to center the modal
/// - `items`: News entries to display
/// - `selected`: Index of the currently highlighted news item
///
/// Output:
/// - Draws the news list, updates overall/list rects, and marks unread items with theme symbols.
///
/// Details:
/// - Styles critical headlines, honors user-configured read symbols, and surfaces keybindings from
///   the keymap in the footer line.
/// - Separates data preparation from rendering for reduced complexity.
pub fn render_news(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    items: &[NewsFeedItem],
    selected: usize,
    scroll: u16,
) {
    let (rect, content_lines, keybinds_rect) = prepare_news_modal(app, area, items, selected);
    render_news_modal(f, rect, content_lines, keybinds_rect, app, scroll);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::NewsFeedItem;

    #[test]
    fn test_is_critical_news() {
        assert!(is_critical_news("Critical security update"));
        assert!(is_critical_news("CRITICAL: Important"));
        assert!(is_critical_news("Require manual intervention"));
        assert!(is_critical_news("Requires manual intervention"));
        assert!(is_critical_news("Corrupting filesystem"));
        assert!(!is_critical_news("Regular update"));
        assert!(!is_critical_news("Minor bug fix"));
    }

    #[test]
    fn test_compute_item_colors() {
        let (fg_normal, bg_normal) = compute_item_colors(false, false);
        let (fg_critical, _bg_critical) = compute_item_colors(false, true);
        let (_fg_selected, bg_selected) = compute_item_colors(true, false);
        let (_fg_selected_critical, bg_selected_critical) = compute_item_colors(true, true);

        assert_eq!(bg_normal, None);
        assert!(bg_selected.is_some());
        assert!(bg_selected_critical.is_some());
        // Critical items should have red foreground
        assert_ne!(fg_normal, fg_critical);
    }

    #[test]
    fn test_calculate_modal_rect() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let rect = calculate_modal_rect(area);

        // Width should be 2/3 of area width
        assert_eq!(rect.width, 66);
        // Should be centered horizontally
        assert_eq!(rect.x, 17);
        // Should be centered vertically: (30 - 20) / 2 = 5
        assert_eq!(rect.y, 5);
        // Height should be capped at MODAL_MAX_HEIGHT
        assert_eq!(rect.height, MODAL_MAX_HEIGHT);
    }

    #[test]
    fn test_format_news_item() {
        let item = NewsFeedItem {
            id: "https://example.com".to_string(),
            date: "2025-01-01".to_string(),
            title: "Test News".to_string(),
            summary: None,
            url: Some("https://example.com".to_string()),
            source: crate::state::types::NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        };

        let line = format_news_item(&item, false, false, false);
        assert!(!line.spans.is_empty());

        let line_critical = format_news_item(&item, false, false, true);
        assert!(!line_critical.spans.is_empty());
    }

    #[test]
    fn test_calculate_list_rect() {
        let rect = Rect {
            x: 10,
            y: 5,
            width: 50,
            height: 20,
        };
        let (x, y, w, h) = calculate_list_rect(rect);

        assert_eq!(x, rect.x + BORDER_WIDTH);
        assert_eq!(y, rect.y + BORDER_WIDTH + HEADER_LINES);
        assert_eq!(w, rect.width.saturating_sub(BORDER_WIDTH * 2));
        assert!(h <= rect.height);
    }
}
