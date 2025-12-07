//! Menu mouse event handling (sort, options, config, panels, import/export).

use std::time::Instant;

use tokio::sync::mpsc;

use crate::events::utils::refresh_selected_details;
use crate::i18n;
use crate::state::{AppState, PackageItem};

use super::menu_options;

/// Check if a point is within a rectangle.
///
/// What: Determines if mouse coordinates are inside a given rectangle.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `rect`: Optional rectangle as (x, y, width, height)
///
/// Output:
/// - `true` if point is within rectangle, `false` otherwise
///
/// Details:
/// - Returns `false` if rectangle is `None`
const fn point_in_rect(mx: u16, my: u16, rect: Option<(u16, u16, u16, u16)>) -> bool {
    if let Some((x, y, w, h)) = rect {
        mx >= x && mx < x + w && my >= y && my < y + h
    } else {
        false
    }
}

/// Handle click on Import button.
///
/// What: Opens `ImportHelp` modal when Import button is clicked.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
fn handle_import_button(app: &mut AppState) -> bool {
    app.modal = crate::state::Modal::ImportHelp;
    false
}

/// Handle click on Updates button.
///
/// What: Opens the available updates modal with scrollable list and triggers refresh.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
///
/// Details:
/// - Loads updates from `~/.config/pacsea/lists/available_updates.txt`
/// - Opens Updates modal with scroll support
/// - Triggers a refresh of the updates list to ensure current data
/// - Opens the modal only after refresh completes
#[allow(clippy::missing_const_for_fn)]
pub fn handle_updates_button(app: &mut AppState) -> bool {
    // Trigger refresh of updates list when button is clicked
    app.refresh_updates = true;
    app.updates_loading = true;
    // Set flag to open modal after refresh completes
    app.pending_updates_modal = true;
    // Don't open modal yet - wait for refresh to complete
    false
}

/// Handle click on Export button.
///
/// What: Exports install list to timestamped file in export directory.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
///
/// Details:
/// - Shows toast message if list is empty or export fails
fn handle_export_button(app: &mut AppState) -> bool {
    let mut names: Vec<String> = app.install_list.iter().map(|p| p.name.clone()).collect();
    names.sort();
    if names.is_empty() {
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.install_list_empty"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        return false;
    }
    let export_dir = crate::theme::config_dir().join("export");
    let _ = std::fs::create_dir_all(&export_dir);
    let date_str = crate::util::today_yyyymmdd_utc();
    let mut serial: u32 = 1;
    let file_path = loop {
        let fname = format!("install_list_{date_str}_{serial}.txt");
        let path = export_dir.join(&fname);
        if !path.exists() {
            break path;
        }
        serial += 1;
        if serial > 9999 {
            break export_dir.join(format!("install_list_{date_str}_fallback.txt"));
        }
    };
    let body = names.join("\n");
    match std::fs::write(&file_path, body) {
        Ok(()) => {
            app.toast_message = Some(crate::i18n::t_fmt1(
                app,
                "app.toasts.exported_to",
                file_path.display(),
            ));
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
            tracing::info!(path = %file_path.display().to_string(), count = names.len(), "export: wrote install list");
        }
        Err(e) => {
            let error_msg = format!("{e}");
            app.toast_message = Some(crate::i18n::t_fmt1(
                app,
                "app.toasts.export_failed",
                &error_msg,
            ));
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            tracing::error!(error = %e, path = %file_path.display().to_string(), "export: failed to write install list");
        }
    }
    false
}

/// Handle click on Arch status label.
///
/// What: Opens status.archlinux.org URL in browser.
///
/// Output:
/// - `false` if handled
fn handle_arch_status() -> bool {
    crate::util::open_url("https://status.archlinux.org");
    false
}

/// Handle click on sort menu button.
///
/// What: Toggles sort menu open/closed state.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
fn handle_sort_button(app: &mut AppState) -> bool {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        handle_news_age_toggle(app);
    } else {
        app.sort_menu_open = !app.sort_menu_open;
        if app.sort_menu_open {
            app.sort_menu_auto_close_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        } else {
            app.sort_menu_auto_close_at = None;
        }
    }
    false
}

/// Handle click on options menu button.
///
/// What: Toggles options menu and closes other menus.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
#[allow(clippy::missing_const_for_fn)]
fn handle_options_button(app: &mut AppState) -> bool {
    app.options_menu_open = !app.options_menu_open;
    if app.options_menu_open {
        app.panels_menu_open = false;
        app.config_menu_open = false;
        app.artix_filter_menu_open = false;
    }
    false
}

