use crate::i18n;
use crate::state::AppState;
use crate::state::types::{NewsFeedSource, NewsReadFilter};
use crate::theme::theme;
use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};
use unicode_width::UnicodeWidthStr;

/// What: Build list items for news feed results.
///
/// Inputs:
/// - `app`: Application state for i18n translations
/// - `news_loading`: Whether news is currently loading
/// - `news_results`: Reference to news results
/// - `news_read_ids`: Set of read news IDs
/// - `news_read_urls`: Set of read news URLs
///
/// Output:
/// - Tuple of `(Vec<ListItem>, bool)` where the boolean indicates if `app.news_list_state.select(None)` should be called.
///
/// Details:
/// - Shows "Loading..." if loading and no cached items exist
/// - Otherwise builds items from `news_results` with read/unread indicators
/// - Applies keyword highlighting to titles for Arch News items
pub fn build_news_list_items<'a>(
    app: &AppState,
    news_loading: bool,
    news_results: &'a [crate::state::types::NewsFeedItem],
    news_read_ids: &'a std::collections::HashSet<String>,
    news_read_urls: &'a std::collections::HashSet<String>,
) -> (Vec<ListItem<'a>>, bool) {
    let th = theme();
    let prefs = crate::theme::settings();

    if news_loading && news_results.is_empty() {
        // Only show "Loading..." if no cached items exist (first-time load)
        // Show additional info that first load may take longer due to rate limiting
        // and that it may affect package management operations
        (
            vec![
                ListItem::new(Line::from(ratatui::text::Span::styled(
                    i18n::t(app, "app.loading.news"),
                    Style::default().fg(th.overlay1),
                ))),
                ListItem::new(Line::from(ratatui::text::Span::styled(
                    i18n::t(app, "app.loading.news_first_load_hint"),
                    Style::default().fg(th.subtext0),
                ))),
                ListItem::new(Line::from(ratatui::text::Span::styled(
                    i18n::t(app, "app.loading.news_pkg_impact_hint"),
                    Style::default().fg(th.yellow),
                ))),
            ],
            true, // needs to select None
        )
    } else {
        // Show cached items immediately (even while loading fresh news)
        (
            news_results
                .iter()
                .map(|item| build_news_list_item(item, news_read_ids, news_read_urls, &th, &prefs))
                .collect(),
            false, // doesn't need to select None
        )
    }
}

/// What: Build a single list item for a news feed item.
///
/// Inputs:
/// - `item`: News feed item to render
/// - `app`: Application state for read status
/// - `th`: Theme for colors
/// - `prefs`: Theme preferences for symbols
///
/// Output:
/// - `ListItem` widget for the news feed item
///
/// Details:
/// - Determines read/unread status and applies appropriate styling
/// - Applies keyword highlighting to titles for Arch News items
fn build_news_list_item(
    item: &crate::state::types::NewsFeedItem,
    news_read_ids: &std::collections::HashSet<String>,
    news_read_urls: &std::collections::HashSet<String>,
    th: &crate::theme::Theme,
    prefs: &crate::theme::Settings,
) -> ListItem<'static> {
    let is_read = news_read_ids.contains(&item.id)
        || item
            .url
            .as_ref()
            .is_some_and(|u| news_read_urls.contains(u));
    let read_symbol = if is_read {
        prefs.news_read_symbol.clone()
    } else {
        prefs.news_unread_symbol.clone()
    };
    let read_style = if is_read {
        Style::default().fg(th.overlay1)
    } else {
        Style::default().fg(th.green)
    };
    let (source_label, source_color) = match item.source {
        NewsFeedSource::ArchNews => ("Arch", th.sapphire),
        NewsFeedSource::SecurityAdvisory => ("Advisory", th.yellow),
        NewsFeedSource::InstalledPackageUpdate => ("Update", th.green),
        NewsFeedSource::AurPackageUpdate => ("AUR Upd", th.mauve),
        NewsFeedSource::AurComment => ("AUR Cmt", th.yellow),
    };
    let sev = item
        .severity
        .as_ref()
        .map_or_else(String::new, |s| format!("{s:?}"));
    // Apply keyword highlighting to title for Arch News
    let highlight_style = ratatui::style::Style::default()
        .fg(th.yellow)
        .add_modifier(Modifier::BOLD);
    let title_spans = if matches!(item.source, NewsFeedSource::ArchNews) {
        render_aur_comment_keywords(&item.title, th, highlight_style)
    } else {
        vec![ratatui::text::Span::raw(item.title.clone())]
    };

    let mut spans = vec![
        ratatui::text::Span::styled(
            format!("{read_symbol} "),
            read_style.add_modifier(Modifier::BOLD),
        ),
        ratatui::text::Span::styled(
            format!("[{source_label}]"),
            Style::default().fg(source_color),
        ),
        ratatui::text::Span::raw(" "),
        ratatui::text::Span::raw(item.date.clone()),
        ratatui::text::Span::raw(" "),
    ];
    spans.extend(title_spans);
    if !sev.is_empty() {
        spans.push(ratatui::text::Span::raw(" "));
        spans.push(ratatui::text::Span::styled(
            format!("[{sev}]"),
            Style::default().fg(th.yellow),
        ));
    }
    if let Some(summary) = item.summary.as_ref() {
        spans.push(ratatui::text::Span::raw(" – "));
        spans.extend(render_summary_spans(summary, th, item.source));
    }
    let item_style = if is_read {
        Style::default().fg(th.subtext1)
    } else {
        Style::default().fg(th.text)
    };
    ListItem::new(Line::from(spans)).style(item_style)
}

