use crate::state::AppState;
use crate::state::types::AppMode;
use crate::theme::theme;
use ratatui::{
    Frame,
    prelude::Rect,
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem},
};

/// Dropdown menu rendering module.
mod dropdowns;
/// Search results list rendering module.
mod list;
/// News feed rendering module.
mod news;
/// Sort menu rendering module.
mod sort_menu;
/// Status bar rendering module.
mod status;
/// Title bar rendering module.
mod title;
/// Utility functions for results rendering.
mod utils;

/// What: Context struct containing all extracted values needed for rendering.
///
/// Inputs: Values extracted from `AppState` to avoid borrow conflicts.
///
/// Output: Grouped context data.
///
/// Details: Reduces data flow complexity by grouping related values together.
pub struct RenderContext {
    /// Number of search results.
    pub results_len: usize,
    /// Optional repositories configuration.
    pub optional_repos: OptionalRepos,
    /// Menu states for dropdowns and menus.
    pub menu_states: MenuStates,
    /// Filter states for search results.
    pub filter_states: FilterStates,
}

/// What: Optional repository availability flags.
///
/// Inputs: Individual boolean flags for each optional repo.
///
/// Output: Struct containing all optional repo flags.
///
/// Details: Used to pass multiple optional repo flags as a single parameter.
#[allow(clippy::struct_excessive_bools)]
pub struct OptionalRepos {
    /// Whether `EndeavourOS` repository is available.
    pub has_eos: bool,
    /// Whether `CachyOS` repository is available.
    pub has_cachyos: bool,
    /// Whether `Artix` repository is available.
    pub has_artix: bool,
    /// Whether `Artix Omniverse` repository is available.
    pub has_artix_omniverse: bool,
    /// Whether `Artix Universe` repository is available.
    pub has_artix_universe: bool,
    /// Whether `Artix Lib32` repository is available.
    pub has_artix_lib32: bool,
    /// Whether `Artix Galaxy` repository is available.
    pub has_artix_galaxy: bool,
    /// Whether `Artix World` repository is available.
    pub has_artix_world: bool,
    /// Whether `Artix System` repository is available.
    pub has_artix_system: bool,
    /// Whether `Manjaro` repository is available.
    pub has_manjaro: bool,
}

/// What: Menu open/closed states.
///
/// Inputs: Individual boolean flags for each menu.
///
/// Output: Struct containing all menu states.
///
/// Details: Used to pass multiple menu states as a single parameter.
#[allow(clippy::struct_excessive_bools)]
pub struct MenuStates {
    /// Whether the sort menu is open.
    pub sort_menu_open: bool,
    /// Whether the config menu is open.
    pub config_menu_open: bool,
    /// Whether the panels menu is open.
    pub panels_menu_open: bool,
    /// Whether the options menu is open.
    pub options_menu_open: bool,
    /// Whether the collapsed menu is open.
    pub collapsed_menu_open: bool,
}

/// What: Filter toggle states.
///
/// Inputs: Individual boolean flags for each filter.
///
/// Output: Struct containing all filter states.
///
/// Details: Used to pass multiple filter states as a single parameter.
#[allow(clippy::struct_excessive_bools)]
pub struct FilterStates {
    /// Whether to show AUR packages.
    pub show_aur: bool,
    /// Whether to show core repository packages.
    pub show_core: bool,
    /// Whether to show extra repository packages.
    pub show_extra: bool,
    /// Whether to show multilib repository packages.
    pub show_multilib: bool,
    /// Whether to show `EndeavourOS` repository packages.
    pub show_eos: bool,
    /// Whether to show `CachyOS` repository packages.
    pub show_cachyos: bool,
    /// Whether to show `Artix` repository packages.
    pub show_artix: bool,
    /// Whether to show `Artix Omniverse` repository packages.
    pub show_artix_omniverse: bool,
    /// Whether to show `Artix Universe` repository packages.
    pub show_artix_universe: bool,
    /// Whether to show `Artix Lib32` repository packages.
    pub show_artix_lib32: bool,
    /// Whether to show `Artix Galaxy` repository packages.
    pub show_artix_galaxy: bool,
    /// Whether to show `Artix World` repository packages.
    pub show_artix_world: bool,
    /// Whether to show `Artix System` repository packages.
    pub show_artix_system: bool,
    /// Whether to show `Manjaro` repository packages.
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
/// - Reduces data flow complexity by extracting all data in one operation and batching mutations.
pub fn render_results(f: &mut Frame, app: &mut AppState, area: Rect) {
    if matches!(app.app_mode, AppMode::News) {
        render_news_results(f, app, area);
        return;
    }
    // Keep selection centered within the visible results list when possible
    utils::center_selection(app, area);

    // Extract all data needed for rendering in one operation to reduce data flow complexity
    let ctx = utils::extract_render_context(app);

    // Build title and record rects (mutates app)
    let title_spans = title::build_title_spans_from_context(app, &ctx, area);
    title::record_title_rects_from_context(app, &ctx, area);

    // Render list widget
    render_list_widget(f, app, area, &title_spans);

    // Render status and sort menu, record rects (all mutate app)
    status::render_status(f, app, area);
    let btn_x = app.sort_button_rect.map_or(area.x, |(x, _, _, _)| x);
    sort_menu::render_sort_menu(f, app, area, btn_x);
    utils::record_results_rect(app, area);
}

/// What: Render the list widget with title and items.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state for list items and `list_state`
/// - `area`: Target rectangle for the results block
/// - `title_spans`: Pre-built title spans
///
/// Output:
/// - Renders the list widget with items and title.
///
/// Details:
/// - Builds list items only for visible viewport to improve performance.
/// - Mutates `app.list_state` during rendering.
fn render_list_widget(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    title_spans: &[ratatui::text::Span<'static>],
) {
    let th = theme();
    let list_offset = app.list_state.offset();
    let viewport_rows = area.height.saturating_sub(2) as usize;
    let start = list_offset;
    let end = std::cmp::min(app.results.len(), start + viewport_rows);
    // Settings are cached; avoid per-frame reloads by fetching once and cloning.
    let prefs = crate::theme::settings();

    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let in_viewport = i >= start && i < end;
            list::build_list_item(p, app, &th, &prefs, in_viewport)
        })
        .collect();

