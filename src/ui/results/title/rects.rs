use ratatui::prelude::Rect;
use unicode_width::UnicodeWidthStr;

use crate::state::AppState;

use super::super::OptionalRepos;
use super::i18n::build_title_i18n_strings;
use super::layout::calculate_title_layout_info;
use super::types::{CoreFilterLabels, LayoutState, OptionalReposLabels, TitleLayoutInfo};

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
pub(super) fn record_title_rects(app: &mut AppState, area: Rect, optional_repos: &OptionalRepos) {
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
