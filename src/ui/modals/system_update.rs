use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::{Theme, theme};

/// What: Get style for a row based on cursor position.
fn row_style(th: &Theme, cursor: usize, row: usize) -> Style {
    if cursor == row {
        Style::default()
            .fg(th.crust)
            .bg(th.lavender)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    }
}

/// What: Create a checkbox line with label.
fn checkbox_line(th: &Theme, checked: bool, label: String, style: Style) -> Line<'static> {
    let mark = if checked { "[x]" } else { "[ ]" };
    Line::from(vec![
        Span::styled(format!("{mark} "), Style::default().fg(th.overlay1)),
        Span::styled(label, style),
    ])
}

#[allow(clippy::too_many_arguments)]
/// What: Render the system update modal with toggles for mirror/pacman/AUR/cache actions.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `do_mirrors`, `do_pacman`, `force_sync`, `do_aur`, `do_cache`: Selected operations
/// - `country_idx`: Selected country index for mirrors
/// - `countries`: Available country list
/// - `mirror_count`: Desired number of mirrors
/// - `cursor`: Currently highlighted row index
///
/// Output:
/// - Draws the update configuration dialog, highlighting the focused row and showing shortcuts.
///
/// Details:
/// - Formats checkbox rows, displays the effective country list from settings.
/// - Pacman update shows sync mode on same line, toggled with Left/Right arrows.
#[allow(clippy::many_single_char_names, clippy::fn_params_excessive_bools)]
pub fn render_system_update(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    do_mirrors: bool,
    do_pacman: bool,
    force_sync: bool,
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
    let rect = Rect {
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

    // Row 0: Update Arch Mirrors
    lines.push(checkbox_line(
        &th,
        do_mirrors,
        i18n::t(app, "app.modals.system_update.entries.update_arch_mirrors"),
        row_style(&th, cursor, 0),
    ));

    // Row 1: Update Pacman with sync mode selector
    let mode_style = if cursor == 1 {
        Style::default().fg(th.yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.overlay1)
    };
    let sync_mode = if force_sync {
        i18n::t(app, "app.modals.system_update.sync_mode.force")
    } else {
        i18n::t(app, "app.modals.system_update.sync_mode.normal")
    };
    let mark = if do_pacman { "[x]" } else { "[ ]" };
    lines.push(Line::from(vec![
        Span::styled(format!("{mark} "), Style::default().fg(th.overlay1)),
        Span::styled(
            i18n::t(app, "app.modals.system_update.entries.update_pacman"),
            row_style(&th, cursor, 1),
        ),
        Span::styled("  ◄ ", mode_style),
        Span::styled(sync_mode, mode_style),
        Span::styled(" ►", mode_style),
    ]));

    // Row 2: Update AUR
    lines.push(checkbox_line(
        &th,
        do_aur,
        i18n::t(app, "app.modals.system_update.entries.update_aur"),
        row_style(&th, cursor, 2),
    ));

    // Row 3: Remove Cache
    lines.push(checkbox_line(
        &th,
        do_cache,
        i18n::t(app, "app.modals.system_update.entries.remove_cache"),
        row_style(&th, cursor, 3),
    ));

    // Row 4: Country selector (mirrors)
    lines.push(Line::from(""));
    let worldwide_text = i18n::t(app, "app.modals.system_update.worldwide");
    let country_label = countries.get(country_idx).unwrap_or(&worldwide_text);
    let prefs = crate::theme::settings();
    let conf_countries = if prefs.selected_countries.trim().is_empty() {
        worldwide_text.clone()
    } else {
        prefs.selected_countries
    };
    let shown_countries = if country_label == &worldwide_text {
        conf_countries.as_str()
    } else {
        country_label
    };
    lines.push(Line::from(vec![
        Span::styled(
            i18n::t(app, "app.modals.system_update.country_label"),
            Style::default().fg(th.overlay1),
        ),
        Span::styled(shown_countries.to_string(), row_style(&th, cursor, 4)),
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