    let list = List::new(items)
        .style(Style::default().fg(th.text).bg(th.base))
        .block(
            Block::default()
                .title(Line::from(title_spans.to_vec()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        )
        .highlight_style(Style::default().bg(th.surface1))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

/// What: Render dropdown menus (Config/Lists, Panels, Options) on top layer.
///
/// This function should be called after all other UI elements are rendered
/// to ensure dropdowns appear on top.
pub use dropdowns::render_dropdowns;

/// What: Render news results in the results area.
///
/// Inputs:
/// - `f`: Frame to render into.
/// - `app`: Application state.
/// - `area`: Area to render within.
///
/// Output: Renders news feed items as a list.
///
/// Details: Renders news feed items from `app.news_results` as a list with source labels.
fn render_news_results(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    // Record results rect first (before any other mutations)
    app.results_rect = Some((area.x, area.y, area.width, area.height));

    // Extract all immutable data we need first (clone to avoid borrowing)
    let news_loading = app.news_loading;
    let news_results = app.news_results.clone();
    let news_read_ids = app.news_read_ids.clone();
    let news_read_urls = app.news_read_urls.clone();
    let needs_select_none = news_loading && news_results.is_empty();

    // Now do all mutable operations first
    // Handle news_list_state mutation
    if needs_select_none {
        app.news_list_state.select(None);
    }

    // Build title spans and record rects (mutates app button/filter rects)
    let title_spans = news::build_news_title_spans_and_record_rects(app, area);

    // Build and render list inline to avoid storing items across mutable operations
    f.render_stateful_widget(
        List::new(
            news::build_news_list_items(
                app,
                news_loading,
                &news_results,
                &news_read_ids,
                &news_read_urls,
            )
            .0,
        )
        .style(Style::default().fg(th.text).bg(th.base))
        .block(
            Block::default()
                .title(Line::from(title_spans))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        )
        .highlight_style(Style::default().bg(th.surface1))
        .highlight_symbol("> "),
        area,
        &mut app.news_list_state,
    );
    let btn_x = app.sort_button_rect.map_or(area.x, |(x, _, _, _)| x);
    sort_menu::render_sort_menu(f, app, area, btn_x);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::NewsFeedSource;

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
    /// - `app`: `AppState` to populate with translations
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
        translations.insert("app.results.buttons.menu".to_string(), "Menu".to_string());
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
        translations.insert("app.news.filters.arch".to_string(), "Arch".to_string());
        translations.insert(
            "app.news.filters.advisories".to_string(),
            "Advisories".to_string(),
        );
        translations.insert(
            "app.news.filters.installed_only".to_string(),
            "Installed".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    #[test]
    fn results_sets_title_button_rects_and_status_rect() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(120, 20);
        let mut term = Terminal::new(backend).expect("failed to create test terminal");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        // Seed minimal results to render
        app.results = vec![crate::state::PackageItem {
            name: "pkg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: Some(1.0),
            out_of_date: None,
            orphaned: false,
        }];
        app.arch_status_text = "All systems operational".into();
        app.arch_status_color = crate::state::ArchStatusColor::Operational;

        term.draw(|f| {
            let area = f.area();
            render_results(f, &mut app, area);
        })
        .expect("failed to draw test terminal");

        assert!(app.sort_button_rect.is_some());
        assert!(app.options_button_rect.is_some());
        assert!(app.config_button_rect.is_some());
        assert!(app.panels_button_rect.is_some());
        assert!(app.arch_status_rect.is_some());
        assert!(app.results_rect.is_some());
    }

    #[test]
    fn news_filters_leave_gap_between_aur_comments_and_read_toggle() {
        use ratatui::{Terminal, backend::TestBackend};

        let backend = TestBackend::new(160, 10);
        let mut term = Terminal::new(backend).expect("failed to create test terminal");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.app_mode = AppMode::News;
        app.news_results = vec![crate::state::types::NewsFeedItem {
            id: "1".into(),
            date: "2025-01-01".into(),
            title: "Example update".into(),
            summary: None,
            url: None,
            source: NewsFeedSource::AurComment,
            severity: None,
            packages: Vec::new(),
        }];

        term.draw(|f| {
            let area = f.area();
            render_results(f, &mut app, area);
        })
        .expect("failed to draw test terminal");

        let buffer = term.backend().buffer();
        let mut title_line = String::new();
        for x in 0..buffer.area.width {
            title_line.push_str(buffer[(x, 0)].symbol());
        }

        let trimmed = title_line.trim_end();
        assert!(
            trimmed.contains("[AUR Comments]  [All]"),
            "expected spacing between AUR comments and read filters, saw: {trimmed}"
        );
    }
}
