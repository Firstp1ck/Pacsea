//! Menu mouse event handling (sort, options, config, panels, import/export).

use tokio::sync::mpsc;

use crate::events::utils::refresh_selected_details;
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
/// What: Opens the available updates modal with scrollable list.
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
pub(crate) fn handle_updates_button(app: &mut AppState) -> bool {
    let updates_file = crate::theme::lists_dir().join("available_updates.txt");

    // Load updates from file and parse into structured format
    let entries = if updates_file.exists() {
        std::fs::read_to_string(&updates_file)
            .ok()
            .map(|content| {
                content
                    .lines()
                    .filter_map(|line| {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            // Parse format: "name - old_version -> name - new_version"
                            trimmed.find(" -> ").and_then(|arrow_pos| {
                                let before_arrow = trimmed[..arrow_pos].trim();
                                let after_arrow = trimmed[arrow_pos + 4..].trim();

                                // Parse "name - old_version" from before_arrow
                                before_arrow.rfind(" - ").and_then(|dash_pos| {
                                    let name = before_arrow[..dash_pos].trim().to_string();
                                    let old_version =
                                        before_arrow[dash_pos + 3..].trim().to_string();

                                    // Parse "name - new_version" from after_arrow
                                    after_arrow.rfind(" - ").map(|dash_pos| {
                                        let new_version =
                                            after_arrow[dash_pos + 3..].trim().to_string();
                                        (name, old_version, new_version)
                                    })
                                })
                            })
                        }
                    })
                    .collect::<Vec<(String, String, String)>>()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    app.modal = crate::state::Modal::Updates { entries, scroll: 0 };
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
    app.sort_menu_open = !app.sort_menu_open;
    if app.sort_menu_open {
        app.sort_menu_auto_close_at =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
    } else {
        app.sort_menu_auto_close_at = None;
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
fn handle_panels_button(app: &mut AppState) -> bool {
    app.panels_menu_open = !app.panels_menu_open;
    if app.panels_menu_open {
        app.options_menu_open = false;
        app.config_menu_open = false;
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
        app.sort_menu_open = false;
        app.sort_menu_auto_close_at = None;
        crate::logic::sort_results_preserve_selection(app);
        if app.results.is_empty() {
            app.list_state.select(None);
        } else {
            app.selected = 0;
            app.list_state.select(Some(0));
            refresh_selected_details(app, details_tx);
        }
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
        match row {
            0 => handle_installed_only_toggle(app, details_tx),
            1 => handle_system_update_option(app),
            2 => handle_news_option(app),
            3 => handle_optional_deps_option(app),
            _ => return None,
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
        do_aur: true,
        do_cache: false,
        country_idx: initial_country_idx,
        countries,
        mirror_count: prefs.mirror_count,
        cursor: 0,
    };
}

/// Handle news option.
///
/// What: Fetches latest Arch news and opens News modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Details:
/// - Shows alert modal if fetch fails or times out
fn handle_news_option(app: &mut AppState) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let res = match rt {
            Ok(rt) => rt.block_on(crate::sources::fetch_arch_news(10)),
            Err(e) => Err::<Vec<crate::state::NewsItem>, _>(format!("rt: {e}").into()),
        };
        let _ = tx.send(res);
    });
    match rx.recv_timeout(std::time::Duration::from_secs(3)) {
        Ok(Ok(list)) => {
            app.modal = crate::state::Modal::News {
                items: list,
                selected: 0,
            };
        }
        Ok(Err(e)) => {
            app.modal = crate::state::Modal::Alert {
                message: format!("Failed to fetch news: {e}"),
            };
        }
        Err(_) => {
            app.modal = crate::state::Modal::Alert {
                message: "Timed out fetching news".to_string(),
            };
        }
    }
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
/// - Exports installed packages list if row 4 is clicked
fn handle_config_menu_click(_mx: u16, my: u16, app: &mut AppState) -> Option<bool> {
    if let Some((_x, y, _w, _h)) = app.config_menu_rect {
        let row = my.saturating_sub(y) as usize;
        let settings_path = crate::theme::config_dir().join("settings.conf");
        let theme_path = crate::theme::config_dir().join("theme.conf");
        let keybinds_path = crate::theme::config_dir().join("keybinds.conf");
        let install_path = app.install_path.clone();
        let recent_path = app.recent_path.clone();
        let installed_list_path = crate::theme::config_dir().join("installed_packages.txt");
        if row == 4 {
            let mut names: Vec<String> = crate::index::explicit_names().into_iter().collect();
            names.sort();
            let body = names.join("\n");
            let _ = std::fs::write(&installed_list_path, body);
        }

        let target = match row {
            0 => settings_path,
            1 => theme_path,
            2 => keybinds_path,
            3 => install_path,
            4 => installed_list_path,
            5 => recent_path,
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

    // Click outside any menu closes all menus
    close_all_menus(app);
    None
}
