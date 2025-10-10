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
                    let label = if repo.eq_ignore_ascii_case("eos")
                        || repo.eq_ignore_ascii_case("endeavouros")
                    {
                        "EOS".to_string()
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
    // Filter toggles: [AUR] [core] [extra] [multilib] [EOS]
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
    title_spans.push(Span::raw(" "));
    title_spans.push(filt("EOS", app.results_filter_show_eos));

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
    app.results_filter_eos_rect = Some(rec_rect(x_cursor, eos_label));

    // Right-aligned Options button: compute remaining space and append to title spans
    let inner_width = area.width.saturating_sub(2); // exclude borders
    let consumed_left = (results_title_text.len()
        + 2 // spaces before Sort
        + sort_button_label.len()
        + 2 // spaces after Sort
        + aur_label.len()
        + 1 // space
        + core_label.len()
        + 1 // space
        + extra_label.len()
        + 1 // space
        + multilib_label.len()
        + 1 // space
        + eos_label.len()) as u16;
    // Minimum single space before Options when possible
    let options_w = options_button_label.len() as u16;
    let pad = inner_width.saturating_sub(consumed_left.saturating_add(options_w));
    let mut options_btn_x: Option<u16> = None;
    if pad >= 1 {
        title_spans.push(Span::raw(" ".repeat(pad as usize)));
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

        // Record clickable rect for Options button at the computed right edge
        let x = area
            .x
            .saturating_add(1) // left border inset
            .saturating_add(inner_width.saturating_sub(options_w));
        options_btn_x = Some(x);
        app.options_button_rect = Some((x, btn_y, options_w, 1));
    } else {
        app.options_button_rect = None;
    }

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

    // Optional: render Options dropdown overlay near the right button
    app.options_menu_rect = None;
    if app.options_menu_open {
        let label_toggle = if app.installed_only_mode {
            "List all packages"
        } else {
            "List installed packages"
        };
        let opts = [label_toggle, "Update System"];
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