/// What: Context struct containing data needed for building news title spans.
///
/// Inputs: Extracted data from `AppState` for title building.
///
/// Output: Grouped context data.
///
/// Details: Reduces data flow complexity by grouping related values together.
#[allow(clippy::struct_excessive_bools)]
struct NewsTitleContext {
    /// Title text showing news feed status and count
    title_text: String,
    /// Sort button label text
    sort_label: String,
    /// Date filter button label text
    date_label: String,
    /// Options menu button label text
    options_label: String,
    /// Panels menu button label text
    panels_label: String,
    /// Config menu button label text
    config_label: String,
    /// Arch news filter label text
    arch_filter_label: String,
    /// Security advisories filter label text
    advisory_filter_label: String,
    /// Package updates filter label text
    updates_filter_label: String,
    /// AUR updates filter label text
    aur_updates_filter_label: String,
    /// AUR comments filter label text
    aur_comments_filter_label: String,
    /// Read status filter label text
    read_filter_label: String,
    /// Whether sort menu is currently open
    sort_menu_open: bool,
    /// Whether options menu is currently open
    options_menu_open: bool,
    /// Whether panels menu is currently open
    panels_menu_open: bool,
    /// Whether config menu is currently open
    config_menu_open: bool,
    /// Whether to show Arch news filter
    news_filter_show_arch_news: bool,
    /// Whether to show advisories filter
    news_filter_show_advisories: bool,
    /// Whether to show package updates filter
    news_filter_show_pkg_updates: bool,
    /// Whether to show AUR updates filter
    news_filter_show_aur_updates: bool,
    /// Whether to show AUR comments filter
    news_filter_show_aur_comments: bool,
    /// Current read status filter setting
    news_filter_read_status: NewsReadFilter,
}

