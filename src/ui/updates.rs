use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::{AppState, types::AppMode};
use crate::theme::theme;
use crate::ui::results::{
    clear_top_bar_menu_rects, render_top_bar_menu_cluster, top_bar_menu_cluster_width,
};

/// Minimum width reserved for the Updates/News label before the menu cluster.
const MIN_UPDATES_LABEL_SLOT: u16 = 8;

/// What: Render the updates available button at the top of the window and lockout status on the right.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state containing updates count, loading state, and faillock status
/// - `area`: Target rectangle for the updates button (should be 1 line high)
///
/// Output:
/// - Draws the updates button and lockout status, records clickable rectangle in `app.updates_button_rect`
///
/// Details:
/// - In Package mode: Shows "Updates available (X)" if count > 0, "No updates available" if count is 0,
///   or "Checking updates..." if still loading
/// - In News mode: Shows "News Ready" if news are available, "No News available" if no news,
///   or "Loading news..." if still loading
/// - Config/Lists, Panels, and Options (or collapsed Menu) render on this same row to the right of the
///   updates/news label (before lockout text when present).
/// - Shows lockout status on the right if user is locked out
/// - Button is styled similar to other buttons in the UI
/// - Records clickable rectangle for mouse interaction
pub fn render_updates_button(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    // Determine button text based on app mode
    let button_text = if matches!(app.app_mode, AppMode::News) {
        // News mode: show news button
        if app.news_loading {
            i18n::t(app, "app.news_button.loading")
        } else if app.news_ready {
            i18n::t(app, "app.news_button.ready")
        } else {
            i18n::t(app, "app.news_button.none")
        }
    } else {
        // Package mode: show updates button
        if app.updates_loading {
            i18n::t(app, "app.updates_button.loading")
        } else if let Some(count) = app.updates_count {
            if count > 0 {
                if app.updates_last_check_authoritative == Some(false) {
                    i18n::t_fmt1(app, "app.updates_button.available_degraded", count)
                } else {
                    i18n::t_fmt1(app, "app.updates_button.available", count)
                }
            } else if app.updates_last_check_authoritative == Some(false) {
                i18n::t(app, "app.updates_button.none_maybe_stale")
            } else {
                i18n::t(app, "app.updates_button.none")
            }
        } else {
            i18n::t(app, "app.updates_button.none")
        }
    };

    // Check if lockout status should be displayed
    let lockout_text: Option<String> = if app.faillock_locked {
        app.faillock_remaining_minutes.map_or_else(
            || Some(i18n::t(app, "app.updates_button.locked")),
            |remaining| {
                if remaining > 0 {
                    Some(i18n::t_fmt1(
                        app,
                        "app.updates_button.locked_with_time",
                        remaining,
                    ))
                } else {
                    Some(i18n::t(app, "app.updates_button.locked"))
                }
            },
        )
    } else {
        None
    };

    let lockout_w = lockout_text.as_ref().map_or(0u16, |lockout| {
        u16::try_from(lockout.width()).unwrap_or(20).min(
            area.width
                .saturating_sub(MIN_UPDATES_LABEL_SLOT.saturating_add(1)),
        )
    });

    let width_for_menus_and_label = area.width.saturating_sub(lockout_w);
    let max_menu_w = width_for_menus_and_label.saturating_sub(MIN_UPDATES_LABEL_SLOT);
    let menu_cluster_w = top_bar_menu_cluster_width(max_menu_w, app);

    if menu_cluster_w == 0 {
        clear_top_bar_menu_rects(app);
    }

    let chunks: Vec<Rect> = match (lockout_w > 0, menu_cluster_w > 0) {
        (true, true) => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(MIN_UPDATES_LABEL_SLOT),
                Constraint::Length(menu_cluster_w),
                Constraint::Length(lockout_w),
            ])
            .split(area)
            .to_vec(),
        (true, false) => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(MIN_UPDATES_LABEL_SLOT),
                Constraint::Length(lockout_w),
            ])
            .split(area)
            .to_vec(),
        (false, true) => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(MIN_UPDATES_LABEL_SLOT),
                Constraint::Length(menu_cluster_w),
            ])
            .split(area)
            .to_vec(),
        (false, false) => vec![area],
    };

    let updates_chunk = chunks[0];
    if matches!(app.app_mode, AppMode::News) {
        render_news_button_inner(f, app, updates_chunk, &button_text, &th);
    } else {
        render_updates_button_inner(f, app, updates_chunk, &button_text, &th);
    }

    if menu_cluster_w > 0 && chunks.len() >= 2 {
        render_top_bar_menu_cluster(f, app, chunks[1]);
    }

    if lockout_w > 0 {
        let lock_chunk = if menu_cluster_w > 0 {
            chunks.get(2).copied()
        } else {
            chunks.get(1).copied()
        };
        if let (Some(lockout), Some(lock_chunk)) = (lockout_text, lock_chunk) {
            let lockout_style = Style::default()
                .fg(th.red)
                .bg(th.base)
                .add_modifier(Modifier::BOLD);
            let lockout_line = Line::from(Span::styled(lockout, lockout_style));
            let lockout_paragraph = Paragraph::new(lockout_line)
                .alignment(Alignment::Right)
                .block(
                    Block::default()
                        .borders(ratatui::widgets::Borders::NONE)
                        .style(Style::default().bg(th.base)),
                );
            f.render_widget(lockout_paragraph, lock_chunk);
        }
    }
}

/// What: Render the updates button inner content.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Target rectangle
/// - `button_text`: Button text to display
/// - `th`: Theme
///
/// Output:
/// - Draws the updates button and records clickable rectangle
///
/// Details:
/// - Center-aligns the label within `area` (matches the historical full-row center when no menus fit)
///   and sizes the hit target to the displayed text width at the centered position.
fn render_updates_button_inner(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    button_text: &str,
    th: &crate::theme::Theme,
) {
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
        spans.push(Span::styled(button_text.to_string(), button_style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center).block(
        Block::default()
            .borders(ratatui::widgets::Borders::NONE)
            .style(Style::default().bg(th.base)),
    );

    // Render the button
    f.render_widget(paragraph, area);

    let button_width = u16::try_from(button_text.width())
        .unwrap_or(u16::MAX)
        .min(area.width);
    let button_x = area
        .x
        .saturating_add(area.width.saturating_sub(button_width) / 2);
    app.updates_button_rect = Some((button_x, area.y, button_width, area.height));
}

/// What: Render the news button inner content.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Target rectangle
/// - `button_text`: Button text to display
/// - `th`: Theme
///
/// Output:
/// - Draws the news button and records clickable rectangle
///
/// Details:
/// - Center-aligns the label within `area` and sizes the hit target to the displayed text width at the
///   centered position.
fn render_news_button_inner(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    button_text: &str,
    th: &crate::theme::Theme,
) {
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
        spans.push(Span::styled(button_text.to_string(), button_style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center).block(
        Block::default()
            .borders(ratatui::widgets::Borders::NONE)
            .style(Style::default().bg(th.base)),
    );

    // Render the button
    f.render_widget(paragraph, area);

    let button_width = u16::try_from(button_text.width())
        .unwrap_or(u16::MAX)
        .min(area.width);
    let button_x = area
        .x
        .saturating_add(area.width.saturating_sub(button_width) / 2);
    app.news_button_rect = Some((button_x, area.y, button_width, area.height));
}
