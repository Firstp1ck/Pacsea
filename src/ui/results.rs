use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::state::{AppState, SortMode, Source};
use crate::theme::theme;

/// Render the top results list and title controls.
///
/// Keeps the selection centered when possible, displays badges and descriptions,
/// and records hit-test rectangles for the Sort button, dropdown, and filter
/// toggles.
pub fn render_results(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    // Detect availability of optional repos from the official index
    let (has_eos, has_cachyos) = {
        let mut eos = false;
        let mut cach = false;
        for it in crate::index::all_official().iter() {
            if let Source::Official { repo, .. } = &it.source {
                let r = repo.to_lowercase();
                if !eos && (r == "eos" || r == "endeavouros") {
                    eos = true;
                }
                if !cach && r.starts_with("cachyos") {
                    cach = true;
                }
                if eos && cach {
                    break;
                }
            }
        }
        (eos, cach)
    };

    // Keep selection centered within the visible results list when possible
    {
        let viewport_rows = area.height.saturating_sub(2) as usize; // account for borders
        let len = app.results.len();
        let selected_idx = if app.results.is_empty() {
            None
        } else {
            Some(app.selected.min(len - 1))
        };

        if viewport_rows > 0 && len > viewport_rows {
            let selected = selected_idx.unwrap_or(0);
            let max_offset = len.saturating_sub(viewport_rows);
            let desired = selected.saturating_sub(viewport_rows / 2).min(max_offset);
            if app.list_state.offset() != desired {
                let mut st = ratatui::widgets::ListState::default().with_offset(desired);
                st.select(selected_idx);
                app.list_state = st;
            } else {
                // ensure selection is set
                app.list_state.select(selected_idx);
            }
        } else {
            // Small lists: ensure offset is 0 and selection is applied
            if app.list_state.offset() != 0 {
                let mut st = ratatui::widgets::ListState::default().with_offset(0);
                st.select(selected_idx);
                app.list_state = st;
            } else {
                app.list_state.select(selected_idx);
            }
        }
    }

    // Results list (top)
    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|p| {
            let (src, color) = match &p.source {
                Source::Official { repo, .. } => {
                    let rl = repo.to_lowercase();
                    let label = if rl == "eos" || rl == "endeavouros" {
                        "EOS".to_string()
                    } else if rl.starts_with("cachyos") {
                        "CachyOS".to_string()
                    } else {
                        repo.to_string()
                    };
                    (label, th.green)
                }
                Source::Aur => ("AUR".to_string(), th.yellow),
            };
            let desc = if p.description.is_empty() {
                app.details_cache
                    .get(&p.name)
                    .map(|d| d.description.clone())
                    .unwrap_or_default()
            } else {
                p.description.clone()
            };
            let installed = crate::index::is_installed(&p.name);
            let mut segs: Vec<Span> = Vec::new();
            // Popularity (AUR) shown before repo label when available
            if let Some(pop) = p.popularity {
                segs.push(Span::styled(
                    format!("Pop: {pop:.2} "),
                    Style::default().fg(th.overlay1),
                ));
            }
            // Repo / source label
            segs.push(Span::styled(format!("{src} "), Style::default().fg(color)));
            // Name and version
            segs.push(Span::styled(
                p.name.clone(),
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            ));
            segs.push(Span::styled(
                format!("  {}", p.version),
                Style::default().fg(th.overlay1),
            ));
            if !desc.is_empty() {
                segs.push(Span::raw("  - "));
                segs.push(Span::styled(desc, Style::default().fg(th.overlay2)));
            }
            if installed {
                segs.push(Span::raw("  "));
                segs.push(Span::styled(
                    "[Installed]",
                    Style::default().fg(th.green).add_modifier(Modifier::BOLD),
                ));
            }
            ListItem::new(Line::from(segs))
        })
        .collect();

    // Build title with Sort button, filter toggles, and a right-aligned Options button
    let results_title_text = format!("Results ({})", app.results.len());
    let sort_button_label = "Sort v".to_string();
    let options_button_label = "Options v".to_string();
    let panels_button_label = "Panels v".to_string();
    let config_button_label = "Config/Lists v".to_string();
    let mut title_spans: Vec<Span> = vec![Span::styled(
        results_title_text.clone(),
        Style::default().fg(th.overlay1),
    )];
    title_spans.push(Span::raw("  "));
    // Style the sort button differently when menu is open
    let btn_style = if app.sort_menu_open {
        Style::default()
            .fg(th.crust)
            .bg(th.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(th.mauve)
            .bg(th.surface2)
            .add_modifier(Modifier::BOLD)
    };
    title_spans.push(Span::styled(sort_button_label.clone(), btn_style));
    title_spans.push(Span::raw("  "));
    // Filter toggles: [AUR] [core] [extra] [multilib] and optional [EOS]/[CachyOS]
    let filt = |label: &str, on: bool| -> Span<'static> {
        let (fg, bg) = if on {
            (th.crust, th.green)
        } else {
            (th.mauve, th.surface2)
        };
        Span::styled(
            format!("[{label}]"),
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
        )
    };
    title_spans.push(filt("AUR", app.results_filter_show_aur));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt("core", app.results_filter_show_core));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt("extra", app.results_filter_show_extra));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt("multilib", app.results_filter_show_multilib));
    if has_eos {
        title_spans.push(Span::raw(" "));
        title_spans.push(filt("EOS", app.results_filter_show_eos));
    }
    if has_cachyos {
        title_spans.push(Span::raw(" "));
        title_spans.push(filt("CachyOS", app.results_filter_show_cachyos));
    }

    // Estimate and record clickable rects for controls on the title line (top border row)
    let mut x_cursor = area
        .x
        .saturating_add(1) // left border inset
        .saturating_add(results_title_text.len() as u16)
        .saturating_add(2); // two spaces before Sort
    let btn_w = sort_button_label.len() as u16;
    let btn_x = x_cursor;
    let btn_y = area.y; // top border row
    app.sort_button_rect = Some((btn_x, btn_y, btn_w, 1));
    x_cursor = x_cursor.saturating_add(btn_w).saturating_add(2); // space after sort

    // Filter rects in sequence, with single space between
    let rec_rect = |start_x: u16, label: &str| -> (u16, u16, u16, u16) {
        (start_x, btn_y, label.len() as u16, 1)
    };
    let aur_label = "[AUR]";
    app.results_filter_aur_rect = Some(rec_rect(x_cursor, aur_label));
    x_cursor = x_cursor
        .saturating_add(aur_label.len() as u16)
        .saturating_add(1);
    let core_label = "[core]";
    app.results_filter_core_rect = Some(rec_rect(x_cursor, core_label));
    x_cursor = x_cursor
        .saturating_add(core_label.len() as u16)
        .saturating_add(1);
    let extra_label = "[extra]";
    app.results_filter_extra_rect = Some(rec_rect(x_cursor, extra_label));
    x_cursor = x_cursor
        .saturating_add(extra_label.len() as u16)
        .saturating_add(1);
    let multilib_label = "[multilib]";
    app.results_filter_multilib_rect = Some(rec_rect(x_cursor, multilib_label));
    x_cursor = x_cursor
        .saturating_add(multilib_label.len() as u16)
        .saturating_add(1);
    let eos_label = "[EOS]";
    if has_eos {
        app.results_filter_eos_rect = Some(rec_rect(x_cursor, eos_label));
        x_cursor = x_cursor
            .saturating_add(eos_label.len() as u16)
            .saturating_add(1);
    } else {
        app.results_filter_eos_rect = None;
    }
    let cachyos_label = "[CachyOS]";
    if has_cachyos {
        app.results_filter_cachyos_rect = Some(rec_rect(x_cursor, cachyos_label));
    } else {
        app.results_filter_cachyos_rect = None;
    }

    // Right-aligned Config/Lists, Panels and Options buttons: compute remaining space and append to title spans
    let inner_width = area.width.saturating_sub(2); // exclude borders
    let mut consumed_left = (results_title_text.len()
        + 2 // spaces before Sort
        + sort_button_label.len()
        + 2 // spaces after Sort
        + aur_label.len()
        + 1 // space
        + core_label.len()
        + 1 // space
        + extra_label.len()
        + 1 // space
        + multilib_label.len()) as u16;
    if has_eos {
        consumed_left = consumed_left.saturating_add(1 + eos_label.len() as u16);
    }
    if has_cachyos {
        consumed_left = consumed_left.saturating_add(1 + cachyos_label.len() as u16);
    }
    // Minimum single space before right-side buttons when possible
    let options_w = options_button_label.len() as u16;
    let panels_w = panels_button_label.len() as u16;
    let config_w = config_button_label.len() as u16;
    let right_w = config_w
        .saturating_add(1)
        .saturating_add(panels_w)
        .saturating_add(1)
        .saturating_add(options_w); // "Config/Lists" + space + "Panels" + space + "Options"
    let pad = inner_width.saturating_sub(consumed_left.saturating_add(right_w));
    let mut options_btn_x: Option<u16> = None;
    let mut panels_btn_x: Option<u16> = None;
    if pad >= 1 {
        title_spans.push(Span::raw(" ".repeat(pad as usize)));
        let cfg_btn_style = if app.config_menu_open {
            Style::default()
                .fg(th.crust)
                .bg(th.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD)
        };
        title_spans.push(Span::styled(config_button_label.clone(), cfg_btn_style));
        title_spans.push(Span::raw(" "));
        let pan_btn_style = if app.panels_menu_open {
            Style::default()
                .fg(th.crust)
                .bg(th.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD)
        };
        title_spans.push(Span::styled(panels_button_label.clone(), pan_btn_style));
        title_spans.push(Span::raw(" "));
        let opt_btn_style = if app.options_menu_open {
            Style::default()
                .fg(th.crust)
                .bg(th.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD)
        };
        title_spans.push(Span::styled(options_button_label.clone(), opt_btn_style));

        // Record clickable rects at the computed right edge (Panels to the left of Options)
        let opt_x = area
            .x
            .saturating_add(1) // left border inset
            .saturating_add(inner_width.saturating_sub(options_w));
        let pan_x = opt_x.saturating_sub(1).saturating_sub(panels_w);
        let cfg_x = pan_x.saturating_sub(1).saturating_sub(config_w);
        options_btn_x = Some(opt_x);
        panels_btn_x = Some(pan_x);
        app.config_button_rect = Some((cfg_x, btn_y, config_w, 1));
        app.options_button_rect = Some((opt_x, btn_y, options_w, 1));
        app.panels_button_rect = Some((pan_x, btn_y, panels_w, 1));
    } else {
        app.config_button_rect = None;
        app.options_button_rect = None;
        app.panels_button_rect = None;
    }

    // Build a custom block title with an additional status line on the bottom border.
    // Render the list normally first.
    let list = List::new(items)
        .style(Style::default().fg(th.text).bg(th.base))
        .block(
            Block::default()
                .title(Line::from(title_spans))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        )
        .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.list_state);

    // Draw status label on the bottom border line of the Results block
    // Bottom border y coordinate is area.y + area.height - 1
    let status_text = format!("Status: {}", app.arch_status_text);
    let sx = area.x.saturating_add(2); // a bit of left padding after corner
    let sy = area.y.saturating_add(area.height.saturating_sub(1));
    let maxw = area.width.saturating_sub(4); // avoid right corner
    let mut content = status_text.clone();
    if content.len() as u16 > maxw {
        content.truncate(maxw as usize);
    }
    // Compute style to blend with border line
    // Compose a dot + text with color depending on status
    let mut dot = "";
    let mut dot_color = th.overlay1;
    match app.arch_status_color {
        crate::state::ArchStatusColor::Operational => {
            dot = "●";
            dot_color = th.green;
        }
        crate::state::ArchStatusColor::IncidentToday => {
            dot = "●";
            dot_color = th.yellow;
        }
        crate::state::ArchStatusColor::None => {
            // If we have a nominal message, still show a green dot
            if app
                .arch_status_text
                .to_lowercase()
                .contains("arch systems nominal")
            {
                dot = "●";
                dot_color = th.green;
            }
        }
    }
    let style_text = Style::default()
        .fg(th.mauve)
        .bg(th.base)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let line = Paragraph::new(Line::from(vec![
        Span::styled(
            dot.to_string(),
            Style::default()
                .fg(dot_color)
                .bg(th.base)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(content.clone(), style_text),
    ]));
    // Record clickable rect centered within the available width
    let cw = ((content.len() + dot.len() + 1) as u16).min(maxw); // +1 for the space
    let pad_left = maxw.saturating_sub(cw) / 2;
    let start_x = sx.saturating_add(pad_left);
    // Clickable rect only over the text portion, not the dot or space
    let click_start_x = start_x.saturating_add((dot.len() + 1) as u16);
    app.arch_status_rect = Some((
        click_start_x,
        sy,
        (content.len() as u16).min(maxw.saturating_sub((dot.len() + 1) as u16)),
        1,
    ));
    let rect = ratatui::prelude::Rect {
        x: start_x,
        y: sy,
        width: cw,
        height: 1,
    };
    f.render_widget(line, rect);

    // Optional: render sort dropdown overlay near the button
    app.sort_menu_rect = None;
    if app.sort_menu_open {
        let opts = ["Alphabetical", "AUR popularity", "Best matches"];
        let widest = opts.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let w = widest.saturating_add(2).min(area.width.saturating_sub(2));
        // Place menu just under the title, aligned to button if possible
        let rect_w = w.saturating_add(2);
        let max_x = area.x + area.width.saturating_sub(rect_w);
        let menu_x = btn_x.min(max_x);
        let menu_y = area.y.saturating_add(1); // just below top border
        let h = (opts.len() as u16) + 2; // borders
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.sort_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        // Build lines with current mode highlighted
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let is_selected = matches!(
                (i, app.sort_mode),
                (0, SortMode::RepoThenName)
                    | (1, SortMode::AurPopularityThenOfficial)
                    | (2, SortMode::BestMatches)
            );
            let mark = if is_selected { "✔ " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(th.crust)
                    .bg(th.lavender)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(th.text)
            };
            lines.push(Line::from(vec![
                Span::styled(mark.to_string(), Style::default().fg(th.overlay1)),
                Span::styled(text.to_string(), style),
            ]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(Span::styled(" Sort by ", Style::default().fg(th.overlay1)))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.surface2)),
            );
        f.render_widget(Clear, rect);
        f.render_widget(menu, rect);
    }

    // Optional: render Config/Lists dropdown overlay near its button
    app.config_menu_rect = None;
    if app.config_menu_open {
        let opts = [
            "Config -> pacsea.conf",
            "Install List -> install_list.json",
            "Installed Packages -> installed_list.json",
            "Recent Searches -> recent_searches.json",
        ];
        let widest = opts.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let w = widest.saturating_add(2).min(area.width.saturating_sub(2));
        // Place menu under the Config/Lists button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = area.x + area.width.saturating_sub(rect_w);
        let cbx = app
            .config_button_rect
            .map(|(x, _, _, _)| x)
            .unwrap_or(max_x);
        let menu_x = cbx.min(max_x);
        let menu_y = area.y.saturating_add(1); // just below top border
        let h = (opts.len() as u16) + 2; // borders
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.config_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        let mut lines: Vec<Line> = Vec::new();
        for text in opts.iter() {
            lines.push(Line::from(vec![Span::styled(
                text.to_string(),
                Style::default().fg(th.text),
            )]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(Span::styled(
                        " Config/Lists ",
                        Style::default().fg(th.overlay1),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.surface2)),
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
        let w = widest.saturating_add(2).min(area.width.saturating_sub(2));
        // Place menu under the Panels button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = area.x + area.width.saturating_sub(rect_w);
        let pbx = panels_btn_x.unwrap_or(max_x);
        let menu_x = pbx.min(max_x);
        let menu_y = area.y.saturating_add(1); // just below top border
        let h = (opts.len() as u16) + 2; // borders
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.panels_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        let mut lines: Vec<Line> = Vec::new();
        for text in opts.iter() {
            lines.push(Line::from(vec![Span::styled(
                text.to_string(),
                Style::default().fg(th.text),
            )]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(Span::styled(" Panels ", Style::default().fg(th.overlay1)))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.surface2)),
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
        let opts = [label_toggle, "Update System", "News"];
        let widest = opts.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let w = widest.saturating_add(2).min(area.width.saturating_sub(2));
        // Place menu under the Options button aligned to its right edge
        let rect_w = w.saturating_add(2);
        let max_x = area.x + area.width.saturating_sub(rect_w);
        let obx = options_btn_x.unwrap_or(max_x);
        let menu_x = obx.min(max_x);
        let menu_y = area.y.saturating_add(1); // just below top border
        let h = (opts.len() as u16) + 2; // borders
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: rect_w,
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.options_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        // Build lines (single selectable option)
        let mut lines: Vec<Line> = Vec::new();
        for text in opts.iter() {
            lines.push(Line::from(vec![Span::styled(
                text.to_string(),
                Style::default().fg(th.text),
            )]));
        }
        let menu = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(Span::styled(" Options ", Style::default().fg(th.overlay1)))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.surface2)),
            );
        f.render_widget(Clear, rect);
        f.render_widget(menu, rect);
    }

    // Record inner results rect for mouse hit-testing (inside borders)
    app.results_rect = Some((
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    ));
}
