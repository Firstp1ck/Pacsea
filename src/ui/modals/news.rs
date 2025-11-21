use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, NewsItem};
use crate::theme::theme;

// Option 2: Extract constants for magic numbers
const MODAL_WIDTH_RATIO: u16 = 2;
const MODAL_WIDTH_DIVISOR: u16 = 3;
const MODAL_HEIGHT_PADDING: u16 = 8;
const MODAL_MAX_HEIGHT: u16 = 20;
const BORDER_WIDTH: u16 = 1;
const HEADER_LINES: u16 = 2;
const TOTAL_HEADER_FOOTER_LINES: u16 = 4;

/// What: Determine if a news item title indicates a critical announcement.
///
/// Inputs:
/// - `title`: The news item title to check
///
/// Output:
/// - `true` if the title contains critical keywords, `false` otherwise
///
/// Details:
/// - Checks for "critical", "require manual intervention", or "requires manual intervention"
///   in the lowercase title text.
fn is_critical_news(title: &str) -> bool {
    let title_lower = title.to_lowercase();
    title_lower.contains("critical")
        || title_lower.contains("require manual intervention")
        || title_lower.contains("requires manual intervention")
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

/// What: Format a single news item into a styled line for display.
///
/// Inputs:
/// - `item`: The news item to format
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
fn format_news_item(
    item: &NewsItem,
    is_selected: bool,
    is_read: bool,
    is_critical: bool,
) -> Line<'static> {
    let prefs = crate::theme::settings();
    let symbol = if is_read {
        &prefs.news_read_symbol
    } else {
        &prefs.news_unread_symbol
    };
    let line_text = format!("{} {}  {}", symbol, item.date, item.title);
    let (fg, bg) = compute_item_colors(is_selected, is_critical);
    let style = if let Some(bg_color) = bg {
        Style::default().fg(fg).bg(bg_color)
    } else {
        Style::default().fg(fg)
    };
    Line::from(Span::styled(line_text, style))
}

/// What: Build the footer line with dynamic keybindings from the keymap.
///
/// Inputs:
/// - `app`: Application state containing keymap and i18n context
///
/// Output:
/// - `Line<'static>` containing the formatted footer hint
///
/// Details:
/// - Extracts key labels from keymap, falling back to defaults if unavailable.
/// - Replaces placeholders in the i18n template with actual key labels.
fn build_footer(app: &AppState) -> Line<'static> {
    let th = theme();
    let mark_read_key = app
        .keymap
        .news_mark_read
        .first()
        .map(|k| k.label())
        .unwrap_or_else(|| "R".to_string());
    let mark_all_read_key = app
        .keymap
        .news_mark_all_read
        .first()
        .map(|k| k.label())
        .unwrap_or_else(|| "Ctrl+R".to_string());
    let footer_template = i18n::t(app, "app.modals.news.footer_hint");
    let footer_text = footer_template
        .replace("{}", &mark_read_key)
        .replace("{}", &mark_all_read_key);
    Line::from(Span::styled(footer_text, Style::default().fg(th.subtext1)))
}

/// What: Build all content lines for the news modal including header, items, and footer.
///
/// Inputs:
/// - `app`: Application state for i18n and read status tracking
/// - `items`: News entries to display
/// - `selected`: Index of the currently highlighted news item
///
/// Output:
/// - `Vec<Line<'static>>` containing all formatted lines for the modal
///
/// Details:
/// - Includes heading, empty line, news items (or "none" message), empty line, and footer.
/// - Applies critical styling and read markers to items.
fn build_news_lines(app: &AppState, items: &[NewsItem], selected: usize) -> Vec<Line<'static>> {
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
            let is_read = app.news_read_urls.contains(&item.url);
            lines.push(format_news_item(item, is_selected, is_read, is_critical));
        }
    }

    lines.push(Line::from(""));
    lines.push(build_footer(app));

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
/// - Accounts for borders, header lines, and footer lines.
/// - Used for mouse click detection on news items.
fn calculate_list_rect(rect: Rect) -> (u16, u16, u16, u16) {
    let list_inner_x = rect.x + BORDER_WIDTH;
    let list_inner_y = rect.y + BORDER_WIDTH + HEADER_LINES;
    let list_inner_w = rect.width.saturating_sub(BORDER_WIDTH * 2);
    let inner_h = rect.height.saturating_sub(BORDER_WIDTH * 2);
    let list_rows = inner_h.saturating_sub(TOTAL_HEADER_FOOTER_LINES);
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
                .borders(Borders::ALL)
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
/// - `items`: News entries to display
/// - `selected`: Index of the currently highlighted news item
///
/// Output:
/// - Tuple of `(Rect, Vec<Line<'static>>)` containing the modal rect and content lines
///
/// Details:
/// - Calculates modal dimensions and position.
/// - Builds all content lines including header, items, and footer.
/// - Updates app state with rect information for mouse hit-testing.
fn prepare_news_modal(
    app: &mut AppState,
    area: Rect,
    items: &[NewsItem],
    selected: usize,
) -> (Rect, Vec<Line<'static>>) {
    let rect = calculate_modal_rect(area);
    app.news_rect = Some((rect.x, rect.y, rect.width, rect.height));
    app.news_list_rect = Some(calculate_list_rect(rect));
    let lines = build_news_lines(app, items, selected);
    (rect, lines)
}

/// What: Render the prepared news modal content to the frame.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `rect`: Modal rectangle position and dimensions
/// - `lines`: Content lines to display
/// - `app`: Application state for i18n (used in paragraph building)
///
/// Output:
/// - Draws the modal widget to the frame
///
/// Details:
/// - Clears the area first, then renders the styled paragraph.
fn render_news_modal(f: &mut Frame, rect: Rect, lines: Vec<Line<'static>>, app: &AppState) {
    f.render_widget(Clear, rect);
    let paragraph = build_news_paragraph(app, lines);
    f.render_widget(paragraph, rect);
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
    items: &[NewsItem],
    selected: usize,
) {
    let (rect, lines) = prepare_news_modal(app, area, items, selected);
    render_news_modal(f, rect, lines, app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::NewsItem;

    #[test]
    fn test_is_critical_news() {
        assert!(is_critical_news("Critical security update"));
        assert!(is_critical_news("CRITICAL: Important"));
        assert!(is_critical_news("Require manual intervention"));
        assert!(is_critical_news("Requires manual intervention"));
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
        let item = NewsItem {
            date: "2025-01-01".to_string(),
            title: "Test News".to_string(),
            url: "https://example.com".to_string(),
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