/// Handle click on config menu button.
///
/// What: Toggles config menu and closes other menus.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
#[allow(clippy::missing_const_for_fn)]
fn handle_config_button(app: &mut AppState) -> bool {
    app.config_menu_open = !app.config_menu_open;
    if app.config_menu_open {
        app.options_menu_open = false;
        app.panels_menu_open = false;
    }
    false
}

/// Handle click on panels menu button.
///
/// What: Toggles panels menu and closes other menus.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
#[allow(clippy::missing_const_for_fn)]
fn handle_panels_button(app: &mut AppState) -> bool {
    app.panels_menu_open = !app.panels_menu_open;
    if app.panels_menu_open {
        app.options_menu_open = false;
        app.config_menu_open = false;
        app.artix_filter_menu_open = false;
    }
    false
}

/// Handle click on collapsed menu button.
///
/// What: Toggles collapsed menu and closes other menus.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if handled
#[allow(clippy::missing_const_for_fn)]
fn handle_collapsed_menu_button(app: &mut AppState) -> bool {
    app.collapsed_menu_open = !app.collapsed_menu_open;
    if app.collapsed_menu_open {
        app.options_menu_open = false;
        app.config_menu_open = false;
        app.panels_menu_open = false;
        app.artix_filter_menu_open = false;
    }
    false
}

/// Handle click inside sort menu.
///
/// What: Changes sort mode based on clicked row and refreshes results.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
fn handle_sort_menu_click(
    _mx: u16,
    my: u16,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    if let Some((_x, y, _w, _h)) = app.sort_menu_rect {
        let row = my.saturating_sub(y) as usize;
        if matches!(app.app_mode, crate::state::types::AppMode::News) {
            match row {
                0 => app.news_sort_mode = crate::state::types::NewsSortMode::DateDesc,
                1 => app.news_sort_mode = crate::state::types::NewsSortMode::DateAsc,
                2 => app.news_sort_mode = crate::state::types::NewsSortMode::Title,
                3 => app.news_sort_mode = crate::state::types::NewsSortMode::SourceThenTitle,
                _ => return None,
            }
            app.refresh_news_results();
        } else {
            match row {
                0 => {
                    app.sort_mode = crate::state::SortMode::RepoThenName;
                    crate::theme::save_sort_mode(app.sort_mode);
                }
                1 => {
                    app.sort_mode = crate::state::SortMode::AurPopularityThenOfficial;
                    crate::theme::save_sort_mode(app.sort_mode);
                }
                2 => {
                    app.sort_mode = crate::state::SortMode::BestMatches;
                    crate::theme::save_sort_mode(app.sort_mode);
                }
                _ => return None,
            }
            crate::logic::sort_results_preserve_selection(app);
            if app.results.is_empty() {
                app.list_state.select(None);
            } else {
                app.selected = 0;
                app.list_state.select(Some(0));
                refresh_selected_details(app, details_tx);
            }
        }
        app.sort_menu_open = false;
        app.sort_menu_auto_close_at = None;
        Some(false)
    } else {
        None
    }
}

/// Handle click inside options menu.
///
/// What: Handles clicks on options menu items (installed-only toggle, system update, news, optional deps).
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
fn handle_options_menu_click(
    _mx: u16,
    my: u16,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    if let Some((_x, y, _w, _h)) = app.options_menu_rect {
        let row = my.saturating_sub(y) as usize;
        let news_mode = matches!(app.app_mode, crate::state::types::AppMode::News);
        if news_mode {
            match row {
                0 => handle_system_update_option(app),
                1 => handle_optional_deps_option(app),
                2 => handle_mode_toggle(app, details_tx),
                _ => return None,
            }
        } else {
            match row {
                0 => handle_installed_only_toggle(app, details_tx),
                1 => handle_system_update_option(app),
                2 => handle_optional_deps_option(app),
                3 => handle_mode_toggle(app, details_tx),
                _ => return None,
            }
        }
        app.options_menu_open = false;
        Some(false)
    } else {
        None
    }
}

