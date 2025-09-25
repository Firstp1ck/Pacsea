use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Position,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    state::{AppState, Focus, Source},
    theme::theme,
};

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

    // Middle row split: left input, middle recent, right install list
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
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
            let line = Line::from(vec![
                Span::styled(
                    p.name.clone(),
                    Style::default()
                        .fg(if install_focused {
                            th.text
                        } else {
                            th.subtext0
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", p.version),
                    Style::default().fg(if install_focused {
                        th.overlay1
                    } else {
                        th.surface2
                    }),
                ),
            ]);
            ListItem::new(line)
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

    // Details (bottom)
    let details_lines = crate::ui_helpers::format_details_lines(app, chunks[2].width, &th);
    let details = Paragraph::new(details_lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    "Package Info",
                    Style::default().fg(th.overlay1),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        );
    f.render_widget(details, chunks[2]);

    // Keybindings footer inside Package Info (four lines: Search, Recent, Install, Global)
    let inner = ratatui::prelude::Rect {
        x: chunks[2].x + 1,
        y: chunks[2].y + 1,
        width: chunks[2].width.saturating_sub(2),
        height: chunks[2].height.saturating_sub(2),
    };
    if inner.height >= 4 && inner.width >= 1 {
        let footer_rect = ratatui::prelude::Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(4),
            width: inner.width,
            height: 4,
        };

        // Styles per pane depending on focus
        let (search_lbl, search_keys) = if matches!(app.focus, Focus::Search) {
            (
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                Style::default().fg(th.text),
            )
        } else {
            (
                Style::default().fg(th.overlay1),
                Style::default().fg(th.subtext1),
            )
        };
        let (recent_lbl, recent_keys) = if matches!(app.focus, Focus::Recent) {
            (
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                Style::default().fg(th.text),
            )
        } else {
            (
                Style::default().fg(th.overlay1),
                Style::default().fg(th.subtext1),
            )
        };
        let (install_lbl, install_keys) = if matches!(app.focus, Focus::Install) {
            (
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                Style::default().fg(th.text),
            )
        } else {
            (
                Style::default().fg(th.overlay1),
                Style::default().fg(th.subtext1),
            )
        };
        let global_lbl = Style::default().fg(th.overlay1);
        let global_keys = Style::default().fg(th.subtext1);

        // SEARCH line
        let l1: Vec<Span> = vec![
            Span::styled("SEARCH:", search_lbl),
            Span::styled(" ↑/↓ PgUp/PgDn", search_keys),
            Span::raw("  "),
            Span::styled("Space", search_keys),
            Span::styled("=add", search_keys),
            Span::raw("  "),
            Span::styled("Enter", search_keys),
            Span::styled("=install", search_keys),
            Span::raw("  "),
            Span::styled("←/→ Tab/S-Tab", search_keys),
            Span::styled("=switch", search_keys),
            Span::raw("  "),
            Span::styled("Type", search_keys),
            Span::styled("=query", search_keys),
            Span::raw("  "),
            Span::styled("Backspace", search_keys),
            Span::styled("=delete", search_keys),
        ];

        // RECENT line
        let l2: Vec<Span> = vec![
            Span::styled("RECENT:", recent_lbl),
            Span::styled(" j/k or ↑/↓", recent_keys),
            Span::raw("  "),
            Span::styled("Enter", recent_keys),
            Span::styled("=use", recent_keys),
            Span::raw("  "),
            Span::styled("Space", recent_keys),
            Span::styled("=add", recent_keys),
            Span::raw("  "),
            Span::styled("/", recent_keys),
            Span::styled("=find, Enter next, Esc cancel", recent_keys),
            Span::raw("  "),
            Span::styled("Esc", recent_keys),
            Span::styled("=to Search", recent_keys),
            Span::raw("  "),
            Span::styled("←/→ Tab/S-Tab", recent_keys),
            Span::styled("=switch", recent_keys),
        ];

        // INSTALL line
        let l3: Vec<Span> = vec![
            Span::styled("INSTALL:", install_lbl),
            Span::styled(" j/k or ↑/↓", install_keys),
            Span::raw("  "),
            Span::styled("Enter", install_keys),
            Span::styled("=confirm", install_keys),
            Span::raw("  "),
            Span::styled("Del", install_keys),
            Span::styled("=remove", install_keys),
            Span::raw("  "),
            Span::styled("Shift+Del", install_keys),
            Span::styled("=clear", install_keys),
            Span::raw("  "),
            Span::styled("/", install_keys),
            Span::styled("=find, Enter next, Esc cancel", install_keys),
            Span::raw("  "),
            Span::styled("Esc", install_keys),
            Span::styled("=to Search", install_keys),
            Span::raw("  "),
            Span::styled("←/→ Tab/S-Tab", install_keys),
            Span::styled("=switch", install_keys),
        ];

        // GLOBAL line
        let l4: Vec<Span> = vec![
            Span::styled("GLOBALS:", global_lbl),
            Span::styled(" Ctrl+C=exit", global_keys),
            Span::raw("  "),
            Span::styled("Esc=exit (Search)", global_keys),
            Span::raw("  "),
            Span::styled("Popup dialog: Enter=confirm, Esc=cancel", global_keys),
        ];

        let kb = Paragraph::new(vec![
            // Order: GLOBALS, SEARCH, INSTALL, RECENT
            Line::from(l4),
            Line::from(l1),
            Line::from(l3),
            Line::from(l2),
        ])
        .style(Style::default().fg(th.subtext1).bg(th.base))
        .wrap(Wrap { trim: true });
        f.render_widget(kb, footer_rect);
    }

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
            let lines = vec![
                Line::from(Span::styled(
                    "Connection issue",
                    Style::default().fg(th.red).add_modifier(Modifier::BOLD),
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
                            " Network Error ",
                            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.red))
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
        crate::state::Modal::None => {}
    }
}
