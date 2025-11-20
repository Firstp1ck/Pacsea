use ratatui::{
    Frame,
    prelude::Rect,
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem},
};

use crate::state::AppState;
use crate::theme::theme;

mod dropdowns;
mod list;
mod sort_menu;
mod status;
mod title;
mod utils;

/// What: Context struct containing all extracted values needed for rendering.
///
/// Inputs: Values extracted from AppState to avoid borrow conflicts.
///
/// Output: Grouped context data.
///
/// Details: Reduces data flow complexity by grouping related values together.
pub struct RenderContext {
    pub results_len: usize,
    pub optional_repos: OptionalRepos,
    pub menu_states: MenuStates,
    pub filter_states: FilterStates,
}

/// What: Optional repository availability flags.
///
/// Inputs: Individual boolean flags for each optional repo.
///
/// Output: Struct containing all optional repo flags.
///
/// Details: Used to pass multiple optional repo flags as a single parameter.
pub struct OptionalRepos {
    pub has_eos: bool,
    pub has_cachyos: bool,
    pub has_artix: bool,
    pub has_artix_omniverse: bool,
    pub has_artix_universe: bool,
    pub has_artix_lib32: bool,
    pub has_artix_galaxy: bool,
    pub has_artix_world: bool,
    pub has_artix_system: bool,
    pub has_manjaro: bool,
}

/// What: Menu open/closed states.
///
/// Inputs: Individual boolean flags for each menu.
///
/// Output: Struct containing all menu states.
///
/// Details: Used to pass multiple menu states as a single parameter.
pub struct MenuStates {
    pub sort_menu_open: bool,
    pub config_menu_open: bool,
    pub panels_menu_open: bool,
    pub options_menu_open: bool,
}