/// Handle installed-only mode toggle.
///
/// What: Toggles between showing all packages and only explicitly installed packages.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Details:
/// - When enabling, saves installed packages list to config directory
fn handle_installed_only_toggle(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    use std::collections::HashSet;
    if app.installed_only_mode {
        if let Some(prev) = app.results_backup_for_toggle.take() {
            app.all_results = prev;
        }
        app.installed_only_mode = false;
        app.right_pane_focus = crate::state::RightPaneFocus::Install;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        crate::events::utils::refresh_selected_details(app, details_tx);
    } else {
        app.results_backup_for_toggle = Some(app.all_results.clone());
        let explicit = crate::index::explicit_names();
        let mut items: Vec<crate::state::PackageItem> = crate::index::all_official()
            .into_iter()
            .filter(|p| explicit.contains(&p.name))
            .collect();
        let official_names: HashSet<String> = items.iter().map(|p| p.name.clone()).collect();
        for name in explicit {
            if !official_names.contains(&name) {
                let is_eos = crate::index::is_eos_name(&name);
                let src = if is_eos {
                    crate::state::Source::Official {
                        repo: "EOS".to_string(),
                        arch: String::new(),
                    }
                } else {
                    crate::state::Source::Aur
                };
                items.push(crate::state::PackageItem {
                    name: name.clone(),
                    version: String::new(),
                    description: String::new(),
                    source: src,
                    popularity: None,
                    out_of_date: None,
                    orphaned: false,
                });
            }
        }
        app.all_results = items;
        app.installed_only_mode = true;
        app.right_pane_focus = crate::state::RightPaneFocus::Remove;
        crate::logic::apply_filters_and_sort_preserve_selection(app);
        crate::events::utils::refresh_selected_details(app, details_tx);

        let path = crate::theme::config_dir().join("installed_packages.txt");
        let mut names: Vec<String> = crate::index::explicit_names().into_iter().collect();
        names.sort();
        let body = names.join("\n");
        let _ = std::fs::write(path, body);
    }
}

/// What: Toggle between package mode and news feed mode.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details when switching back to package mode
pub(in crate::events) fn handle_mode_toggle(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        app.app_mode = crate::state::types::AppMode::Package;
        if app.results.is_empty() {
            app.list_state.select(None);
        } else {
            app.selected = app.selected.min(app.results.len().saturating_sub(1));
            app.list_state.select(Some(app.selected));
            refresh_selected_details(app, details_tx);
        }
    } else {
        app.app_mode = crate::state::types::AppMode::News;
        if app.news_results.is_empty() {
            app.news_list_state.select(None);
            app.news_selected = 0;
        } else {
            app.news_selected = 0;
            app.news_list_state.select(Some(0));
        }
    }
    crate::theme::save_app_start_mode(matches!(app.app_mode, crate::state::types::AppMode::News));
}

/// What: Toggle news maximum age filter between 7, 30, 90 days, and no limit.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - Cycles through age options: 7 days → 30 days → 90 days → no limit → 7 days
/// - Refreshes news results after changing the filter
pub(in crate::events) fn handle_news_age_toggle(app: &mut AppState) {
    const AGES: [Option<u32>; 4] = [Some(7), Some(30), Some(90), None];
    let current = app.news_max_age_days;
    let next = AGES
        .iter()
        .cycle()
        .skip_while(|&&v| v != current)
        .nth(1)
        .copied()
        .unwrap_or(Some(7));
    app.news_max_age_days = next;
    app.refresh_news_results();
    let age_label = app.news_max_age_days.map_or_else(
        || i18n::t(app, "app.results.options_menu.news_age_all"),
        |d| i18n::t_fmt1(app, "app.results.options_menu.news_age_days", d.to_string()),
    );
    app.toast_message = Some(age_label);
    app.toast_expires_at = Some(Instant::now() + std::time::Duration::from_secs(3));
    crate::theme::save_news_max_age_days(app.news_max_age_days);
}

/// Handle system update option.
///
/// What: Opens `SystemUpdate` modal with default settings.
///
/// Inputs:
/// - `app`: Mutable application state
fn handle_system_update_option(app: &mut AppState) {
    let countries = vec![
        "Worldwide".to_string(),
        "Germany".to_string(),
        "United States".to_string(),
        "United Kingdom".to_string(),
        "France".to_string(),
        "Netherlands".to_string(),
        "Sweden".to_string(),
        "Canada".to_string(),
        "Australia".to_string(),
        "Japan".to_string(),
    ];
    let prefs = crate::theme::settings();
    let initial_country_idx = {
        let sel = prefs
            .selected_countries
            .split(',')
            .next()
            .map_or_else(|| "Worldwide".to_string(), |s| s.trim().to_string());
        countries.iter().position(|c| c == &sel).unwrap_or(0)
    };
    app.modal = crate::state::Modal::SystemUpdate {
        do_mirrors: false,
        do_pacman: true,
        force_sync: false,
        do_aur: true,
        do_cache: false,
        country_idx: initial_country_idx,
        countries,
        mirror_count: prefs.mirror_count,
        cursor: 0,
    };
}

