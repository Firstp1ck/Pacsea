//! TUI rendering for Pacsea.
//!
//! This module renders the full terminal user interface using `ratatui`.
//! The layout is split vertically into three regions:
//!
//! 1) Results list (top): shows search matches and keeps the current selection
//!    centered when possible
//! 2) Middle row (three columns): Recent (left), Search input (center), and
//!    Install list (right), each styled based on focus
//! 3) Details pane (bottom): rich package information with a clickable URL and
//!    a contextual help footer displaying keybindings
//!
//! The renderer also draws modal overlays for alerts and install confirmation.
//! It updates `app.url_button_rect` to make the URL clickable when available.
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Position,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::theme::KeyChord;
use crate::{
    state::{AppState, Focus, Source},
    theme::theme,
};

/// Render a full frame of the Pacsea TUI.
///
/// This function is the single entry point for drawing the interface and is
/// meant to be called each tick or after state changes.
///
/// Arguments:
///
/// - `f`: `ratatui` frame to render into
/// - `app`: mutable application state; updated during rendering for selection
///   offsets, cursor position, and clickable URL geometry
///
/// Behavior summary:
///
/// - Applies the global theme and background
/// - Ensures the results selection remains centered within the list viewport
///   by adjusting the list state's internal offset
/// - Renders the top results list with source badges and install markers
/// - Renders the middle row with Recent, Search (with visible cursor), and
///   Install panes; titles and colors reflect focus
/// - Computes and stores a clickable rectangle for the details URL when it is
///   present, enabling mouse interactions handled elsewhere
/// - Shows a help footer with keybindings inside the details area
/// - Draws modal overlays for network alerts and install confirmations
pub fn ui(f: &mut Frame, app: &mut AppState) {
    let th = theme();
    let area = f.area();

    // Background
    let bg = Block::default().style(Style::default().bg(th.base));
    f.render_widget(bg, area);

    let total_h = area.height;
    let search_h: u16 = 5; // give a bit more room for history pane
    let bottom_h: u16 = total_h.saturating_mul(2) / 3; // 2/3 of full height
    let top_h: u16 = total_h.saturating_sub(search_h).saturating_sub(bottom_h);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_h),
            Constraint::Length(search_h),
            Constraint::Length(bottom_h),
        ])
        .split(area);

    // Keep selection centered within the visible results list when possible
    {
        let viewport_rows = chunks[0].height.saturating_sub(2) as usize; // account for borders
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
                Source::Official { repo, .. } => (repo.to_string(), th.green),
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
            let mut segs = vec![
                Span::styled(format!("{src} "), Style::default().fg(color)),
                Span::styled(
                    p.name.clone(),
                    Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {}", p.version), Style::default().fg(th.overlay1)),
            ];
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

    let list = List::new(items)
        .style(Style::default().fg(th.text).bg(th.base))
        .block(
            Block::default()
                .title(Span::styled(
                    format!("Results ({})", app.results.len()),
                    Style::default().fg(th.overlay1),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        )
        .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);
    // Record inner results rect for mouse hit-testing (inside borders)
    app.results_rect = Some((
        chunks[0].x + 1,
        chunks[0].y + 1,
        chunks[0].width.saturating_sub(2),
        chunks[0].height.saturating_sub(2),
    ));

    // Middle row split: left input, middle recent, right install list
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(app.layout_left_pct.min(100)),
            Constraint::Percentage(app.layout_center_pct.min(100)),
            Constraint::Percentage(app.layout_right_pct.min(100)),
        ])
        .split(chunks[1]);

    // Search input (center)
    let search_focused = matches!(app.focus, Focus::Search);
    let input_line = Line::from(vec![
        Span::styled(
            "> ",
            Style::default().fg(if search_focused {
                th.sapphire
            } else {
                th.overlay1
            }),
        ),
        Span::styled(
            app.input.as_str().to_string(),
            Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
        ),
    ]);
    let search_title = if search_focused {
        "Search (focused)"
    } else {
        "Search"
    };
    let search_title_color = if search_focused {
        th.mauve
    } else {
        th.overlay1
    };
    let input = Paragraph::new(input_line)
        .style(
            Style::default()
                .fg(if search_focused { th.text } else { th.subtext0 })
                .bg(th.base),
        )
        .block(
            Block::default()
                .title(Span::styled(
                    search_title,
                    Style::default().fg(search_title_color),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if search_focused {
                    th.mauve
                } else {
                    th.surface1
                })),
        );
    f.render_widget(input, middle[1]);

    // Cursor in input
    let right = middle[1].x + middle[1].width.saturating_sub(1);
    let x = std::cmp::min(middle[1].x + 1 + 2 + app.input.len() as u16, right);
    let y = middle[1].y + 1;
    f.set_cursor_position(Position::new(x, y));

    // Recent searches (left) with filtering
    let recent_focused = matches!(app.focus, Focus::Recent);
    let rec_inds = crate::ui_helpers::filtered_recent_indices(app);
    let rec_items: Vec<ListItem> = rec_inds
        .iter()
        .filter_map(|&i| app.recent.get(i))
        .map(|s| {
            ListItem::new(Span::styled(
                s.clone(),
                Style::default().fg(if recent_focused { th.text } else { th.subtext0 }),
            ))
        })
        .collect();
    let mut recent_title_spans: Vec<Span> = vec![Span::styled(
        if recent_focused {
            "Recent (focused)"
        } else {
            "Recent"
        },
        Style::default().fg(if recent_focused {
            th.mauve
        } else {
            th.overlay1
        }),
    )];
    if recent_focused && let Some(pat) = &app.pane_find {
        recent_title_spans.push(Span::raw("  "));
        recent_title_spans.push(Span::styled(
            "/",
            Style::default()
                .fg(th.sapphire)
                .add_modifier(Modifier::BOLD),
        ));
        recent_title_spans.push(Span::styled(pat.clone(), Style::default().fg(th.text)));
    }
    let rec_block = Block::default()
        .title(Line::from(recent_title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if recent_focused {
            th.mauve
        } else {
            th.surface1
        }));
    let rec_list = List::new(rec_items)
        .style(
            Style::default()
                .fg(if recent_focused { th.text } else { th.subtext0 })
                .bg(th.base),
        )
        .block(rec_block)
        .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(rec_list, middle[0], &mut app.history_state);

    // Install List (right) with filtering
    let install_focused = matches!(app.focus, Focus::Install);
    let install_inds = crate::ui_helpers::filtered_install_indices(app);
    let install_items: Vec<ListItem> = install_inds
        .iter()
        .filter_map(|&i| app.install_list.get(i))
        .map(|p| {
            let (src, color) = match &p.source {
                Source::Official { repo, .. } => (repo.to_string(), th.green),
                Source::Aur => ("AUR".to_string(), th.yellow),
            };
            let segs = vec![
                Span::styled(format!("{src} "), Style::default().fg(color)),
                Span::styled(
                    p.name.clone(),
                    Style::default()
                        .fg(if install_focused { th.text } else { th.subtext0 })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", p.version),
                    Style::default().fg(if install_focused { th.overlay1 } else { th.surface2 }),
                ),
            ];
            ListItem::new(Line::from(segs))
        })
        .collect();
    let mut install_title_spans: Vec<Span> = vec![Span::styled(
        if install_focused {
            "Install List (focused)"
        } else {
            "Install List"
        },
        Style::default().fg(if install_focused {
            th.mauve
        } else {
            th.overlay1
        }),
    )];
    if install_focused && let Some(pat) = &app.pane_find {
        install_title_spans.push(Span::raw("  "));
        install_title_spans.push(Span::styled(
            "/",
            Style::default()
                .fg(th.sapphire)
                .add_modifier(Modifier::BOLD),
        ));
        install_title_spans.push(Span::styled(pat.clone(), Style::default().fg(th.text)));
    }
    let install_block = Block::default()
        .title(Line::from(install_title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if install_focused {
            th.mauve
        } else {
            th.surface1
        }));
    let install_list = List::new(install_items)
        .style(
            Style::default()
                .fg(if install_focused {
                    th.text
                } else {
                    th.subtext0
                })
                .bg(th.base),
        )
        .block(install_block)
        .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(install_list, middle[2], &mut app.install_state);

    // Details (bottom): determine rendering areas first
    let container_area = chunks[2];
    let (details_area, pkgb_area_opt) = if app.pkgb_visible {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(container_area);
        (split[0], Some(split[1]))
    } else {
        (container_area, None)
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
            if txt.to_lowercase().contains("show pkgbuild") {
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
        let pkgb = Paragraph::new(visible)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title(Span::styled("PKGBUILD", Style::default().fg(th.overlay1)))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(th.surface2)),
            );
        f.render_widget(pkgb, pkgb_area);
    }

    // Help footer with keybindings in the bottom of Package Info pane
    {
        let help_h: u16 = 5;
        let footer_container = if app.pkgb_visible {
            details_area
        } else {
            chunks[2]
        };
        if footer_container.height > help_h + 2 {
            let x = footer_container.x + 1; // inside border
            let y = footer_container.y + footer_container.height.saturating_sub(1 + help_h);
            let w = footer_container.width.saturating_sub(2);
            let h = help_h;
            let rect = ratatui::prelude::Rect {
                x,
                y,
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

            let key_style = Style::default()
                .fg(th.text)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD);
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
            // Switch pane
            if let (Some(n), Some(p)) = (km.pane_next.first(), km.pane_prev.first()) {
                s_spans.extend([
                    Span::styled(format!("[{} / {}]", n.label(), p.label()), key_style),
                    Span::raw(" switch pane"),
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
            if let (Some(n), Some(p)) = (km.pane_next.first(), km.pane_prev.first()) {
                i_spans.extend([
                    Span::styled(format!("[{} / {}]", n.label(), p.label()), key_style),
                    Span::raw(" switch pane"),
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
            if let (Some(n), Some(p)) = (km.pane_next.first(), km.pane_prev.first()) {
                r_spans.extend([
                    Span::styled(format!("[{} / {}]", n.label(), p.label()), key_style),
                    Span::raw(" switch pane"),
                ]);
            }

            let lines = vec![
                Line::from(g_spans),
                Line::from(s_spans),
                Line::from(i_spans),
                Line::from(r_spans),
            ];
            let footer = Paragraph::new(lines)
                .style(Style::default().fg(th.subtext1).bg(th.base))
                .wrap(Wrap { trim: true });
            f.render_widget(footer, rect);
        }
    }

    // Removed URL button; URL text itself is clickable in mouse mode

    // Modal overlay for alerts
    match &app.modal {
        crate::state::Modal::Alert { message } => {
            let w = area.width.saturating_sub(10).min(80);
            let h = 7;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            // Choose labels depending on error type (config vs network/other)
            let is_config = message.contains("Unknown key")
                || message.contains("Missing required keys")
                || message.contains("Missing '='")
                || message.contains("Missing key before '='")
                || message.contains("Duplicate key")
                || message.contains("Invalid color")
                || message.to_lowercase().contains("theme configuration");
            let header_text = if is_config {
                "Configuration error"
            } else {
                "Connection issue"
            };
            let box_title = if is_config {
                " Configuration Error "
            } else {
                " Network Error "
            };
            let header_color = if is_config { th.mauve } else { th.red };
            let lines = vec![
                Line::from(Span::styled(
                    header_text,
                    Style::default()
                        .fg(header_color)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(message.clone(), Style::default().fg(th.text))),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter or Esc to close",
                    Style::default().fg(th.subtext1),
                )),
            ];
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            box_title,
                            Style::default()
                                .fg(header_color)
                                .add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(header_color))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::ConfirmInstall { items } => {
            let w = area.width.saturating_sub(6).min(90);
            let h = area.height.saturating_sub(6).min(20);
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
                "Confirm installation",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Nothing to install",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for p in items.iter().take((h as usize).saturating_sub(6)) {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", p.name),
                        Style::default().fg(th.text),
                    )));
                }
                if items.len() + 6 > h as usize {
                    lines.push(Line::from(Span::styled(
                        "…",
                        Style::default().fg(th.subtext1),
                    )));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter to confirm or Esc to cancel",
                Style::default().fg(th.subtext1),
            )));
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Confirm Install ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::Help => {
            // Full-screen translucent help overlay
            let w = area.width.saturating_sub(6).min(96);
            let h = area.height.saturating_sub(4).min(28);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            let km = &app.keymap;

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Pacsea Help",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            // Utility to format a binding line
            let fmt = |label: &str, chord: KeyChord| -> Line<'static> {
                Line::from(vec![
                    Span::styled(
                        format!("{:18}", label),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", chord.label()),
                        Style::default()
                            .fg(th.text)
                            .bg(th.surface2)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])
            };

            if let Some(k) = km.help_overlay.first().copied() {
                lines.push(fmt("Help overlay", k));
            }
            if let Some(k) = km.exit.first().copied() {
                lines.push(fmt("Exit", k));
            }
            if let Some(k) = km.reload_theme.first().copied() {
                lines.push(fmt("Reload theme", k));
            }
            if let Some(k) = km.pane_next.first().copied() {
                lines.push(fmt("Next pane", k));
            }
            if let Some(k) = km.pane_prev.first().copied() {
                lines.push(fmt("Previous pane", k));
            }
            lines.push(Line::from(""));

            // Dynamic section for per-pane actions based on keymap
            lines.push(Line::from(Span::styled(
                "Search:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.search_move_up.first(), km.search_move_down.first()) {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
                lines.push(fmt(
                    "  Page",
                    KeyChord {
                        code: pu.code,
                        mods: pu.mods,
                    },
                ));
                lines.push(fmt(
                    "  Page",
                    KeyChord {
                        code: pd.code,
                        mods: pd.mods,
                    },
                ));
            }
            if let Some(k) = km.search_add.first().copied() {
                lines.push(fmt("  Add", k));
            }
            if let Some(k) = km.search_install.first().copied() {
                lines.push(fmt("  Install", k));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Install:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first())
            {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let Some(k) = km.install_confirm.first().copied() {
                lines.push(fmt("  Confirm", k));
            }
            if let Some(k) = km.install_remove.first().copied() {
                lines.push(fmt("  Remove", k));
            }
            if let Some(k) = km.install_clear.first().copied() {
                lines.push(fmt("  Clear", k));
            }
            if let Some(k) = km.install_find.first().copied() {
                lines.push(fmt("  Find", k));
            }
            if let Some(k) = km.install_to_search.first().copied() {
                lines.push(fmt("  To Search", k));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Recent:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let Some(k) = km.recent_use.first().copied() {
                lines.push(fmt("  Use", k));
            }
            if let Some(k) = km.recent_add.first().copied() {
                lines.push(fmt("  Add", k));
            }
            if let Some(k) = km.recent_find.first().copied() {
                lines.push(fmt("  Find", k));
            }
            if let Some(k) = km.recent_to_search.first().copied() {
                lines.push(fmt("  To Search", k));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter or Esc to close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Help ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::None => {}
    }
}
