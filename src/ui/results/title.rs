use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::Span,
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

use super::{FilterStates, MenuStates, OptionalRepos, RenderContext};

/// What: Calculate consumed horizontal space for optional repos.
///
/// Inputs:
/// - `repos`: Optional repository flags
/// - `labels`: Pre-formatted label strings for each repo
///
/// Output: Total consumed width in characters.
///
/// Details: Sums up the width of all available optional repos plus spacing.
fn calculate_optional_repos_width(repos: &OptionalRepos, labels: &OptionalReposLabels) -> u16 {
    let mut width = 0u16;
    if repos.has_eos {
        width = width.saturating_add(1 + labels.eos.len() as u16);
    }
    if repos.has_cachyos {
        width = width.saturating_add(1 + labels.cachyos.len() as u16);
    }
    if repos.has_artix {
        width = width.saturating_add(1 + labels.artix.len() as u16);
    }
    if repos.has_artix_omniverse {
        width = width.saturating_add(1 + labels.artix_omniverse.len() as u16);
    }
    if repos.has_artix_universe {
        width = width.saturating_add(1 + labels.artix_universe.len() as u16);
    }
    if repos.has_artix_lib32 {
        width = width.saturating_add(1 + labels.artix_lib32.len() as u16);
    }
    if repos.has_artix_galaxy {
        width = width.saturating_add(1 + labels.artix_galaxy.len() as u16);
    }
    if repos.has_artix_world {
        width = width.saturating_add(1 + labels.artix_world.len() as u16);
    }
    if repos.has_artix_system {
        width = width.saturating_add(1 + labels.artix_system.len() as u16);
    }
    if repos.has_manjaro {
        width = width.saturating_add(1 + labels.manjaro.len() as u16);
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
/// Output: Base consumed width in characters.
///
/// Details: Calculates space for fixed elements that are always present.
fn calculate_base_consumed_space(
    results_title_text: &str,
    sort_button_label: &str,
    core_labels: &CoreFilterLabels,
) -> u16 {
    (results_title_text.len()
        + 2 // spaces before Sort
        + sort_button_label.len()
        + 2 // spaces after Sort
        + core_labels.aur.len()
        + 1 // space
        + core_labels.core.len()
        + 1 // space
        + core_labels.extra.len()
        + 1 // space
        + core_labels.multilib.len()) as u16
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
/// - `app`: Application state for i18n
/// - `has_repo`: Whether repo is available
/// - `filter_key`: i18n key for filter label
/// - `is_active`: Whether filter is active
/// - `filt`: Filter rendering closure
///
/// Output: Option containing filter span, or None if not available.
///
/// Details: Returns Some(span) if repo is available, None otherwise.
fn render_optional_filter(
    app: &AppState,
    has_repo: bool,
    filter_key: &str,
    is_active: bool,
    filt: &dyn Fn(&str, bool) -> Span<'static>,
) -> Option<Span<'static>> {
    if has_repo {
        Some(filt(&i18n::t(app, filter_key), is_active))
    } else {
        None
    }
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
/// - Extracts values from context and delegates to `build_title_spans_from_values`.
pub fn build_title_spans_from_context(
    app: &AppState,
    ctx: &RenderContext,
    area: Rect,
) -> Vec<Span<'static>> {
    build_title_spans_from_values(
        app,
        ctx.results_len,
        area,
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
/// - `area`: Target rectangle for the results block
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
pub fn build_title_spans_from_values(
    app: &AppState,
    results_len: usize,
    area: Rect,
    optional_repos: &OptionalRepos,
    menu_states: &MenuStates,
    filter_states: &FilterStates,
) -> Vec<Span<'static>> {
    let th = theme();
    let results_title_text = format!("{} ({})", i18n::t(app, "app.results.title"), results_len);
    let sort_button_label = format!("{} v", i18n::t(app, "app.results.buttons.sort"));
    let options_button_label = format!("{} v", i18n::t(app, "app.results.buttons.options"));
    let panels_button_label = format!("{} v", i18n::t(app, "app.results.buttons.panels"));
    let config_button_label = format!("{} v", i18n::t(app, "app.results.buttons.config_lists"));
    let mut title_spans: Vec<Span> = vec![Span::styled(
        results_title_text.clone(),
        Style::default().fg(th.overlay1),
    )];
    title_spans.push(Span::raw("  "));
    // Style the sort button differently when menu is open
    let btn_style = get_button_style(menu_states.sort_menu_open);
    title_spans.push(Span::styled(sort_button_label.clone(), btn_style));
    title_spans.push(Span::raw("  "));
    // Filter toggles: [AUR] [core] [extra] [multilib] and optional [EOS]/[CachyOS]
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
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.aur"),
        filter_states.show_aur,
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.core"),
        filter_states.show_core,
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.extra"),
        filter_states.show_extra,
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.multilib"),
        filter_states.show_multilib,
    ));
    // Render optional EOS and CachyOS filters
    if let Some(span) = render_optional_filter(
        app,
        optional_repos.has_eos,
        "app.results.filters.eos",
        filter_states.show_eos,
        &filt,
    ) {
        title_spans.push(Span::raw(" "));
        title_spans.push(span);
    }
    if let Some(span) = render_optional_filter(
        app,
        optional_repos.has_cachyos,
        "app.results.filters.cachyos",
        filter_states.show_cachyos,
        &filt,
    ) {
        title_spans.push(Span::raw(" "));
        title_spans.push(span);
    }
    // Right-aligned Config/Lists, Panels and Options buttons: compute remaining space first
    // to determine if we should show Artix-specific repo filters
    let inner_width = area.width.saturating_sub(2); // exclude borders
    let core_labels = CoreFilterLabels {
        aur: format!("[{}]", i18n::t(app, "app.results.filters.aur")),
        core: format!("[{}]", i18n::t(app, "app.results.filters.core")),
        extra: format!("[{}]", i18n::t(app, "app.results.filters.extra")),
        multilib: format!("[{}]", i18n::t(app, "app.results.filters.multilib")),
    };
    let optional_labels = OptionalReposLabels {
        eos: format!("[{}]", i18n::t(app, "app.results.filters.eos")),
        cachyos: format!("[{}]", i18n::t(app, "app.results.filters.cachyos")),
        artix: format!("[{}]", i18n::t(app, "app.results.filters.artix")),
        artix_omniverse: format!("[{}]", i18n::t(app, "app.results.filters.artix_omniverse")),
        artix_universe: format!("[{}]", i18n::t(app, "app.results.filters.artix_universe")),
        artix_lib32: format!("[{}]", i18n::t(app, "app.results.filters.artix_lib32")),
        artix_galaxy: format!("[{}]", i18n::t(app, "app.results.filters.artix_galaxy")),
        artix_world: format!("[{}]", i18n::t(app, "app.results.filters.artix_world")),
        artix_system: format!("[{}]", i18n::t(app, "app.results.filters.artix_system")),
        manjaro: format!("[{}]", i18n::t(app, "app.results.filters.manjaro")),
    };

    // Calculate consumed space with all filters first
    let base_consumed =
        calculate_base_consumed_space(&results_title_text, &sort_button_label, &core_labels);
    let optional_consumed = calculate_optional_repos_width(optional_repos, &optional_labels);
    let consumed_left = base_consumed.saturating_add(optional_consumed);

    // Minimum single space before right-side buttons when possible
    let options_w = options_button_label.len() as u16;
    let panels_w = panels_button_label.len() as u16;
    let config_w = config_button_label.len() as u16;
    let right_w = config_w
        .saturating_add(1)
        .saturating_add(panels_w)
        .saturating_add(1)
        .saturating_add(options_w); // "Config/Lists" + space + "Panels" + space + "Options"
    let mut pad = inner_width.saturating_sub(consumed_left.saturating_add(right_w));

    // If not enough space, hide Artix-specific repo filters (keep generic Artix filter)
    let mut show_artix_specific_repos = true;
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
        let consumed_without_specific = base_consumed.saturating_add(
            calculate_optional_repos_width(&repos_without_specific, &optional_labels),
        );
        pad = inner_width.saturating_sub(consumed_without_specific.saturating_add(right_w));
        if pad >= 1 {
            show_artix_specific_repos = false;
        }
    }

    // Render Artix filter (with dropdown indicator if specific filters are hidden)
    if optional_repos.has_artix {
        title_spans.push(Span::raw(" "));
        let artix_label_text = i18n::t(app, "app.results.filters.artix");
        let artix_text = if show_artix_specific_repos {
            format!("[{artix_label_text}]")
        } else {
            format!("[{artix_label_text}] v")
        };
        let (fg, bg) = if filter_states.show_artix {
            (th.crust, th.green)
        } else {
            (th.mauve, th.surface2)
        };
        title_spans.push(Span::styled(
            artix_text,
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
        ));
    }

    // Render Artix-specific repo filters if there's space (before Manjaro)
    if show_artix_specific_repos {
        let artix_filters = [
            (
                optional_repos.has_artix_omniverse,
                "app.results.filters.artix_omniverse",
                filter_states.show_artix_omniverse,
            ),
            (
                optional_repos.has_artix_universe,
                "app.results.filters.artix_universe",
                filter_states.show_artix_universe,
            ),
            (
                optional_repos.has_artix_lib32,
                "app.results.filters.artix_lib32",
                filter_states.show_artix_lib32,
            ),
            (
                optional_repos.has_artix_galaxy,
                "app.results.filters.artix_galaxy",
                filter_states.show_artix_galaxy,
            ),
            (
                optional_repos.has_artix_world,
                "app.results.filters.artix_world",
                filter_states.show_artix_world,
            ),
            (
                optional_repos.has_artix_system,
                "app.results.filters.artix_system",
                filter_states.show_artix_system,
            ),
        ];
        for (has_repo, filter_key, is_active) in artix_filters {
            if let Some(span) = render_optional_filter(app, has_repo, filter_key, is_active, &filt)
            {
                title_spans.push(Span::raw(" "));
                title_spans.push(span);
            }
        }
    }

    // Render Manjaro filter
    if let Some(span) = render_optional_filter(
        app,
        optional_repos.has_manjaro,
        "app.results.filters.manjaro",
        filter_states.show_manjaro,
        &filt,
    ) {
        title_spans.push(Span::raw(" "));
        title_spans.push(span);
    }

    if pad >= 1 {
        title_spans.push(Span::raw(" ".repeat(pad as usize)));
        // Render Config/Lists button with underlined first char (C)
        let cfg_btn_style = get_button_style(menu_states.config_menu_open);
        title_spans.extend(render_button_with_underline(
            &config_button_label,
            cfg_btn_style,
        ));
        title_spans.push(Span::raw(" "));
        // Render Panels button with underlined first char (P)
        let pan_btn_style = get_button_style(menu_states.panels_menu_open);
        title_spans.extend(render_button_with_underline(
            &panels_button_label,
            pan_btn_style,
        ));
        title_spans.push(Span::raw(" "));
        // Render Options button with underlined first char (O)
        let opt_btn_style = get_button_style(menu_states.options_menu_open);
        title_spans.extend(render_button_with_underline(
            &options_button_label,
            opt_btn_style,
        ));
    }

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
    core_labels: CoreFilterLabels,
    optional_labels: OptionalReposLabels,
    inner_width: u16,
    show_artix_specific_repos: bool,
    pad: u16,
}

