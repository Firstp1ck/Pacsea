use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use crate::theme::theme;

use super::super::{FilterStates, MenuStates, OptionalRepos};
use super::types::TitleI18nStrings;

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
pub(super) fn render_title_prefix(
    i18n: &TitleI18nStrings,
    results_len: usize,
) -> Vec<Span<'static>> {
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
pub(super) fn render_sort_button(i18n: &TitleI18nStrings, is_open: bool) -> Vec<Span<'static>> {
    let sort_button_label = format!("{} v", i18n.sort_button);
    let btn_style = get_button_style(is_open);
    vec![Span::styled(sort_button_label, btn_style)]
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
pub(super) fn render_core_filters(
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
pub(super) fn render_optional_eos_cachyos_filters(
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
pub(super) fn render_artix_filter(
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
pub(super) fn render_artix_specific_filters(
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
pub(super) fn render_manjaro_filter(
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
pub(super) fn render_right_aligned_buttons(
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
