//! Filter toggle mouse event handling.

use crate::state::AppState;

/// Check if a point is within a rectangle.
///
/// What: Determines if mouse coordinates fall within the bounds of a rectangle.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `rect`: Optional rectangle (x, y, width, height)
///
/// Output:
/// - `true` if the point is within the rectangle, `false` otherwise.
///
/// Details:
/// - Returns `false` if `rect` is `None`.
/// - Uses inclusive start and exclusive end bounds for width and height.
const fn is_point_in_rect(mx: u16, my: u16, rect: Option<(u16, u16, u16, u16)>) -> bool {
    if let Some((x, y, w, h)) = rect {
        mx >= x && mx < x + w && my >= y && my < y + h
    } else {
        false
    }
}

/// Toggle a simple boolean filter if the mouse click is within its rectangle.
///
/// What: Checks if mouse coordinates are within a filter's rectangle and toggles the filter if so.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `rect`: Optional rectangle for the filter toggle area
/// - `toggle_fn`: Closure that toggles the filter state
/// - `app`: Mutable application state for applying filters and sort
///
/// Output:
/// - `true` if the filter was toggled, `false` otherwise.
///
/// Details:
/// - If the click is within the rectangle, calls the toggle function and applies filters/sort.
fn try_toggle_simple_filter<F>(
    mx: u16,
    my: u16,
    rect: Option<(u16, u16, u16, u16)>,
    toggle_fn: F,
    app: &mut AppState,
) -> bool
where
    F: FnOnce(&mut AppState),
{
    if is_point_in_rect(mx, my, rect) {
        toggle_fn(app);
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        true
    } else {
        false
    }
}

/// Toggle all Artix repository filters together.
///
/// What: Sets all individual Artix repository filters to the same state (all on or all off).
///
/// Inputs:
/// - `app`: Mutable application state containing Artix filter states
///
/// Output: None (modifies state in place)
///
/// Details:
/// - Checks if all individual Artix filters are currently on.
/// - If all are on, turns all off; otherwise turns all on.
/// - Updates the main Artix filter state to match.
/// - Applies filters and sort after toggling.
fn toggle_all_artix_filters(app: &mut AppState) {
    let all_on = app.results_filter_show_artix_omniverse
        && app.results_filter_show_artix_universe
        && app.results_filter_show_artix_lib32
        && app.results_filter_show_artix_galaxy
        && app.results_filter_show_artix_world
        && app.results_filter_show_artix_system;

    let new_state = !all_on;
    app.results_filter_show_artix_omniverse = new_state;
    app.results_filter_show_artix_universe = new_state;
    app.results_filter_show_artix_lib32 = new_state;
    app.results_filter_show_artix_galaxy = new_state;
    app.results_filter_show_artix_world = new_state;
    app.results_filter_show_artix_system = new_state;
    app.results_filter_show_artix = new_state;
    crate::logic::apply_filters_and_sort_preserve_selection(app);
}

/// Check if Artix-specific filters are hidden (dropdown mode).
///
/// What: Determines if individual Artix repository filter rectangles are all hidden.
///
/// Inputs:
/// - `app`: Application state containing Artix filter rectangles
///
/// Output:
/// - `true` if all individual Artix filter rectangles are `None`, `false` otherwise.
///
/// Details:
/// - Returns `true` when in dropdown mode (all individual filters hidden).
const fn has_hidden_artix_filters(app: &AppState) -> bool {
    app.results_filter_artix_omniverse_rect.is_none()
        && app.results_filter_artix_universe_rect.is_none()
        && app.results_filter_artix_lib32_rect.is_none()
        && app.results_filter_artix_galaxy_rect.is_none()
        && app.results_filter_artix_world_rect.is_none()
        && app.results_filter_artix_system_rect.is_none()
}

/// Handle mouse click on the main Artix filter toggle.
///
/// What: Processes clicks on the main Artix filter button with special handling for dropdown mode.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
///
/// Details:
/// - In dropdown mode (hidden filters), toggles the dropdown menu.
/// - Otherwise, toggles all Artix filters together.
fn handle_artix_main_filter_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if !is_point_in_rect(mx, my, app.results_filter_artix_rect) {
        return false;
    }

    if has_hidden_artix_filters(app) {
        app.artix_filter_menu_open = !app.artix_filter_menu_open;
    } else {
        toggle_all_artix_filters(app);
    }
    true
}

/// Update the main Artix filter state based on individual filter states.
///
/// What: Sets the main Artix filter to true if any individual Artix filter is enabled.
///
/// Inputs:
/// - `app`: Mutable application state containing Artix filter states
///
/// Output: None (modifies state in place)
///
/// Details:
/// - The main Artix filter is enabled if at least one individual Artix filter is enabled.
fn update_main_artix_filter_state(app: &mut AppState) {
    app.results_filter_show_artix = app.results_filter_show_artix_omniverse
        || app.results_filter_show_artix_universe
        || app.results_filter_show_artix_lib32
        || app.results_filter_show_artix_galaxy
        || app.results_filter_show_artix_world
        || app.results_filter_show_artix_system;
}

