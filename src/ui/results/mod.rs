use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem},
};

use crate::state::{AppState, Source};
use crate::theme::theme;

mod dropdowns;
mod list;
mod sort_menu;
mod status;
mod title;
mod utils;

/// What: Render the top results list and title controls.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (results, selection, rects)
/// - `area`: Target rectangle for the results block
///
/// Output:
/// - Draws the results list and updates hit-test rectangles for Sort/Filters/Buttons and status.
///
/// Details:
/// - Keeps selection centered when possible; shows repo/labels, versions, descriptions, and
///   install markers.
/// - Builds the title with Sort button, filter toggles, and right-aligned options/config/panels.
/// - Renders dropdown overlays for Sort/Options/Config/Panels when open, and records rects.
pub fn render_results(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    // Detect availability of optional repos from all_results (unfiltered) to keep chips visible
    let (has_eos, has_cachyos, has_manjaro) = utils::detect_optional_repos(app);

    // Keep selection centered within the visible results list when possible
    utils::center_selection(app, area);

    // Extract values needed for title building before mutating app
    let results_len = app.results.len();
    let sort_menu_open = app.sort_menu_open;
    let config_menu_open = app.config_menu_open;
    let panels_menu_open = app.panels_menu_open;
    let options_menu_open = app.options_menu_open;
    let results_filter_show_aur = app.results_filter_show_aur;
    let results_filter_show_core = app.results_filter_show_core;
    let results_filter_show_extra = app.results_filter_show_extra;
    let results_filter_show_multilib = app.results_filter_show_multilib;
    let results_filter_show_eos = app.results_filter_show_eos;
    let results_filter_show_cachyos = app.results_filter_show_cachyos;
    let results_filter_show_manjaro = app.results_filter_show_manjaro;

    // Build title with Sort button, filter toggles, and a right-aligned Options button
    // (using extracted values to avoid borrow conflicts)
    let title_spans = title::build_title_spans_from_values(
        results_len,
        area,
        has_eos,
        has_cachyos,
        has_manjaro,
        sort_menu_open,
        config_menu_open,
        panels_menu_open,
        options_menu_open,
        results_filter_show_aur,
        results_filter_show_core,
        results_filter_show_extra,
        results_filter_show_multilib,
        results_filter_show_eos,
        results_filter_show_cachyos,
        results_filter_show_manjaro,
    );

    // Record clickable rects for title bar controls (mutates app)
    title::record_title_rects(app, area, has_eos, has_cachyos, has_manjaro);

    // Extract sort button x position for sort menu positioning
    let btn_x = app.sort_button_rect.map(|(x, _, _, _)| x).unwrap_or(area.x);

    // Build list items (only visible ones for performance)
    // Extract offset before building to avoid borrowing app.list_state
    let list_offset = app.list_state.offset();

    // Build the list items inline to avoid borrow conflicts with app.list_state
    // This must be done inline because Rust's borrow checker is too conservative
    // when we try to mutate app.list_state after calling a function that borrows app
    let items: Vec<ListItem> = {
        let prefs = crate::theme::settings();
        let viewport_rows = area.height.saturating_sub(2) as usize;
        let start = list_offset;
        let end = std::cmp::min(app.results.len(), start + viewport_rows);

        app.results
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // For rows outside the viewport, render a cheap empty item
                if i < start || i >= end {
                    return ListItem::new(Line::raw(""));
                }

                let (src, color) = match &p.source {
                    Source::Official { repo, .. } => {
                        let owner = app
                            .details_cache
                            .get(&p.name)
                            .map(|d| d.owner.clone())
                            .unwrap_or_default();
                        let label = crate::logic::distro::label_for_official(repo, &p.name, &owner);
                        let color = if label == "EOS" || label == "CachyOS" || label == "Manjaro" {
                            th.sapphire
                        } else {
                            th.green
                        };
                        (label, color)
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
                if let Some(pop) = p.popularity {
                    segs.push(Span::styled(
                        format!("Pop: {pop:.2} "),
                        Style::default().fg(th.overlay1),
                    ));
                }
                segs.push(Span::styled(format!("{src} "), Style::default().fg(color)));
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
                {
                    let in_install = app
                        .install_list
                        .iter()
                        .any(|it| it.name.eq_ignore_ascii_case(&p.name));
                    let in_remove = app
                        .remove_list
                        .iter()
                        .any(|it| it.name.eq_ignore_ascii_case(&p.name));
                    let in_downgrade = app
                        .downgrade_list
                        .iter()
                        .any(|it| it.name.eq_ignore_ascii_case(&p.name));

                    if in_install || in_remove || in_downgrade {
                        let (label, color) = if in_remove {
                            ("[-]", th.red)
                        } else if in_downgrade {
                            ("[↓]", th.yellow)
                        } else {
                            ("[+]", th.green)
                        };
                        match prefs.package_marker {
                            crate::theme::PackageMarker::FullLine => {
                                let mut item = ListItem::new(Line::from(segs));
                                let bgc = if in_install {
                                    if let ratatui::style::Color::Rgb(r, g, b) = color {
                                        ratatui::style::Color::Rgb(
                                            ((r as u16 * 85) / 100) as u8,
                                            ((g as u16 * 85) / 100) as u8,
                                            ((b as u16 * 85) / 100) as u8,
                                        )
                                    } else {
                                        color
                                    }
                                } else {
                                    color
                                };
                                item = item.style(Style::default().fg(th.crust).bg(bgc));
                                item
                            }
                            crate::theme::PackageMarker::Front => {
                                let mut new_segs: Vec<Span> = Vec::new();
                                new_segs.push(Span::styled(
                                    label.to_string(),
                                    Style::default()
                                        .fg(th.crust)
                                        .bg(color)
                                        .add_modifier(Modifier::BOLD),
                                ));
                                new_segs.push(Span::raw(" "));
                                new_segs.extend(segs);
                                ListItem::new(Line::from(new_segs))
                            }
                            crate::theme::PackageMarker::End => {
                                let mut new_segs = segs;
                                new_segs.push(Span::raw(" "));
                                new_segs.push(Span::styled(
                                    label.to_string(),
                                    Style::default()
                                        .fg(th.crust)
                                        .bg(color)
                                        .add_modifier(Modifier::BOLD),
                                ));
                                ListItem::new(Line::from(new_segs))
                            }
                        }
                    } else {
                        ListItem::new(Line::from(segs))
                    }
                }
            })
            .collect()
    };

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
        .highlight_style(Style::default().bg(th.surface1))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.list_state);

    // Draw status label on the bottom border line of the Results block
    status::render_status(f, app, area);

    // Optional: render sort dropdown overlay near the button
    sort_menu::render_sort_menu(f, app, area, btn_x);

    // Record inner results rect for mouse hit-testing
    utils::record_results_rect(app, area);
}

/// What: Render dropdown menus (Config/Lists, Panels, Options) on top layer.
///
/// This function should be called after all other UI elements are rendered
/// to ensure dropdowns appear on top.
pub use dropdowns::render_dropdowns;

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Results render computes title button rects and status label rect
    ///
    /// - Input: One result, operational status message
    /// - Output: Sort/Options/Config/Panels rects and arch_status/results rects are Some
    #[test]
    fn results_sets_title_button_rects_and_status_rect() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(120, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        // Seed minimal results to render
        app.results = vec![crate::state::PackageItem {
            name: "pkg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: Some(1.0),
        }];
        app.arch_status_text = "All systems operational".into();
        app.arch_status_color = crate::state::ArchStatusColor::Operational;

        term.draw(|f| {
            let area = f.area();
            render_results(f, &mut app, area);
        })
        .unwrap();

        assert!(app.sort_button_rect.is_some());
        assert!(app.options_button_rect.is_some());
        assert!(app.config_button_rect.is_some());
        assert!(app.panels_button_rect.is_some());
        assert!(app.arch_status_rect.is_some());
        assert!(app.results_rect.is_some());
    }
}
