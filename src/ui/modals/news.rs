use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::{AppState, NewsItem};
use crate::theme::theme;

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
pub fn render_news(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    items: &[NewsItem],
    selected: usize,
) {
    let th = theme();
    let w = (area.width * 2) / 3;
    let h = area.height.saturating_sub(8).min(20);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    // Record outer and inner rects for mouse hit-testing
    app.news_rect = Some((rect.x, rect.y, rect.width, rect.height));

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Arch Linux News",
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            "No news items available.",
            Style::default().fg(th.subtext1),
        )));
    } else {
        for (i, it) in items.iter().enumerate() {
            let tl = it.title.to_lowercase();
            let is_critical = tl.contains("critical")
                || tl.contains("require manual intervention")
                || tl.contains("requires manual intervention");
            let style = if selected == i {
                let fg = if is_critical { th.red } else { th.text };
                Style::default().fg(fg).bg(th.surface1)
            } else {
                let fg = if is_critical { th.red } else { th.text };
                Style::default().fg(fg)
            };
            let prefs = crate::theme::settings();
            let line = format!(
                "{} {}  {}",
                if app.news_read_urls.contains(&it.url) {
                    &prefs.news_read_symbol
                } else {
                    &prefs.news_unread_symbol
                },
                it.date,
                it.title
            );
            lines.push(Line::from(Span::styled(line, style)));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "Up/Down: select  •  Enter: open  •  {}: mark read  •  {}: mark all read  •  Esc: close",
            app.keymap
                .news_mark_read
                .first()
                .map(|k| k.label())
                .unwrap_or_else(|| "R".to_string()),
            app.keymap
                .news_mark_all_read
                .first()
                .map(|k| k.label())
                .unwrap_or_else(|| "Ctrl+R".to_string())
        ),
        Style::default().fg(th.subtext1),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " News ",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);

    // The list content starts two lines after title and blank line, and ends before footer hint lines.
    // Approximate inner list area (exclude 1-char borders):
    let list_inner_x = rect.x + 1;
    let list_inner_y = rect.y + 1 + 2; // header + blank line
    let list_inner_w = rect.width.saturating_sub(2);
    // Compute visible rows budget: total height minus borders, header (2 lines), footer (2 lines)
    let inner_h = rect.height.saturating_sub(2);
    let list_rows = inner_h.saturating_sub(4);
    app.news_list_rect = Some((list_inner_x, list_inner_y, list_inner_w, list_rows));
}
