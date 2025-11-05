use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::state::{AppState, Focus, RightPaneFocus};
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
    // Reserve footer height: baseline lines + optional Normal Mode line
    // Baseline: GLOBALS, SEARCH, INSTALL, RECENT (4). In installed-only mode, split to 5 lines: GLOBALS, SEARCH, DOWNGRADE, REMOVE, RECENT.
    let baseline_lines: u16 = if app.installed_only_mode { 5 } else { 4 };
    let base_help_h: u16 = if app.show_keybinds_footer {
        baseline_lines
    } else {
        0
    };
    // Compute adaptive extra rows for Search Normal Mode footer based on available width
    let km = &app.keymap;
    let footer_w: u16 = bottom_container.width.saturating_sub(2);
    let nm_rows: u16 = if matches!(app.focus, Focus::Search) && app.search_normal_mode {
        // Build the same labels used in the footer
        let toggle_label = km
            .search_normal_toggle
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "Esc".to_string());
        let insert_label = km
            .search_normal_insert
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "i".to_string());
        let left_label = km
            .search_normal_select_left
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "h".to_string());
        let right_label = km
            .search_normal_select_right
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "l".to_string());
        let delete_label = km
            .search_normal_delete
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "d".to_string());

        let line1 = format!(
            "Normal Mode (Focused Search Window):  [{}] toggle, [{}] insert, [j / k] move, [Ctrl+d / Ctrl+u] page, [{} / {}] Select text, [{}] Delete text",
            toggle_label, insert_label, left_label, right_label, delete_label
        );
        // Menus and Import/Export on an additional line when present
        let mut line2 = String::new();
        if !km.config_menu_toggle.is_empty()
            || !km.options_menu_toggle.is_empty()
            || !km.panels_menu_toggle.is_empty()
            || (!app.installed_only_mode
                && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty()))
        {
            // Menus
            if !km.config_menu_toggle.is_empty()
                || !km.options_menu_toggle.is_empty()
                || !km.panels_menu_toggle.is_empty()
            {
                line2.push_str("  •  Open Menus: ");
                if let Some(k) = km.config_menu_toggle.first() {
                    line2.push_str(&format!("[{}] Config", k.label()));
                }
                if let Some(k) = km.options_menu_toggle.first() {
                    if !line2.ends_with("menus: ") {
                        line2.push_str(", ");
                    }
                    line2.push_str(&format!("[{}] Options", k.label()));
                }
                if let Some(k) = km.panels_menu_toggle.first() {
                    if !line2.ends_with("menus: ") {
                        line2.push_str(", ");
                    }
                    line2.push_str(&format!("[{}] Panels", k.label()));
                }
            }
            // Import / Export
            if !app.installed_only_mode
                && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty())
            {
                line2.push_str("  •  ");
                if let Some(k) = km.search_normal_import.first() {
                    line2.push_str(&format!("[{}] Import", k.label()));
                    if let Some(k2) = km.search_normal_export.first() {
                        line2.push_str(&format!(", [{}] Export", k2.label()));
                    }
                } else if let Some(k) = km.search_normal_export.first() {
                    line2.push_str(&format!("[{}] Export", k.label()));
                }
            }
        }
        let w = if footer_w == 0 { 1 } else { footer_w };
        let rows1 = ((line1.len() as u16).div_ceil(w)).max(1);
        let rows2 = if line2.is_empty() {
            0
        } else {
            ((line2.len() as u16).div_ceil(w)).max(1)
        };
        rows1 + rows2
    } else {
        0
    };
    let help_h: u16 = if app.show_keybinds_footer {
        base_help_h.saturating_add(nm_rows)
    } else {
        0
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

    let mut details_lines = crate::ui::helpers::format_details_lines(app, details_area.width, &th);
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
        // Title with clickable "Copy Package Build" button and optional "Reload PKGBUILD" button
        let check_button_label = "Copy Package Build".to_string();
        let mut pkgb_title_spans: Vec<Span> =
            vec![Span::styled("PKGBUILD", Style::default().fg(th.overlay1))];
        pkgb_title_spans.push(Span::raw("  "));
        let check_btn_style = Style::default()
            .fg(th.mauve)
            .bg(th.surface2)
            .add_modifier(Modifier::BOLD);
        pkgb_title_spans.push(Span::styled(check_button_label.clone(), check_btn_style));

        // Check if PKGBUILD is for a different package than currently selected
        let current_package = app.results.get(app.selected).map(|i| i.name.as_str());
        let needs_reload = app.pkgb_package_name.as_deref() != current_package && app.pkgb_package_name.is_some();
        
        // Record clickable rect for the "Copy Package Build" button on the top border row
        let btn_y = pkgb_area.y;
        let btn_x = pkgb_area
            .x
            .saturating_add(1)
            .saturating_add("PKGBUILD".len() as u16)
            .saturating_add(2);
        let btn_w = check_button_label.len() as u16;
        app.pkgb_check_button_rect = Some((btn_x, btn_y, btn_w, 1));

        // Add "Reload PKGBUILD" button if needed
        app.pkgb_reload_button_rect = None;
        if needs_reload {
            pkgb_title_spans.push(Span::raw("  "));
            let reload_button_label = "Reload PKGBUILD".to_string();
            let reload_btn_style = Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD);
            pkgb_title_spans.push(Span::styled(reload_button_label.clone(), reload_btn_style));
            
            // Record clickable rect for the reload button
            let reload_btn_x = btn_x.saturating_add(btn_w).saturating_add(2);
            let reload_btn_w = reload_button_label.len() as u16;
            app.pkgb_reload_button_rect = Some((reload_btn_x, btn_y, reload_btn_w, 1));
        }

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
            // Subpane label colors when installed-only mode splits the right pane
            let downgrade_label_color = if matches!(app.focus, Focus::Install)
                && matches!(app.right_pane_focus, RightPaneFocus::Downgrade)
            {
                th.mauve
            } else {
                th.overlay1
            };
            let remove_label_color = if matches!(app.focus, Focus::Install)
                && matches!(app.right_pane_focus, RightPaneFocus::Remove)
            {
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
                    "GLOBALS:  ",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            if let Some(k) = km.exit.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Exit"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.help_overlay.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Help"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.reload_theme.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Reload theme"),
                    sep.clone(),
                ]);
            }
            // Menu toggles are shown under Search (Normal mode) now
            if let Some(k) = km.show_pkgbuild.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Show/Hide PKGBUILD"),
                    sep.clone(),
                ]);
            }
            // Change sorting (global) using configured keybind
            if let Some(k) = km.change_sort.first() {
                g_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Change Sort Mode"),
                    sep.clone(),
                ]);
            }
            // (Pane focus left/right intentionally omitted from footer)

            // SEARCH
            let mut s_spans: Vec<Span> = vec![
                Span::styled(
                    "SEARCH:   ",
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
                    Span::raw(" Move"),
                    sep.clone(),
                ]);
            }
            // Page
            if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
                s_spans.extend([
                    Span::styled(format!("[{} / {}]", pu.label(), pd.label()), key_style),
                    Span::raw(" Move Page"),
                    sep.clone(),
                ]);
            }
            // Add / Install
            if let Some(k) = km.search_add.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Add to install"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.search_install.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Install"),
                    sep.clone(),
                ]);
            }
            // Normal Mode toggle (always visible in footer)
            if let Some(k) = km.search_normal_toggle.first() {
                s_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Normal Mode"),
                    sep.clone(),
                ]);
            }
            // (Pane next, delete char, and focus left/right intentionally omitted from footer)

            // INSTALL or split into DOWNGRADE and REMOVE when installed-only mode is active
            // Helper to build common spans for right-pane actions
            let build_right_spans = |label: &str, label_color, confirm_text: &str| {
                let mut spans: Vec<Span> = vec![
                    Span::styled(
                        label.to_string(),
                        Style::default()
                            .fg(label_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                ];
                if let (Some(up), Some(dn)) =
                    (km.install_move_up.first(), km.install_move_down.first())
                {
                    spans.extend([
                        Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                        Span::raw(" Move"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_confirm.first() {
                    spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(format!(" {}", confirm_text)),
                        sep.clone(),
                    ]);
                }
                if !km.install_remove.is_empty() {
                    let keys = km
                        .install_remove
                        .iter()
                        .map(|c| c.label())
                        .collect::<Vec<_>>()
                        .join(" / ");
                    spans.extend([
                        Span::styled(format!("[{keys}]"), key_style),
                        Span::raw(" Remove from List"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_clear.first() {
                    spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(" Clear"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_find.first() {
                    spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(" Search (Enter next, Esc cancel)"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_to_search.first() {
                    spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(" Go to Search"),
                        sep.clone(),
                    ]);
                }
                spans
            };

            let (right_lines_install, right_lines_split) = if app.installed_only_mode {
                let d_spans = build_right_spans(
                    "DOWNGRADE:",
                    downgrade_label_color,
                    "Confirm package Downgrade",
                );
                let r_spans =
                    build_right_spans("REMOVE:   ", remove_label_color, "Confirm package Removal");
                (None, Some((Line::from(d_spans), Line::from(r_spans))))
            } else {
                let mut i_spans: Vec<Span> = vec![
                    Span::styled(
                        "INSTALL:  ",
                        Style::default()
                            .fg(install_label_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                ];
                if let (Some(up), Some(dn)) =
                    (km.install_move_up.first(), km.install_move_down.first())
                {
                    i_spans.extend([
                        Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                        Span::raw(" Move"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_confirm.first() {
                    i_spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(" Confirm"),
                        sep.clone(),
                    ]);
                }
                if !km.install_remove.is_empty() {
                    let keys = km
                        .install_remove
                        .iter()
                        .map(|c| c.label())
                        .collect::<Vec<_>>()
                        .join(" / ");
                    i_spans.extend([
                        Span::styled(format!("[{keys}]"), key_style),
                        Span::raw(" Remove from List"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_clear.first() {
                    i_spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(" Clear"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_find.first() {
                    i_spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(" Search (Enter next, Esc cancel)"),
                        sep.clone(),
                    ]);
                }
                if let Some(k) = km.install_to_search.first() {
                    i_spans.extend([
                        Span::styled(format!("[{}]", k.label()), key_style),
                        Span::raw(" Go to Search"),
                        sep.clone(),
                    ]);
                }
                (Some(Line::from(i_spans)), None)
            };

            // RECENT
            let mut r_spans: Vec<Span> = vec![
                Span::styled(
                    "RECENT:   ",
                    Style::default()
                        .fg(recent_label_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
                r_spans.extend([
                    Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                    Span::raw(" Move"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_use.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Add to Search"),
                    sep.clone(),
                ]);
            }
            if !km.recent_remove.is_empty() {
                let keys = km
                    .recent_remove
                    .iter()
                    .map(|c| c.label())
                    .collect::<Vec<_>>()
                    .join(" / ");
                r_spans.extend([
                    Span::styled(format!("[{keys}]"), key_style),
                    Span::raw(" Remove from List"),
                    sep.clone(),
                ]);
            }
            // Clear all entries in Recent: configurable keybind (fallback to Shift+Del label)
            if let Some(k) = km.recent_clear.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Clear"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_add.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Add first match to Install list"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_find.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Search (Enter next, Esc cancel)"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.recent_to_search.first() {
                r_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Go to Search"),
                    sep.clone(),
                ]);
            }
            // (Pane next and focus right intentionally omitted from footer)

            // Optional Normal Mode line when Search is focused and active
            let mut lines: Vec<Line> = vec![Line::from(g_spans), Line::from(s_spans)];
            if let Some(i_line) = right_lines_install {
                lines.push(i_line);
            }
            if let Some((d_line, rm_line)) = right_lines_split {
                lines.push(d_line);
                lines.push(rm_line);
            }
            lines.push(Line::from(r_spans));
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
                        "Normal Mode:",
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
                    Span::raw(" Select text, "),
                    Span::styled(format!("[{delete_label}]"), key_style),
                    Span::raw(" Delete text"),
                    // Close first line (base Normal Mode help)
                ];
                lines.push(Line::from(n_spans));

                // Second line: menus and import/export (if any)
                let mut n2_spans: Vec<Span> = Vec::new();

                // Menus: show configured Normal-mode menu toggles
                if !km.config_menu_toggle.is_empty()
                    || !km.options_menu_toggle.is_empty()
                    || !km.panels_menu_toggle.is_empty()
                {
                    n2_spans.push(Span::raw("  •  Open Menus: "));
                    let mut any = false;
                    if let Some(k) = km.config_menu_toggle.first() {
                        n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                        n2_spans.push(Span::raw(" Config"));
                        any = true;
                    }
                    if let Some(k) = km.options_menu_toggle.first() {
                        if any {
                            n2_spans.push(Span::raw(", "));
                        }
                        n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                        n2_spans.push(Span::raw(" Options"));
                        any = true;
                    }
                    if let Some(k) = km.panels_menu_toggle.first() {
                        if any {
                            n2_spans.push(Span::raw(", "));
                        }
                        n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                        n2_spans.push(Span::raw(" Panels"));
                    }
                }

                // Import/Export shortcuts on the same second line
                if !app.installed_only_mode
                    && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty())
                {
                    n2_spans.push(Span::raw("  • Install List:  "));
                    if let Some(k) = km.search_normal_import.first() {
                        n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                        n2_spans.push(Span::raw(" Import"));
                        if let Some(k2) = km.search_normal_export.first() {
                            n2_spans.push(Span::raw(", "));
                            n2_spans.push(Span::styled(format!("[{}]", k2.label()), key_style));
                            n2_spans.push(Span::raw(" Export"));
                        }
                    } else if let Some(k) = km.search_normal_export.first() {
                        n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                        n2_spans.push(Span::raw(" Export"));
                    }
                }

                if !n2_spans.is_empty() {
                    lines.push(Line::from(n2_spans));
                }
            }
            // Bottom-align the content within the reserved footer area
            // Reserve exactly the number of wrapped rows needed
            let content_lines: u16 = if matches!(app.focus, Focus::Search) && app.search_normal_mode
            {
                baseline_lines.saturating_add(nm_rows)
            } else {
                baseline_lines
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

#[cfg(test)]
mod tests {
    /// What: Details render sets URL/PKGBUILD rects and mouse flags
    ///
    /// - Input: AppState with URL present and PKGBUILD visible
    /// - Output: details/url/PKGBUILD rects are Some; mouse is disabled in details
    #[test]
    fn details_sets_url_and_pkgb_rects() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();

        let mut app = crate::state::AppState {
            ..Default::default()
        };
        app.details = crate::state::PackageDetails {
            repository: "extra".into(),
            name: "ripgrep".into(),
            version: "14".into(),
            description: String::new(),
            architecture: "x86_64".into(),
            url: "https://example.com".into(),
            licenses: vec![],
            groups: vec![],
            provides: vec![],
            depends: vec![],
            opt_depends: vec![],
            required_by: vec![],
            optional_for: vec![],
            conflicts: vec![],
            replaces: vec![],
            download_size: None,
            install_size: None,
            owner: String::new(),
            build_date: String::new(),
            popularity: None,
        };
        // Show PKGBUILD area
        app.pkgb_visible = true;
        app.pkgb_text = Some("line1\nline2\nline3".into());

        term.draw(|f| {
            let area = f.area();
            super::render_details(f, &mut app, area);
        })
        .unwrap();

        assert!(app.details_rect.is_some());
        assert!(app.url_button_rect.is_some());
        assert!(app.pkgb_button_rect.is_some());
        assert!(app.pkgb_check_button_rect.is_some());
        assert!(app.pkgb_rect.is_some());
        assert!(app.mouse_disabled_in_details);
    }
}
