//! Default implementation for `AppState`.

use super::AppState;
use super::defaults;
use super::defaults_cache;

impl Default for AppState {
    /// What: Construct a default, empty [`AppState`] with initialized paths, selection states, and timers.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Returns an `AppState` instance with sensible defaults for all fields.
    ///
    /// Details:
    /// - Delegates initialization to helper functions that group related fields logically.
    /// - Initializes paths for persisted data (recent searches, cache, news, install list, etc.) under the configured lists directory.
    /// - Sets selection indices to zero, result buffers to empty, and UI flags to default visibility states.
    /// - All repository filters default to showing everything.
    /// - Initializes timers, scroll positions, and modal states to their default values.
    #[allow(clippy::too_many_lines)]
    fn default() -> Self {
        let (
            recent_path,
            cache_path,
            news_read_path,
            install_path,
            official_index_path,
            deps_cache_path,
            files_cache_path,
            services_cache_path,
        ) = defaults::default_paths();
        let (
            results_filter_show_aur,
            results_filter_show_core,
            results_filter_show_extra,
            results_filter_show_multilib,
            results_filter_show_eos,
            results_filter_show_cachyos,
            results_filter_show_artix,
            results_filter_show_artix_omniverse,
            results_filter_show_artix_universe,
            results_filter_show_artix_lib32,
            results_filter_show_artix_galaxy,
            results_filter_show_artix_world,
            results_filter_show_artix_system,
            filter_rects,
        ) = defaults::default_filters();
        let [
            results_filter_aur_rect,
            results_filter_core_rect,
            results_filter_extra_rect,
            results_filter_multilib_rect,
            results_filter_eos_rect,
            results_filter_cachyos_rect,
            results_filter_artix_rect,
            results_filter_artix_omniverse_rect,
            results_filter_artix_universe_rect,
            results_filter_artix_lib32_rect,
            results_filter_artix_galaxy_rect,
            results_filter_artix_world_rect,
            results_filter_artix_system_rect,
        ] = filter_rects;

        let (
            input,
            results,
            all_results,
            results_backup_for_toggle,
            selected,
            details,
            list_state,
            modal,
            previous_modal,
            dry_run,
            focus,
            last_input_change,
            last_saved_value,
            latest_query_id,
            next_query_id,
        ) = defaults::default_search_state();

        let (recent, history_state, recent_path, recent_dirty) =
            defaults::default_recent_state(recent_path);

        let (details_cache, cache_path, cache_dirty) =
            defaults::default_details_cache_state(cache_path);

        let (news_read_urls, news_read_path, news_read_dirty) =
            defaults::default_news_state(news_read_path);

        let (
            install_list,
            install_state,
            remove_list,
            remove_state,
            downgrade_list,
            downgrade_state,
            install_path,
            install_dirty,
            last_install_change,
        ) = defaults::default_install_lists_state(install_path);

        let (show_recent_pane, show_install_pane, show_keybinds_footer, pane_find) =
            defaults::default_ui_visibility_state();

        let (search_normal_mode, fuzzy_search_enabled, search_caret, search_select_anchor) =
            defaults::default_search_input_state();

        let (official_index_path, loading_index, details_focus) =
            defaults::default_index_state(official_index_path);

        let (scroll_moves, ring_resume_at, need_ring_prefetch) =
            defaults::default_scroll_prefetch_state();

        let (
            url_button_rect,
            vt_url_rect,
            install_import_rect,
            install_export_rect,
            arch_status_text,
            arch_status_rect,
            arch_status_color,
            updates_count,
            updates_list,
            updates_button_rect,
            updates_loading,
            refresh_updates,
        ) = defaults::default_clickable_rects_state();

        let (
            pkgb_button_rect,
            pkgb_check_button_rect,
            pkgb_reload_button_rect,
            pkgb_visible,
            pkgb_text,
            pkgb_package_name,
            pkgb_reload_requested_at,
            pkgb_reload_requested_for,
            pkgb_scroll,
            pkgb_rect,
        ) = defaults::default_pkgbuild_state();

        let (toast_message, toast_expires_at) = defaults::default_toast_state();

        let (
            layout_left_pct,
            layout_center_pct,
            layout_right_pct,
            keymap,
            locale,
            translations,
            translations_fallback,
        ) = defaults::default_settings_state();

        let (
            results_rect,
            details_rect,
            details_scroll,
            recent_rect,
            install_rect,
            downgrade_rect,
            mouse_disabled_in_details,
            last_mouse_pos,
            mouse_capture_enabled,
        ) = defaults::default_mouse_hit_test_state();

        let (
            news_rect,
            news_list_rect,
            updates_modal_rect,
            updates_modal_content_rect,
            help_scroll,
            help_rect,
            preflight_tab_rects,
            preflight_content_rect,
        ) = defaults::default_modal_rects_state();

        let (
            sort_mode,
            sort_menu_open,
            sort_button_rect,
            sort_menu_rect,
            sort_menu_auto_close_at,
            options_menu_open,
            options_button_rect,
            options_menu_rect,
            panels_menu_open,
            panels_button_rect,
            panels_menu_rect,
            config_menu_open,
            artix_filter_menu_open,
            artix_filter_menu_rect,
            config_button_rect,
            config_menu_rect,
        ) = defaults::default_sorting_menus_state();

        let (installed_only_mode, right_pane_focus, package_marker) =
            defaults::default_results_mode_state();

        let (
            refresh_installed_until,
            next_installed_refresh_at,
            pending_install_names,
            pending_remove_names,
        ) = defaults_cache::default_cache_refresh_state();

        let (
            install_list_deps,
            remove_preflight_summary,
            remove_cascade_mode,
            deps_resolving,
            deps_cache_path,
            deps_cache_dirty,
        ) = defaults_cache::default_deps_cache_state(deps_cache_path);

        let (install_list_files, files_resolving, files_cache_path, files_cache_dirty) =
            defaults_cache::default_files_cache_state(files_cache_path);

        let (
            install_list_services,
            services_resolving,
            services_cache_path,
            services_cache_dirty,
            service_resolve_now,
            active_service_request,
            next_service_request_id,
            services_pending_signature,
            pending_service_plan,
        ) = defaults_cache::default_services_cache_state(services_cache_path);

        let (install_list_sandbox, sandbox_resolving, sandbox_cache_path, sandbox_cache_dirty) =
            defaults_cache::default_sandbox_cache_state();

        let (
            preflight_summary_items,
            preflight_deps_items,
            preflight_files_items,
            preflight_services_items,
            preflight_sandbox_items,
            preflight_summary_resolving,
            preflight_deps_resolving,
            preflight_files_resolving,
            preflight_services_resolving,
            preflight_sandbox_resolving,
            preflight_cancelled,
        ) = defaults_cache::default_preflight_state();

        Self {
            input,
            results,
            all_results,
            results_backup_for_toggle,
            selected,
            details,
            list_state,
            modal,
            previous_modal,
            dry_run,
            recent,
            history_state,
            focus,
            last_input_change,
            last_saved_value,
            recent_path,
            recent_dirty,
            latest_query_id,
            next_query_id,
            details_cache,
            cache_path,
            cache_dirty,
            news_read_urls,
            news_read_path,
            news_read_dirty,
            install_list,
            install_state,
            remove_list,
            remove_state,
            downgrade_list,
            downgrade_state,
            install_path,
            install_dirty,
            last_install_change,
            show_recent_pane,
            show_install_pane,
            show_keybinds_footer,
            pane_find,
            search_normal_mode,
            fuzzy_search_enabled,
            search_caret,
            search_select_anchor,
            official_index_path,
            loading_index,
            details_focus,
            scroll_moves,
            ring_resume_at,
            need_ring_prefetch,
            url_button_rect,
            vt_url_rect,
            install_import_rect,
            install_export_rect,
            arch_status_text,
            arch_status_rect,
            arch_status_color,
            updates_count,
            updates_list,
            updates_button_rect,
            updates_loading,
            refresh_updates,
            pkgb_button_rect,
            pkgb_check_button_rect,
            pkgb_reload_button_rect,
            pkgb_visible,
            pkgb_text,
            pkgb_package_name,
            pkgb_reload_requested_at,
            pkgb_reload_requested_for,
            pkgb_scroll,
            pkgb_rect,
            toast_message,
            toast_expires_at,
            layout_left_pct,
            layout_center_pct,
            layout_right_pct,
            keymap,
            locale,
            translations,
            translations_fallback,
            results_rect,
            details_rect,
            details_scroll,
            recent_rect,
            install_rect,
            downgrade_rect,
            mouse_disabled_in_details,
            last_mouse_pos,
            mouse_capture_enabled,
            news_rect,
            news_list_rect,
            updates_modal_rect,
            updates_modal_content_rect,
            help_scroll,
            help_rect,
            preflight_tab_rects,
            preflight_content_rect,
            sort_mode,
            sort_menu_open,
            sort_button_rect,
            sort_menu_rect,
            sort_menu_auto_close_at,
            options_menu_open,
            options_button_rect,
            options_menu_rect,
            panels_menu_open,
            panels_button_rect,
            panels_menu_rect,
            config_menu_open,
            artix_filter_menu_open,
            artix_filter_menu_rect,
            config_button_rect,
            config_menu_rect,
            installed_only_mode,
            right_pane_focus,
            package_marker,
            results_filter_show_aur,
            results_filter_show_core,
            results_filter_show_extra,
            results_filter_show_multilib,
            results_filter_show_eos,
            results_filter_show_cachyos,
            results_filter_show_artix,
            results_filter_show_artix_omniverse,
            results_filter_show_artix_universe,
            results_filter_show_artix_lib32,
            results_filter_show_artix_galaxy,
            results_filter_show_artix_world,
            results_filter_show_artix_system,
            results_filter_show_manjaro: true,
            results_filter_aur_rect,
            results_filter_core_rect,
            results_filter_extra_rect,
            results_filter_multilib_rect,
            results_filter_eos_rect,
            results_filter_cachyos_rect,
            results_filter_artix_rect,
            results_filter_artix_omniverse_rect,
            results_filter_artix_universe_rect,
            results_filter_artix_lib32_rect,
            results_filter_artix_galaxy_rect,
            results_filter_artix_world_rect,
            results_filter_artix_system_rect,
            results_filter_manjaro_rect: None,
            fuzzy_indicator_rect: None,
            refresh_installed_until,
            next_installed_refresh_at,
            pending_install_names,
            pending_remove_names,
            install_list_deps,
            remove_preflight_summary,
            remove_cascade_mode,
            deps_resolving,
            deps_cache_path,
            deps_cache_dirty,
            install_list_files,
            files_resolving,
            files_cache_path,
            files_cache_dirty,
            install_list_services,
            services_resolving,
            services_cache_path,
            services_cache_dirty,
            service_resolve_now,
            active_service_request,
            next_service_request_id,
            services_pending_signature,
            pending_service_plan,
            install_list_sandbox,
            sandbox_resolving,
            sandbox_cache_path,
            sandbox_cache_dirty,
            preflight_summary_items,
            preflight_deps_items,
            preflight_files_items,
            preflight_services_items,
            preflight_sandbox_items,
            preflight_summary_resolving,
            preflight_deps_resolving,
            preflight_files_resolving,
            preflight_services_resolving,
            preflight_sandbox_resolving,
            preflight_cancelled,
        }
    }
}