/// What: Filter toggle states.
///
/// Inputs: Individual boolean flags for each filter.
///
/// Output: Struct containing all filter states.
///
/// Details: Used to pass multiple filter states as a single parameter.
pub struct FilterStates {
    pub show_aur: bool,
    pub show_core: bool,
    pub show_extra: bool,
    pub show_multilib: bool,
    pub show_eos: bool,
    pub show_cachyos: bool,
    pub show_artix: bool,
    pub show_artix_omniverse: bool,
    pub show_artix_universe: bool,
    pub show_artix_lib32: bool,
    pub show_artix_galaxy: bool,
    pub show_artix_world: bool,
    pub show_artix_system: bool,
    pub show_manjaro: bool,
}

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

    // Keep selection centered within the visible results list when possible
    utils::center_selection(app, area);

    // Extract values needed for title building before mutating app
    // Group into context struct to reduce data flow complexity
    let (has_eos, has_cachyos, has_artix, has_artix_repos, has_manjaro) =
        utils::detect_optional_repos(app);
    let (
        has_artix_omniverse,
        has_artix_universe,
        has_artix_lib32,
        has_artix_galaxy,
        has_artix_world,
        has_artix_system,
    ) = has_artix_repos;

    let ctx = RenderContext {
        results_len: app.results.len(),
        optional_repos: OptionalRepos {
            has_eos,
            has_cachyos,
            has_artix,
            has_artix_omniverse,
            has_artix_universe,
            has_artix_lib32,
            has_artix_galaxy,
            has_artix_world,
            has_artix_system,
            has_manjaro,
        },
        menu_states: MenuStates {
            sort_menu_open: app.sort_menu_open,
            config_menu_open: app.config_menu_open,
            panels_menu_open: app.panels_menu_open,
            options_menu_open: app.options_menu_open,
        },
        filter_states: FilterStates {
            show_aur: app.results_filter_show_aur,
            show_core: app.results_filter_show_core,
            show_extra: app.results_filter_show_extra,
            show_multilib: app.results_filter_show_multilib,
            show_eos: app.results_filter_show_eos,
            show_cachyos: app.results_filter_show_cachyos,
            show_artix: app.results_filter_show_artix,
            show_artix_omniverse: app.results_filter_show_artix_omniverse,
            show_artix_universe: app.results_filter_show_artix_universe,
            show_artix_lib32: app.results_filter_show_artix_lib32,
            show_artix_galaxy: app.results_filter_show_artix_galaxy,
            show_artix_world: app.results_filter_show_artix_world,
            show_artix_system: app.results_filter_show_artix_system,
            show_manjaro: app.results_filter_show_manjaro,
        },
    };

    // Build title with Sort button, filter toggles, and a right-aligned Options button
    // (using context struct to avoid borrow conflicts and reduce data flow complexity)
    let title_spans = title::build_title_spans_from_context(app, &ctx, area);

    // Record clickable rects for title bar controls (mutates app)
    title::record_title_rects_from_context(app, &ctx, area);

    // Extract sort button x position for sort menu positioning
    let btn_x = app.sort_button_rect.map(|(x, _, _, _)| x).unwrap_or(area.x);

    // Build list items (only visible ones for performance)
    // Extract offset before building to avoid borrowing app.list_state
    let list_offset = app.list_state.offset();

    // Build the list items using helper functions to reduce complexity
    let items: Vec<ListItem> = {
        let prefs = crate::theme::settings();
        let viewport_rows = area.height.saturating_sub(2) as usize;
        let start = list_offset;
        let end = std::cmp::min(app.results.len(), start + viewport_rows);

        app.results
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let in_viewport = i >= start && i < end;
                list::build_list_item(p, app, &th, &prefs, in_viewport)
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

    /// What: Ensure rendering results populates button rectangles and status overlays without panic.
    ///
    /// Inputs:
    /// - Single search result plus an operational status message.
    ///
    /// Output:
    /// - Sort, Options, Config, Panels button rectangles, along with status and results rects, become `Some`.
    ///
    /// Details:
    /// - Uses a `TestBackend` terminal to exercise layout code and verify hit-test regions are recorded.
    ///
    /// What: Initialize minimal English translations for tests.
    ///
    /// Inputs:
    /// - `app`: AppState to populate with translations
    ///
    /// Output:
    /// - Populates `app.translations` and `app.translations_fallback` with minimal English translations
    ///
    /// Details:
    /// - Sets up only the translations needed for tests to pass
    fn init_test_translations(app: &mut crate::state::AppState) {
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert("app.results.title".to_string(), "Results".to_string());
        translations.insert("app.results.buttons.sort".to_string(), "Sort".to_string());
        translations.insert(
            "app.results.buttons.options".to_string(),
            "Options".to_string(),
        );
        translations.insert(
            "app.results.buttons.panels".to_string(),
            "Panels".to_string(),
        );
        translations.insert(
            "app.results.buttons.config_lists".to_string(),
            "Config/Lists".to_string(),
        );
        translations.insert("app.results.filters.aur".to_string(), "AUR".to_string());
        translations.insert("app.results.filters.core".to_string(), "core".to_string());
        translations.insert("app.results.filters.extra".to_string(), "extra".to_string());
        translations.insert(
            "app.results.filters.multilib".to_string(),
            "multilib".to_string(),
        );
        translations.insert("app.results.filters.eos".to_string(), "EOS".to_string());
        translations.insert(
            "app.results.filters.cachyos".to_string(),
            "CachyOS".to_string(),
        );
        translations.insert("app.results.filters.artix".to_string(), "Artix".to_string());
        translations.insert(
            "app.results.filters.artix_omniverse".to_string(),
            "OMNI".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_universe".to_string(),
            "UNI".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_lib32".to_string(),
            "LIB32".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_galaxy".to_string(),
            "GALAXY".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_world".to_string(),
            "WORLD".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_system".to_string(),
            "SYSTEM".to_string(),
        );
        translations.insert(
            "app.results.filters.manjaro".to_string(),
            "Manjaro".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    #[test]
    fn results_sets_title_button_rects_and_status_rect() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(120, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
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
