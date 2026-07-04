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

/// What: Column gap between the two config-editor top labels when both strings are non-empty.
///
/// Inputs: None.
///
/// Output: Gap width in terminal columns.
///
/// Details: Keeps the pair readable when centered as a group.
const CONFIG_EDITOR_DUAL_LABEL_GAP: u16 = 2;

/// What: Build the package-mode updates strip label for the top bar.
///
/// Inputs:
/// - `app`: Application state (`updates_loading`, `updates_count`, `updates_last_check_authoritative`).
///
/// Output:
/// - Localized string matching the Package-mode updates button.
///
/// Details:
/// - Mirrors the non-News branch of `render_updates_button` so Config editor can show the same text
///   alongside the news strip without duplicating conditionals.
fn package_updates_top_bar_label(app: &AppState) -> String {
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
}

/// What: Build the news-mode top bar strip label.
///
/// Inputs:
/// - `app`: Application state (`news_loading`, `news_ready`).
///
/// Output:
/// - Localized string matching the News-mode news button.
///
/// Details:
/// - Used by News mode and by Config editor when showing both top strips.
fn news_top_bar_label(app: &AppState) -> String {
    if app.news_loading {
        i18n::t(app, "app.news_button.loading")
    } else if app.news_ready {
        i18n::t(app, "app.news_button.ready")
    } else {
        i18n::t(app, "app.news_button.none")
    }
}

/// What: Draw updates and news top labels as one horizontally centered group (config editor).
///
/// Inputs:
/// - `f`: Ratatui frame.
/// - `app`: Application state (stores clickable rects).
/// - `area`: Label-slot rectangle for the row.
/// - `updates_label` / `news_label`: Localized strings for each strip.
/// - `th`: Active theme.
///
/// Output:
/// - Renders both labels and assigns `updates_button_rect` and `news_button_rect`.
///
/// Details:
/// - When `updates_label` plus gap plus `news_label` fits in `area.width`, the combined block is
///   centered in `area`. If not, falls back to a 50/50 horizontal split so both labels remain usable
///   in narrow terminals.
fn render_config_editor_dual_top_labels(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    updates_label: &str,
    news_label: &str,
    th: &crate::theme::Theme,
) {
    let w_u = updates_label.width();
    let w_n = news_label.width();
    let gap_u16 = if w_u > 0 && w_n > 0 {
        CONFIG_EDITOR_DUAL_LABEL_GAP
    } else {
        0
    };
    let desired = w_u.saturating_add(usize::from(gap_u16)).saturating_add(w_n);
    if desired == 0 {
        app.updates_button_rect = None;
        app.news_button_rect = None;
        return;
    }

    let avail = usize::from(area.width);
    if desired <= avail {
        let left_pad = (avail - desired) / 2;
        let start_x = area.x.saturating_add(u16::try_from(left_pad).unwrap_or(0));
        let w_updates = u16::try_from(w_u).unwrap_or(area.width).min(area.width);
        let updates_rect = Rect {
            x: start_x,
            y: area.y,
            width: w_updates,
            height: area.height,
        };
        let news_x = start_x.saturating_add(w_updates).saturating_add(gap_u16);
        let max_news_w = area.x.saturating_add(area.width).saturating_sub(news_x);
        let w_news = u16::try_from(w_n).unwrap_or(0).min(max_news_w);
        let news_rect = Rect {
            x: news_x,
            y: area.y,
            width: w_news,
            height: area.height,
        };
        render_updates_button_inner(f, app, updates_rect, updates_label, th);
        render_news_button_inner(f, app, news_rect, news_label, th);
        return;
    }

    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    render_updates_button_inner(f, app, halves[0], updates_label, th);
    render_news_button_inner(f, app, halves[1], news_label, th);
}

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
/// - In Config editor mode: Shows both package updates and news labels as one centered group (with a
///   narrow-terminal fallback split); two click targets.
/// - Config/Lists, Panels, and Options (or collapsed Menu) render on this same row to the right of the
///   updates/news label (before lockout text when present).
/// - Shows lockout status on the right if user is locked out
/// - Button is styled similar to other buttons in the UI
/// - Records clickable rectangle for mouse interaction
pub fn render_updates_button(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

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
    match app.app_mode {
        AppMode::ConfigEditor => {
            let updates_label = package_updates_top_bar_label(app);
            let news_label = news_top_bar_label(app);
            render_config_editor_dual_top_labels(
                f,
                app,
                updates_chunk,
                &updates_label,
                &news_label,
                &th,
            );
        }
        AppMode::News => {
            let label = news_top_bar_label(app);
            render_news_button_inner(f, app, updates_chunk, &label, &th);
        }
        AppMode::Package => {
            let label = package_updates_top_bar_label(app);
            render_updates_button_inner(f, app, updates_chunk, &label, &th);
        }
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