/// Handle optional deps option.
///
/// What: Builds optional dependencies rows and opens `OptionalDeps` modal.
///
/// Inputs:
/// - `app`: Mutable application state
fn handle_optional_deps_option(app: &mut AppState) {
    let rows = menu_options::build_optional_deps_rows(app);
    app.modal = crate::state::Modal::OptionalDeps { rows, selected: 0 };
}

/// Handle click inside config menu.
///
/// What: Opens config files in terminal editors based on clicked row.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
///
/// Details:
fn handle_config_menu_click(_mx: u16, my: u16, app: &mut AppState) -> Option<bool> {
    if let Some((_x, y, _w, _h)) = app.config_menu_rect {
        let row = my.saturating_sub(y) as usize;
        let settings_path = crate::theme::config_dir().join("settings.conf");
        let theme_path = crate::theme::config_dir().join("theme.conf");
        let keybinds_path = crate::theme::config_dir().join("keybinds.conf");

        let target = match row {
            0 => settings_path,
            1 => theme_path,
            2 => keybinds_path,
            _ => {
                app.config_menu_open = false;
                app.artix_filter_menu_open = false;
                return Some(false);
            }
        };

        #[cfg(target_os = "windows")]
        {
            crate::util::open_file(&target);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let path_str = target.display().to_string();
            let editor_cmd = format!(
                "((command -v nvim >/dev/null 2>&1 || sudo pacman -Qi neovim >/dev/null 2>&1) && nvim '{path_str}') || \
                 ((command -v vim >/dev/null 2>&1 || sudo pacman -Qi vim >/dev/null 2>&1) && vim '{path_str}') || \
                 ((command -v hx >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && hx '{path_str}') || \
                 ((command -v helix >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && helix '{path_str}') || \
                 ((command -v emacsclient >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacsclient -t '{path_str}') || \
                 ((command -v emacs >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacs -nw '{path_str}') || \
                 ((command -v nano >/dev/null 2>&1 || sudo pacman -Qi nano >/dev/null 2>&1) && nano '{path_str}') || \
                 (echo 'No terminal editor found (nvim/vim/emacsclient/emacs/hx/helix/nano).'; echo 'File: {path_str}'; read -rn1 -s _ || true)",
            );
            let cmds = vec![editor_cmd];
            std::thread::spawn(move || {
                crate::install::spawn_shell_commands_in_terminal(&cmds);
            });
        }

        app.config_menu_open = false;
        Some(false)
    } else {
        None
    }
}

/// Handle click inside panels menu.
///
/// What: Toggles panel visibility based on clicked row.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
fn handle_panels_menu_click(_mx: u16, my: u16, app: &mut AppState) -> Option<bool> {
    if let Some((_x, y, _w, _h)) = app.panels_menu_rect {
        let row = my.saturating_sub(y) as usize;
        let news_mode = matches!(app.app_mode, crate::state::types::AppMode::News);
        if news_mode {
            match row {
                0 => {
                    app.show_news_history_pane = !app.show_news_history_pane;
                    if !app.show_news_history_pane
                        && matches!(app.focus, crate::state::Focus::Recent)
                    {
                        app.focus = crate::state::Focus::Search;
                    }
                }
                1 => {
                    app.show_news_bookmarks_pane = !app.show_news_bookmarks_pane;
                    if !app.show_news_bookmarks_pane
                        && matches!(app.focus, crate::state::Focus::Install)
                    {
                        app.focus = crate::state::Focus::Search;
                    }
                }
                2 => {
                    app.show_keybinds_footer = !app.show_keybinds_footer;
                    crate::theme::save_show_keybinds_footer(app.show_keybinds_footer);
                }
                _ => return None,
            }
        } else {
            match row {
                0 => {
                    app.show_recent_pane = !app.show_recent_pane;
                    if !app.show_recent_pane && matches!(app.focus, crate::state::Focus::Recent) {
                        app.focus = crate::state::Focus::Search;
                    }
                    crate::theme::save_show_recent_pane(app.show_recent_pane);
                }
                1 => {
                    app.show_install_pane = !app.show_install_pane;
                    if !app.show_install_pane && matches!(app.focus, crate::state::Focus::Install) {
                        app.focus = crate::state::Focus::Search;
                    }
                    crate::theme::save_show_install_pane(app.show_install_pane);
                }
                2 => {
                    app.show_keybinds_footer = !app.show_keybinds_footer;
                    crate::theme::save_show_keybinds_footer(app.show_keybinds_footer);
                }
                _ => return None,
            }
        }
        Some(false)
    } else {
        None
    }
}

