use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

const MODAL_WIDTH_RATIO: u16 = 2;
const MODAL_WIDTH_DIVISOR: u16 = 3;
const MODAL_HEIGHT_PADDING: u16 = 8;
const MODAL_MAX_HEIGHT: u16 = 25;
const BORDER_WIDTH: u16 = 2;
const HEADER_LINES: u16 = 2;
const FOOTER_LINES: u16 = 1;
const TOTAL_HEADER_FOOTER_LINES: u16 = HEADER_LINES + FOOTER_LINES;

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

/// What: Parse markdown content into styled lines for display.
///
/// Inputs:
/// - `content`: Markdown content string
/// - `scroll`: Scroll offset in lines
/// - `max_lines`: Maximum number of lines to display
///
/// Output:
/// - Vector of styled lines for rendering
///
/// Details:
/// - Basic markdown parsing: headers (#), bold (**text**), code blocks (triple backticks)
/// - Applies appropriate styling based on markdown syntax
fn parse_markdown(content: &str, scroll: u16, max_lines: usize) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();
    let content_lines: Vec<&str> = content.lines().collect();
    let scroll_usize = scroll as usize;
    let start_idx = scroll_usize.min(content_lines.len());
    let end_idx = (start_idx + max_lines).min(content_lines.len());

    for line in content_lines.iter().take(end_idx).skip(start_idx) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        // Check for headers
        if trimmed.starts_with("# ") {
            let text = trimmed.strip_prefix("# ").unwrap_or(trimmed).to_string();
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
        } else if trimmed.starts_with("## ") {
            let text = trimmed.strip_prefix("## ").unwrap_or(trimmed).to_string();
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(th.mauve),
            )));
        } else if trimmed.starts_with("### ") {
            let text = trimmed.strip_prefix("### ").unwrap_or(trimmed).to_string();
            lines.push(Line::from(Span::styled(
                text,
                Style::default()
                    .fg(th.subtext1)
                    .add_modifier(Modifier::BOLD),
            )));
        } else if trimmed.starts_with("```") {
            // Code block marker - skip or style differently
            lines.push(Line::from(Span::styled(
                trimmed.to_string(),
                Style::default().fg(th.subtext0),
            )));
        } else {
            // Regular text - handle bold markers
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut remaining = trimmed.to_string();
            let mut in_bold = false;

            while !remaining.is_empty() {
                if let Some(pos) = remaining.find("**") {
                    if pos > 0 {
                        spans.push(Span::styled(
                            remaining[..pos].to_string(),
                            Style::default().fg(th.text),
                        ));
                    }
                    remaining = remaining[pos + 2..].to_string();
                    in_bold = !in_bold;
                } else {
                    spans.push(Span::styled(
                        remaining.clone(),
                        if in_bold {
                            Style::default().fg(th.text).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(th.text)
                        },
                    ));
                    break;
                }
            }

            if spans.is_empty() {
                spans.push(Span::styled(
                    trimmed.to_string(),
                    Style::default().fg(th.text),
                ));
            }

            lines.push(Line::from(spans));
        }
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
    Line::from(Span::styled(footer_text, Style::default().fg(th.subtext1)))
}

/// What: Build all content lines for the announcement modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `content`: Markdown content to display
/// - `scroll`: Scroll offset in lines
/// - `rect`: Modal rectangle for calculating available space
///
/// Output:
/// - `Vec<Line<'static>>` containing all formatted lines for the modal
///
/// Details:
/// - Includes heading, empty line, content lines, empty line, and footer
fn build_announcement_lines(
    app: &AppState,
    content: &str,
    scroll: u16,
    rect: Rect,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    // Header
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.announcement.title"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Content area
    let available_height = rect
        .height
        .saturating_sub(TOTAL_HEADER_FOOTER_LINES + BORDER_WIDTH);
    let max_content_lines = available_height as usize;
    let content_lines = parse_markdown(content, scroll, max_content_lines);
    lines.extend(content_lines);

    // Footer
    lines.push(Line::from(""));
    lines.push(build_footer(app));

    lines
}

/// What: Build styled paragraph widget for the announcement modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `lines`: Content lines to display
///
/// Output:
/// - `Paragraph` widget with appropriate styling
///
/// Details:
/// - Wraps text and applies border styling
fn build_announcement_paragraph(_app: &AppState, lines: Vec<Line<'static>>) -> Paragraph<'static> {
    let th = theme();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.subtext0));

    Paragraph::new(lines).block(block).wrap(Wrap { trim: true })
}

/// What: Render the announcement modal with markdown content.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (records rect)
/// - `area`: Full screen area used to center the modal
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
    content: &str,
    scroll: u16,
) {
    let rect = calculate_modal_rect(area);
    app.announcement_rect = Some((rect.x, rect.y, rect.width, rect.height));

    let lines = build_announcement_lines(app, content, scroll, rect);
    f.render_widget(Clear, rect);
    let paragraph = build_announcement_paragraph(app, lines);
    f.render_widget(paragraph, rect);
}