/// What: Extract context data needed for building news title spans.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - `NewsTitleContext` with all extracted data
///
/// Details:
/// - Extracts all data needed for title building to reduce data flow complexity
fn extract_news_title_context(app: &AppState) -> NewsTitleContext {
    let title_text = if app.news_loading {
        "News Feed (loading...)".to_string()
    } else {
        format!("News Feed ({})", app.news_results.len())
    };
    let age_label = app
        .news_max_age_days
        .map_or_else(|| "All".to_string(), |d| format!("{d} Days"));
    let sort_label = format!("{} v", i18n::t(app, "app.results.buttons.sort"));
    let date_label = format!("Date: {age_label}");
    let options_label = format!("{} v", i18n::t(app, "app.results.buttons.options"));
    let panels_label = format!("{} v", i18n::t(app, "app.results.buttons.panels"));
    let config_label = format!("{} v", i18n::t(app, "app.results.buttons.config_lists"));
    let arch_filter_label = format!("[{}]", i18n::t(app, "app.news.filters.arch"));
    let advisory_filter_label = if !app.news_filter_show_advisories {
        "[Advisories Off]".to_string()
    } else if app.news_filter_installed_only {
        "[Advisories Installed]".to_string()
    } else {
        "[Advisories All]".to_string()
    };
    let updates_filter_label = "[Updates]".to_string();
    let aur_updates_filter_label = "[AUR Upd]".to_string();
    let aur_comments_filter_label = "[AUR Comments]".to_string();
    let read_filter_label = match app.news_filter_read_status {
        NewsReadFilter::All => "[All]".to_string(),
        NewsReadFilter::Read => "[Read]".to_string(),
        NewsReadFilter::Unread => "[Unread]".to_string(),
    };

    NewsTitleContext {
        title_text,
        sort_label,
        date_label,
        options_label,
        panels_label,
        config_label,
        arch_filter_label,
        advisory_filter_label,
        updates_filter_label,
        aur_updates_filter_label,
        aur_comments_filter_label,
        read_filter_label,
        sort_menu_open: app.sort_menu_open,
        options_menu_open: app.options_menu_open,
        panels_menu_open: app.panels_menu_open,
        config_menu_open: app.config_menu_open,
        news_filter_show_arch_news: app.news_filter_show_arch_news,
        news_filter_show_advisories: app.news_filter_show_advisories,
        news_filter_show_pkg_updates: app.news_filter_show_pkg_updates,
        news_filter_show_aur_updates: app.news_filter_show_aur_updates,
        news_filter_show_aur_comments: app.news_filter_show_aur_comments,
        news_filter_read_status: app.news_filter_read_status,
    }
}

/// What: Calculate widths for all UI elements in the title bar.
///
/// Inputs:
/// - `ctx`: Title context with labels
///
/// Output:
/// - Struct containing all calculated widths
///
/// Details:
/// - Calculates Unicode-aware widths for proper layout positioning
struct TitleWidths {
    /// Width of the title text span
    title: u16,
    /// Width of the Arch news filter span
    arch: u16,
    /// Width of the advisory filter span
    advisory: u16,
    /// Width of the updates filter span
    updates: u16,
    /// Width of the AUR updates filter span
    aur_updates: u16,
    /// Width of the AUR comments filter span
    aur_comments: u16,
    /// Width of the read filter span
    read: u16,
    /// Width of the date button span
    date: u16,
    /// Width of the sort button span
    sort: u16,
    /// Width of the options button span
    options: u16,
    /// Width of the panels button span
    panels: u16,
    /// Width of the config button span
    config: u16,
}

/// Calculate display widths for all UI elements in the title bar.
///
/// What: Computes Unicode-aware widths for proper layout positioning.
///
/// Inputs:
/// - `ctx`: Title context containing all label texts
///
/// Output:
/// - `TitleWidths` struct with calculated widths for all elements
///
/// Details:
/// - Uses `unicode_width` to handle multi-byte characters correctly
/// - Returns `u16::MAX` for any calculation that fails (extremely unlikely)
fn calculate_title_widths(ctx: &NewsTitleContext) -> TitleWidths {
    TitleWidths {
        title: u16::try_from(ctx.title_text.width()).unwrap_or(u16::MAX),
        arch: u16::try_from(ctx.arch_filter_label.width()).unwrap_or(u16::MAX),
        advisory: u16::try_from(ctx.advisory_filter_label.width()).unwrap_or(u16::MAX),
        updates: u16::try_from(ctx.updates_filter_label.width()).unwrap_or(u16::MAX),
        aur_updates: u16::try_from(ctx.aur_updates_filter_label.width()).unwrap_or(u16::MAX),
        aur_comments: u16::try_from(ctx.aur_comments_filter_label.width()).unwrap_or(u16::MAX),
        read: u16::try_from(ctx.read_filter_label.width()).unwrap_or(u16::MAX),
        date: u16::try_from(ctx.date_label.width()).unwrap_or(u16::MAX),
        sort: u16::try_from(ctx.sort_label.width()).unwrap_or(u16::MAX),
        options: u16::try_from(ctx.options_label.width()).unwrap_or(u16::MAX),
        panels: u16::try_from(ctx.panels_label.width()).unwrap_or(u16::MAX),
        config: u16::try_from(ctx.config_label.width()).unwrap_or(u16::MAX),
    }
}

