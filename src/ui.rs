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
            let mut segs: Vec<Span> = Vec::new();
            // Popularity (AUR) shown before repo label when available
            if let Some(pop) = p.popularity {
                segs.push(Span::styled(
                    format!("Pop: {:.2} ", pop),
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

    // Build title with Sort button and filter toggles
    let results_title_text = format!("Results ({})", app.results.len());
    let sort_button_label = "Sort ▾".to_string();
    let mut title_spans: Vec<Span> = vec![Span::styled(
        results_title_text.clone(),
        Style::default().fg(th.overlay1),
    )];
    title_spans.push(Span::raw("  "));
    // Style the button differently when menu is open
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
    // Filter toggles: [AUR] [core] [extra] [multilib]
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

    // Estimate and record clickable rects for controls on the title line
    // Title is rendered on the top border row at y = chunks[0].y
    let mut x_cursor = chunks[0]
        .x
        .saturating_add(1) // left border inset
        .saturating_add(results_title_text.len() as u16)
        .saturating_add(2); // two spaces before Sort
    let btn_w = sort_button_label.len() as u16;
    let btn_x = x_cursor;
    let btn_y = chunks[0].y; // top border row
    app.sort_button_rect = Some((btn_x, btn_y, btn_w, 1));
    x_cursor = x_cursor.saturating_add(btn_w).saturating_add(2); // space after sort

    // Filter rects in sequence, with single space between
    let rec_rect = |start_x: u16, label: &str| -> (u16, u16, u16, u16) {
        // label already includes brackets, so width is exact label length
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

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // Optional: render sort dropdown overlay near the button
    app.sort_menu_rect = None;
    if app.sort_menu_open {
        let opts = ["Alphabetical", "AUR popularity", "Best matches"];
        let widest = opts.iter().map(|s| s.len()).max().unwrap_or(0) as u16;
        let w = widest
            .saturating_add(2)
            .min(chunks[0].width.saturating_sub(2));
        // Place menu just under the title, aligned to button if possible
        let menu_x = btn_x.min(chunks[0].x + chunks[0].width.saturating_sub(1 + w));
        let menu_y = chunks[0].y.saturating_add(1); // just below top border
        let h = (opts.len() as u16) + 2; // borders
        let rect = ratatui::prelude::Rect {
            x: menu_x,
            y: menu_y,
            width: w.saturating_add(2),
            height: h,
        };
        // Record inner list area for hit-testing (exclude borders)
        app.sort_menu_rect = Some((rect.x + 1, rect.y + 1, w, h.saturating_sub(2)));

        // Build lines with current mode highlighted
        let mut lines: Vec<Line> = Vec::new();
        for (i, text) in opts.iter().enumerate() {
            let is_selected = matches!(
                (i, app.sort_mode),
                (0, crate::state::SortMode::RepoThenName)
                    | (1, crate::state::SortMode::AurPopularityThenOfficial)
                    | (2, crate::state::SortMode::BestMatches)
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
    // Record inner Recent rect for mouse hit-testing (inside borders)
    app.recent_rect = Some((
        middle[0].x + 1,
        middle[0].y + 1,
        middle[0].width.saturating_sub(2),
        middle[0].height.saturating_sub(2),
    ));

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
            let mut segs: Vec<Span> = Vec::new();
            // Popularity (AUR) shown before repo label when available, like Results pane
            if let Some(pop) = p.popularity {
                segs.push(Span::styled(
                    format!("Pop: {:.2} ", pop),
                    Style::default().fg(th.overlay1),
                ));
            }
            // Repo / source label
            segs.push(Span::styled(format!("{src} "), Style::default().fg(color)));
            // Name and version
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
    // Record inner Install rect for mouse hit-testing (inside borders)
    app.install_rect = Some((
        middle[2].x + 1,
        middle[2].y + 1,
        middle[2].width.saturating_sub(2),
        middle[2].height.saturating_sub(2),
    ));

    // Details (bottom): reserve space for footer, then render content (details/PKGBUILD)
    let bottom_container = chunks[2];
    let base_help_h: u16 = 5;
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
    {
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
            // (BackTab sort-cycling hint removed)

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
            // Switch pane (only show when both next and prev are configured)
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
            // Pane switch (only when both are configured)
            if let (Some(n), Some(p)) = (km.pane_next.first(), km.pane_prev.first()) {
                i_spans.extend([
                    Span::styled(format!("[{} / {}]", n.label(), p.label()), key_style),
                    Span::raw(" switch pane"),
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
            if let (Some(n), Some(p)) = (km.pane_next.first(), km.pane_prev.first()) {
                r_spans.extend([
                    Span::styled(format!("[{} / {}]", n.label(), p.label()), key_style),
                    Span::raw(" switch pane"),
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
                    Span::styled(format!("[{}]", toggle_label), key_style),
                    Span::raw(" toggle, "),
                    Span::styled(format!("[{}]", insert_label), key_style),
                    Span::raw(" insert, "),
                    Span::styled("[j / k]", key_style),
                    Span::raw(" move, "),
                    Span::styled("[Ctrl+d / Ctrl+u]", key_style),
                    Span::raw(" page, "),
                    Span::styled(format!("[{} / {}]", left_label, right_label), key_style),
                    Span::raw(" select, "),
                    Span::styled(format!("[{}]", delete_label), key_style),
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
            } else if {
                let ml = message.to_lowercase();
                ml.contains("clipboard") || ml.contains("wl-copy") || ml.contains("xclip") || ml.contains("wl-clipboard")
            } {
                "Clipboard Copy"
            } else {
                "Connection issue"
            };
            let is_clipboard = {
                let ml = message.to_lowercase();
                ml.contains("clipboard") || ml.contains("wl-copy") || ml.contains("xclip") || ml.contains("wl-clipboard")
            };
            let box_title = if is_config {
                " Configuration Error "
            } else if is_clipboard {
                " Clipboard Copy "
            } else {
                " Connection issue "
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
                        Style::default().fg(th.text).add_modifier(Modifier::BOLD),
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
            if let Some(k) = km.pane_left.first().copied() {
                lines.push(fmt("Focus left", k));
            }
            if let Some(k) = km.pane_right.first().copied() {
                lines.push(fmt("Focus right", k));
            }
            if let Some(k) = km.show_pkgbuild.first().copied() {
                lines.push(fmt("Show PKGBUILD", k));
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
            if let Some(k) = km.search_backspace.first().copied() {
                lines.push(fmt("  Delete", k));
            }

            // Search normal mode
            if km
                .search_normal_toggle
                .first()
                .or(km.search_normal_insert.first())
                .or(km.search_normal_select_left.first())
                .or(km.search_normal_select_right.first())
                .or(km.search_normal_delete.first())
                .is_some()
            {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Search (Normal mode):",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                )));
                if let Some(k) = km.search_normal_toggle.first().copied() {
                    lines.push(fmt("  Toggle normal", k));
                }
                if let Some(k) = km.search_normal_insert.first().copied() {
                    lines.push(fmt("  Insert mode", k));
                }
                if let Some(k) = km.search_normal_select_left.first().copied() {
                    lines.push(fmt("  Select left", k));
                }
                if let Some(k) = km.search_normal_select_right.first().copied() {
                    lines.push(fmt("  Select right", k));
                }
                if let Some(k) = km.search_normal_delete.first().copied() {
                    lines.push(fmt("  Delete", k));
                }
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
            // (Focus left shown in Global section)

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
            if let Some(k) = km.recent_remove.first().copied() {
                lines.push(fmt("  Remove", k));
            }
            // Explicit: Shift+Del clears Recent (display only)
            lines.push(fmt(
                "  Clear",
                crate::theme::KeyChord {
                    code: crossterm::event::KeyCode::Delete,
                    mods: crossterm::event::KeyModifiers::SHIFT,
                },
            ));
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