/// What: Layout state tracker for recording rectangles.
///
/// Inputs: Initial x position and y position.
///
/// Output: Struct that tracks current x cursor position and y position.
///
/// Details: Encapsulates layout state to avoid manual x_cursor tracking.
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
    /// Output: New LayoutState instance.
    ///
    /// Details: Initializes layout state with starting position.
    fn new(x: u16, y: u16) -> Self {
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
    fn record_rect(&self, label: &str) -> (u16, u16, u16, u16) {
        (self.x, self.y, label.len() as u16, 1)
    }
}

/// What: Calculate shared layout information for title bar.
///
/// Inputs:
/// - `app`: Application state for i18n and results count
/// - `area`: Target rectangle for the results block
/// - `optional_repos`: Optional repository availability flags
///
/// Output: TitleLayoutInfo containing all calculated layout values.
///
/// Details: Performs all layout calculations shared between rendering and rect recording.
fn calculate_title_layout_info(
    app: &AppState,
    area: Rect,
    optional_repos: &OptionalRepos,
) -> TitleLayoutInfo {
    let results_title_text = format!(
        "{} ({})",
        i18n::t(app, "app.results.title"),
        app.results.len()
    );
    let sort_button_label = format!("{} v", i18n::t(app, "app.results.buttons.sort"));
    let options_button_label = format!("{} v", i18n::t(app, "app.results.buttons.options"));
    let panels_button_label = format!("{} v", i18n::t(app, "app.results.buttons.panels"));
    let config_button_label = format!("{} v", i18n::t(app, "app.results.buttons.config_lists"));

    let inner_width = area.width.saturating_sub(2); // exclude borders
    let core_labels = CoreFilterLabels {
        aur: "[AUR]".to_string(),
        core: "[core]".to_string(),
        extra: "[extra]".to_string(),
        multilib: "[multilib]".to_string(),
    };
    let optional_labels = OptionalReposLabels {
        eos: "[EOS]".to_string(),
        cachyos: "[CachyOS]".to_string(),
        artix: format!("[{}]", i18n::t(app, "app.results.filters.artix")),
        artix_omniverse: format!("[{}]", i18n::t(app, "app.results.filters.artix_omniverse")),
        artix_universe: format!("[{}]", i18n::t(app, "app.results.filters.artix_universe")),
        artix_lib32: format!("[{}]", i18n::t(app, "app.results.filters.artix_lib32")),
        artix_galaxy: format!("[{}]", i18n::t(app, "app.results.filters.artix_galaxy")),
        artix_world: format!("[{}]", i18n::t(app, "app.results.filters.artix_world")),
        artix_system: format!("[{}]", i18n::t(app, "app.results.filters.artix_system")),
        manjaro: format!("[{}]", i18n::t(app, "app.results.filters.manjaro")),
    };

    // Calculate consumed space with all filters first
    let base_consumed =
        calculate_base_consumed_space(&results_title_text, &sort_button_label, &core_labels);
    let optional_consumed = calculate_optional_repos_width(optional_repos, &optional_labels);
    let consumed_left = base_consumed.saturating_add(optional_consumed);

    let options_w = options_button_label.len() as u16;
    let panels_w = panels_button_label.len() as u16;
    let config_w = config_button_label.len() as u16;
    let right_w = config_w
        .saturating_add(1)
        .saturating_add(panels_w)
        .saturating_add(1)
        .saturating_add(options_w);
    let mut pad = inner_width.saturating_sub(consumed_left.saturating_add(right_w));

    // If not enough space, hide Artix-specific repo filters (keep generic Artix filter)
    let mut show_artix_specific_repos = true;
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
        pad = inner_width.saturating_sub(consumed_without_specific.saturating_add(right_w));
        if pad >= 1 {
            show_artix_specific_repos = false;
        }
    }

    TitleLayoutInfo {
        results_title_text,
        sort_button_label,
        options_button_label,
        panels_button_label,
        config_button_label,
        core_labels,
        optional_labels,
        inner_width,
        show_artix_specific_repos,
        pad,
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
    app.results_filter_aur_rect = Some(layout.record_rect(&core_labels.aur));
    layout.advance(core_labels.aur.len() as u16, 1);

    app.results_filter_core_rect = Some(layout.record_rect(&core_labels.core));
    layout.advance(core_labels.core.len() as u16, 1);

    app.results_filter_extra_rect = Some(layout.record_rect(&core_labels.extra));
    layout.advance(core_labels.extra.len() as u16, 1);

    app.results_filter_multilib_rect = Some(layout.record_rect(&core_labels.multilib));
    layout.advance(core_labels.multilib.len() as u16, 1);
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
/// Details: Records rectangles for EOS, CachyOS, Artix, Artix-specific repos, and Manjaro filters.
fn record_optional_repo_rects(
    app: &mut AppState,
    layout: &mut LayoutState,
    optional_repos: &OptionalRepos,
    optional_labels: &OptionalReposLabels,
    show_artix_specific_repos: bool,
) {
    // Record EOS filter
    if optional_repos.has_eos {
        app.results_filter_eos_rect = Some(layout.record_rect(&optional_labels.eos));
        layout.advance(optional_labels.eos.len() as u16, 1);
    } else {
        app.results_filter_eos_rect = None;
    }

    // Record CachyOS filter
    if optional_repos.has_cachyos {
        app.results_filter_cachyos_rect = Some(layout.record_rect(&optional_labels.cachyos));
        layout.advance(optional_labels.cachyos.len() as u16, 1);
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
        layout.advance(artix_label_with_indicator.len() as u16, 1);
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
                layout.advance(label.len() as u16, 1);
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

/// What: Record rectangles for right-aligned buttons (Config/Lists, Panels, Options).
///
/// Inputs:
/// - `app`: Mutable application state (rects will be updated)
/// - `area`: Target rectangle for the results block
/// - `layout_info`: Title layout information
/// - `btn_y`: Y position for buttons
///
/// Output: Updates app with right-aligned button rectangles.
///
/// Details: Records rectangles for Config/Lists, Panels, and Options buttons at the right edge.
fn record_right_aligned_button_rects(
    app: &mut AppState,
    area: Rect,
    layout_info: &TitleLayoutInfo,
    btn_y: u16,
) {
    if layout_info.pad >= 1 {
        // Record clickable rects at the computed right edge (Panels to the left of Options)
        let options_w = layout_info.options_button_label.len() as u16;
        let panels_w = layout_info.panels_button_label.len() as u16;
        let config_w = layout_info.config_button_label.len() as u16;
        let opt_x = area
            .x
            .saturating_add(1) // left border inset
            .saturating_add(layout_info.inner_width.saturating_sub(options_w));
        let pan_x = opt_x.saturating_sub(1).saturating_sub(panels_w);
        let cfg_x = pan_x.saturating_sub(1).saturating_sub(config_w);
        app.config_button_rect = Some((cfg_x, btn_y, config_w, 1));
        app.options_button_rect = Some((opt_x, btn_y, options_w, 1));
        app.panels_button_rect = Some((pan_x, btn_y, panels_w, 1));
    } else {
        app.config_button_rect = None;
        app.options_button_rect = None;
        app.panels_button_rect = None;
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
    record_title_rects(app, area, &ctx.optional_repos)
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
    // Calculate shared layout information
    let layout_info = calculate_title_layout_info(app, area, optional_repos);

    // Initialize layout state starting after title and sort button
    let btn_y = area.y; // top border row
    let initial_x = area
        .x
        .saturating_add(1) // left border inset
        .saturating_add(layout_info.results_title_text.len() as u16)
        .saturating_add(2) // two spaces before Sort
        .saturating_add(layout_info.sort_button_label.len() as u16)
        .saturating_add(2); // space after sort
    let mut layout = LayoutState::new(initial_x, btn_y);

    // Record sort button rect
    let sort_btn_x = area
        .x
        .saturating_add(1)
        .saturating_add(layout_info.results_title_text.len() as u16)
        .saturating_add(2);
    app.sort_button_rect = Some((
        sort_btn_x,
        btn_y,
        layout_info.sort_button_label.len() as u16,
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
