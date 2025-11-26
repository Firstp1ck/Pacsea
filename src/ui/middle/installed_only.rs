use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List},
};

use crate::i18n;
use crate::state::AppState;
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
    let dg_indices: Vec<usize> = (0..app.downgrade_list.len()).collect();
    let downgrade_selected_idx = app.downgrade_state.selected();
    let downgrade_items = crate::ui::middle::install::build_package_list_items(
        &dg_indices,
        &app.downgrade_list,
        downgrade_selected_idx,
        install_focused
            && matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            ),
        |name| crate::ui::helpers::is_package_loading_preflight(app, name),
    );
    let downgrade_is_focused = install_focused
        && matches!(
            app.right_pane_focus,
            crate::state::RightPaneFocus::Downgrade
        );
    let inner_rect = render_package_list_widget(
        f,
        downgrade_items,
        area,
        i18n::t(app, "app.titles.downgrade_list_focused"),
        i18n::t(app, "app.titles.downgrade_list"),
        downgrade_is_focused,
        &mut app.downgrade_state,
    );
    app.downgrade_rect = Some((
        inner_rect.x,
        inner_rect.y,
        inner_rect.width,
        inner_rect.height,
    ));
}

/// What: Build a styled block for a package list widget.
///
/// Inputs:
/// - `title`: Block title text
/// - `is_focused`: Whether the list is focused
///
/// Output:
/// - Styled `Block` widget ready for use in a list.
///
/// Details:
/// - Applies focused/unfocused styling to title and border.
fn build_package_list_block(title: String, is_focused: bool) -> Block<'static> {
    let th = theme();
    Block::default()
        .title(Line::from(vec![Span::styled(
            title,
            Style::default().fg(if is_focused { th.mauve } else { th.overlay1 }),
        )]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if is_focused { th.mauve } else { th.surface1 }))
}

/// What: Build and render a package list widget with title and styling.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `items`: List items to render
/// - `area`: Target rectangle
/// - `title_focused`: Title for focused state
/// - `title_unfocused`: Title for unfocused state
/// - `is_focused`: Whether the list is focused
/// - `state`: List state for rendering
///
/// Output:
/// - Renders the list widget and returns the inner rect.
///
/// Details:
/// - Creates a block with title, styles the list, and renders it.
fn render_package_list_widget(
    f: &mut Frame,
    items: Vec<ratatui::widgets::ListItem<'_>>,
    area: Rect,
    title_focused: String,
    title_unfocused: String,
    is_focused: bool,
    state: &mut ratatui::widgets::ListState,
) -> Rect {
    let th = theme();
    let title = if is_focused {
        title_focused
    } else {
        title_unfocused
    };
    let block = build_package_list_block(title, is_focused);
    let list = List::new(items)
        .style(
            Style::default()
                .fg(if is_focused { th.text } else { th.subtext0 })
                .bg(th.base),
        )
        .block(block)
        .highlight_style(Style::default().fg(th.text).bg(th.surface2))
        .highlight_symbol("");
    f.render_stateful_widget(list, area, state);
    Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
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
    let rm_indices: Vec<usize> = (0..app.remove_list.len()).collect();
    let remove_selected_idx = app.remove_state.selected();
    let remove_items = crate::ui::middle::install::build_package_list_items(
        &rm_indices,
        &app.remove_list,
        remove_selected_idx,
        install_focused && matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove),
        |name| crate::ui::helpers::is_package_loading_preflight(app, name),
    );
    let remove_is_focused =
        install_focused && matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove);
    render_package_list_widget(
        f,
        remove_items,
        area,
        i18n::t(app, "app.titles.remove_list_focused"),
        i18n::t(app, "app.titles.remove_list"),
        remove_is_focused,
        &mut app.remove_state,
    );
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
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.downgrade_list.push(crate::state::PackageItem {
            name: "downgrade-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        });
        app.remove_list.push(crate::state::PackageItem {
            name: "remove-pkg".to_string(),
            version: "2.0.0".to_string(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
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
        let mut app = crate::state::AppState::default();
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
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);

        term.draw(|f| {
            let area = f.area();
            render_installed_only(f, &mut app, area);
        })
        .expect("Failed to render installed-only pane with split area");

        let downgrade_rect = app
            .downgrade_rect
            .expect("downgrade_rect should be set after rendering");
        let install_rect = app
            .install_rect
            .expect("install_rect should be set after rendering"); // Maps to Remove list

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
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);

        term.draw(|f| {
            let area = f.area();
            render_installed_only(f, &mut app, area);
        })
        .expect("Failed to render installed-only pane to record downgrade rect");

        assert!(app.downgrade_rect.is_some());
        let (x, y, w, h) = app
            .downgrade_rect
            .expect("downgrade_rect should be Some after is_some() check");
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
        let mut app = crate::state::AppState::default();
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