/// Handle click inside collapsed menu.
///
/// What: Opens the appropriate menu based on clicked row.
///
/// Inputs:
/// - `mx`: Mouse X coordinate
/// - `my`: Mouse Y coordinate
/// - `app`: Mutable application state
///
/// Output:
/// - `Some(false)` if handled, `None` otherwise
///
/// Details:
/// - Row 0: Opens Config/Lists menu
/// - Row 1: Opens Panels menu
/// - Row 2: Opens Options menu
#[allow(clippy::missing_const_for_fn)]
fn handle_collapsed_menu_click(_mx: u16, my: u16, app: &mut AppState) -> Option<bool> {
    if let Some((_x, y, _w, _h)) = app.collapsed_menu_rect {
        let row = my.saturating_sub(y) as usize;
        app.collapsed_menu_open = false;
        match row {
            0 => {
                app.config_menu_open = true;
                app.options_menu_open = false;
                app.panels_menu_open = false;
                app.artix_filter_menu_open = false;
            }
            1 => {
                app.panels_menu_open = true;
                app.options_menu_open = false;
                app.config_menu_open = false;
                app.artix_filter_menu_open = false;
            }
            2 => {
                app.options_menu_open = true;
                app.panels_menu_open = false;
                app.config_menu_open = false;
                app.artix_filter_menu_open = false;
            }
            _ => return None,
        }
        Some(false)
    } else {
        None
    }
}

/// Close all open menus.
///
/// What: Closes all menus when clicking outside any menu.
///
/// Inputs:
/// - `app`: Mutable application state
#[allow(clippy::missing_const_for_fn)]
fn close_all_menus(app: &mut AppState) {
    if app.sort_menu_open {
        app.sort_menu_open = false;
        app.sort_menu_auto_close_at = None;
    }
    if app.options_menu_open {
        app.options_menu_open = false;
    }
    if app.panels_menu_open {
        app.panels_menu_open = false;
    }
    if app.config_menu_open {
        app.config_menu_open = false;
    }
    if app.collapsed_menu_open {
        app.collapsed_menu_open = false;
    }
}

