use ratatui::prelude::Rect;

use super::{FilterStates, MenuStates, OptionalRepos, RenderContext};
use crate::state::{AppState, Source};

/// What: Detect availability of optional repos from the unfiltered results set.
///
/// Inputs:
/// - `app`: Application state providing `all_results`
///
/// Output:
/// - Tuple `(has_eos, has_cachyos, has_artix, has_artix_repos, has_manjaro)` indicating which repo chips to show.
///   `has_artix_repos` is a tuple of (omniverse, universe, lib32, galaxy, world, system) booleans.
///
/// Details:
/// - Scans official result sources and package names to infer EOS/CachyOS/Artix/Manjaro presence, short
///   circuiting once all are detected.
#[allow(clippy::type_complexity)]
pub fn detect_optional_repos(
    app: &AppState,
) -> (bool, bool, bool, (bool, bool, bool, bool, bool, bool), bool) {
    let mut eos = false;
    let mut cach = false;
    let mut artix = false;
    let mut artix_omniverse = false;
    let mut artix_universe = false;
    let mut artix_lib32 = false;
    let mut artix_galaxy = false;
    let mut artix_world = false;
    let mut artix_system = false;
    let mut manj = false;
    for it in &app.all_results {
        if let Source::Official { repo, .. } = &it.source {
            let r = repo.to_lowercase();
            if !eos && crate::index::is_eos_repo(&r) {
                eos = true;
            }
            if !cach && crate::index::is_cachyos_repo(&r) {
                cach = true;
            }
            if !artix && crate::index::is_artix_repo(&r) {
                artix = true;
            }
            if !artix_omniverse && crate::index::is_artix_omniverse(&r) {
                artix_omniverse = true;
            }
            if !artix_universe && crate::index::is_artix_universe(&r) {
                artix_universe = true;
            }
            if !artix_lib32 && crate::index::is_artix_lib32(&r) {
                artix_lib32 = true;
            }
            if !artix_galaxy && crate::index::is_artix_galaxy(&r) {
                artix_galaxy = true;
            }
            if !artix_world && crate::index::is_artix_world(&r) {
                artix_world = true;
            }
            if !artix_system && crate::index::is_artix_system(&r) {
                artix_system = true;
            }
        }
        // Treat presence by name prefix rather than repo value
        if !manj && crate::index::is_name_manjaro(&it.name) {
            manj = true;
        }
        if eos
            && cach
            && artix
            && manj
            && artix_omniverse
            && artix_universe
            && artix_lib32
            && artix_galaxy
            && artix_world
            && artix_system
        {
            break;
        }
    }
    (
        eos,
        cach,
        artix,
        (
            artix_omniverse,
            artix_universe,
            artix_lib32,
            artix_galaxy,
            artix_world,
            artix_system,
        ),
        manj,
    )
}

/// What: Keep the results selection centered within the visible viewport when possible.
///
/// Inputs:
/// - `app`: Mutable application state containing the list state and results
/// - `area`: Rect describing the results block (used to derive viewport rows)
///
/// Output:
/// - Adjusts `app.list_state` offset/selection to keep the highlight in view.
///
/// Details:
/// - Recenters around the selected index for long lists, resets offset for short lists, and ensures
///   the selection is applied even when the filtered list shrinks.
pub fn center_selection(app: &mut AppState, area: Rect) {
    let viewport_rows = area.height.saturating_sub(2) as usize; // account for borders
    let len = app.results.len();
    let selected_idx = if app.results.is_empty() {
        None
    } else {
        Some(app.selected.min(len - 1))
    };

    if viewport_rows > 0 && len > viewport_rows {
        let selected = selected_idx.unwrap_or(0);
        let max_offset = len.saturating_sub(viewport_rows);
        let desired = selected.saturating_sub(viewport_rows / 2).min(max_offset);
        if app.list_state.offset() == desired {
            // ensure selection is set
            app.list_state.select(selected_idx);
        } else {
            let mut st = ratatui::widgets::ListState::default().with_offset(desired);
            st.select(selected_idx);
            app.list_state = st;
        }
    } else {
        // Small lists: ensure offset is 0 and selection is applied
        if app.list_state.offset() != 0 {
            let mut st = ratatui::widgets::ListState::default().with_offset(0);
            st.select(selected_idx);
            app.list_state = st;
        } else {
            app.list_state.select(selected_idx);
        }
    }
}

/// What: Extract all data needed for rendering from `AppState` in one operation.
///
/// Inputs:
/// - `app`: Application state to extract data from
///
/// Output:
/// - `RenderContext` containing all extracted values
///
/// Details:
/// - Reduces data flow complexity by extracting all needed values in a single function call
///   instead of multiple individual field accesses.
pub fn extract_render_context(app: &AppState) -> RenderContext {
    let (has_eos, has_cachyos, has_artix, has_artix_repos, has_manjaro) =
        detect_optional_repos(app);
    let (
        has_artix_omniverse,
        has_artix_universe,
        has_artix_lib32,
        has_artix_galaxy,
        has_artix_world,
        has_artix_system,
    ) = has_artix_repos;

    RenderContext {
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
            collapsed_menu_open: app.collapsed_menu_open,
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
    }
}

/// What: Record the inner results rect for mouse hit-testing (inside borders).
///
/// Inputs:
/// - `app`: Mutable application state receiving the results rect
/// - `area`: Rect of the overall results block
///
/// Output:
/// - Updates `app.results_rect` with the inner content rectangle.
///
/// Details:
/// - Offsets by one cell to exclude borders and reduces width/height accordingly for accurate
///   click detection.
#[allow(clippy::missing_const_for_fn)]
pub fn record_results_rect(app: &mut AppState, area: Rect) {
    // Record inner results rect for mouse hit-testing (inside borders)
    app.results_rect = Some((
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    ));
}
