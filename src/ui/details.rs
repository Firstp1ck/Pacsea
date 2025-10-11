use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::state::{AppState, Focus};
use crate::theme::{KeyChord, theme};

/// Render the bottom details pane and optional PKGBUILD viewer.
///
/// Updates geometry fields on [`AppState`] for mouse hit-testing and draws a
/// contextual footer with keybindings. When `app.pkgb_visible` is true, splits
/// the area to show the PKGBUILD content with scroll support.
pub fn render_details(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    // Details (bottom): reserve space for footer, then render content (details/PKGBUILD)
    let bottom_container = area;
    let base_help_h: u16 = if app.show_keybinds_footer { 5 } else { 0 };
    let help_h: u16 = if matches!(app.focus, Focus::Search) && app.search_normal_mode {
        base_help_h.saturating_add(1)
    } else {
        base_help_h
    };
    let content_container = ratatui::prelude::Rect {
        x: bottom_container.x,
        y: bottom_container.y,
        width: bottom_container.width,
        height: bottom_container.height.saturating_sub(help_h),
    };
    let (details_area, pkgb_area_opt) = if app.pkgb_visible {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_container);
        (split[0], Some(split[1]))
    } else {
        (content_container, None)
    };

    let mut details_lines = crate::ui_helpers::format_details_lines(app, details_area.width, &th);
    // Record details inner rect for mouse hit-testing
    app.details_rect = Some((
        details_area.x + 1,
        details_area.y + 1,
        details_area.width.saturating_sub(2),
        details_area.height.saturating_sub(2),
    ));

    // Find the URL line, style it as a link, and record its rect; also compute PKGBUILD rect
    app.url_button_rect = None;
    app.pkgb_button_rect = None;
    let border_inset = 1u16;
    let content_x = details_area.x.saturating_add(border_inset);
    let content_y = details_area.y.saturating_add(border_inset);
    let inner_w: u16 = details_area.width.saturating_sub(2);
    let mut cur_y: u16 = content_y;
    for line in details_lines.iter_mut() {
        if line.spans.len() >= 2 {
            let key_txt = line.spans[0].content.to_string();
            if key_txt.starts_with("URL:") {
                let url_txt = app.details.url.clone();
                let mut style = Style::default().fg(th.text);
                if !url_txt.is_empty() {
                    style = Style::default()
                        .fg(th.mauve)
                        .add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
                }
                line.spans[1] = Span::styled(url_txt.clone(), style);
                if !url_txt.is_empty() {
                    let key_len = key_txt.len() as u16;
                    let x_start = content_x.saturating_add(key_len);
                    let max_w = inner_w.saturating_sub(key_len);
                    let w = url_txt.len().min(max_w as usize) as u16;
                    if w > 0 {
                        app.url_button_rect = Some((x_start, cur_y, w, 1));
                    }
                }
            }
        }
        if line.spans.len() == 1 {
            let txt = line.spans[0].content.to_string();
            let lowered = txt.to_lowercase();
            if lowered.contains("show pkgbuild") || lowered.contains("hide pkgbuild") {
                let x_start = content_x;
                let w = txt.len().min(inner_w as usize) as u16;
                if w > 0 {
                    app.pkgb_button_rect = Some((x_start, cur_y, w, 1));
                }
            }
        }
        // advance y accounting for wrapping
        let line_len: usize = line.spans.iter().map(|s| s.content.len()).sum();
        let rows = if inner_w == 0 {
            1
        } else {
            (line_len as u16).div_ceil(inner_w).max(1)
        };
        cur_y = cur_y.saturating_add(rows);
    }

    let details_block = Block::default()
        .title(Span::styled(
            "Package Info",
            Style::default().fg(th.overlay1),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.surface2));
    let details = Paragraph::new(details_lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: true })
        .block(details_block.clone());
    f.render_widget(details, details_area);

    // Allow terminal to mark/select text in details: ignore clicks within details by default
    app.mouse_disabled_in_details = true;

    if let Some(pkgb_area) = pkgb_area_opt {
        let pkgb_text = app.pkgb_text.as_deref().unwrap_or("Loading PKGBUILD…");
        // Remember PKGBUILD rect for mouse interactions (scrolling)
        app.pkgb_rect = Some((
            pkgb_area.x + 1,
            pkgb_area.y + 1,
            pkgb_area.width.saturating_sub(2),
            pkgb_area.height.saturating_sub(2),
        ));
        // Apply vertical scroll offset by trimming top lines
        let mut visible = String::new();
        let mut skip = app.pkgb_scroll as usize;
        for line in pkgb_text.lines() {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            visible.push_str(line);
            visible.push('\n');
        }
        // Title with clickable "Check Package Build" button
        let check_button_label = "Check Package Build".to_string();
        let mut pkgb_title_spans: Vec<Span> =
            vec![Span::styled("PKGBUILD", Style::default().fg(th.overlay1))];
        pkgb_title_spans.push(Span::raw("  "));
        let check_btn_style = Style::default()
            .fg(th.mauve)
            .bg(th.surface2)
            .add_modifier(Modifier::BOLD);
        pkgb_title_spans.push(Span::styled(check_button_label.clone(), check_btn_style));

        // Record clickable rect for the title button on the top border row
        let btn_y = pkgb_area.y;
        let btn_x = pkgb_area
            .x
            .saturating_add(1)
            .saturating_add("PKGBUILD".len() as u16)
            .saturating_add(2);
        let btn_w = check_button_label.len() as u16;
        app.pkgb_check_button_rect = Some((btn_x, btn_y, btn_w, 1));

        let pkgb = Paragraph::new(visible)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title(Line::from(pkgb_title_spans))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.surface2)),
            );
        f.render_widget(pkgb, pkgb_area);
    }

    // Help footer with keybindings in the bottom of Package Info pane
    if app.show_keybinds_footer {
        // Footer occupies the bottom rows of the bottom container using reserved height above
        let footer_container = bottom_container;
        if footer_container.height > help_h + 2 {
            let x = footer_container.x + 1; // inside border
            let y_top = footer_container.y + footer_container.height.saturating_sub(help_h);
            let w = footer_container.width.saturating_sub(2);
            let h = help_h;
            let footer_rect = ratatui::prelude::Rect {
                x,
                y: y_top,
                width: w,
                height: h,
            };

            let search_label_color = if matches!(app.focus, Focus::Search) {
                th.mauve
            } else {
                th.overlay1
            };
            let install_label_color = if matches!(app.focus, Focus::Install) {
                th.mauve
            } else {
                th.overlay1
            };
            let recent_label_color = if matches!(app.focus, Focus::Recent) {
                th.mauve
            } else {
                th.overlay1
            };

            let key_style = Style::default().fg(th.text).add_modifier(Modifier::BOLD);
            let sep = Span::styled("  |  ", Style::default().fg(th.overlay2));

            // GLOBALS (dynamic from keymap)
            let km = &app.keymap;
            let mut g_spans: Vec<Span> = vec![
                Span::styled(
                    "GLOBALS:",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            if let Some(k) = km.exit.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" exit"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.help_overlay.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" help"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.reload_theme.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" reload theme"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.show_pkgbuild.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" PKGBUILD"),
                    sep.clone(),
                ]);
            }
            // Change sorting (global) using configured keybind
            if let Some(k) = km.change_sort.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" change sorting"),
                    sep.clone(),
                ]);
            }

            // SEARCH
            let mut s_spans: Vec<Span> = vec![
                Span::styled(
                    "SEARCH:",
                    Style::default()
                        .fg(search_label_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            // Move
            if let (Some(up), Some(dn)) = (km.search_move_up.first(), km.search_move_down.first()) {
                s_spans.extend([
                    Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                    Span::raw(" move"),
                    sep.clone(),
                ]);
            }
            // Page
            if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
                s_spans.extend([
                    Span::styled(format!("[{} / {}]", pu.label(), pd.label()), key_style),
                    Span::raw(" page"),
                    sep.clone(),
                ]);
            }
            // Add / Install
            if let Some(k) = km.search_add.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" add"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.search_install.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" install"),
                    sep.clone(),
                ]);
            }
            // Switch pane (next only)
            if let Some(n) = km.pane_next.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", n.label()), key_style),
                    Span::raw(" next pane"),
                    sep.clone(),
                ]);
            }
            // Backspace
            if let Some(k) = km.search_backspace.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" delete"),
                    sep.clone(),
                ]);
            }
            // Focus left/right within Search
            if let Some(k) = km.search_focus_left.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" focus left"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.search_focus_right.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" focus right"),
                    sep.clone(),
                ]);
            }

            // INSTALL
            let mut i_spans: Vec<Span> = vec![
                Span::styled(
                    "INSTALL:",
                    Style::default()
                        .fg(install_label_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first())
            {
                i_spans.extend([
                    Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                    Span::raw(" move"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_confirm.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" confirm"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_remove.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" remove"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_clear.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" clear"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_find.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" find (Enter next, Esc cancel)"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_to_search.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" to Search"),
                    sep.clone(),
                ]);
            }
            // Pane switch (next only)
            if let Some(n) = km.pane_next.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", n.label()), key_style),
                    Span::raw(" next pane"),
                    sep.clone(),
                ]);
            }
            // Focus left within Install
            if let Some(k) = km.install_focus_left.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" to Search"),
                    sep.clone(),
                ]);
            }

            // RECENT
            let mut r_spans: Vec<Span> = vec![
                Span::styled(
                    "RECENT:",
                    Style::default()
                        .fg(recent_label_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
                r_spans.extend([
                    Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                    Span::raw(" move"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_use.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" use"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_add.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" add"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_find.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" find (Enter next, Esc cancel)"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_to_search.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" to Search"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_remove.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" delete"),
                    sep.clone(),
                ]);
            }
            if let Some(n) = km.pane_next.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", n.label()), key_style),
                    Span::raw(" next pane"),
                ]);
            }
            // Focus right within Recent
            if let Some(k) = km.recent_focus_right.first() {
                r_spans.extend([
                    sep.clone(),
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" to Search"),
                ]);
            }

            // Optional Normal Mode line when Search is focused and active
            let mut lines: Vec<Line> = vec![
                Line::from(g_spans),
                Line::from(s_spans),
                Line::from(i_spans),
                Line::from(r_spans),
            ];
            if matches!(app.focus, Focus::Search) && app.search_normal_mode {
                // Use configured labels
                let label = |v: &Vec<KeyChord>, def: &str| {
                    v.first()
                        .map(|c| c.label())
                        .unwrap_or_else(|| def.to_string())
                };
                let toggle_label = label(&km.search_normal_toggle, "Esc");
                let insert_label = label(&km.search_normal_insert, "i");
                let left_label = label(&km.search_normal_select_left, "h");
                let right_label = label(&km.search_normal_select_right, "l");
                let delete_label = label(&km.search_normal_delete, "d");

                let n_spans: Vec<Span> = vec![
                    Span::styled(
                        "Normal Mode (Focused Search Window):",
                        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(format!("[{toggle_label}]"), key_style),
                    Span::raw(" toggle, "),
                    Span::styled(format!("[{insert_label}]"), key_style),
                    Span::raw(" insert, "),
                    Span::styled("[j / k]", key_style),
                    Span::raw(" move, "),
                    Span::styled("[Ctrl+d / Ctrl+u]", key_style),
                    Span::raw(" page, "),
                    Span::styled(format!("[{left_label} / {right_label}]"), key_style),
                    Span::raw(" select, "),
                    Span::styled(format!("[{delete_label}]"), key_style),
                    Span::raw(" delete"),
                ];
                lines.push(Line::from(n_spans));
            }
            // Bottom-align the content within the reserved footer area
            let content_lines: u16 = if matches!(app.focus, Focus::Search) && app.search_normal_mode
            {
                5
            } else {
                4
            };
            let content_y = y_top + h.saturating_sub(content_lines);
            let content_rect = ratatui::prelude::Rect {
                x,
                y: content_y,
                width: w,
                height: content_lines,
            };
            // Fill the whole reserved footer area with a uniform background
            f.render_widget(
                Block::default().style(Style::default().bg(th.base)),
                footer_rect,
            );
            let footer = Paragraph::new(lines)
                .style(Style::default().fg(th.subtext1))
                .wrap(Wrap { trim: true });
            f.render_widget(footer, content_rect);
        }
    }
}
