use crate::i18n;
use crate::state::AppState;
use crate::state::types::{AppMode, NewsFeedSource};
use crate::theme::theme;
use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem},
};
use unicode_width::UnicodeWidthStr;

/// Dropdown menu rendering module.
mod dropdowns;
/// Search results list rendering module.
mod list;
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
#[allow(clippy::too_many_lines)]
fn render_news_results(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let items: Vec<ListItem> = app
        .news_results
        .iter()
        .map(|item| {
            let source_label = match item.source {
                NewsFeedSource::ArchNews => "Arch",
                NewsFeedSource::SecurityAdvisory => "Advisory",
            };
            let source_color = match item.source {
                NewsFeedSource::ArchNews => th.sapphire,
                NewsFeedSource::SecurityAdvisory => th.yellow,
            };
            let sev = item
                .severity
                .as_ref()
                .map_or_else(String::new, |s| format!("{s:?}"));
            let mut spans = vec![
                ratatui::text::Span::styled(
                    format!("[{source_label}]"),
                    Style::default().fg(source_color),
                ),
                ratatui::text::Span::raw(" "),
                ratatui::text::Span::raw(item.date.clone()),
                ratatui::text::Span::raw(" "),
                ratatui::text::Span::raw(item.title.clone()),
            ];
            if !sev.is_empty() {
                spans.push(ratatui::text::Span::raw(" "));
                spans.push(ratatui::text::Span::styled(
                    format!("[{sev}]"),
                    Style::default().fg(th.yellow),
                ));
            }
            ListItem::new(Line::from(spans)).style(Style::default().fg(th.text))
        })
        .collect();
    let title_text = format!("News Feed ({})", app.news_results.len());
    let age_label = app
        .news_max_age_days
        .map_or_else(|| "All".to_string(), |d| format!("{d} Days"));
    let date_label = format!("Date: {age_label}");
    let options_label = format!("{} v", i18n::t(app, "app.results.buttons.options"));
    let panels_label = format!("{} v", i18n::t(app, "app.results.buttons.panels"));
    let config_label = format!("{} v", i18n::t(app, "app.results.buttons.config_lists"));
    let arch_filter_label = format!("[{}]", i18n::t(app, "app.news.filters.arch"));
    let advisory_filter_label = format!("[{}]", i18n::t(app, "app.news.filters.advisories"));
    let installed_filter_label = format!("[{}]", i18n::t(app, "app.news.filters.installed_only"));

    let button_style = |is_open: bool| -> Style {
        if is_open {
            Style::default()
                .fg(th.crust)
                .bg(th.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD)
        }
    };

    let render_button = |label: &str, is_open: bool| -> Vec<Span<'static>> {
        let style = button_style(is_open);
        let mut spans = Vec::new();
        if let Some(first) = label.chars().next() {
            let rest = &label[first.len_utf8()..];
            spans.push(Span::styled(
                first.to_string(),
                style.add_modifier(Modifier::UNDERLINED),
            ));
            spans.push(Span::styled(rest.to_string(), style));
        } else {
            spans.push(Span::styled(label.to_string(), style));
        }
        spans
    };

    let render_filter = |label: &str, active: bool| -> Span<'static> {
        let (fg, bg) = if active {
            (th.crust, th.green)
        } else {
            (th.mauve, th.surface2)
        };
        Span::styled(
            label.to_string(),
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
        )
    };

    let date_button_spans = render_button(&date_label, false);
    let options_button_spans = render_button(&options_label, app.options_menu_open);
    let panels_button_spans = render_button(&panels_label, app.panels_menu_open);
    let config_button_spans = render_button(&config_label, app.config_menu_open);
    let arch_filter_span = render_filter(&arch_filter_label, app.news_filter_show_arch_news);
    let advisory_filter_span =
        render_filter(&advisory_filter_label, app.news_filter_show_advisories);
    let installed_filter_span =
        render_filter(&installed_filter_label, app.news_filter_installed_only);

    let inner_width = area.width.saturating_sub(2);
    let title_width = u16::try_from(title_text.width()).unwrap_or(u16::MAX);
    let arch_width = u16::try_from(arch_filter_label.width()).unwrap_or(u16::MAX);
    let advisory_width = u16::try_from(advisory_filter_label.width()).unwrap_or(u16::MAX);
    let installed_width = u16::try_from(installed_filter_label.width()).unwrap_or(u16::MAX);
    let date_width = u16::try_from(date_label.width()).unwrap_or(u16::MAX);
    let options_width = u16::try_from(options_label.width()).unwrap_or(u16::MAX);
    let panels_width = u16::try_from(panels_label.width()).unwrap_or(u16::MAX);
    let config_width = u16::try_from(config_label.width()).unwrap_or(u16::MAX);

    let mut title_spans: Vec<Span<'static>> = Vec::new();
    title_spans.push(Span::styled(title_text, Style::default().fg(th.overlay1)));
    title_spans.push(Span::raw("  "));

    let mut x_cursor = area
        .x
        .saturating_add(1)
        .saturating_add(title_width)
        .saturating_add(2);

    app.news_filter_arch_rect = Some((x_cursor, area.y, arch_width, 1));
    title_spans.push(arch_filter_span);
    x_cursor = x_cursor.saturating_add(arch_width).saturating_add(1);
    title_spans.push(Span::raw(" "));

    app.news_filter_advisory_rect = Some((x_cursor, area.y, advisory_width, 1));
    title_spans.push(advisory_filter_span);
    x_cursor = x_cursor.saturating_add(advisory_width).saturating_add(1);
    title_spans.push(Span::raw(" "));

    app.news_filter_installed_rect = Some((x_cursor, area.y, installed_width, 1));
    title_spans.push(installed_filter_span);
    x_cursor = x_cursor.saturating_add(installed_width).saturating_add(2);
    title_spans.push(Span::raw("  "));

    let options_x = area
        .x
        .saturating_add(1)
        .saturating_add(inner_width.saturating_sub(options_width));
    let panels_x = options_x.saturating_sub(1).saturating_sub(panels_width);
    let config_x = panels_x.saturating_sub(1).saturating_sub(config_width);
    let date_x = x_cursor;
    let gap_after_date = config_x.saturating_sub(date_x.saturating_add(date_width));

    title_spans.extend(date_button_spans);
    title_spans.push(Span::raw(" ".repeat(gap_after_date as usize)));
    title_spans.extend(config_button_spans);
    title_spans.push(Span::raw(" "));
    title_spans.extend(panels_button_spans);
    title_spans.push(Span::raw(" "));
    title_spans.extend(options_button_spans);

    app.sort_button_rect = Some((date_x, area.y, date_width, 1));
    app.config_button_rect = Some((config_x, area.y, config_width, 1));
    app.panels_button_rect = Some((panels_x, area.y, panels_width, 1));
    app.options_button_rect = Some((options_x, area.y, options_width, 1));

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
    app.results_rect = Some((area.x, area.y, area.width, area.height));
    f.render_stateful_widget(list, area, &mut app.news_list_state);
}

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
}
