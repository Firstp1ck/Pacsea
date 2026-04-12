//! Config / Panels / Options (and collapsed Menu) on the top updates row.

use ratatui::{
    Frame,
    prelude::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::state::AppState;
use crate::theme::theme;

use super::i18n::build_title_i18n_strings;
use super::rendering::{get_button_style, render_button_with_underline};

/// What: Unicode display width as `u16` for layout math.
///
/// Inputs:
/// - `s`: Text whose width to measure.
///
/// Output:
/// - Display width, or `u16::MAX` on conversion failure.
fn display_width_u16(s: &str) -> u16 {
    u16::try_from(s.width()).unwrap_or(u16::MAX)
}

/// What: Width to reserve on the top row for Config/Panels/Options (or collapsed Menu).
///
/// Inputs:
/// - `max_width`: Maximum columns available for the menu cluster.
/// - `app`: Application state (unused; reserved for future per-mode labels).
///
/// Output:
/// - Column count to pass as `Constraint::Length` for the menu chunk (possibly zero).
///
/// Details:
/// - Prefers three separate buttons when they fit; otherwise a single Menu button; otherwise `0`.
#[must_use]
pub fn top_bar_menu_cluster_width(max_width: u16, app: &AppState) -> u16 {
    let i18n = build_title_i18n_strings(app);
    let options_label = format!("{} v", i18n.options_button);
    let panels_label = format!("{} v", i18n.panels_button);
    let config_label = format!("{} v", i18n.config_button);
    let menu_label = format!("{} v", i18n.menu_button);
    let right_w = display_width_u16(&config_label)
        .saturating_add(1)
        .saturating_add(display_width_u16(&panels_label))
        .saturating_add(1)
        .saturating_add(display_width_u16(&options_label));
    let menu_w = display_width_u16(&menu_label);
    if max_width >= right_w {
        right_w
    } else if max_width >= menu_w {
        menu_w
    } else {
        0
    }
}

/// What: Clear hit-test rects for top-row menu controls.
///
/// Inputs:
/// - `app`: Application state.
///
/// Output:
/// - Sets the four menu-related rects to `None`.
///
/// Details:
/// - Call when the updates row does not render the menu cluster so stale rects are not reused.
#[allow(clippy::missing_const_for_fn)] // Mutates `AppState` fields; not a const operation.
pub fn clear_top_bar_menu_rects(app: &mut AppState) {
    app.config_button_rect = None;
    app.panels_button_rect = None;
    app.options_button_rect = None;
    app.collapsed_menu_button_rect = None;
}

/// What: Draw Config/Panels/Options (or collapsed Menu) and record mouse rects.
///
/// Inputs:
/// - `f`: Ratatui frame.
/// - `app`: Application state.
/// - `area`: One-row region on the top bar (typically the right-hand chunk next to Updates/News).
///
/// Output:
/// - Renders buttons and updates `config_button_rect`, `panels_button_rect`, `options_button_rect`,
///   or `collapsed_menu_button_rect`.
///
/// Details:
/// - When `area.width` is too small for even the collapsed menu, rects are cleared and nothing is drawn.
pub fn render_top_bar_menu_cluster(f: &mut Frame, app: &mut AppState, area: Rect) {
    clear_top_bar_menu_rects(app);
    if area.width == 0 || area.height == 0 {
        return;
    }

    let th = theme();
    let i18n = build_title_i18n_strings(app);
    let options_label = format!("{} v", i18n.options_button);
    let panels_label = format!("{} v", i18n.panels_button);
    let config_label = format!("{} v", i18n.config_button);
    let menu_label = format!("{} v", i18n.menu_button);

    let opt_w = display_width_u16(&options_label);
    let pan_w = display_width_u16(&panels_label);
    let cfg_w = display_width_u16(&config_label);
    let menu_w = display_width_u16(&menu_label);
    let right_w = cfg_w
        .saturating_add(1)
        .saturating_add(pan_w)
        .saturating_add(1)
        .saturating_add(opt_w);

    let y = area.y;

    if area.width >= right_w {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let pad = area.width.saturating_sub(right_w);
        if pad >= 1 {
            spans.push(Span::raw(" ".repeat(usize::from(pad))));
        }
        spans.extend(render_button_with_underline(
            &config_label,
            get_button_style(app.config_menu_open),
        ));
        spans.push(Span::raw(" "));
        spans.extend(render_button_with_underline(
            &panels_label,
            get_button_style(app.panels_menu_open),
        ));
        spans.push(Span::raw(" "));
        spans.extend(render_button_with_underline(
            &options_label,
            get_button_style(app.options_menu_open),
        ));

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line)
            .alignment(Alignment::Left)
            .block(Block::default().style(Style::default().bg(th.base)));
        f.render_widget(paragraph, area);

        let opt_x = area.x.saturating_add(area.width.saturating_sub(opt_w));
        let pan_x = opt_x.saturating_sub(1).saturating_sub(pan_w);
        let cfg_x = pan_x.saturating_sub(1).saturating_sub(cfg_w);
        app.config_button_rect = Some((cfg_x, y, cfg_w, 1));
        app.panels_button_rect = Some((pan_x, y, pan_w, 1));
        app.options_button_rect = Some((opt_x, y, opt_w, 1));
    } else if area.width >= menu_w {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let pad = area.width.saturating_sub(menu_w);
        if pad >= 1 {
            spans.push(Span::raw(" ".repeat(usize::from(pad))));
        }
        spans.extend(render_button_with_underline(
            &menu_label,
            get_button_style(app.collapsed_menu_open),
        ));
        let line = Line::from(spans);
        let paragraph = Paragraph::new(line)
            .alignment(Alignment::Left)
            .block(Block::default().style(Style::default().bg(th.base)));
        f.render_widget(paragraph, area);

        let mx = area.x.saturating_add(area.width.saturating_sub(menu_w));
        app.collapsed_menu_button_rect = Some((mx, y, menu_w, 1));
    }
}
