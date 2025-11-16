//! Filter toggle mouse event handling.

use crate::state::AppState;

/// Handle mouse events for filter toggles.
///
/// What: Process mouse clicks on filter toggle labels in the Results title bar to enable/disable
/// repository filters (AUR, Core, Extra, Multilib, EOS, CachyOS, Artix, Manjaro).
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
    // Toggle filters when clicking their labels
    if let Some((x, y, w, h)) = app.results_filter_aur_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_aur = !app.results_filter_show_aur;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_core_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_core = !app.results_filter_show_core;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_extra_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_extra = !app.results_filter_show_extra;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_multilib_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_multilib = !app.results_filter_show_multilib;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_eos_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_eos = !app.results_filter_show_eos;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_cachyos_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_cachyos = !app.results_filter_show_cachyos;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_artix_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        // Check if Artix-specific filters are hidden (dropdown mode)
        let has_hidden_filters = app.results_filter_artix_omniverse_rect.is_none()
            && app.results_filter_artix_universe_rect.is_none()
            && app.results_filter_artix_lib32_rect.is_none()
            && app.results_filter_artix_galaxy_rect.is_none()
            && app.results_filter_artix_world_rect.is_none()
            && app.results_filter_artix_system_rect.is_none();

        if has_hidden_filters {
            // Toggle dropdown instead of filters
            app.artix_filter_menu_open = !app.artix_filter_menu_open;
        } else {
            // Normal behavior: toggle all Artix filters
            // Check if all individual Artix repo filters are on
            let all_on = app.results_filter_show_artix_omniverse
                && app.results_filter_show_artix_universe
                && app.results_filter_show_artix_lib32
                && app.results_filter_show_artix_galaxy
                && app.results_filter_show_artix_world
                && app.results_filter_show_artix_system;

            // If all are on, turn all off; otherwise turn all on
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
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_artix_omniverse_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_artix_omniverse = !app.results_filter_show_artix_omniverse;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_artix_universe_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_artix_universe = !app.results_filter_show_artix_universe;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_artix_lib32_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_artix_lib32 = !app.results_filter_show_artix_lib32;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_artix_galaxy_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_artix_galaxy = !app.results_filter_show_artix_galaxy;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_artix_world_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_artix_world = !app.results_filter_show_artix_world;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_artix_system_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_artix_system = !app.results_filter_show_artix_system;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    if let Some((x, y, w, h)) = app.results_filter_manjaro_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.results_filter_show_manjaro = !app.results_filter_show_manjaro;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        return Some(false);
    }
    // If Artix filter dropdown open, handle clicks inside menu
    if app.artix_filter_menu_open
        && let Some((x, y, w, h)) = app.artix_filter_menu_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let row = my.saturating_sub(y) as usize; // 0-based within options
        match row {
            0 => {
                // Toggle all Artix filters
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
            _ => {}
        }
        // Update the main Artix filter state based on individual filters
        app.results_filter_show_artix = app.results_filter_show_artix_omniverse
            || app.results_filter_show_artix_universe
            || app.results_filter_show_artix_lib32
            || app.results_filter_show_artix_galaxy
            || app.results_filter_show_artix_world
            || app.results_filter_show_artix_system;
        return Some(false);
    }

    None
}
