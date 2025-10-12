use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
};

use crate::state::{AppState, Focus, Source};
use crate::theme::theme;

/// Render the middle row: Recent (left), Search input (center), Install list (right).
///
/// Also positions the cursor within the input, shows in-pane find indicators,
/// and records inner rectangles for mouse hit-testing.
pub fn render_middle(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    // Middle row split: left Recent, middle Search input, right Install list
    // If a pane is hidden, reassign its percentage to the center pane.
    let left_pct = if app.show_recent_pane {
        app.layout_left_pct.min(100)
    } else {
        0
    };
    let mut right_pct = if app.show_install_pane {
        app.layout_right_pct.min(100)
    } else {
        0
    };
    // In installed-only mode, enlarge the right pane so Downgrade and Remove lists are each ~50% wider
    if app.installed_only_mode && right_pct > 0 {
        let max_right = 100u16.saturating_sub(left_pct);
        let widened = ((right_pct as u32 * 3) / 2) as u16; // 1.5x
        right_pct = widened.min(max_right);
    }
    let center_pct = 100u16
        .saturating_sub(left_pct)
        .saturating_sub(right_pct)
        .min(100);
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(center_pct),
            Constraint::Percentage(right_pct),
        ])
        .split(area);

    // Search input (center)
    let search_focused = matches!(app.focus, Focus::Search);
    // Build input line with optional selection highlight in Search normal mode
    let mut input_spans: Vec<Span> = Vec::new();
    input_spans.push(Span::styled(
        "> ",
        Style::default().fg(if search_focused {
            th.sapphire
        } else {
            th.overlay1
        }),
    ));
    if search_focused && app.search_normal_mode {
        let caret_ci = app.search_caret;
        let (sel_from_ci, sel_to_ci) = if let Some(anchor) = app.search_select_anchor {
            (anchor.min(caret_ci), anchor.max(caret_ci))
        } else {
            (caret_ci, caret_ci)
        };
        let cc = app.input.chars().count();
        let sel_from_ci = sel_from_ci.min(cc);
        let sel_to_ci = sel_to_ci.min(cc);
        let from_b = {
            if sel_from_ci == 0 {
                0
            } else {
                app.input
                    .char_indices()
                    .map(|(i, _)| i)
                    .nth(sel_from_ci)
                    .unwrap_or(app.input.len())
            }
        };
        let to_b = {
            if sel_to_ci == 0 {
                0
            } else {
                app.input
                    .char_indices()
                    .map(|(i, _)| i)
                    .nth(sel_to_ci)
                    .unwrap_or(app.input.len())
            }
        };
        let pre = &app.input[..from_b];
        let sel = &app.input[from_b..to_b];
        let post = &app.input[to_b..];
        if !pre.is_empty() {
            input_spans.push(Span::styled(
                pre.to_string(),
                Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
            ));
        }
        if sel_from_ci != sel_to_ci {
            input_spans.push(Span::styled(
                sel.to_string(),
                Style::default()
                    .fg(th.crust)
                    .bg(th.lavender)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if !post.is_empty() {
            input_spans.push(Span::styled(
                post.to_string(),
                Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
            ));
        }
    } else {
        input_spans.push(Span::styled(
            app.input.as_str().to_string(),
            Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
        ));
    }
    let input_line = Line::from(input_spans);
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
    // Cursor x: align to caret in characters from start (prefix "> ")
    let caret_cols: u16 = if search_focused {
        let mut ci: u16 = 0;
        let mut it = app.input.chars();
        for _ in 0..app.search_caret {
            if it.next().is_some() {
                ci = ci.saturating_add(1);
            } else {
                break;
            }
        }
        ci
    } else {
        app.input.len() as u16
    };
    let x = std::cmp::min(middle[1].x + 1 + 2 + caret_cols, right);
    let y = middle[1].y + 1;
    f.set_cursor_position(Position::new(x, y));
    // No status footer here; it is rendered on the Results pane bottom border

    // Recent searches (left) with filtering (render only if visible and has width)
    if app.show_recent_pane && middle[0].width > 0 {
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
        // Record inner Recent rect for mouse hit-testing (inside borders)
        app.recent_rect = Some((
            middle[0].x + 1,
            middle[0].y + 1,
            middle[0].width.saturating_sub(2),
            middle[0].height.saturating_sub(2),
        ));
    } else {
        app.recent_rect = None;
    }

    // Install/Remove List (right) with filtering (render only if visible and has width)
    if app.show_install_pane && middle[2].width > 0 {
        let install_focused = matches!(app.focus, Focus::Install);

        if app.installed_only_mode {
            // In installed-only mode, split the right pane into Downgrade (left) and Remove (right)
            let right_split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(middle[2]);

            // Downgrade List (left)
            let dg_indices: Vec<usize> = (0..app.downgrade_list.len()).collect();
            let downgrade_items: Vec<ListItem> = dg_indices
                .iter()
                .filter_map(|&i| app.downgrade_list.get(i))
                .map(|p| {
                    let (src, color) = match &p.source {
                        Source::Official { repo, .. } => (repo.to_string(), th.green),
                        Source::Aur => ("AUR".to_string(), th.yellow),
                    };
                    let mut segs: Vec<Span> = Vec::new();
                    if let Some(pop) = p.popularity {
                        segs.push(Span::styled(
                            format!("Pop: {pop:.2} "),
                            Style::default().fg(th.overlay1),
                        ));
                    }
                    segs.push(Span::styled(format!("{src} "), Style::default().fg(color)));
                    segs.push(Span::styled(
                        p.name.clone(),
                        Style::default()
                            .fg(if install_focused {
                                th.text
                            } else {
                                th.subtext0
                            })
                            .add_modifier(Modifier::BOLD),
                    ));
                    segs.push(Span::styled(
                        format!("  {}", p.version),
                        Style::default().fg(if install_focused {
                            th.overlay1
                        } else {
                            th.surface2
                        }),
                    ));
                    ListItem::new(Line::from(segs))
                })
                .collect();
            let downgrade_is_focused = install_focused
                && matches!(
                    app.right_pane_focus,
                    crate::state::RightPaneFocus::Downgrade
                );
            let downgrade_title = if downgrade_is_focused {
                "Downgrade List (focused)"
            } else {
                "Downgrade List"
            };
            let downgrade_block = Block::default()
                .title(Line::from(vec![Span::styled(
                    downgrade_title,
                    Style::default().fg(if downgrade_is_focused {
                        th.mauve
                    } else {
                        th.overlay1
                    }),
                )]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if downgrade_is_focused {
                    th.mauve
                } else {
                    th.surface1
                }));
            let downgrade_list = List::new(downgrade_items)
                .style(
                    Style::default()
                        .fg(if downgrade_is_focused {
                            th.text
                        } else {
                            th.subtext0
                        })
                        .bg(th.base),
                )
                .block(downgrade_block)
                .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
                .highlight_symbol("▶ ");
            f.render_stateful_widget(downgrade_list, right_split[0], &mut app.downgrade_state);
            // Record inner Downgrade rect
            app.downgrade_rect = Some((
                right_split[0].x + 1,
                right_split[0].y + 1,
                right_split[0].width.saturating_sub(2),
                right_split[0].height.saturating_sub(2),
            ));

            // Remove List (right)
            let rm_indices: Vec<usize> = (0..app.remove_list.len()).collect();
            let remove_items: Vec<ListItem> = rm_indices
                .iter()
                .filter_map(|&i| app.remove_list.get(i))
                .map(|p| {
                    let (src, color) = match &p.source {
                        Source::Official { repo, .. } => (repo.to_string(), th.green),
                        Source::Aur => ("AUR".to_string(), th.yellow),
                    };
                    let mut segs: Vec<Span> = Vec::new();
                    if let Some(pop) = p.popularity {
                        segs.push(Span::styled(
                            format!("Pop: {pop:.2} "),
                            Style::default().fg(th.overlay1),
                        ));
                    }
                    segs.push(Span::styled(format!("{src} "), Style::default().fg(color)));
                    segs.push(Span::styled(
                        p.name.clone(),
                        Style::default()
                            .fg(if install_focused {
                                th.text
                            } else {
                                th.subtext0
                            })
                            .add_modifier(Modifier::BOLD),
                    ));
                    segs.push(Span::styled(
                        format!("  {}", p.version),
                        Style::default().fg(if install_focused {
                            th.overlay1
                        } else {
                            th.surface2
                        }),
                    ));
                    ListItem::new(Line::from(segs))
                })
                .collect();
            let remove_is_focused = install_focused
                && matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove);
            let remove_title = if remove_is_focused {
                "Remove List (focused)"
            } else {
                "Remove List"
            };
            let remove_block = Block::default()
                .title(Line::from(vec![Span::styled(
                    remove_title,
                    Style::default().fg(if remove_is_focused {
                        th.mauve
                    } else {
                        th.overlay1
                    }),
                )]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if remove_is_focused {
                    th.mauve
                } else {
                    th.surface1
                }));
            let remove_list = List::new(remove_items)
                .style(
                    Style::default()
                        .fg(if remove_is_focused {
                            th.text
                        } else {
                            th.subtext0
                        })
                        .bg(th.base),
                )
                .block(remove_block)
                .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
                .highlight_symbol("▶ ");
            f.render_stateful_widget(remove_list, right_split[1], &mut app.remove_state);

            // Record inner Install rect for mouse hit-testing (map to Remove list area)
            app.install_rect = Some((
                right_split[1].x + 1,
                right_split[1].y + 1,
                right_split[1].width.saturating_sub(2),
                right_split[1].height.saturating_sub(2),
            ));
        } else {
            // Normal Install List (single right pane)
            let indices: Vec<usize> = crate::ui_helpers::filtered_install_indices(app);
            let install_items: Vec<ListItem> = indices
                .iter()
                .filter_map(|&i| app.install_list.get(i))
                .map(|p| {
                    let (src, color) = match &p.source {
                        Source::Official { repo, .. } => (repo.to_string(), th.green),
                        Source::Aur => ("AUR".to_string(), th.yellow),
                    };
                    let mut segs: Vec<Span> = Vec::new();
                    if let Some(pop) = p.popularity {
                        segs.push(Span::styled(
                            format!("Pop: {pop:.2} "),
                            Style::default().fg(th.overlay1),
                        ));
                    }
                    segs.push(Span::styled(format!("{src} "), Style::default().fg(color)));
                    segs.push(Span::styled(
                        p.name.clone(),
                        Style::default()
                            .fg(if install_focused {
                                th.text
                            } else {
                                th.subtext0
                            })
                            .add_modifier(Modifier::BOLD),
                    ));
                    segs.push(Span::styled(
                        format!("  {}", p.version),
                        Style::default().fg(if install_focused {
                            th.overlay1
                        } else {
                            th.surface2
                        }),
                    ));
                    ListItem::new(Line::from(segs))
                })
                .collect();
            let title_text = if install_focused {
                "Install List (focused)"
            } else {
                "Install List"
            };
            let install_block = Block::default()
                .title(Line::from(vec![Span::styled(
                    title_text,
                    Style::default().fg(if install_focused {
                        th.mauve
                    } else {
                        th.overlay1
                    }),
                )]))
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
            app.install_rect = Some((
                middle[2].x + 1,
                middle[2].y + 1,
                middle[2].width.saturating_sub(2),
                middle[2].height.saturating_sub(2),
            ));
        }
    } else {
        app.install_rect = None;
        // If Install pane is hidden and currently focused, move focus to Search
        if matches!(app.focus, Focus::Install) {
            app.focus = Focus::Search;
        }
    }
}