/// What: Build all styled spans for buttons and filters.
///
/// Inputs:
/// - `ctx`: Title context with labels and states
/// - `th`: Theme for styling
///
/// Output:
/// - Struct containing all styled spans
///
/// Details:
/// - Creates styled spans for all buttons and filters in the title bar
struct TitleSpans {
    /// Title span showing news feed status
    title: Span<'static>,
    /// Sort button spans
    sort_button: Vec<Span<'static>>,
    /// Arch news filter span
    arch_filter: Span<'static>,
    /// Advisory filter span
    advisory_filter: Span<'static>,
    /// Package updates filter span
    updates_filter: Span<'static>,
    /// AUR updates filter span
    aur_updates_filter: Span<'static>,
    /// AUR comments filter span
    aur_comments_filter: Span<'static>,
    /// Read filter span
    read_filter: Span<'static>,
    /// Date button spans
    date_button: Vec<Span<'static>>,
    /// Config button spans
    config_button: Vec<Span<'static>>,
    /// Panels button spans
    panels_button: Vec<Span<'static>>,
    /// Options button spans
    options_button: Vec<Span<'static>>,
}

/// Build styled spans for all buttons and filters in the title bar.
///
/// What: Creates ratatui spans with proper styling and theming.
///
/// Inputs:
/// - `ctx`: Title context containing labels and state information
///
/// Output:
/// - `TitleSpans` struct containing all styled spans ready for rendering
///
/// Details:
/// - Applies theme colors and styles based on menu states
/// - Handles button underlining and filter highlighting
/// - Uses consistent styling patterns across all UI elements
fn build_title_spans(ctx: &NewsTitleContext) -> TitleSpans {
    let th = theme();

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

    TitleSpans {
        title: Span::styled(ctx.title_text.clone(), Style::default().fg(th.overlay1)),
        sort_button: render_button(&ctx.sort_label, ctx.sort_menu_open),
        arch_filter: render_filter(&ctx.arch_filter_label, ctx.news_filter_show_arch_news),
        advisory_filter: render_filter(&ctx.advisory_filter_label, ctx.news_filter_show_advisories),
        updates_filter: render_filter(&ctx.updates_filter_label, ctx.news_filter_show_pkg_updates),
        aur_updates_filter: render_filter(
            &ctx.aur_updates_filter_label,
            ctx.news_filter_show_aur_updates,
        ),
        aur_comments_filter: render_filter(
            &ctx.aur_comments_filter_label,
            ctx.news_filter_show_aur_comments,
        ),
        read_filter: render_filter(
            &ctx.read_filter_label,
            !matches!(ctx.news_filter_read_status, NewsReadFilter::All),
        ),
        date_button: render_button(&ctx.date_label, false),
        config_button: render_button(&ctx.config_label, ctx.config_menu_open),
        panels_button: render_button(&ctx.panels_label, ctx.panels_menu_open),
        options_button: render_button(&ctx.options_label, ctx.options_menu_open),
    }
}

