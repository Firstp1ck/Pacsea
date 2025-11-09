use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

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
        let opts = [
            "Settings -> settings.conf",
            "Theme -> theme.conf",
            "Keybindings -> keybinds.conf",
            "Install List -> install_list.json",
            "Installed Packages -> installed_packages.txt",
            "Recent Searches -> recent_searches.json",
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
                Span::styled(text.to_string(), Style::default().fg(th.text)),
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
                            "C",
                            Style::default()
                                .fg(th.overlay1)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled("onfig/Lists ", Style::default().fg(th.overlay1)),
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
            "Hide Recent"
        } else {
            "Show Recent"
        };
        let label_install = if app.show_install_pane {
            "Hide Install List"
        } else {
            "Show Install List"
        };
        let label_keybinds = if app.show_keybinds_footer {
            "Hide Keybinds"
        } else {
            "Show Keybinds"
        };
        let opts = [label_recent, label_install, label_keybinds];
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
                Span::styled(text.to_string(), Style::default().fg(th.text)),
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
                            "P",
                            Style::default()
                                .fg(th.overlay1)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled("anels ", Style::default().fg(th.overlay1)),
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
            "List all packages"
        } else {
            "List installed packages"
        };
        let opts = [label_toggle, "Update System", "News", "TUI Optional Dep's"];
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
                Span::styled(text.to_string(), Style::default().fg(th.text)),
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
                            "O",
                            Style::default()
                                .fg(th.overlay1)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled("ptions ", Style::default().fg(th.overlay1)),
                    ]))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.mauve)),
            );
        f.render_widget(Clear, rect);
        f.render_widget(menu, rect);
    }
}
