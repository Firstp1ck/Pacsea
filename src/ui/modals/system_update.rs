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

#[allow(clippy::too_many_arguments)]
/// What: Render the system update modal with toggles for mirror/pacman/AUR/cache actions.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `do_mirrors`, `do_pacman`, `do_aur`, `do_cache`: Selected operations
/// - `country_idx`: Selected country index for mirrors
/// - `countries`: Available country list
/// - `mirror_count`: Desired number of mirrors
/// - `cursor`: Currently highlighted row index
///
/// Output:
/// - Draws the update configuration dialog, highlighting the focused row and showing shortcuts.
///
/// Details:
/// - Formats checkbox rows, displays the effective country list from settings, and surfaces key
///   hints for toggling, adjusting country, and running the update.
#[allow(clippy::many_single_char_names, clippy::fn_params_excessive_bools)]
pub fn render_system_update(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    do_mirrors: bool,
    do_pacman: bool,
    do_aur: bool,
    do_cache: bool,
    country_idx: usize,
    countries: &[String],
    mirror_count: u16,
    cursor: usize,
) {
    let th = theme();
    let w = area.width.saturating_sub(8).min(80);
    let h = 14;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.system_update.heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let mark = |b: bool| if b { "[x]" } else { "[ ]" };

    let entries: [(String, bool); 4] = [
        (
            i18n::t(app, "app.modals.system_update.entries.update_arch_mirrors"),
            do_mirrors,
        ),
        (
            i18n::t(app, "app.modals.system_update.entries.update_pacman"),
            do_pacman,
        ),
        (
            i18n::t(app, "app.modals.system_update.entries.update_aur"),
            do_aur,
        ),
        (
            i18n::t(app, "app.modals.system_update.entries.remove_cache"),
            do_cache,
        ),
    ];

    for (i, (label, on)) in entries.iter().enumerate() {
        let style = if cursor == i {
            Style::default()
                .fg(th.crust)
                .bg(th.lavender)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.text)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", mark(*on)), Style::default().fg(th.overlay1)),
            Span::styled((*label).clone(), style),
        ]));
    }

    // Country selector (mirrors)
    lines.push(Line::from(""));
    let worldwide_text = i18n::t(app, "app.modals.system_update.worldwide");
    let country_label = if country_idx < countries.len() {
        &countries[country_idx]
    } else {
        &worldwide_text
    };
    // Read configured countries and mirror count from settings for display
    let prefs = crate::theme::settings();
    let conf_countries = if prefs.selected_countries.trim().is_empty() {
        worldwide_text.clone()
    } else {
        prefs.selected_countries
    };
    // If Worldwide is selected, show the configured countries
    let shown_countries = if country_label == &worldwide_text {
        conf_countries.as_str()
    } else {
        country_label
    };
    let style = if cursor == entries.len() {
        Style::default()
            .fg(th.crust)
            .bg(th.lavender)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    };
    lines.push(Line::from(vec![
        Span::styled(
            i18n::t(app, "app.modals.system_update.country_label"),
            Style::default().fg(th.overlay1),
        ),
        Span::styled(shown_countries.to_string(), style),
        Span::raw("  •  "),
        Span::styled(
            i18n::t_fmt1(app, "app.modals.system_update.count_label", mirror_count),
            Style::default().fg(th.overlay1),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.system_update.footer_hint"),
        Style::default().fg(th.subtext1),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" {} ", i18n::t(app, "app.modals.system_update.title")),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