/// What: Build title spans for news feed title bar and record hit-test rectangles.
///
/// Inputs:
/// - `app`: Application state for filter states and labels (mutated to record rects)
/// - `area`: Rendering area for width calculations
///
/// Output:
/// - Vector of `Span` widgets for the title bar
///
/// Details:
/// - Builds title with loading indicator, filters, buttons, and right-aligned controls
/// - Records hit-test rectangles for buttons and filters in `app`
/// - Calculates layout positions for proper spacing
pub fn build_news_title_spans_and_record_rects(
    app: &mut AppState,
    area: Rect,
) -> Vec<Span<'static>> {
    // Extract context data to reduce data flow complexity
    let ctx = extract_news_title_context(app);
    let widths = calculate_title_widths(&ctx);
    let spans = build_title_spans(&ctx);

    let inner_width = area.width.saturating_sub(2);

    // Build the left side of the title bar
    let mut title_spans: Vec<Span<'static>> = Vec::new();
    title_spans.push(spans.title);
    title_spans.push(Span::raw("  "));

    // Position cursor after title
    let mut x_cursor = area
        .x
        .saturating_add(1)
        .saturating_add(widths.title)
        .saturating_add(2);

    // Add sort button
    title_spans.extend(spans.sort_button);
    x_cursor = x_cursor.saturating_add(widths.sort).saturating_add(2);
    title_spans.push(Span::raw(" "));

    // Add filters with spacing
    title_spans.push(spans.arch_filter);
    x_cursor = x_cursor.saturating_add(widths.arch).saturating_add(1);
    title_spans.push(Span::raw(" "));

    title_spans.push(spans.advisory_filter);
    x_cursor = x_cursor.saturating_add(widths.advisory).saturating_add(1);
    title_spans.push(Span::raw(" "));

    title_spans.push(spans.updates_filter);
    x_cursor = x_cursor.saturating_add(widths.updates).saturating_add(1);
    title_spans.push(Span::raw(" "));

    title_spans.push(spans.aur_updates_filter);
    x_cursor = x_cursor
        .saturating_add(widths.aur_updates)
        .saturating_add(1);
    title_spans.push(Span::raw(" "));

    title_spans.push(spans.aur_comments_filter);
    x_cursor = x_cursor.saturating_add(widths.aur_comments);
    title_spans.push(Span::raw("  "));
    x_cursor = x_cursor.saturating_add(2);
    title_spans.push(spans.read_filter);
    x_cursor = x_cursor.saturating_add(widths.read);
    title_spans.push(Span::raw("  "));
    x_cursor = x_cursor.saturating_add(2);

    // Calculate right-aligned button positions
    let options_x = area
        .x
        .saturating_add(1)
        .saturating_add(inner_width.saturating_sub(widths.options));
    let panels_x = options_x.saturating_sub(1).saturating_sub(widths.panels);
    let config_x = panels_x.saturating_sub(1).saturating_sub(widths.config);
    let date_x = x_cursor;
    let gap_after_date = config_x.saturating_sub(date_x.saturating_add(widths.date));

    // Add right-aligned buttons with gap
    title_spans.extend(spans.date_button);
    title_spans.push(Span::raw(" ".repeat(gap_after_date as usize)));
    title_spans.extend(spans.config_button);
    title_spans.push(Span::raw(" "));
    title_spans.extend(spans.panels_button);
    title_spans.push(Span::raw(" "));
    title_spans.extend(spans.options_button);

    // Record hit-test rectangles for buttons and filters
    let mut x_cursor_rect = area
        .x
        .saturating_add(1)
        .saturating_add(widths.title)
        .saturating_add(2);

    app.sort_button_rect = Some((x_cursor_rect, area.y, widths.sort, 1));
    x_cursor_rect = x_cursor_rect.saturating_add(widths.sort).saturating_add(2);

    app.news_filter_arch_rect = Some((x_cursor_rect, area.y, widths.arch, 1));
    x_cursor_rect = x_cursor_rect.saturating_add(widths.arch).saturating_add(1);

    app.news_filter_advisory_rect = Some((x_cursor_rect, area.y, widths.advisory, 1));
    x_cursor_rect = x_cursor_rect
        .saturating_add(widths.advisory)
        .saturating_add(1);

    app.news_filter_updates_rect = Some((x_cursor_rect, area.y, widths.updates, 1));
    x_cursor_rect = x_cursor_rect
        .saturating_add(widths.updates)
        .saturating_add(1);

    app.news_filter_aur_updates_rect = Some((x_cursor_rect, area.y, widths.aur_updates, 1));
    x_cursor_rect = x_cursor_rect
        .saturating_add(widths.aur_updates)
        .saturating_add(1);

    app.news_filter_aur_comments_rect = Some((x_cursor_rect, area.y, widths.aur_comments, 1));
    x_cursor_rect = x_cursor_rect
        .saturating_add(widths.aur_comments)
        .saturating_add(2);
    app.news_filter_read_rect = Some((x_cursor_rect, area.y, widths.read, 1));
    let _ = x_cursor_rect.saturating_add(widths.read).saturating_add(2);

    app.news_age_button_rect = Some((date_x, area.y, widths.date, 1));
    app.config_button_rect = Some((config_x, area.y, widths.config, 1));
    app.panels_button_rect = Some((panels_x, area.y, widths.panels, 1));
    app.options_button_rect = Some((options_x, area.y, widths.options, 1));

    title_spans
}