/// Handle mouse clicks inside the Artix filter dropdown menu.
///
/// What: Processes clicks on menu items to toggle individual Artix filters or all at once.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
///
/// Details:
/// - Row 0 toggles all Artix filters together.
/// - Rows 1-6 toggle individual Artix repository filters.
/// - Updates the main Artix filter state after any toggle.
fn handle_artix_dropdown_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if !app.artix_filter_menu_open {
        return false;
    }

    let Some((x, y, w, h)) = app.artix_filter_menu_rect else {
        return false;
    };

    if !is_point_in_rect(mx, my, Some((x, y, w, h))) {
        return false;
    }

    let row = my.saturating_sub(y) as usize;
    match row {
        0 => toggle_all_artix_filters(app),
        1 => {
            app.results_filter_show_artix_omniverse = !app.results_filter_show_artix_omniverse;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
        }
        2 => {
            app.results_filter_show_artix_universe = !app.results_filter_show_artix_universe;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
        }
        3 => {
            app.results_filter_show_artix_lib32 = !app.results_filter_show_artix_lib32;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
        }
        4 => {
            app.results_filter_show_artix_galaxy = !app.results_filter_show_artix_galaxy;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
        }
        5 => {
            app.results_filter_show_artix_world = !app.results_filter_show_artix_world;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
        }
        6 => {
            app.results_filter_show_artix_system = !app.results_filter_show_artix_system;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
        }
        _ => return false,
    }

    update_main_artix_filter_state(app);
    true
}

/// Handle mouse events for filter toggles.
///
/// What: Process mouse clicks on filter toggle labels in the Results title bar to enable/disable
/// repository filters (`AUR`, `Core`, `Extra`, `Multilib`, `EOS`, `CachyOS`, `Artix`, `Manjaro`).
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state containing filter state and UI rectangles
///
/// Output:
/// - `Some(bool)` if the event was handled (consumed by a filter toggle), `None` if not handled.
///   The boolean value indicates whether the application should exit (always `false` here).
///
/// Details:
/// - Individual filters: Clicking a filter label toggles that filter and applies filters/sort.
/// - Artix main filter: When individual Artix repo filters are visible, toggles all Artix filters
///   together (all on -> all off, otherwise all on). When hidden (dropdown mode), toggles the dropdown menu.
/// - Artix dropdown menu: Handles clicks on menu items to toggle individual Artix repo filters or all at once.
///   Updates the main Artix filter state based on individual filter states.
pub(super) fn handle_filters_mouse(mx: u16, my: u16, app: &mut AppState) -> Option<bool> {
    // Handle Artix dropdown menu first (higher priority)
    if handle_artix_dropdown_click(mx, my, app) {
        return Some(false);
    }

    // Handle main Artix filter
    if handle_artix_main_filter_click(mx, my, app) {
        return Some(false);
    }

    // Handle simple filters
    if handle_simple_filter_toggles(mx, my, app) {
        return Some(false);
    }
    None
}

/// What: Try toggling all simple filters in sequence.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if any filter was toggled, `false` otherwise.
///
/// Details:
/// - Tries each filter in order and returns immediately if one is toggled.
fn handle_simple_filter_toggles(mx: u16, my: u16, app: &mut AppState) -> bool {
    try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_aur_rect,
        |a| a.results_filter_show_aur = !a.results_filter_show_aur,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_core_rect,
        |a| a.results_filter_show_core = !a.results_filter_show_core,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_extra_rect,
        |a| a.results_filter_show_extra = !a.results_filter_show_extra,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_multilib_rect,
        |a| a.results_filter_show_multilib = !a.results_filter_show_multilib,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_eos_rect,
        |a| a.results_filter_show_eos = !a.results_filter_show_eos,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_cachyos_rect,
        |a| a.results_filter_show_cachyos = !a.results_filter_show_cachyos,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_artix_omniverse_rect,
        |a| a.results_filter_show_artix_omniverse = !a.results_filter_show_artix_omniverse,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_artix_universe_rect,
        |a| a.results_filter_show_artix_universe = !a.results_filter_show_artix_universe,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_artix_lib32_rect,
        |a| a.results_filter_show_artix_lib32 = !a.results_filter_show_artix_lib32,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_artix_galaxy_rect,
        |a| a.results_filter_show_artix_galaxy = !a.results_filter_show_artix_galaxy,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_artix_world_rect,
        |a| a.results_filter_show_artix_world = !a.results_filter_show_artix_world,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_artix_system_rect,
        |a| a.results_filter_show_artix_system = !a.results_filter_show_artix_system,
        app,
    ) || try_toggle_simple_filter(
        mx,
        my,
        app.results_filter_manjaro_rect,
        |a| a.results_filter_show_manjaro = !a.results_filter_show_manjaro,
        app,
    )
}