/// Handle mouse events for menus (sort, options, config, panels, import/export).
///
/// What: Process mouse interactions with menu buttons, dropdown menus, and action buttons
/// in the title bar and Install pane.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state containing menu state and UI rectangles
/// - `details_tx`: Channel to request package details when selection changes
///
/// Output:
/// - `Some(bool)` if the event was handled (consumed by a menu), `None` if not handled.
///   The boolean value indicates whether the application should exit (always `false` here).
///
/// Details:
/// - Import/Export buttons: Import opens `ImportHelp` modal; Export writes install list to timestamped file.
/// - Arch status label: Opens status.archlinux.org URL.
/// - Sort menu: Toggle button opens/closes sort menu; menu items change sort mode and refresh results.
/// - Options menu: Toggle button opens/closes menu; items toggle installed-only mode, open `SystemUpdate`/News,
///   or build `OptionalDeps` modal.
/// - Config menu: Toggle button opens/closes menu; items open config files in terminal editors.
/// - Panels menu: Toggle button opens/closes menu; items toggle Recent/Install panes and keybinds footer.
/// - Menu auto-close: Clicking outside any open menu closes it.
pub(super) fn handle_menus_mouse(
    mx: u16,
    my: u16,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    // Check button clicks first
    if point_in_rect(mx, my, app.updates_button_rect) {
        return Some(handle_updates_button(app));
    }
    if point_in_rect(mx, my, app.install_import_rect) {
        return Some(handle_import_button(app));
    }
    if point_in_rect(mx, my, app.install_export_rect) {
        return Some(handle_export_button(app));
    }
    if point_in_rect(mx, my, app.arch_status_rect) {
        return Some(handle_arch_status());
    }
    if point_in_rect(mx, my, app.sort_button_rect) {
        return Some(handle_sort_button(app));
    }
    if point_in_rect(mx, my, app.options_button_rect) {
        return Some(handle_options_button(app));
    }
    if point_in_rect(mx, my, app.config_button_rect) {
        return Some(handle_config_button(app));
    }
    if point_in_rect(mx, my, app.panels_button_rect) {
        return Some(handle_panels_button(app));
    }
    if point_in_rect(mx, my, app.collapsed_menu_button_rect) {
        return Some(handle_collapsed_menu_button(app));
    }

    // Check menu clicks if menus are open
    if app.sort_menu_open && point_in_rect(mx, my, app.sort_menu_rect) {
        return handle_sort_menu_click(mx, my, app, details_tx);
    }
    if app.options_menu_open && point_in_rect(mx, my, app.options_menu_rect) {
        return handle_options_menu_click(mx, my, app, details_tx);
    }
    if app.config_menu_open && point_in_rect(mx, my, app.config_menu_rect) {
        return handle_config_menu_click(mx, my, app);
    }
    if app.panels_menu_open && point_in_rect(mx, my, app.panels_menu_rect) {
        return handle_panels_menu_click(mx, my, app);
    }
    if app.collapsed_menu_open && point_in_rect(mx, my, app.collapsed_menu_rect) {
        return handle_collapsed_menu_click(mx, my, app);
    }

    // Click outside any menu closes all menus
    close_all_menus(app);
    None
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::state::types::AppMode;
    use crate::util::parse_update_entry;

    fn seed_news_age_translations(app: &mut crate::state::AppState) {
        let mut translations = HashMap::new();
        translations.insert(
            "app.results.options_menu.news_age_all".to_string(),
            "News age: all time".to_string(),
        );
        translations.insert(
            "app.results.options_menu.news_age_days".to_string(),
            "News age: {} days".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    /// What: Test that updates parsing correctly extracts old and new versions.
    ///
    /// Inputs:
    /// - Sample update file content with format `"name - old_version -> name - new_version"`
    ///
    /// Output:
    /// - Verifies that `old_version` and `new_version` are correctly parsed and different
    ///
    /// Details:
    /// - Creates a temporary updates file with known content
    /// - Calls `handle_updates_button` to parse it
    /// - Verifies that the parsed entries have correct old and new versions
    #[test]
    fn test_updates_parsing_extracts_correct_versions() {
        // Test the parsing logic with various formats
        let test_cases = vec![
            (
                "package-a - 1.0.0 -> package-a - 2.0.0",
                "package-a",
                "1.0.0",
                "2.0.0",
            ),
            (
                "package-b - 3.1.0 -> package-b - 3.2.0",
                "package-b",
                "3.1.0",
                "3.2.0",
            ),
            (
                "bat - 0.26.0-1 -> bat - 0.26.0-1",
                "bat",
                "0.26.0-1",
                "0.26.0-1",
            ),
            (
                "comgr - 2:6.4.4-2 -> comgr - 2:6.4.4-2",
                "comgr",
                "2:6.4.4-2",
                "2:6.4.4-2",
            ),
        ];

        for (input, expected_name, expected_old, expected_new) in test_cases {
            let entries: Vec<(String, String, String)> =
                input.lines().filter_map(parse_update_entry).collect();

            assert_eq!(entries.len(), 1, "Failed to parse: {input}");
            let (name, old_version, new_version) = &entries[0];
            assert_eq!(name, expected_name, "Wrong name for: {input}");
            assert_eq!(old_version, expected_old, "Wrong old_version for: {input}");
            assert_eq!(new_version, expected_new, "Wrong new_version for: {input}");
        }
    }

    #[test]
    fn news_age_toggle_sets_toast_and_cycles_value() {
        let mut app = crate::state::AppState::default();
        seed_news_age_translations(&mut app);
        app.news_max_age_days = Some(7);
        app.app_mode = AppMode::News;

        handle_news_age_toggle(&mut app);

        assert_eq!(app.news_max_age_days, Some(30));
        assert!(app.toast_message.as_ref().is_some());
        assert!(app.toast_expires_at.is_some());
    }

    // Removed: News options menu no longer includes a News age entry.
}
