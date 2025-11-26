use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::{AppState, PackageItem, Source};
use crate::theme::theme;

/// What: Build list items for a package list with selection and loading indicators.
///
/// Inputs:
/// - `indices`: Display indices into the package list
/// - `packages`: Full package list
/// - `selected_idx`: Currently selected display index
/// - `focused`: Whether the pane is focused
/// - `app`: Application state for loading checks
///
/// Output:
/// - Vector of `ListItem` widgets ready for rendering
///
/// Details:
/// - Adds selection indicator, loading indicator, popularity, source, name, and version.
pub(super) fn build_package_list_items<'a, F>(
    indices: &[usize],
    packages: &'a [PackageItem],
    selected_idx: Option<usize>,
    focused: bool,
    is_loading: F,
) -> Vec<ListItem<'a>>
where
    F: Fn(&str) -> bool,
{
    let th = theme();
    indices
        .iter()
        .enumerate()
        .filter_map(|(display_idx, &i)| packages.get(i).map(|p| (display_idx, p)))
        .map(|(display_idx, p)| {
            let (src, color) = match &p.source {
                Source::Official { repo, .. } => (repo.clone(), th.green),
                Source::Aur => ("AUR".to_string(), th.yellow),
            };
            let mut segs: Vec<Span> = Vec::new();

            // Add selection indicator manually if this item is selected
            let is_selected = selected_idx == Some(display_idx);
            if is_selected {
                segs.push(Span::styled(
                    "▶ ",
                    Style::default()
                        .fg(if focused { th.text } else { th.subtext0 })
                        .bg(if focused { th.surface2 } else { th.base }),
                ));
            } else {
                // Add spacing to align with selected items
                segs.push(Span::raw("  "));
            }

            // Add loading indicator if package is being processed
            if is_loading(&p.name) {
                segs.push(Span::styled(
                    "⟳ ",
                    Style::default()
                        .fg(th.sapphire)
                        .bg(if is_selected && focused {
                            th.surface2
                        } else {
                            th.base
                        })
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                // Add spacing when not loading to maintain alignment
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
                    .fg(if focused { th.text } else { th.subtext0 })
                    .add_modifier(Modifier::BOLD),
            ));
            segs.push(Span::styled(
                format!("  {}", p.version),
                Style::default().fg(if focused { th.overlay1 } else { th.surface2 }),
            ));
            ListItem::new(Line::from(segs))
        })
        .collect()
}

/// What: Render the normal Install list (single right pane) with Import/Export buttons.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (install list, focus, selection)
/// - `area`: Target rectangle for the install pane
///
/// Output:
/// - Draws the install list and Import/Export buttons, records inner rect for mouse hit-testing.
///
/// Details:
/// - Shows filtered install list items with selection indicators and loading indicators.
/// - Import/Export buttons are rendered at the bottom border.
pub fn render_install(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let install_focused = matches!(app.focus, crate::state::Focus::Install);

    // Normal Install List (single right pane)
    let indices: Vec<usize> = crate::ui::helpers::filtered_install_indices(app);
    let selected_idx = app.install_state.selected();
    let install_items = build_package_list_items(
        &indices,
        &app.install_list,
        selected_idx,
        install_focused,
        |name| crate::ui::helpers::is_package_loading_preflight(app, name),
    );
    let title_text = if install_focused {
        i18n::t(app, "app.titles.install_list_focused")
    } else {
        i18n::t(app, "app.titles.install_list")
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
        .highlight_style(Style::default().fg(th.text).bg(th.surface2))
        .highlight_symbol(""); // Empty symbol since we're adding it manually
    f.render_stateful_widget(install_list, area, &mut app.install_state);
    app.install_rect = Some((
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    ));

    // Bottom border action buttons: Export (left) and Import (right)
    render_buttons(f, app, area, install_focused);
}

/// What: Render Import/Export buttons at the bottom border of the install pane.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (for i18n and storing button rects)
/// - `area`: Target rectangle for the install pane
/// - `install_focused`: Whether the install pane is focused
///
/// Output:
/// - Draws Import and Export buttons, records their rects for mouse hit-testing.
///
/// Details:
/// - Import button is on the far right, Export is to its left with a 2-space gap.
/// - First character of each label is underlined to indicate keyboard shortcut.
fn render_buttons(f: &mut Frame, app: &mut AppState, area: Rect, install_focused: bool) {
    let th = theme();
    let import_label = i18n::t(app, "app.actions.import");
    let export_label = i18n::t(app, "app.actions.export");
    // Style similar to other title buttons
    let btn_style_active = Style::default()
        .fg(th.crust)
        .bg(th.green)
        .add_modifier(Modifier::BOLD);
    let btn_style_inactive = Style::default()
        .fg(th.mauve)
        .bg(th.surface2)
        .add_modifier(Modifier::BOLD);
    let style = if install_focused {
        btn_style_active
    } else {
        btn_style_inactive
    };

    let inner_w = area.width.saturating_sub(2);
    let sy = area.y + area.height.saturating_sub(1);

    // Import button on the far right
    // Use Unicode display width, not byte length, to handle wide characters
    let import_w = u16::try_from(import_label.width()).unwrap_or(u16::MAX);
    let import_sx = area.x + 1 + inner_w.saturating_sub(import_w);
    let import_rect = Rect {
        x: import_sx,
        y: sy,
        width: import_w.min(inner_w),
        height: 1,
    };
    // Split label for styling: first character underlined, rest normal
    let import_first_char = import_label
        .chars()
        .next()
        .map(|c| c.to_string())
        .unwrap_or_default();
    let import_suffix = import_label.chars().skip(1).collect::<String>();
    let import_line = Paragraph::new(Line::from(vec![
        Span::styled(import_first_char, style.add_modifier(Modifier::UNDERLINED)),
        Span::styled(import_suffix, style),
    ]));
    app.install_import_rect = Some((
        import_rect.x,
        import_rect.y,
        import_rect.width,
        import_rect.height,
    ));
    f.render_widget(import_line, import_rect);

    // Export button to the left of Import with 2 spaces gap
    let gap: u16 = 2;
    let export_w = u16::try_from(export_label.width()).unwrap_or(u16::MAX);
    let export_max_w = inner_w;
    let export_right = import_rect.x.saturating_sub(gap);
    let export_sx = if export_w > export_right.saturating_sub(area.x + 1) {
        area.x + 1
    } else {
        export_right.saturating_sub(export_w)
    };
    let export_rect = Rect {
        x: export_sx,
        y: sy,
        width: export_w.min(export_max_w),
        height: 1,
    };
    // Split label for styling: first character underlined, rest normal
    let export_first_char = export_label
        .chars()
        .next()
        .map(|c| c.to_string())
        .unwrap_or_default();
    let export_suffix = export_label.chars().skip(1).collect::<String>();
    let export_line = Paragraph::new(Line::from(vec![
        Span::styled(export_first_char, style.add_modifier(Modifier::UNDERLINED)),
        Span::styled(export_suffix, style),
    ]));
    app.install_export_rect = Some((
        export_rect.x,
        export_rect.y,
        export_rect.width,
        export_rect.height,
    ));
    f.render_widget(export_line, export_rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// What: Initialize minimal English translations for install tests.
    ///
    /// Inputs:
    /// - `app`: `AppState` to populate with translations
    ///
    /// Output:
    /// - Populates `app.translations` and `app.translations_fallback` with install-related translations
    ///
    /// Details:
    /// - Sets up only the translations needed for install rendering tests.
    fn init_test_translations(app: &mut crate::state::AppState) {
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert("app.titles.install_list".to_string(), "Install".to_string());
        translations.insert(
            "app.titles.install_list_focused".to_string(),
            "Install".to_string(),
        );
        translations.insert("app.actions.import".to_string(), "Import".to_string());
        translations.insert("app.actions.export".to_string(), "Export".to_string());
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    /// What: Verify install list renders and records rect when visible.
    ///
    /// Inputs:
    /// - Install pane is visible with some packages in install list
    ///
    /// Output:
    /// - Install list renders and `app.install_rect` is set to inner rectangle.
    ///
    /// Details:
    /// - Tests that rect is recorded with borders excluded.
    #[test]
    fn install_renders_and_records_rect() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.install_list.push(crate::state::PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        });

        term.draw(|f| {
            let area = f.area();
            render_install(f, &mut app, area);
        })
        .expect("Failed to render install pane");

        assert!(app.install_rect.is_some());
        assert!(app.install_import_rect.is_some());
        assert!(app.install_export_rect.is_some());
    }

    /// What: Verify install list renders buttons with correct rects.
    ///
    /// Inputs:
    /// - Install pane with Import/Export buttons
    ///
    /// Output:
    /// - Import and Export button rects are recorded correctly.
    ///
    /// Details:
    /// - Tests that Import button is on the right, Export is to its left with gap.
    #[test]
    fn install_renders_buttons_with_correct_rects() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);

        term.draw(|f| {
            let area = f.area();
            render_install(f, &mut app, area);
        })
        .expect("Failed to render install pane buttons");

        let import_rect = app
            .install_import_rect
            .expect("install_import_rect should be set after rendering");
        let export_rect = app
            .install_export_rect
            .expect("install_export_rect should be set after rendering");

        // Import should be to the right of Export
        assert!(import_rect.0 > export_rect.0);
        // Both should be on the same row (bottom border)
        assert_eq!(import_rect.1, export_rect.1);
        // Import should be at or near the far right (accounting for borders)
        // The area width is 100, so with borders (x+1, width-2), the right edge is at 99
        assert!(import_rect.0 + import_rect.2 >= 99);
    }

    /// What: Verify install list renders with selection indicators.
    ///
    /// Inputs:
    /// - Install list with packages and a selected item
    ///
    /// Output:
    /// - Selected item displays selection indicator "▶ ".
    ///
    /// Details:
    /// - Tests that selection state affects rendering.
    #[test]
    fn install_renders_with_selection() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.install_list.push(crate::state::PackageItem {
            name: "package1".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        });
        app.install_list.push(crate::state::PackageItem {
            name: "package2".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        });
        app.install_state.select(Some(0));

        term.draw(|f| {
            let area = f.area();
            render_install(f, &mut app, area);
        })
        .expect("Failed to render install pane with selection");

        // Should render without panic with selection
        assert!(app.install_rect.is_some());
    }

    /// What: Verify install list renders differently when focused vs unfocused.
    ///
    /// Inputs:
    /// - Install list rendered first unfocused, then focused
    ///
    /// Output:
    /// - Button styles change based on focus state.
    ///
    /// Details:
    /// - Tests that focus affects button styling (active vs inactive).
    #[test]
    fn install_renders_with_focus_styling() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);

        // Render unfocused
        app.focus = crate::state::Focus::Search;
        term.draw(|f| {
            let area = f.area();
            render_install(f, &mut app, area);
        })
        .expect("Failed to render unfocused install pane");
        let unfocused_import_rect = app.install_import_rect;

        // Render focused
        app.focus = crate::state::Focus::Install;
        term.draw(|f| {
            let area = f.area();
            render_install(f, &mut app, area);
        })
        .expect("Failed to render focused install pane");
        let focused_import_rect = app.install_import_rect;

        // Rects should be the same position, but styling differs
        assert_eq!(unfocused_import_rect, focused_import_rect);
    }

    /// What: Verify install list handles empty list correctly.
    ///
    /// Inputs:
    /// - Install list with no packages
    ///
    /// Output:
    /// - Install list renders without panic.
    ///
    /// Details:
    /// - Tests edge case where install list is empty.
    #[test]
    fn install_handles_empty_list() {
        let backend = TestBackend::new(100, 30);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.install_list.clear();

        term.draw(|f| {
            let area = f.area();
            render_install(f, &mut app, area);
        })
        .expect("Failed to render empty install pane");

        // Should render empty list without panic
        assert!(app.install_rect.is_some());
        assert!(app.install_import_rect.is_some());
        assert!(app.install_export_rect.is_some());
    }
}
