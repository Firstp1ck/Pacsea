use ratatui::{prelude::Rect, text::Span};

use crate::state::AppState;

use super::{FilterStates, MenuStates, OptionalRepos, RenderContext};

/// What: Internationalization (i18n) string building for title rendering.
///
/// Details: Provides functions to build pre-computed i18n strings for title bar elements.
/// What: Internationalization (i18n) string building for title rendering.
///
/// Details: Provides functions to build pre-computed i18n strings for title bar elements.
mod i18n;
/// What: Layout calculation for title bar elements.
///
/// Details: Calculates positions and dimensions for title bar components.
mod layout;
/// What: Rectangle recording for title bar clickable areas.
///
/// Details: Records clickable rectangles for title bar controls.
mod rects;
/// What: Rendering functions for title bar elements.
///
/// Details: Provides focused rendering functions for individual title bar components.
mod rendering;
/// What: Type definitions for title rendering.
///
/// Details: Defines structs and types used for title bar rendering and layout.
mod types;
/// What: Width calculation utilities for title bar.
///
/// Details: Provides functions to calculate widths for title bar elements.
mod width;

use i18n::build_title_i18n_strings;
use layout::calculate_title_layout_info;
use rects::record_title_rects;
use rendering::{
    render_artix_filter, render_artix_specific_filters, render_core_filters, render_manjaro_filter,
    render_optional_eos_cachyos_filters, render_right_aligned_buttons, render_sort_button,
    render_title_prefix,
};

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
