use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::Span,
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

use super::{FilterStates, MenuStates, OptionalRepos, RenderContext};

/// What: Pre-computed i18n strings for title rendering.
///
/// Inputs: Individual i18n strings from `AppState`.
///
/// Output: Struct containing all i18n strings needed for title rendering.
///
/// Details: Reduces data flow complexity by pre-computing all i18n strings upfront.
struct TitleI18nStrings {
    results_title: String,
    sort_button: String,
    options_button: String,
    panels_button: String,
    config_button: String,
    menu_button: String,
    filter_aur: String,
    filter_core: String,
    filter_extra: String,
    filter_multilib: String,
    filter_eos: String,
    filter_cachyos: String,
    filter_artix: String,
    filter_artix_omniverse: String,
    filter_artix_universe: String,
    filter_artix_lib32: String,
    filter_artix_galaxy: String,
    filter_artix_world: String,
    filter_artix_system: String,
    filter_manjaro: String,
}

/// What: Build `TitleI18nStrings` from `AppState`.
///
/// Inputs:
/// - `app`: Application state for i18n
///
/// Output: `TitleI18nStrings` containing all pre-computed i18n strings.
///
/// Details: Extracts all i18n strings needed for title rendering in one place.
fn build_title_i18n_strings(app: &AppState) -> TitleI18nStrings {
    TitleI18nStrings {
        results_title: i18n::t(app, "app.results.title"),
        sort_button: i18n::t(app, "app.results.buttons.sort"),
        options_button: i18n::t(app, "app.results.buttons.options"),
        panels_button: i18n::t(app, "app.results.buttons.panels"),
        config_button: i18n::t(app, "app.results.buttons.config_lists"),
        menu_button: i18n::t(app, "app.results.buttons.menu"),
        filter_aur: i18n::t(app, "app.results.filters.aur"),
        filter_core: i18n::t(app, "app.results.filters.core"),
        filter_extra: i18n::t(app, "app.results.filters.extra"),
        filter_multilib: i18n::t(app, "app.results.filters.multilib"),
        filter_eos: i18n::t(app, "app.results.filters.eos"),
        filter_cachyos: i18n::t(app, "app.results.filters.cachyos"),
        filter_artix: i18n::t(app, "app.results.filters.artix"),
        filter_artix_omniverse: i18n::t(app, "app.results.filters.artix_omniverse"),
        filter_artix_universe: i18n::t(app, "app.results.filters.artix_universe"),
        filter_artix_lib32: i18n::t(app, "app.results.filters.artix_lib32"),
        filter_artix_galaxy: i18n::t(app, "app.results.filters.artix_galaxy"),
        filter_artix_world: i18n::t(app, "app.results.filters.artix_world"),
        filter_artix_system: i18n::t(app, "app.results.filters.artix_system"),
        filter_manjaro: i18n::t(app, "app.results.filters.manjaro"),
    }
}