/// What: Render summary spans with source-aware highlighting (updates vs AUR comments vs Arch News).
///
/// Inputs:
/// - `summary`: Summary text to render
/// - `th`: Theme for colors
/// - `source`: News feed source type
///
/// Output:
/// - Vector of styled spans for the summary
///
/// Details:
/// - Applies different highlighting based on source type
/// - Updates get full highlight, AUR comments and Arch News get keyword highlighting
pub fn render_summary_spans(
    summary: &str,
    th: &crate::theme::Theme,
    source: NewsFeedSource,
) -> Vec<ratatui::text::Span<'static>> {
    let highlight_style = ratatui::style::Style::default()
        .fg(th.yellow)
        .add_modifier(Modifier::BOLD);
    let normal = ratatui::style::Style::default().fg(th.subtext1);

    if matches!(
        source,
        NewsFeedSource::InstalledPackageUpdate | NewsFeedSource::AurPackageUpdate
    ) {
        return vec![ratatui::text::Span::styled(
            summary.to_string(),
            highlight_style,
        )];
    }

    if matches!(source, NewsFeedSource::AurComment) {
        return render_aur_comment_keywords(summary, th, highlight_style);
    }

    // Apply keyword highlighting to Arch News (same as AUR comments)
    if matches!(source, NewsFeedSource::ArchNews) {
        return render_aur_comment_keywords(summary, th, highlight_style);
    }

    vec![ratatui::text::Span::styled(
        summary.to_string(),
        normal.add_modifier(Modifier::BOLD),
    )]
}

/// What: Highlight AUR comment summaries and Arch News with red/green keywords and normal text.
///
/// Inputs:
/// - `summary`: Text to highlight
/// - `th`: Theme for colors
/// - `base`: Base style for normal text
///
/// Output:
/// - Vector of styled spans with keyword highlighting
///
/// Details:
/// - Highlights negative words (crash, bug, error, etc.) in red
/// - Highlights positive words (fix, patch, resolve, etc.) in green
/// - Other text uses the base style
pub fn render_aur_comment_keywords(
    summary: &str,
    th: &crate::theme::Theme,
    base: ratatui::style::Style,
) -> Vec<ratatui::text::Span<'static>> {
    let normal = base;
    let neg = ratatui::style::Style::default()
        .fg(th.red)
        .add_modifier(Modifier::BOLD);
    let pos = ratatui::style::Style::default()
        .fg(th.green)
        .add_modifier(Modifier::BOLD);

    let negative_words = [
        "crash",
        "crashed",
        "crashes",
        "critical",
        "bug",
        "bugs",
        "fail",
        "fails",
        "failed",
        "failure",
        "failures",
        "issue",
        "issues",
        "trouble",
        "troubles",
        "panic",
        "segfault",
        "broken",
        "regression",
        "hang",
        "freeze",
        "unstable",
        "error",
        "errors",
        "require manual intervention",
        "requires manual intervention",
        "corrupting",
    ];
    let positive_words = [
        "fix",
        "fixed",
        "fixes",
        "patch",
        "patched",
        "solve",
        "solved",
        "solves",
        "solution",
        "resolve",
        "resolved",
        "resolves",
        "workaround",
    ];
    let neg_set: std::collections::HashSet<&str> = negative_words.into_iter().collect();
    let pos_set: std::collections::HashSet<&str> = positive_words.into_iter().collect();

    let mut spans = Vec::new();
    for token in summary.split_inclusive(' ') {
        let cleaned = token
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .to_ascii_lowercase();
        let style = if pos_set.contains(cleaned.as_str()) {
            pos
        } else if neg_set.contains(cleaned.as_str()) {
            neg
        } else {
            normal.add_modifier(Modifier::BOLD)
        };
        spans.push(ratatui::text::Span::styled(token.to_string(), style));
    }
    spans
}
