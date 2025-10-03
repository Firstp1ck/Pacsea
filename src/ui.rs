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
///    offsets, cursor position, and clickable URL geometry
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
    let mut details_lines = crate::ui_helpers::format_details_lines(app, chunks[2].width, &th);

    // Find the URL line and, when mouse mode is enabled, style it as a link and record its rect
    app.url_button_rect = None;
    for (row_idx, line) in details_lines.iter_mut().enumerate() {
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
                    let border_inset = 1u16;
                    let content_x = chunks[2].x.saturating_add(border_inset);
                    let content_y = chunks[2].y.saturating_add(border_inset);
                    let key_len = key_txt.len() as u16;
                    let x_start = content_x.saturating_add(key_len);
                    let y = content_y.saturating_add(row_idx as u16);
                    let max_w = chunks[2].width.saturating_sub(2).saturating_sub(key_len);
                    let w = url_txt.len().min(max_w as usize) as u16;
                    if w > 0 {
                        app.url_button_rect = Some((x_start, y, w, 1));
                    }
                }
                break;
            }
        }
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
    f.render_widget(details, chunks[2]);

    // Help footer with keybindings in the bottom of Package Info pane
    {
        let help_h: u16 = 5;
        if chunks[2].height > help_h + 2 {
            let x = chunks[2].x + 1; // inside border
            let y = chunks[2].y + chunks[2].height.saturating_sub(1 + help_h);
            let w = chunks[2].width.saturating_sub(2);
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

            // GLOBALS
            let mut g_spans: Vec<Span> = vec![
                Span::styled(
                    "GLOBALS:",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            g_spans.extend([
                Span::styled("[Ctrl+C]", key_style),
                Span::raw(" exit"),
                sep.clone(),
                Span::styled("[Esc]", key_style),
                Span::raw(" exit (Search)"),
                sep.clone(),
                Span::styled("[Enter]", key_style),
                Span::raw(" confirm popup"),
                sep.clone(),
                Span::styled("[Esc]", key_style),
                Span::raw(" cancel popup"),
                sep.clone(),
                Span::styled("[Ctrl+R]", key_style),
                Span::raw(" reload theme"),
            ]);

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
            s_spans.extend([
                Span::styled("[↑/↓]", key_style),
                Span::raw(" move"),
                sep.clone(),
                Span::styled("[PgUp/PgDn]", key_style),
                Span::raw(" page"),
                sep.clone(),
                Span::styled("[Space]", key_style),
                Span::raw(" add"),
                sep.clone(),
                Span::styled("[Enter]", key_style),
                Span::raw(" install"),
                sep.clone(),
                Span::styled("[Tab/S-Tab]", key_style),
                Span::raw(" switch pane"),
                sep.clone(),
                Span::styled("[Type]", key_style),
                Span::raw(" query"),
                sep.clone(),
                Span::styled("[Backspace]", key_style),
                Span::raw(" delete"),
            ]);

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
            i_spans.extend([
                Span::styled("[j/k or ↑/↓]", key_style),
                Span::raw(" move"),
                sep.clone(),
                Span::styled("[Enter]", key_style),
                Span::raw(" confirm"),
                sep.clone(),
                Span::styled("[Del]", key_style),
                Span::raw(" remove"),
                sep.clone(),
                Span::styled("[Shift+Del]", key_style),
                Span::raw(" clear"),
                sep.clone(),
                Span::styled("[/]", key_style),
                Span::raw(" find (Enter next, Esc cancel)"),
                sep.clone(),
                Span::styled("[Esc]", key_style),
                Span::raw(" to Search"),
                sep.clone(),
                Span::styled("[Tab/S-Tab]", key_style),
                Span::raw(" switch pane"),
            ]);

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
            r_spans.extend([
                Span::styled("[j/k or ↑/↓]", key_style),
                Span::raw(" move"),
                sep.clone(),
                Span::styled("[Enter]", key_style),
                Span::raw(" use"),
                sep.clone(),
                Span::styled("[Space]", key_style),
                Span::raw(" add"),
                sep.clone(),
                Span::styled("[/]", key_style),
                Span::raw(" find (Enter next, Esc cancel)"),
                sep.clone(),
                Span::styled("[Esc]", key_style),
                Span::raw(" to Search"),
                sep.clone(),
                Span::styled("[Tab/S-Tab]", key_style),
                Span::raw(" switch pane"),
            ]);

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
            let header_text = if is_config { "Configuration error" } else { "Connection issue" };
            let box_title = if is_config { " Configuration Error " } else { " Network Error " };
            let header_color = if is_config { th.mauve } else { th.red };
            let lines = vec![
                Line::from(Span::styled(
                    header_text,
                    Style::default().fg(header_color).add_modifier(Modifier::BOLD),
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
                            Style::default().fg(header_color).add_modifier(Modifier::BOLD),
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
        crate::state::Modal::None => {}
    }
}
