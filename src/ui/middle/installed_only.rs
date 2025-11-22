use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem},
};

use crate::i18n;
use crate::state::{AppState, Source};
use crate::theme::theme;

/// What: Render Downgrade and Remove lists side-by-side in installed-only mode.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (downgrade/remove lists, focus, selection)
/// - `area`: Target rectangle for the right pane (will be split 50/50)
///
/// Output:
/// - Draws Downgrade (left) and Remove (right) lists, records rects for mouse hit-testing.
///
/// Details:
/// - Splits the area horizontally into two equal panes.
/// - Import/Export buttons are not shown in installed-only mode.
pub fn render_installed_only(f: &mut Frame, app: &mut AppState, area: Rect) {
    let install_focused = matches!(app.focus, crate::state::Focus::Install);

    // In installed-only mode, split the right pane into Downgrade (left) and Remove (right)
    let right_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Downgrade List (left)
    render_downgrade_list(f, app, right_split[0], install_focused);

    // Remove List (right)
    render_remove_list(f, app, right_split[1], install_focused);

    // Record inner Install rect for mouse hit-testing (map to Remove list area)
    app.install_rect = Some((
        right_split[1].x + 1,
        right_split[1].y + 1,
        right_split[1].width.saturating_sub(2),
        right_split[1].height.saturating_sub(2),
    ));
    // Import/Export buttons not shown in installed-only mode
    app.install_import_rect = None;
    app.install_export_rect = None;
}

