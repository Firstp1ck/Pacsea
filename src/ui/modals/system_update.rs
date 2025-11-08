use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::theme::theme;

#[allow(clippy::too_many_arguments)]
pub fn render_system_update(
    f: &mut Frame,
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
        "System Update",
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let mark = |b: bool| if b { "[x]" } else { "[ ]" };

    let entries: [(&str, bool); 4] = [
        ("Update Arch Mirrors", do_mirrors),
        ("Update Pacman (sudo pacman -Syyu)", do_pacman),
        ("Update AUR (paru/yay)", do_aur),
        ("Remove Cache (pacman/yay)", do_cache),
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
            Span::styled((*label).to_string(), style),
        ]));
    }

    // Country selector (mirrors)
    lines.push(Line::from(""));
    let country_label = if country_idx < countries.len() {
        &countries[country_idx]
    } else {
        "Worldwide"
    };
    // Read configured countries and mirror count from settings for display
    let prefs = crate::theme::settings();
    let conf_countries = if prefs.selected_countries.trim().is_empty() {
        "Worldwide".to_string()
    } else {
        prefs.selected_countries.clone()
    };
    // If Worldwide is selected, show the configured countries
    let shown_countries = if country_label == "Worldwide" {
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
        Span::styled("Country (Mirrors): ", Style::default().fg(th.overlay1)),
        Span::styled(shown_countries.to_string(), style),
        Span::raw("  •  "),
        Span::styled(
            format!("Count: {}", mirror_count),
            Style::default().fg(th.overlay1),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Space: toggle  •  Left/Right: change country  •  -/+ change count  •  Enter: run  •  Esc: cancel",
        Style::default().fg(th.subtext1),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " Update System ",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
