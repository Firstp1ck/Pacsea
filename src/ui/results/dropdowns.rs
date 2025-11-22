use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
#[allow(clippy::cognitive_complexity)]
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
        // Use Unicode display width, not byte length, to handle wide characters like →
        let widest = opts
            .iter()
            .map(|s| u16::try_from(s.width()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0);
        // Calculate max number width first to include it in total width
        let max_num_width = u16::try_from(format!("{}", opts.len()).len()).unwrap_or(u16::MAX);
        // Width must accommodate: widest text + spacing + max number width
        let w = widest
            .saturating_add(max_num_width)
            .saturating_add(2) // spacing between text and number
            .min(results_area.width.saturating_sub(2));
        // Place menu below the Config/Lists button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
        let cbx = app
            .config_button_rect
            .map(|(x, _, _, _)| x)
            .unwrap_or(max_x);
        let menu_x = cbx.min(max_x);
        let h = u16::try_from(opts.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2); // borders
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
        // Use Unicode display width for accurate alignment with wide characters
        let spacing = 2u16;
        let num_start_col = widest + spacing; // Column where numbers start (display width)
        let total_line_width = w; // All lines must be exactly this display width
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let num_str = format!("{}", i + 1);
            // Pad number to max_num_width for right alignment (numbers are ASCII, so len() = width)
            let num_width = u16::try_from(num_str.len()).unwrap_or(u16::MAX);
            let num_padding = max_num_width.saturating_sub(num_width);
            let padded_num = format!("{}{}", " ".repeat(num_padding as usize), num_str);

            // Calculate padding using display width, not byte length
            let text_display_width = u16::try_from(text.width()).unwrap_or(u16::MAX);
            let text_padding = widest.saturating_sub(text_display_width);

            // Build complete line
            let mut complete_line = format!(
                "{}{}{}{}",
                text,
                " ".repeat(text_padding as usize),
                " ".repeat(spacing as usize),
                padded_num
            );

            // Ensure line has exactly total_line_width display width
            let current_width = u16::try_from(complete_line.width()).unwrap_or(u16::MAX);
            if current_width < total_line_width {
                complete_line.push_str(&" ".repeat((total_line_width - current_width) as usize));
            } else if current_width > total_line_width {
                // Truncate by display width, not byte length
                let mut truncated = String::new();
                let mut width_so_far = 0u16;
                for ch in complete_line.chars() {
                    let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
                    if width_so_far + ch_width > total_line_width {
                        break;
                    }
                    truncated.push(ch);
                    width_so_far += ch_width;
                }
                complete_line = truncated;
            }

            // Split at num_start_col display width for styling
            let mut text_part = String::new();
            let mut width_so_far = 0u16;
            for ch in complete_line.chars() {
                let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
                if width_so_far + ch_width > num_start_col {
                    break;
                }
                text_part.push(ch);
                width_so_far += ch_width;
            }
            let num_part = complete_line
                .chars()
                .skip(text_part.chars().count())
                .collect::<String>();

            lines.push(Line::from(vec![
                Span::styled(text_part, Style::default().fg(th.text)),
                Span::styled(num_part, Style::default().fg(th.overlay1)),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: false })
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
        // Use Unicode display width, not byte length, to handle wide characters
        let widest = opts
            .iter()
            .map(|s| u16::try_from(s.width()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0);
        // Calculate max number width first to include it in total width
        let max_num_width = u16::try_from(format!("{}", opts.len()).len()).unwrap_or(u16::MAX);
        // Width must accommodate: widest text + spacing + max number width
        let spacing = 2u16;
        let w = widest
            .saturating_add(max_num_width)
            .saturating_add(spacing)
            .min(results_area.width.saturating_sub(2));
        // Place menu below the Panels button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
        let pbx = app
            .panels_button_rect
            .map(|(x, _, _, _)| x)
            .unwrap_or(max_x);
        let menu_x = pbx.min(max_x);
        let h = u16::try_from(opts.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2); // borders
        let menu_y = results_area.y.saturating_add(1); // just below top border (rendered on top layer)
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.panels_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        // Build lines with right-aligned row numbers 1..N
        // Use Unicode display width for accurate alignment with wide characters
        let num_start_col = widest + spacing; // Column where numbers start (display width)
        let total_line_width = w; // All lines must be exactly this display width
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let num_str = format!("{}", i + 1);
            // Pad number to max_num_width for right alignment (numbers are ASCII, so len() = width)
            let num_width = u16::try_from(num_str.len()).unwrap_or(u16::MAX);
            let num_padding = max_num_width.saturating_sub(num_width);
            let padded_num = format!("{}{}", " ".repeat(num_padding as usize), num_str);

            // Calculate padding using display width, not byte length
            let text_display_width = u16::try_from(text.width()).unwrap_or(u16::MAX);
            let text_padding = widest.saturating_sub(text_display_width);

            // Build complete line
            let mut complete_line = format!(
                "{}{}{}{}",
                text,
                " ".repeat(text_padding as usize),
                " ".repeat(spacing as usize),
                padded_num
            );

            // Ensure line has exactly total_line_width display width
            let current_width = u16::try_from(complete_line.width()).unwrap_or(u16::MAX);
            if current_width < total_line_width {
                complete_line.push_str(&" ".repeat((total_line_width - current_width) as usize));
            } else if current_width > total_line_width {
                // Truncate by display width, not byte length
                let mut truncated = String::new();
                let mut width_so_far = 0u16;
                for ch in complete_line.chars() {
                    let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
                    if width_so_far + ch_width > total_line_width {
                        break;
                    }
                    truncated.push(ch);
                    width_so_far += ch_width;
                }
                complete_line = truncated;
            }

            // Split at num_start_col display width for styling
            let mut text_part = String::new();
            let mut width_so_far = 0u16;
            for ch in complete_line.chars() {
                let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
                if width_so_far + ch_width > num_start_col {
                    break;
                }
                text_part.push(ch);
                width_so_far += ch_width;
            }
            let num_part = complete_line
                .chars()
                .skip(text_part.chars().count())
                .collect::<String>();

            lines.push(Line::from(vec![
                Span::styled(text_part, Style::default().fg(th.text)),
                Span::styled(num_part, Style::default().fg(th.overlay1)),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: false })
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
        // Use Unicode display width, not byte length, to handle wide characters
        let widest = opts
            .iter()
            .map(|s| u16::try_from(s.width()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0);
        // Calculate max number width first to include it in total width
        let max_num_width = u16::try_from(format!("{}", opts.len()).len()).unwrap_or(u16::MAX);
        // Width must accommodate: widest text + spacing + max number width
        let w = widest
            .saturating_add(max_num_width)
            .saturating_add(2) // spacing between text and number
            .min(results_area.width.saturating_sub(2));
        // Place menu below the Options button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
        let obx = app
            .options_button_rect
            .map(|(x, _, _, _)| x)
            .unwrap_or(max_x);
        let menu_x = obx.min(max_x);
        let h = u16::try_from(opts.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2); // borders
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
        // Use Unicode display width for accurate alignment with wide characters
        let spacing = 2u16;
        let num_start_col = widest + spacing; // Column where numbers start (display width)
        let total_line_width = w; // All lines must be exactly this display width
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let num_str = format!("{}", i + 1);
            // Pad number to max_num_width for right alignment (numbers are ASCII, so len() = width)
            let num_width = u16::try_from(num_str.len()).unwrap_or(u16::MAX);
            let num_padding = max_num_width.saturating_sub(num_width);
            let padded_num = format!("{}{}", " ".repeat(num_padding as usize), num_str);

            // Calculate padding using display width, not byte length
            let text_display_width = u16::try_from(text.width()).unwrap_or(u16::MAX);
            let text_padding = widest.saturating_sub(text_display_width);

            // Build complete line
            let mut complete_line = format!(
                "{}{}{}{}",
                text,
                " ".repeat(text_padding as usize),
                " ".repeat(spacing as usize),
                padded_num
            );

            // Ensure line has exactly total_line_width display width
            let current_width = u16::try_from(complete_line.width()).unwrap_or(u16::MAX);
            if current_width < total_line_width {
                complete_line.push_str(&" ".repeat((total_line_width - current_width) as usize));
            } else if current_width > total_line_width {
                // Truncate by display width, not byte length
                let mut truncated = String::new();
                let mut width_so_far = 0u16;
                for ch in complete_line.chars() {
                    let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
                    if width_so_far + ch_width > total_line_width {
                        break;
                    }
                    truncated.push(ch);
                    width_so_far += ch_width;
                }
                complete_line = truncated;
            }

            // Split at num_start_col display width for styling
            let mut text_part = String::new();
            let mut width_so_far = 0u16;
            for ch in complete_line.chars() {
                let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
                if width_so_far + ch_width > num_start_col {
                    break;
                }
                text_part.push(ch);
                width_so_far += ch_width;
            }
            let num_part = complete_line
                .chars()
                .skip(text_part.chars().count())
                .collect::<String>();

            lines.push(Line::from(vec![
                Span::styled(text_part, Style::default().fg(th.text)),
                Span::styled(num_part, Style::default().fg(th.overlay1)),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: false })
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
            let widest = opts
                .iter()
                .map(|(s, _)| u16::try_from(s.len()).unwrap_or(u16::MAX))
                .max()
                .unwrap_or(0);
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
            let h = u16::try_from(opts.len())
                .unwrap_or(u16::MAX)
                .saturating_add(2); // borders
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
                    .saturating_sub(u16::try_from(text.len()).unwrap_or(u16::MAX))
                    .saturating_sub(u16::try_from(indicator.len()).unwrap_or(u16::MAX));
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