/// What: Render the Downgrade list in the left half of the installed-only pane.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (downgrade list, focus, selection)
/// - `area`: Target rectangle for the downgrade list
/// - `install_focused`: Whether the install pane is focused
///
/// Output:
/// - Draws the downgrade list and records inner rect for mouse hit-testing.
fn render_downgrade_list(f: &mut Frame, app: &mut AppState, area: Rect, install_focused: bool) {
    let th = theme();
    let dg_indices: Vec<usize> = (0..app.downgrade_list.len()).collect();
    let downgrade_selected_idx = app.downgrade_state.selected();
    let downgrade_items: Vec<ListItem> = dg_indices
        .iter()
        .enumerate()
        .filter_map(|(display_idx, &i)| app.downgrade_list.get(i).map(|p| (display_idx, p)))
        .map(|(display_idx, p)| {
            let (src, color) = match &p.source {
                Source::Official { repo, .. } => (repo.to_string(), th.green),
                Source::Aur => ("AUR".to_string(), th.yellow),
            };
            let mut segs: Vec<Span> = Vec::new();

            // Add selection indicator manually if this item is selected
            let is_selected = downgrade_selected_idx == Some(display_idx);
            if is_selected {
                segs.push(Span::styled(
                    "▶ ",
                    Style::default()
                        .fg(if install_focused {
                            th.text
                        } else {
                            th.subtext0
                        })
                        .bg(if install_focused {
                            th.surface2
                        } else {
                            th.base
                        }),
                ));
            } else {
                // Add spacing to align with selected items
                segs.push(Span::raw("  "));
            }

            // Add loading indicator if package is being processed (same position and style regardless of selection)
            if crate::ui::helpers::is_package_loading_preflight(app, &p.name) {
                // Use explicit style that overrides highlight_style - always sapphire blue and bold
                segs.push(Span::styled(
                    "⟳ ",
                    Style::default()
                        .fg(th.sapphire)
                        .bg(if is_selected && install_focused {
                            th.surface2
                        } else {
                            th.base
                        })
                        .add_modifier(Modifier::BOLD),
                ));
            } else if !is_selected {
                // Add spacing when not loading and not selected to maintain alignment
                segs.push(Span::raw("  "));
            }

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
        i18n::t(app, "app.titles.downgrade_list_focused")
    } else {
        i18n::t(app, "app.titles.downgrade_list")
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
        .highlight_style(Style::default().fg(th.text).bg(th.surface2))
        .highlight_symbol(""); // Empty symbol since we're adding it manually
    f.render_stateful_widget(downgrade_list, area, &mut app.downgrade_state);
    // Record inner Downgrade rect
    app.downgrade_rect = Some((
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    ));
}

/// What: Render the Remove list in the right half of the installed-only pane.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (remove list, focus, selection)
/// - `area`: Target rectangle for the remove list
/// - `install_focused`: Whether the install pane is focused
///
/// Output:
/// - Draws the remove list.
fn render_remove_list(f: &mut Frame, app: &mut AppState, area: Rect, install_focused: bool) {
    let th = theme();
    let rm_indices: Vec<usize> = (0..app.remove_list.len()).collect();
    let remove_selected_idx = app.remove_state.selected();
    let remove_items: Vec<ListItem> = rm_indices
        .iter()
        .enumerate()
        .filter_map(|(display_idx, &i)| app.remove_list.get(i).map(|p| (display_idx, p)))
        .map(|(display_idx, p)| {
            let (src, color) = match &p.source {
                Source::Official { repo, .. } => (repo.to_string(), th.green),
                Source::Aur => ("AUR".to_string(), th.yellow),
            };
            let mut segs: Vec<Span> = Vec::new();

            // Add selection indicator manually if this item is selected
            let is_selected = remove_selected_idx == Some(display_idx);
            if is_selected {
                segs.push(Span::styled(
                    "▶ ",
                    Style::default()
                        .fg(if install_focused {
                            th.text
                        } else {
                            th.subtext0
                        })
                        .bg(if install_focused {
                            th.surface2
                        } else {
                            th.base
                        }),
                ));
            } else {
                // Add spacing to align with selected items
                segs.push(Span::raw("  "));
            }

            // Add loading indicator if package is being processed (same position and style regardless of selection)
            if crate::ui::helpers::is_package_loading_preflight(app, &p.name) {
                // Use explicit style that overrides highlight_style - always sapphire blue and bold
                segs.push(Span::styled(
                    "⟳ ",
                    Style::default()
                        .fg(th.sapphire)
                        .bg(if is_selected && install_focused {
                            th.surface2
                        } else {
                            th.base
                        })
                        .add_modifier(Modifier::BOLD),
                ));
            } else if !is_selected {
                // Add spacing when not loading and not selected to maintain alignment
                segs.push(Span::raw("  "));
            }

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
    let remove_is_focused =
        install_focused && matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove);
    let remove_title = if remove_is_focused {
        i18n::t(app, "app.titles.remove_list_focused")
    } else {
        i18n::t(app, "app.titles.remove_list")
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
        .highlight_style(Style::default().fg(th.text).bg(th.surface2))
        .highlight_symbol(""); // Empty symbol since we're adding it manually
    f.render_stateful_widget(remove_list, area, &mut app.remove_state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// What: Initialize minimal English translations for installed-only tests.
    ///
    /// Inputs:
    /// - `app`: `AppState` to populate with translations
    ///
    /// Output:
    /// - Populates `app.translations` and `app.translations_fallback` with installed-only translations
    ///
    /// Details:
    /// - Sets up only the translations needed for installed-only rendering tests.
    fn init_test_translations(app: &mut crate::state::AppState) {
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert(
            "app.titles.downgrade_list".to_string(),
            "Downgrade".to_string(),
        );
        translations.insert(
            "app.titles.downgrade_list_focused".to_string(),
            "Downgrade".to_string(),
        );
        translations.insert("app.titles.remove_list".to_string(), "Remove".to_string());
        translations.insert(
            "app.titles.remove_list_focused".to_string(),
            "Remove".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    /// What: Verify installed-only mode renders both downgrade and remove lists.
    ///
    /// Inputs:
    /// - Installed-only mode with packages in both downgrade and remove lists
    ///
    /// Output:
    /// - Both lists render and `app.install_rect` maps to Remove list area.
    ///
    /// Details:
    /// - Tests that area is split 50/50 and rects are recorded correctly.
    #[test]
    fn installed_only_renders_both_lists() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        app.downgrade_list.push(crate::state::PackageItem {
            name: "downgrade-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        });
        app.remove_list.push(crate::state::PackageItem {
            name: "remove-pkg".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        });

        term.draw(|f| {
            let area = f.area();
            render_installed_only(f, &mut app, area);
        })
        .expect("Failed to render installed-only pane");

        assert!(app.install_rect.is_some());
        assert!(app.downgrade_rect.is_some());
        // Import/Export buttons should be hidden
        assert!(app.install_import_rect.is_none());
        assert!(app.install_export_rect.is_none());
    }

    /// What: Verify installed-only mode clears button rects.
    ///
    /// Inputs:
    /// - Installed-only mode activated
    ///
    /// Output:
    /// - `app.install_import_rect` and `app.install_export_rect` are set to `None`.
    ///
    /// Details:
    /// - Tests that Import/Export buttons are not shown in installed-only mode.
    #[test]
    fn installed_only_hides_buttons() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        // Set initial button rects
        app.install_import_rect = Some((10, 10, 10, 1));
        app.install_export_rect = Some((20, 10, 10, 1));

        term.draw(|f| {
            let area = f.area();
            render_installed_only(f, &mut app, area);
        })
        .expect("Failed to render installed-only pane without buttons");

        assert!(app.install_import_rect.is_none());
        assert!(app.install_export_rect.is_none());
    }

    /// What: Verify installed-only mode splits area correctly.
    ///
    /// Inputs:
    /// - Area of width 100 split into two lists
    ///
    /// Output:
    /// - Downgrade and Remove lists each get approximately 50% width.
    ///
    /// Details:
    /// - Tests that layout splitting produces two equal panes.
    #[test]
    fn installed_only_splits_area_correctly() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);

        term.draw(|f| {
            let area = f.area();
            render_installed_only(f, &mut app, area);
        })
        .expect("Failed to render installed-only pane with split area");

        let downgrade_rect = app.downgrade_rect.unwrap();
        let install_rect = app.install_rect.unwrap(); // Maps to Remove list

        // Both should have similar widths (accounting for borders)
        let downgrade_width = downgrade_rect.2;
        let remove_width = install_rect.2;
        // Widths should be approximately equal (within 1 pixel due to rounding)
        assert!((i32::from(downgrade_width) - i32::from(remove_width)).abs() <= 1);
    }

    /// What: Verify installed-only mode records downgrade rect.
    ///
    /// Inputs:
    /// - Installed-only mode with downgrade list
    ///
    /// Output:
    /// - `app.downgrade_rect` is set to inner rectangle of downgrade list.
    ///
    /// Details:
    /// - Tests that downgrade rect excludes borders.
    #[test]
    fn installed_only_records_downgrade_rect() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);

        term.draw(|f| {
            let area = f.area();
            render_installed_only(f, &mut app, area);
        })
        .expect("Failed to render installed-only pane to record downgrade rect");

        assert!(app.downgrade_rect.is_some());
        let (x, y, w, h) = app.downgrade_rect.unwrap();
        // Rect should exclude borders
        assert_eq!(x, 1);
        assert_eq!(y, 1);
        assert!(w > 0);
        assert!(h > 0);
    }

    /// What: Verify installed-only mode handles empty lists.
    ///
    /// Inputs:
    /// - Installed-only mode with empty downgrade and remove lists
    ///
    /// Output:
    /// - Both lists render without panic.
    ///
    /// Details:
    /// - Tests edge case where lists are empty.
    #[test]
    fn installed_only_handles_empty_lists() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        app.downgrade_list.clear();
        app.remove_list.clear();

        term.draw(|f| {
            let area = f.area();
            render_installed_only(f, &mut app, area);
        })
        .expect("Failed to render installed-only pane with empty lists");

        // Should render empty lists without panic
        assert!(app.install_rect.is_some());
        assert!(app.downgrade_rect.is_some());
    }
}
