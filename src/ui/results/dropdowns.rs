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

/// What: Render dropdown menus (Config/Lists, Panels, Options) on the overlay layer.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (tracks menu open flags and rects)
/// - `results_area`: Rect of the results pane used for positioning
///
/// Output:
/// - Draws any open dropdowns and records their inner rectangles for hit-testing.
///
/// Details:
/// - Aligns menus with their buttons, clamps width to viewport, clears background, and numbers rows
///   for keyboard shortcuts while ensuring menus render above other content.
pub fn render_dropdowns(f: &mut Frame, app: &mut AppState, results_area: Rect) {
    let th = theme();

    // Optional: render Config/Lists dropdown overlay near its button
    app.config_menu_rect = None;
    if app.config_menu_open {
        let opts: Vec<String> = vec![
            i18n::t(app, "app.results.config_menu.options.settings"),
            i18n::t(app, "app.results.config_menu.options.theme"),
            i18n::t(app, "app.results.config_menu.options.keybindings"),
            i18n::t(app, "app.results.config_menu.options.install_list"),
            i18n::t(app, "app.results.config_menu.options.installed_packages"),
            i18n::t(app, "app.results.config_menu.options.recent_searches"),
        ];
        let widest = opts.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let w = widest
            .saturating_add(2)
            .min(results_area.width.saturating_sub(2));
        // Place menu below the Config/Lists button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
        let cbx = app
            .config_button_rect
            .map(|(x, _, _, _)| x)
            .unwrap_or(max_x);
        let menu_x = cbx.min(max_x);
        let h = (opts.len() as u16) + 2; // borders
        let menu_y = results_area.y.saturating_add(1); // just below top border (rendered on top layer)
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.config_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        // Build lines with right-aligned row numbers 1..N
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let num = format!("{}", i + 1);
            let pad = w.saturating_sub(text.len() as u16).saturating_sub(2);
            let padding = " ".repeat(pad as usize);
            lines.push(Line::from(vec![
                Span::styled(text.clone(), Style::default().fg(th.text)),
                Span::raw(padding),
                Span::styled(num, Style::default().fg(th.overlay1)),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .style(Style::default().bg(th.base))
                    .title(Line::from(vec![
                        Span::styled(" ", Style::default().fg(th.overlay1)),
                        Span::styled(
                            i18n::t(app, "app.results.menus.config_lists.first_letter"),
                            Style::default()
                                .fg(th.overlay1)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(
                            i18n::t(app, "app.results.menus.config_lists.suffix"),
                            Style::default().fg(th.overlay1),
                        ),
                    ]))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.mauve)),
            );
        f.render_widget(Clear, rect);
        f.render_widget(menu, rect);
    }

    // Optional: render Panels dropdown overlay near its button
    app.panels_menu_rect = None;
    if app.panels_menu_open {
        let label_recent = if app.show_recent_pane {
            i18n::t(app, "app.results.panels_menu.hide_recent")
        } else {
            i18n::t(app, "app.results.panels_menu.show_recent")
        };
        let label_install = if app.show_install_pane {
            i18n::t(app, "app.results.panels_menu.hide_install_list")
        } else {
            i18n::t(app, "app.results.panels_menu.show_install_list")
        };
        let label_keybinds = if app.show_keybinds_footer {
            i18n::t(app, "app.results.panels_menu.hide_keybinds")
        } else {
            i18n::t(app, "app.results.panels_menu.show_keybinds")
        };
        let opts: Vec<String> = vec![label_recent, label_install, label_keybinds];
        let widest = opts.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let w = widest
            .saturating_add(2)
            .min(results_area.width.saturating_sub(2));
        // Place menu below the Panels button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
        let pbx = app
            .panels_button_rect
            .map(|(x, _, _, _)| x)
            .unwrap_or(max_x);
        let menu_x = pbx.min(max_x);
        let h = (opts.len() as u16) + 2; // borders
        let menu_y = results_area.y.saturating_add(1); // just below top border (rendered on top layer)
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.panels_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let num = format!("{}", i + 1);
            let pad = w.saturating_sub(text.len() as u16).saturating_sub(2);
            let padding = " ".repeat(pad as usize);
            lines.push(Line::from(vec![
                Span::styled(text.clone(), Style::default().fg(th.text)),
                Span::raw(padding),
                Span::styled(num, Style::default().fg(th.overlay1)),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .style(Style::default().bg(th.base))
                    .title(Line::from(vec![
                        Span::styled(" ", Style::default().fg(th.overlay1)),
                        Span::styled(
                            i18n::t(app, "app.results.menus.panels.first_letter"),
                            Style::default()
                                .fg(th.overlay1)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(
                            i18n::t(app, "app.results.menus.panels.suffix"),
                            Style::default().fg(th.overlay1),
                        ),
                    ]))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.mauve)),
            );
        f.render_widget(Clear, rect);
        f.render_widget(menu, rect);
    }

    // Optional: render Options dropdown overlay near the right button
    app.options_menu_rect = None;
    if app.options_menu_open {
        let label_toggle = if app.installed_only_mode {
            i18n::t(app, "app.results.options_menu.list_all_packages")
        } else {
            i18n::t(app, "app.results.options_menu.list_installed_packages")
        };
        let opts = [
            label_toggle,
            i18n::t(app, "app.results.options_menu.update_system"),
            i18n::t(app, "app.results.options_menu.news"),
            i18n::t(app, "app.results.options_menu.tui_optional_deps"),
        ];
        let widest = opts.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let w = widest
            .saturating_add(2)
            .min(results_area.width.saturating_sub(2));
        // Place menu below the Options button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
        let obx = app
            .options_button_rect
            .map(|(x, _, _, _)| x)
            .unwrap_or(max_x);
        let menu_x = obx.min(max_x);
        let h = (opts.len() as u16) + 2; // borders
        let menu_y = results_area.y.saturating_add(1); // just below top border (rendered on top layer)
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.options_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        // Build lines with right-aligned row numbers 1..N
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let num = format!("{}", i + 1);
            let pad = w.saturating_sub(text.len() as u16).saturating_sub(2);
            let padding = " ".repeat(pad as usize);
            lines.push(Line::from(vec![
                Span::styled(text.clone(), Style::default().fg(th.text)),
                Span::raw(padding),
                Span::styled(num, Style::default().fg(th.overlay1)),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .style(Style::default().bg(th.base))
                    .title(Line::from(vec![
                        Span::styled(" ", Style::default().fg(th.overlay1)),
                        Span::styled(
                            i18n::t(app, "app.results.menus.options.first_letter"),
                            Style::default()
                                .fg(th.overlay1)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(
                            i18n::t(app, "app.results.menus.options.suffix"),
                            Style::default().fg(th.overlay1),
                        ),
                    ]))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.mauve)),
            );
        f.render_widget(Clear, rect);
        f.render_widget(menu, rect);
    }

    // Optional: render Artix filter dropdown overlay near the Artix filter button
    app.artix_filter_menu_rect = None;
    if app.artix_filter_menu_open {
        // Check if Artix-specific filters are hidden (dropdown should only show when they're hidden)
        let has_hidden_filters = app.results_filter_artix_omniverse_rect.is_none()
            && app.results_filter_artix_universe_rect.is_none()
            && app.results_filter_artix_lib32_rect.is_none()
            && app.results_filter_artix_galaxy_rect.is_none()
            && app.results_filter_artix_world_rect.is_none()
            && app.results_filter_artix_system_rect.is_none();

        if has_hidden_filters {
            // Check if all individual Artix repo filters are on
            let all_on = app.results_filter_show_artix_omniverse
                && app.results_filter_show_artix_universe
                && app.results_filter_show_artix_lib32
                && app.results_filter_show_artix_galaxy
                && app.results_filter_show_artix_world
                && app.results_filter_show_artix_system;

            let opts: Vec<(String, bool)> = vec![
                (i18n::t(app, "app.results.filters.artix"), all_on),
                (
                    i18n::t(app, "app.results.filters.artix_omniverse"),
                    app.results_filter_show_artix_omniverse,
                ),
                (
                    i18n::t(app, "app.results.filters.artix_universe"),
                    app.results_filter_show_artix_universe,
                ),
                (
                    i18n::t(app, "app.results.filters.artix_lib32"),
                    app.results_filter_show_artix_lib32,
                ),
                (
                    i18n::t(app, "app.results.filters.artix_galaxy"),
                    app.results_filter_show_artix_galaxy,
                ),
                (
                    i18n::t(app, "app.results.filters.artix_world"),
                    app.results_filter_show_artix_world,
                ),
                (
                    i18n::t(app, "app.results.filters.artix_system"),
                    app.results_filter_show_artix_system,
                ),
            ];
            let widest = opts.iter().map(|(s, _)| s.len()).max().unwrap_or(0) as u16;
            let w = widest
                .saturating_add(4) // space for checkbox indicator
                .saturating_add(2)
                .min(results_area.width.saturating_sub(2));
            // Place menu below the Artix filter button aligned to its left edge
            let rect_w = w.saturating_add(2);
            let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
            let artix_x = app
                .results_filter_artix_rect
                .map(|(x, _, _, _)| x)
                .unwrap_or(max_x);
            let menu_x = artix_x.min(max_x);
            let h = (opts.len() as u16) + 2; // borders
            let menu_y = results_area.y.saturating_add(1); // just below top border
            let rect = ratatui::prelude::Rect {
                x: menu_x,
                y: menu_y,
                width: rect_w,
                height: h,
            };
            // Record inner list area for hit-testing (exclude borders)
            app.artix_filter_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

            // Build lines with checkmarks for enabled filters
            let mut lines: Vec<Line> = Vec::new();
            for (text, enabled) in opts.iter() {
                let indicator = if *enabled { "✓ " } else { "  " };
                let pad = w
                    .saturating_sub(text.len() as u16)
                    .saturating_sub(indicator.len() as u16);
                let padding = " ".repeat(pad as usize);
                lines.push(Line::from(vec![
                    Span::styled(
                        indicator.to_string(),
                        Style::default().fg(if *enabled { th.green } else { th.overlay1 }),
                    ),
                    Span::styled(text.clone(), Style::default().fg(th.text)),
                    Span::raw(padding),
                ]));
            }
            let menu = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.base))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .style(Style::default().bg(th.base))
                        .title(Line::from(vec![Span::styled(
                            "Artix Filters",
                            Style::default().fg(th.overlay1),
                        )]))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(th.mauve)),
                );
            f.render_widget(Clear, rect);
            f.render_widget(menu, rect);
        }
    }
}