/// What: Calculate consumed horizontal space for optional repos.
///
/// Inputs:
/// - `repos`: Optional repository flags
/// - `labels`: Pre-formatted label strings for each repo
///
/// Output: Total consumed width in characters.
///
/// Details: Sums up the width of all available optional repos plus spacing.
/// Uses Unicode display width, not byte length, to handle wide characters.
fn calculate_optional_repos_width(repos: &OptionalRepos, labels: &OptionalReposLabels) -> u16 {
    let mut width = 0u16;
    if repos.has_eos {
        width = width.saturating_add(1 + u16::try_from(labels.eos.width()).unwrap_or(u16::MAX));
    }
    if repos.has_cachyos {
        width = width.saturating_add(1 + u16::try_from(labels.cachyos.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix {
        width = width.saturating_add(1 + u16::try_from(labels.artix.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_omniverse {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_omniverse.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_universe {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_universe.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_lib32 {
        width =
            width.saturating_add(1 + u16::try_from(labels.artix_lib32.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_galaxy {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_galaxy.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_world {
        width =
            width.saturating_add(1 + u16::try_from(labels.artix_world.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_system {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_system.width()).unwrap_or(u16::MAX));
    }
    if repos.has_manjaro {
        width = width.saturating_add(1 + u16::try_from(labels.manjaro.width()).unwrap_or(u16::MAX));
    }
    width
}

/// What: Represents pre-formatted label strings for optional repos.
///
/// Inputs: Individual label strings.
///
/// Output: Struct containing all label strings.
///
/// Details: Used to pass multiple label strings as a single parameter.
struct OptionalReposLabels {
    eos: String,
    cachyos: String,
    artix: String,
    artix_omniverse: String,
    artix_universe: String,
    artix_lib32: String,
    artix_galaxy: String,
    artix_world: String,
    artix_system: String,
    manjaro: String,
}

/// What: Calculate base consumed space (title, sort button, core filters).
///
/// Inputs:
/// - `results_title_text`: Title text with count
/// - `sort_button_label`: Sort button label
/// - `core_labels`: Labels for core filters (AUR, core, extra, multilib)
///
/// Output: Base consumed width in display columns.
///
/// Details: Calculates space for fixed elements that are always present.
/// Uses Unicode display width, not byte length, to handle wide characters.
fn calculate_base_consumed_space(
    results_title_text: &str,
    sort_button_label: &str,
    core_labels: &CoreFilterLabels,
) -> u16 {
    u16::try_from(
        results_title_text.width()
            + 2 // spaces before Sort
            + sort_button_label.width()
            + 2 // spaces after Sort
            + core_labels.aur.width()
            + 1 // space
            + core_labels.core.width()
            + 1 // space
            + core_labels.extra.width()
            + 1 // space
            + core_labels.multilib.width(),
    )
    .unwrap_or(u16::MAX)
}

/// What: Represents labels for core filters.
///
/// Inputs: Individual label strings.
///
/// Output: Struct containing core filter labels.
///
/// Details: Used to pass core filter labels as a single parameter.
struct CoreFilterLabels {
    aur: String,
    core: String,
    extra: String,
    multilib: String,
}

/// What: Get button style based on menu open state.
///
/// Inputs:
/// - `is_open`: Whether the menu is open
///
/// Output: Styled button appearance.
///
/// Details: Returns active style when open, inactive style when closed.
fn get_button_style(is_open: bool) -> Style {
    let th = theme();
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
}

/// What: Render a button with underlined first character.
///
/// Inputs:
/// - `label`: Button label text
/// - `style`: Style to apply
///
/// Output: Vector of spans for the button.
///
/// Details: First character is underlined, rest uses normal style.
fn render_button_with_underline(label: &str, style: Style) -> Vec<Span<'static>> {
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
}

/// What: Render optional filter if available.
///
/// Inputs:
/// - `has_repo`: Whether repo is available
/// - `label`: Pre-computed filter label
/// - `is_active`: Whether filter is active
/// - `filt`: Filter rendering closure
///
/// Output: Option containing filter span, or None if not available.
///
/// Details: Returns Some(span) if repo is available, None otherwise.
fn render_optional_filter(
    has_repo: bool,
    label: &str,
    is_active: bool,
    filt: &dyn Fn(&str, bool) -> Span<'static>,
) -> Option<Span<'static>> {
    if has_repo {
        Some(filt(label, is_active))
    } else {
        None
    }
}

/// What: Render title prefix (title text with count).
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `results_len`: Number of results
///
/// Output: Vector of spans for the title prefix.
///
/// Details: Renders the "Results (N)" text with styling.
fn render_title_prefix(i18n: &TitleI18nStrings, results_len: usize) -> Vec<Span<'static>> {
    let th = theme();
    let results_title_text = format!("{} ({})", i18n.results_title, results_len);
    vec![Span::styled(
        results_title_text,
        Style::default().fg(th.overlay1),
    )]
}

/// What: Render sort button.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `is_open`: Whether sort menu is open
///
/// Output: Vector of spans for the sort button.
///
/// Details: Renders the sort button with appropriate styling based on menu state.
fn render_sort_button(i18n: &TitleI18nStrings, is_open: bool) -> Vec<Span<'static>> {
    let sort_button_label = format!("{} v", i18n.sort_button);
    let btn_style = get_button_style(is_open);
    vec![Span::styled(sort_button_label, btn_style)]
}

/// What: Create filter rendering closure.
///
/// Inputs: None (uses theme).
///
/// Output: Closure that renders a filter label with styling.
///
/// Details: Returns a closure that applies theme styling based on active state.
fn create_filter_renderer() -> impl Fn(&str, bool) -> Span<'static> {
    let th = theme();
    move |label: &str, on: bool| -> Span<'static> {
        let (fg, bg) = if on {
            (th.crust, th.green)
        } else {
            (th.mauve, th.surface2)
        };
        Span::styled(
            format!("[{label}]"),
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
        )
    }
}

/// What: Render core filter buttons (AUR, core, extra, multilib).
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `filter_states`: Filter toggle states
///
/// Output: Vector of spans for core filters.
///
/// Details: Renders the four core filter buttons with spacing.
fn render_core_filters(
    i18n: &TitleI18nStrings,
    filter_states: &FilterStates,
) -> Vec<Span<'static>> {
    let filt = create_filter_renderer();
    vec![
        filt(&i18n.filter_aur, filter_states.show_aur),
        Span::raw(" "),
        filt(&i18n.filter_core, filter_states.show_core),
        Span::raw(" "),
        filt(&i18n.filter_extra, filter_states.show_extra),
        Span::raw(" "),
        filt(&i18n.filter_multilib, filter_states.show_multilib),
    ]
}

/// What: Render optional `EOS` and `CachyOS` filters.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `optional_repos`: Optional repository availability flags
/// - `filter_states`: Filter toggle states
///
/// Output: Vector of spans for optional filters.
///
/// Details: Renders `EOS` and `CachyOS` filters if available.
fn render_optional_eos_cachyos_filters(
    i18n: &TitleI18nStrings,
    optional_repos: &OptionalRepos,
    filter_states: &FilterStates,
) -> Vec<Span<'static>> {
    let filt = create_filter_renderer();
    let mut spans = Vec::new();
    if let Some(span) = render_optional_filter(
        optional_repos.has_eos,
        &i18n.filter_eos,
        filter_states.show_eos,
        &filt,
    ) {
        spans.push(Span::raw(" "));
        spans.push(span);
    }
    if let Some(span) = render_optional_filter(
        optional_repos.has_cachyos,
        &i18n.filter_cachyos,
        filter_states.show_cachyos,
        &filt,
    ) {
        spans.push(Span::raw(" "));
        spans.push(span);
    }
    spans
}

/// What: Render Artix filter with optional dropdown indicator.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `optional_repos`: Optional repository availability flags
/// - `filter_states`: Filter toggle states
/// - `show_artix_specific_repos`: Whether Artix-specific repos are shown
///
/// Output: Vector of spans for Artix filter.
///
/// Details: Renders Artix filter with dropdown indicator if specific repos are hidden.
fn render_artix_filter(
    i18n: &TitleI18nStrings,
    optional_repos: &OptionalRepos,
    filter_states: &FilterStates,
    show_artix_specific_repos: bool,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if optional_repos.has_artix {
        spans.push(Span::raw(" "));
        let artix_text = if show_artix_specific_repos {
            format!("[{}]", i18n.filter_artix)
        } else {
            format!("[{}] v", i18n.filter_artix)
        };
        let th = theme();
        let (fg, bg) = if filter_states.show_artix {
            (th.crust, th.green)
        } else {
            (th.mauve, th.surface2)
        };
        spans.push(Span::styled(
            artix_text,
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
        ));
    }
    spans
}

/// What: Render Artix-specific repository filters.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `optional_repos`: Optional repository availability flags
/// - `filter_states`: Filter toggle states
///
/// Output: Vector of spans for Artix-specific filters.
///
/// Details: Renders all Artix-specific repo filters if available.
fn render_artix_specific_filters(
    i18n: &TitleI18nStrings,
    optional_repos: &OptionalRepos,
    filter_states: &FilterStates,
) -> Vec<Span<'static>> {
    let filt = create_filter_renderer();
    let mut spans = Vec::new();
    let artix_filters = [
        (
            optional_repos.has_artix_omniverse,
            &i18n.filter_artix_omniverse,
            filter_states.show_artix_omniverse,
        ),
        (
            optional_repos.has_artix_universe,
            &i18n.filter_artix_universe,
            filter_states.show_artix_universe,
        ),
        (
            optional_repos.has_artix_lib32,
            &i18n.filter_artix_lib32,
            filter_states.show_artix_lib32,
        ),
        (
            optional_repos.has_artix_galaxy,
            &i18n.filter_artix_galaxy,
            filter_states.show_artix_galaxy,
        ),
        (
            optional_repos.has_artix_world,
            &i18n.filter_artix_world,
            filter_states.show_artix_world,
        ),
        (
            optional_repos.has_artix_system,
            &i18n.filter_artix_system,
            filter_states.show_artix_system,
        ),
    ];
    for (has_repo, label, is_active) in artix_filters {
        if let Some(span) = render_optional_filter(has_repo, label, is_active, &filt) {
            spans.push(Span::raw(" "));
            spans.push(span);
        }
    }
    spans
}

/// What: Render Manjaro filter.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `optional_repos`: Optional repository availability flags
/// - `filter_states`: Filter toggle states
///
/// Output: Vector of spans for Manjaro filter.
///
/// Details: Renders Manjaro filter if available.
fn render_manjaro_filter(
    i18n: &TitleI18nStrings,
    optional_repos: &OptionalRepos,
    filter_states: &FilterStates,
) -> Vec<Span<'static>> {
    let filt = create_filter_renderer();
    let mut spans = Vec::new();
    if let Some(span) = render_optional_filter(
        optional_repos.has_manjaro,
        &i18n.filter_manjaro,
        filter_states.show_manjaro,
        &filt,
    ) {
        spans.push(Span::raw(" "));
        spans.push(span);
    }
    spans
}

/// What: Render right-aligned buttons (Config/Lists, Panels, Options) or collapsed Menu button.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `menu_states`: Menu open/closed states
/// - `pad`: Padding space before buttons (for all three buttons case)
/// - `use_collapsed_menu`: Whether to render collapsed menu button instead of individual buttons
/// - `menu_button_label`: Label for the collapsed menu button
/// - `menu_pad`: Padding space for collapsed menu button (calculated separately)
///
/// Output: Vector of spans for right-aligned buttons.
///
/// Details: Renders either all three buttons or a single collapsed Menu button based on available space.
fn render_right_aligned_buttons(
    i18n: &TitleI18nStrings,
    menu_states: &MenuStates,
    pad: u16,
    use_collapsed_menu: bool,
    menu_button_label: &str,
    menu_pad: u16,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if use_collapsed_menu {
        // Render collapsed menu button if we have space for it
        if menu_pad >= 1 {
            spans.push(Span::raw(" ".repeat(menu_pad as usize)));
            let menu_btn_style = get_button_style(menu_states.collapsed_menu_open);
            spans.extend(render_button_with_underline(
                menu_button_label,
                menu_btn_style,
            ));
        }
    } else if pad >= 1 {
        // Render all three buttons if we have space
        spans.push(Span::raw(" ".repeat(pad as usize)));
        let config_button_label = format!("{} v", i18n.config_button);
        let cfg_btn_style = get_button_style(menu_states.config_menu_open);
        spans.extend(render_button_with_underline(
            &config_button_label,
            cfg_btn_style,
        ));
        spans.push(Span::raw(" "));
        let panels_button_label = format!("{} v", i18n.panels_button);
        let pan_btn_style = get_button_style(menu_states.panels_menu_open);
        spans.extend(render_button_with_underline(
            &panels_button_label,
            pan_btn_style,
        ));
        spans.push(Span::raw(" "));
        let options_button_label = format!("{} v", i18n.options_button);
        let opt_btn_style = get_button_style(menu_states.options_menu_open);
        spans.extend(render_button_with_underline(
            &options_button_label,
            opt_btn_style,
        ));
    }
    spans
}

/// What: Build title spans with Sort button, filter toggles, and right-aligned buttons.
///
/// This version takes a context struct to reduce data flow complexity.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `ctx`: Render context containing all extracted values
/// - `area`: Target rectangle for the results block
///
/// Output:
/// - Vector of `Span` widgets forming the title line
///
/// Details:
/// - Applies theme styling for active buttons, ensures right-side buttons align within the title,
///   and toggles optional repo chips based on availability flags.
/// - Uses pre-computed i18n strings and focused rendering functions to reduce complexity.
pub fn build_title_spans_from_context(
    app: &AppState,
    ctx: &RenderContext,
    area: Rect,
) -> Vec<Span<'static>> {
    let inner_width = area.width.saturating_sub(2); // exclude borders
    build_title_spans_from_values(
        app,
        ctx.results_len,
        inner_width,
        &ctx.optional_repos,
        &ctx.menu_states,
        &ctx.filter_states,
    )
}

/// What: Build title spans with Sort button, filter toggles, and right-aligned buttons.
///
/// This version takes structs instead of individual values to reduce data flow complexity.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `results_len`: Number of results
/// - `inner_width`: Inner width of the area (excluding borders)
/// - `optional_repos`: Optional repository availability flags
/// - `menu_states`: Menu open/closed states
/// - `filter_states`: Filter toggle states
///
/// Output:
/// - Vector of `Span` widgets forming the title line
///
/// Details:
/// - Applies theme styling for active buttons, ensures right-side buttons align within the title,
///   and toggles optional repo chips based on availability flags.
/// - Uses pre-computed i18n strings and focused rendering functions to reduce complexity.
/// - Reuses layout calculation logic from `calculate_title_layout_info`.
fn build_title_spans_from_values(
    app: &AppState,
    results_len: usize,
    inner_width: u16,
    optional_repos: &OptionalRepos,
    menu_states: &MenuStates,
    filter_states: &FilterStates,
) -> Vec<Span<'static>> {
    // Pre-compute all i18n strings to reduce data flow complexity
    let i18n = build_title_i18n_strings(app);

    // Reuse layout calculation logic
    let layout_info = calculate_title_layout_info(&i18n, results_len, inner_width, optional_repos);

    // Build title spans using focused rendering functions
    let mut title_spans = render_title_prefix(&i18n, results_len);
    title_spans.push(Span::raw("  "));
    title_spans.extend(render_sort_button(&i18n, menu_states.sort_menu_open));
    title_spans.push(Span::raw("  "));
    title_spans.extend(render_core_filters(&i18n, filter_states));
    title_spans.extend(render_optional_eos_cachyos_filters(
        &i18n,
        optional_repos,
        filter_states,
    ));
    title_spans.extend(render_artix_filter(
        &i18n,
        optional_repos,
        filter_states,
        layout_info.show_artix_specific_repos,
    ));
    if layout_info.show_artix_specific_repos {
        title_spans.extend(render_artix_specific_filters(
            &i18n,
            optional_repos,
            filter_states,
        ));
    }
    title_spans.extend(render_manjaro_filter(&i18n, optional_repos, filter_states));
    title_spans.extend(render_right_aligned_buttons(
        &i18n,
        menu_states,
        layout_info.pad,
        layout_info.use_collapsed_menu,
        &layout_info.menu_button_label,
        layout_info.menu_pad,
    ));

    title_spans
}

/// What: Shared layout calculation information for title bar.
///
/// Inputs: Calculated values from title text, button labels, and area dimensions.
///
/// Output: Struct containing all layout calculation results.
///
/// Details: Used to share layout calculations between rendering and rect recording functions.
struct TitleLayoutInfo {
    results_title_text: String,
    sort_button_label: String,
    options_button_label: String,
    panels_button_label: String,
    config_button_label: String,
    menu_button_label: String,
    core_labels: CoreFilterLabels,
    optional_labels: OptionalReposLabels,
    inner_width: u16,
    show_artix_specific_repos: bool,
    pad: u16,
    use_collapsed_menu: bool,
    menu_pad: u16,
}

/// What: Layout state tracker for recording rectangles.
///
/// Inputs: Initial x position and y position.
///
/// Output: Struct that tracks current x cursor position and y position.
///
/// Details: Encapsulates layout state to avoid manual `x_cursor` tracking.
struct LayoutState {
    x: u16,
    y: u16,
}

impl LayoutState {
    /// What: Create a new layout state.
    ///
    /// Inputs:
    /// - `x`: Initial x position
    /// - `y`: Y position (constant)
    ///
    /// Output: New `LayoutState` instance.
    ///
    /// Details: Initializes layout state with starting position.
    const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// What: Advance x cursor by label width plus spacing.
    ///
    /// Inputs:
    /// - `label_width`: Width of the label in characters
    /// - `spacing`: Number of spaces after the label (default 1)
    ///
    /// Output: Updated x position.
    ///
    /// Details: Moves x cursor forward by label width plus spacing.
    #[allow(clippy::missing_const_for_fn)]
    fn advance(&mut self, label_width: u16, spacing: u16) -> u16 {
        self.x = self.x.saturating_add(label_width).saturating_add(spacing);
        self.x
    }

    /// What: Record a rectangle at current position.
    ///
    /// Inputs:
    /// - `label`: Label text to measure
    ///
    /// Output: Rectangle tuple (x, y, width, height).
    ///
    /// Details: Creates rectangle at current x position with label width.
    /// Uses Unicode display width, not byte length, to handle wide characters.
    fn record_rect(&self, label: &str) -> (u16, u16, u16, u16) {
        (
            self.x,
            self.y,
            u16::try_from(label.width()).unwrap_or(u16::MAX),
            1,
        )
    }
}

/// What: Calculate shared layout information for title bar.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `results_len`: Number of results
/// - `inner_width`: Inner width of the area (excluding borders)
/// - `optional_repos`: Optional repository availability flags
///
/// Output: `TitleLayoutInfo` containing all calculated layout values.
///
/// Details: Performs all layout calculations shared between rendering and rect recording.
fn calculate_title_layout_info(
    i18n: &TitleI18nStrings,
    results_len: usize,
    inner_width: u16,
    optional_repos: &OptionalRepos,
) -> TitleLayoutInfo {
    let results_title_text = format!("{} ({})", i18n.results_title, results_len);
    let sort_button_label = format!("{} v", i18n.sort_button);
    let options_button_label = format!("{} v", i18n.options_button);
    let panels_button_label = format!("{} v", i18n.panels_button);
    let config_button_label = format!("{} v", i18n.config_button);
    let menu_button_label = format!("{} v", i18n.menu_button);

    let core_labels = CoreFilterLabels {
        aur: format!("[{}]", i18n.filter_aur),
        core: format!("[{}]", i18n.filter_core),
        extra: format!("[{}]", i18n.filter_extra),
        multilib: format!("[{}]", i18n.filter_multilib),
    };
    let optional_labels = OptionalReposLabels {
        eos: format!("[{}]", i18n.filter_eos),
        cachyos: format!("[{}]", i18n.filter_cachyos),
        artix: format!("[{}]", i18n.filter_artix),
        artix_omniverse: format!("[{}]", i18n.filter_artix_omniverse),
        artix_universe: format!("[{}]", i18n.filter_artix_universe),
        artix_lib32: format!("[{}]", i18n.filter_artix_lib32),
        artix_galaxy: format!("[{}]", i18n.filter_artix_galaxy),
        artix_world: format!("[{}]", i18n.filter_artix_world),
        artix_system: format!("[{}]", i18n.filter_artix_system),
        manjaro: format!("[{}]", i18n.filter_manjaro),
    };

    // Calculate consumed space with all filters first
    let base_consumed =
        calculate_base_consumed_space(&results_title_text, &sort_button_label, &core_labels);
    let optional_consumed = calculate_optional_repos_width(optional_repos, &optional_labels);
    let consumed_left = base_consumed.saturating_add(optional_consumed);

    // Use Unicode display width, not byte length, to handle wide characters
    let options_w = u16::try_from(options_button_label.width()).unwrap_or(u16::MAX);
    let panels_w = u16::try_from(panels_button_label.width()).unwrap_or(u16::MAX);
    let config_w = u16::try_from(config_button_label.width()).unwrap_or(u16::MAX);
    let menu_w = u16::try_from(menu_button_label.width()).unwrap_or(u16::MAX);
    let right_w = config_w
        .saturating_add(1)
        .saturating_add(panels_w)
        .saturating_add(1)
        .saturating_add(options_w);
    let mut pad = inner_width.saturating_sub(consumed_left.saturating_add(right_w));

    // If not enough space, hide Artix-specific repo filters (keep generic Artix filter)
    let mut show_artix_specific_repos = true;
    let mut final_consumed_left = consumed_left;
    if pad < 1 {
        // Recalculate without Artix-specific repo filters
        let repos_without_specific = OptionalRepos {
            has_eos: optional_repos.has_eos,
            has_cachyos: optional_repos.has_cachyos,
            has_artix: optional_repos.has_artix,
            has_artix_omniverse: false,
            has_artix_universe: false,
            has_artix_lib32: false,
            has_artix_galaxy: false,
            has_artix_world: false,
            has_artix_system: false,
            has_manjaro: optional_repos.has_manjaro,
        };
        let mut consumed_without_specific = base_consumed.saturating_add(
            calculate_optional_repos_width(&repos_without_specific, &optional_labels),
        );
        // Add 3 extra chars for " v" dropdown indicator if artix is present
        if optional_repos.has_artix {
            consumed_without_specific = consumed_without_specific.saturating_add(3);
        }
        let new_pad = inner_width.saturating_sub(consumed_without_specific.saturating_add(right_w));
        if new_pad >= 1 {
            show_artix_specific_repos = false;
            pad = new_pad;
            final_consumed_left = consumed_without_specific;
        }
    }

    // Check if we need to hide Artix filters when using collapsed menu
    // This must be done before determining collapsed menu to ensure consistency
    // If we might use collapsed menu (pad < 1), check if Artix filters fit
    if pad < 1 && show_artix_specific_repos {
        // Calculate space needed with all filters
        let space_for_collapsed_menu_with_filters =
            consumed_left.saturating_add(menu_w).saturating_add(1);

        // Recalculate without Artix-specific repo filters to check space
        let repos_without_specific = OptionalRepos {
            has_eos: optional_repos.has_eos,
            has_cachyos: optional_repos.has_cachyos,
            has_artix: optional_repos.has_artix,
            has_artix_omniverse: false,
            has_artix_universe: false,
            has_artix_lib32: false,
            has_artix_galaxy: false,
            has_artix_world: false,
            has_artix_system: false,
            has_manjaro: optional_repos.has_manjaro,
        };
        let mut consumed_without_specific = base_consumed.saturating_add(
            calculate_optional_repos_width(&repos_without_specific, &optional_labels),
        );
        // Add 3 extra chars for " v" dropdown indicator if artix is present
        if optional_repos.has_artix {
            consumed_without_specific = consumed_without_specific.saturating_add(3);
        }
        let space_for_collapsed_menu_without_filters = consumed_without_specific
            .saturating_add(menu_w)
            .saturating_add(1);

        // If there's not enough space with filters but enough without, hide them
        if inner_width < space_for_collapsed_menu_with_filters
            && inner_width >= space_for_collapsed_menu_without_filters
        {
            show_artix_specific_repos = false;
            final_consumed_left = consumed_without_specific;
        }
    }

    // Determine if we should use collapsed menu instead of individual buttons
    // Decision logic:
    // - pad is the remaining space after accounting for final_consumed_left + right_w
    // - If pad >= 1: we have space for all three buttons (use_collapsed_menu = false)
    // - If pad < 1: check if we have space for collapsed menu
    //   Calculate space needed for collapsed menu: final_consumed_left + menu_w
    //   If inner_width >= (final_consumed_left + menu_w + 1): use collapsed menu
    //   Otherwise: show nothing
    let use_collapsed_menu = if pad < 1 {
        // Not enough space for all three buttons, check if collapsed menu fits
        let space_needed_for_menu = final_consumed_left.saturating_add(menu_w).saturating_add(1);
        inner_width >= space_needed_for_menu
    } else {
        false
    };

    // Calculate padding for collapsed menu (space after accounting for consumed_left + menu_w)
    let menu_pad = if use_collapsed_menu {
        inner_width.saturating_sub(final_consumed_left.saturating_add(menu_w))
    } else {
        pad
    };

    TitleLayoutInfo {
        results_title_text,
        sort_button_label,
        options_button_label,
        panels_button_label,
        config_button_label,
        menu_button_label,
        core_labels,
        optional_labels,
        inner_width,
        show_artix_specific_repos,
        pad,
        use_collapsed_menu,
        menu_pad,
    }
}

/// What: Record rectangles for core filter buttons (AUR, core, extra, multilib).
///
/// Inputs:
/// - `app`: Mutable application state (rects will be updated)
/// - `layout`: Layout state tracker
/// - `core_labels`: Labels for core filters
///
/// Output: Updates app with core filter rectangles.
///
/// Details: Records rectangles for the four core filter buttons in sequence.
fn record_core_filter_rects(
    app: &mut AppState,
    layout: &mut LayoutState,
    core_labels: &CoreFilterLabels,
) {
    // Use Unicode display width, not byte length, to handle wide characters
    app.results_filter_aur_rect = Some(layout.record_rect(&core_labels.aur));
    layout.advance(
        u16::try_from(core_labels.aur.width()).unwrap_or(u16::MAX),
        1,
    );

    app.results_filter_core_rect = Some(layout.record_rect(&core_labels.core));
    layout.advance(
        u16::try_from(core_labels.core.width()).unwrap_or(u16::MAX),
        1,
    );

    app.results_filter_extra_rect = Some(layout.record_rect(&core_labels.extra));
    layout.advance(
        u16::try_from(core_labels.extra.width()).unwrap_or(u16::MAX),
        1,
    );

    app.results_filter_multilib_rect = Some(layout.record_rect(&core_labels.multilib));
    layout.advance(
        u16::try_from(core_labels.multilib.width()).unwrap_or(u16::MAX),
        1,
    );
}

/// What: Record rectangles for optional repository filters.
///
/// Inputs:
/// - `app`: Mutable application state (rects will be updated)
/// - `layout`: Layout state tracker
/// - `optional_repos`: Optional repository availability flags
/// - `optional_labels`: Labels for optional repos
/// - `show_artix_specific_repos`: Whether to show Artix-specific repo filters
///
/// Output: Updates app with optional repo filter rectangles.
///
/// Details: Records rectangles for `EOS`, `CachyOS`, `Artix`, Artix-specific repos, and `Manjaro` filters.
fn record_optional_repo_rects(
    app: &mut AppState,
    layout: &mut LayoutState,
    optional_repos: &OptionalRepos,
    optional_labels: &OptionalReposLabels,
    show_artix_specific_repos: bool,
) {
    // Record EOS filter
    // Use Unicode display width, not byte length, to handle wide characters
    if optional_repos.has_eos {
        app.results_filter_eos_rect = Some(layout.record_rect(&optional_labels.eos));
        layout.advance(
            u16::try_from(optional_labels.eos.width()).unwrap_or(u16::MAX),
            1,
        );
    } else {
        app.results_filter_eos_rect = None;
    }

    // Record CachyOS filter
    if optional_repos.has_cachyos {
        app.results_filter_cachyos_rect = Some(layout.record_rect(&optional_labels.cachyos));
        layout.advance(
            u16::try_from(optional_labels.cachyos.width()).unwrap_or(u16::MAX),
            1,
        );
    } else {
        app.results_filter_cachyos_rect = None;
    }

    // Record Artix filter (with dropdown indicator if specific filters are hidden)
    if optional_repos.has_artix {
        let artix_label_with_indicator = if show_artix_specific_repos {
            optional_labels.artix.clone()
        } else {
            format!("{} v", optional_labels.artix)
        };
        app.results_filter_artix_rect = Some(layout.record_rect(&artix_label_with_indicator));
        layout.advance(
            u16::try_from(artix_label_with_indicator.width()).unwrap_or(u16::MAX),
            1,
        );
    } else {
        app.results_filter_artix_rect = None;
    }

    // Record Artix-specific repo filter rects only if there's space
    if show_artix_specific_repos {
        let artix_rects = [
            (
                optional_repos.has_artix_omniverse,
                &optional_labels.artix_omniverse,
                &mut app.results_filter_artix_omniverse_rect,
            ),
            (
                optional_repos.has_artix_universe,
                &optional_labels.artix_universe,
                &mut app.results_filter_artix_universe_rect,
            ),
            (
                optional_repos.has_artix_lib32,
                &optional_labels.artix_lib32,
                &mut app.results_filter_artix_lib32_rect,
            ),
            (
                optional_repos.has_artix_galaxy,
                &optional_labels.artix_galaxy,
                &mut app.results_filter_artix_galaxy_rect,
            ),
            (
                optional_repos.has_artix_world,
                &optional_labels.artix_world,
                &mut app.results_filter_artix_world_rect,
            ),
            (
                optional_repos.has_artix_system,
                &optional_labels.artix_system,
                &mut app.results_filter_artix_system_rect,
            ),
        ];
        for (has_repo, label, rect_field) in artix_rects {
            if has_repo {
                *rect_field = Some(layout.record_rect(label));
                // Use Unicode display width, not byte length, to handle wide characters
                layout.advance(u16::try_from(label.width()).unwrap_or(u16::MAX), 1);
            } else {
                *rect_field = None;
            }
        }
    } else {
        // Hide Artix-specific repo filter rects when space is tight
        app.results_filter_artix_omniverse_rect = None;
        app.results_filter_artix_universe_rect = None;
        app.results_filter_artix_lib32_rect = None;
        app.results_filter_artix_galaxy_rect = None;
        app.results_filter_artix_world_rect = None;
        app.results_filter_artix_system_rect = None;
    }

    // Record Manjaro filter
    if optional_repos.has_manjaro {
        app.results_filter_manjaro_rect = Some(layout.record_rect(&optional_labels.manjaro));
    } else {
        app.results_filter_manjaro_rect = None;
    }
}

/// What: Record rectangles for right-aligned buttons (Config/Lists, Panels, Options) or collapsed Menu button.
///
/// Inputs:
/// - `app`: Mutable application state (rects will be updated)
/// - `area`: Target rectangle for the results block
/// - `layout_info`: Title layout information
/// - `btn_y`: Y position for buttons
///
/// Output: Updates app with right-aligned button rectangles.
///
/// Details: Records rectangles for either all three buttons or the collapsed Menu button based on available space.
fn record_right_aligned_button_rects(
    app: &mut AppState,
    area: Rect,
    layout_info: &TitleLayoutInfo,
    btn_y: u16,
) {
    if layout_info.use_collapsed_menu {
        // Record collapsed menu button rect if we have space for it
        if layout_info.menu_pad >= 1 {
            let menu_w = u16::try_from(layout_info.menu_button_label.width()).unwrap_or(u16::MAX);
            let menu_x = area
                .x
                .saturating_add(1) // left border inset
                .saturating_add(layout_info.inner_width.saturating_sub(menu_w));
            app.collapsed_menu_button_rect = Some((menu_x, btn_y, menu_w, 1));
        } else {
            app.collapsed_menu_button_rect = None;
        }
        // Clear individual button rects
        app.config_button_rect = None;
        app.options_button_rect = None;
        app.panels_button_rect = None;
    } else if layout_info.pad >= 1 {
        // Record clickable rects at the computed right edge (Panels to the left of Options)
        // Use Unicode display width, not byte length, to handle wide characters
        let options_w = u16::try_from(layout_info.options_button_label.width()).unwrap_or(u16::MAX);
        let panels_w = u16::try_from(layout_info.panels_button_label.width()).unwrap_or(u16::MAX);
        let config_w = u16::try_from(layout_info.config_button_label.width()).unwrap_or(u16::MAX);
        let opt_x = area
            .x
            .saturating_add(1) // left border inset
            .saturating_add(layout_info.inner_width.saturating_sub(options_w));
        let pan_x = opt_x.saturating_sub(1).saturating_sub(panels_w);
        let cfg_x = pan_x.saturating_sub(1).saturating_sub(config_w);
        app.config_button_rect = Some((cfg_x, btn_y, config_w, 1));
        app.options_button_rect = Some((opt_x, btn_y, options_w, 1));
        app.panels_button_rect = Some((pan_x, btn_y, panels_w, 1));
        // Clear collapsed menu button rect
        app.collapsed_menu_button_rect = None;
    } else {
        app.config_button_rect = None;
        app.options_button_rect = None;
        app.panels_button_rect = None;
        app.collapsed_menu_button_rect = None;
    }
}

/// What: Record clickable rectangles for title bar controls.
///
/// This version takes a context struct to reduce data flow complexity.
///
/// Inputs:
/// - `app`: Mutable application state (rects will be updated)
/// - `ctx`: Render context containing all extracted values
/// - `area`: Target rectangle for the results block
///
/// Output:
/// - Updates `app` with rectangles for filters, buttons, and optional repo chips.
///
/// Details:
/// - Mirrors title layout calculations to align rects with rendered elements and clears entries when
///   controls cannot fit in the available width.
/// - Extracts values from context and delegates to `record_title_rects`.
pub fn record_title_rects_from_context(app: &mut AppState, ctx: &RenderContext, area: Rect) {
    record_title_rects(app, area, &ctx.optional_repos);
}

/// What: Record clickable rectangles for title bar controls.
///
/// Inputs:
/// - `app`: Mutable application state (rects will be updated)
/// - `area`: Target rectangle for the results block
/// - `optional_repos`: Optional repository availability flags
///
/// Output:
/// - Updates `app` with rectangles for filters, buttons, and optional repo chips.
///
/// Details:
/// - Mirrors title layout calculations to align rects with rendered elements and clears entries when
///   controls cannot fit in the available width.
/// - Uses shared layout calculation logic and helper functions to reduce complexity.
pub fn record_title_rects(app: &mut AppState, area: Rect, optional_repos: &OptionalRepos) {
    let inner_width = area.width.saturating_sub(2); // exclude borders
    let i18n = build_title_i18n_strings(app);
    // Calculate shared layout information
    let layout_info =
        calculate_title_layout_info(&i18n, app.results.len(), inner_width, optional_repos);

    // Initialize layout state starting after title and sort button
    // Use Unicode display width, not byte length, to handle wide characters
    let btn_y = area.y; // top border row
    let initial_x = area
        .x
        .saturating_add(1) // left border inset
        .saturating_add(u16::try_from(layout_info.results_title_text.width()).unwrap_or(u16::MAX))
        .saturating_add(2) // two spaces before Sort
        .saturating_add(u16::try_from(layout_info.sort_button_label.width()).unwrap_or(u16::MAX))
        .saturating_add(2); // space after sort
    let mut layout = LayoutState::new(initial_x, btn_y);

    // Record sort button rect
    let sort_btn_x = area
        .x
        .saturating_add(1)
        .saturating_add(u16::try_from(layout_info.results_title_text.width()).unwrap_or(u16::MAX))
        .saturating_add(2);
    app.sort_button_rect = Some((
        sort_btn_x,
        btn_y,
        u16::try_from(layout_info.sort_button_label.width()).unwrap_or(u16::MAX),
        1,
    ));

    // Record core filter rects
    record_core_filter_rects(app, &mut layout, &layout_info.core_labels);

    // Record optional repo filter rects
    record_optional_repo_rects(
        app,
        &mut layout,
        optional_repos,
        &layout_info.optional_labels,
        layout_info.show_artix_specific_repos,
    );

    // Record right-aligned button rects
    record_right_aligned_button_rects(app, area, &layout_info, btn_y);
}
