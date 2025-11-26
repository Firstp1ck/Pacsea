//! Global shortcuts and dropdown menu handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::app::{apply_settings_to_app_state, initialize_locale_system};
use crate::events::utils;
use crate::state::{AppState, PackageItem};
use crate::theme::{reload_theme, settings};

/// What: Close all open dropdown menus when ESC is pressed.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if menus were closed, `false` otherwise
///
/// Details:
/// - Closes sort, options, panels, config, and artix filter menus.
#[allow(clippy::missing_const_for_fn)]
fn close_all_dropdowns(app: &mut AppState) -> bool {
    let any_open = app.sort_menu_open
        || app.options_menu_open
        || app.panels_menu_open
        || app.config_menu_open
        || app.artix_filter_menu_open;
    if any_open {
        app.sort_menu_open = false;
        app.sort_menu_auto_close_at = None;
        app.options_menu_open = false;
        app.panels_menu_open = false;
        app.config_menu_open = false;
        app.artix_filter_menu_open = false;
        true
    } else {
        false
    }
}

/// What: Handle installed-only mode toggle from options menu.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Details:
/// - Toggles between showing all packages and only explicitly installed packages.
/// - When enabling, saves installed packages list to config directory.
fn handle_options_installed_only_toggle(
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
        utils::refresh_selected_details(app, details_tx);
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
        utils::refresh_selected_details(app, details_tx);
        let path = crate::theme::config_dir().join("installed_packages.txt");
        let mut names: Vec<String> = crate::index::explicit_names().into_iter().collect();
        names.sort();
        let body = names.join("\n");
        let _ = std::fs::write(path, body);
    }
}

/// What: Handle system update option from options menu.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Details:
/// - Opens `SystemUpdate` modal with default settings.
fn handle_options_system_update(app: &mut AppState) {
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

/// What: Handle news option from options menu.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Details:
/// - Fetches latest Arch news and opens News modal.
/// - Shows alert modal if fetch fails or times out.
fn handle_options_news(app: &mut AppState) {
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

/// What: Handle optional deps option from options menu.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Details:
/// - Builds optional dependencies rows and opens `OptionalDeps` modal.
fn handle_options_optional_deps(app: &mut AppState) {
    let rows = crate::events::mouse::menu_options::build_optional_deps_rows(app);
    app.modal = crate::state::Modal::OptionalDeps { rows, selected: 0 };
}

/// What: Handle panels menu numeric selection.
///
/// Inputs:
/// - `idx`: Selected menu index (0=recent, 1=install, 2=keybinds)
/// - `app`: Mutable application state
///
/// Details:
/// - Toggles visibility of recent pane, install pane, or keybinds footer.
fn handle_panels_menu_selection(idx: usize, app: &mut AppState) {
    match idx {
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
        _ => {}
    }
}

/// What: Normalize `BackTab` modifiers so that `SHIFT` modifier does not affect matching across terminals.
///
/// Inputs:
/// - `ke`: Key event from crossterm
///
/// Output:
/// - Normalized modifiers (empty for `BackTab`, original modifiers otherwise)
///
/// Details:
/// - `BackTab` normalization ensures consistent keybind matching across different terminal emulators.
const fn normalize_key_modifiers(ke: &KeyEvent) -> KeyModifiers {
    if matches!(ke.code, KeyCode::BackTab) {
        KeyModifiers::empty()
    } else {
        ke.modifiers
    }
}

/// What: Create a normalized key chord from a key event for keybind matching.
///
/// Inputs:
/// - `ke`: Key event from crossterm
///
/// Output:
/// - Tuple of (`KeyCode`, `KeyModifiers`) suitable for matching against `KeyChord` lists
///
/// Details:
/// - Normalizes `BackTab` modifiers before creating the chord.
const fn create_key_chord(ke: &KeyEvent) -> (KeyCode, KeyModifiers) {
    (ke.code, normalize_key_modifiers(ke))
}

/// What: Check if a key event matches any chord in a list of keybinds.
///
/// Inputs:
/// - `ke`: Key event from crossterm
/// - `chords`: List of configured key chords to match against
///
/// Output:
/// - `true` if the key event matches any chord in the list, `false` otherwise
///
/// Details:
/// - Normalizes `BackTab` modifiers before matching.
fn matches_keybind(ke: &KeyEvent, chords: &[crate::theme::KeyChord]) -> bool {
    let chord = create_key_chord(ke);
    chords.iter().any(|c| (c.code, c.mods) == chord)
}

/// What: Handle escape key press - closes dropdown menus.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `Some(false)` if menus were closed, `None` otherwise
///
/// Details:
/// - Closes all open dropdown menus when ESC is pressed.
fn handle_escape(app: &mut AppState) -> Option<bool> {
    if close_all_dropdowns(app) {
        Some(false)
    } else {
        None
    }
}

/// What: Handle help overlay keybind.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if help was opened
///
/// Details:
/// - Opens the Help modal when the help overlay keybind is pressed.
fn handle_help_overlay(app: &mut AppState) -> bool {
    app.modal = crate::state::Modal::Help;
    false
}

/// What: Handle configuration reload keybind.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `query_tx`: Channel sender for query input (to refresh results when installed mode changes)
///
/// Output:
/// - `false` if config was reloaded
///
/// Details:
/// - Reloads theme, settings, keybinds, and locale configuration from disk.
/// - Shows a toast message on success or error modal on failure.
/// - Updates app state with new settings and reloads translations if locale changed.
/// - If `installed_packages_mode` changed, refreshes the explicit cache in the background
///   and triggers a query refresh after the cache refresh completes (to avoid race conditions).
fn handle_reload_config(
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<crate::state::QueryInput>,
) -> bool {
    let mut errors = Vec::new();

    // Reload theme
    if let Err(msg) = reload_theme() {
        errors.push(format!("Theme reload failed: {msg}"));
    }

    // Reload settings and keybinds
    let new_settings = settings();
    let old_locale = app.locale.clone();
    let old_installed_mode = app.installed_packages_mode;
    apply_settings_to_app_state(app, &new_settings);

    // Reload locale if it changed
    if new_settings.locale != old_locale {
        initialize_locale_system(app, &new_settings.locale, &new_settings);
    }

    // Refresh explicit cache if installed packages mode changed
    if app.installed_packages_mode != old_installed_mode {
        let new_mode = app.installed_packages_mode;
        tracing::info!(
            "[Config] installed_packages_mode changed from {:?} to {:?}, refreshing cache",
            old_installed_mode,
            new_mode
        );
        // Prepare query input before spawning (to avoid race condition)
        let id = app.next_query_id;
        app.next_query_id += 1;
        app.latest_query_id = id;
        let query_input = crate::state::QueryInput {
            id,
            text: app.input.clone(),
            fuzzy: app.fuzzy_search_enabled,
        };
        // Clone query_tx to send query after cache refresh completes
        let query_tx_clone = query_tx.clone();
        tokio::spawn(async move {
            // Refresh cache first
            crate::index::refresh_explicit_cache(new_mode).await;
            // Then send query to ensure results use the refreshed cache
            let _ = query_tx_clone.send(query_input);
        });
    }

    // Show result
    if errors.is_empty() {
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.config_reloaded"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    } else {
        app.modal = crate::state::Modal::Alert {
            message: errors.join("\n"),
        };
    }
    false
}

/// What: Handle exit keybind.
///
/// Inputs:
/// - None (uses closure pattern)
///
/// Output:
/// - `true` to signal exit
///
/// Details:
/// - Returns exit signal when exit keybind is pressed.
const fn handle_exit() -> bool {
    true
}

/// What: Handle PKGBUILD viewer toggle keybind.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `pkgb_tx`: Channel to request PKGBUILD content
///
/// Output:
/// - `false` if PKGBUILD was toggled
///
/// Details:
/// - Toggles PKGBUILD viewer visibility and requests content if opening.
fn handle_toggle_pkgbuild(
    app: &mut AppState,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if app.pkgb_visible {
        app.pkgb_visible = false;
        app.pkgb_text = None;
        app.pkgb_package_name = None;
        app.pkgb_scroll = 0;
        app.pkgb_rect = None;
    } else {
        app.pkgb_visible = true;
        app.pkgb_text = None;
        app.pkgb_package_name = None;
        if let Some(item) = app.results.get(app.selected).cloned() {
            let _ = pkgb_tx.send(item);
        }
    }
    false
}

/// What: Handle sort mode change keybind.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - `false` if sort mode was changed
///
/// Details:
/// - Cycles through sort modes, persists preference, re-sorts results, and refreshes details.
fn handle_change_sort(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) -> bool {
    // Cycle through sort modes in fixed order
    app.sort_mode = match app.sort_mode {
        crate::state::SortMode::RepoThenName => crate::state::SortMode::AurPopularityThenOfficial,
        crate::state::SortMode::AurPopularityThenOfficial => crate::state::SortMode::BestMatches,
        crate::state::SortMode::BestMatches => crate::state::SortMode::RepoThenName,
    };
    // Persist preference and apply immediately
    crate::theme::save_sort_mode(app.sort_mode);
    crate::logic::sort_results_preserve_selection(app);
    // Jump selection to top and refresh details
    if app.results.is_empty() {
        app.list_state.select(None);
    } else {
        app.selected = 0;
        app.list_state.select(Some(0));
        utils::refresh_selected_details(app, details_tx);
    }
    // Show the dropdown so the user sees the current option with a check mark
    app.sort_menu_open = true;
    app.sort_menu_auto_close_at =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
    false
}

/// What: Handle numeric menu selection for options menu.
///
/// Inputs:
/// - `idx`: Selected menu index (0=installed-only, 1=update, 2=news, 3=optional deps)
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - `Some(false)` if selection was handled, `None` otherwise
///
/// Details:
/// - Routes numeric selection to appropriate options menu handler and closes menu.
fn handle_options_menu_numeric(
    idx: usize,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    match idx {
        0 => handle_options_installed_only_toggle(app, details_tx),
        1 => handle_options_system_update(app),
        2 => handle_options_news(app),
        3 => handle_options_optional_deps(app),
        _ => return None,
    }
    app.options_menu_open = false;
    Some(false)
}

/// What: Handle numeric menu selection for panels menu.
///
/// Inputs:
/// - `idx`: Selected menu index (0=recent, 1=install, 2=keybinds)
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if selection was handled
///
/// Details:
/// - Routes numeric selection to panels menu handler and keeps menu open.
fn handle_panels_menu_numeric(idx: usize, app: &mut AppState) -> bool {
    handle_panels_menu_selection(idx, app);
    // Keep menu open after toggling panels
    false
}

/// What: Handle numeric menu selection for config menu.
///
/// Inputs:
/// - `idx`: Selected menu index (0=settings, 1=theme, 2=keybinds, 3=install, 4=installed, 5=recent)
/// - `app`: Mutable application state
///
/// Output:
/// - `false` if selection was handled
///
/// Details:
/// - Routes numeric selection to config menu handler.
fn handle_config_menu_numeric(idx: usize, app: &mut AppState) -> bool {
    handle_config_menu_selection(idx, app);
    false
}

/// What: Handle numeric key press when dropdown menus are open.
///
/// Inputs:
/// - `ch`: Character pressed (must be '1'-'9')
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - `Some(false)` if a menu selection was handled, `None` otherwise
///
/// Details:
/// - Routes numeric keys to the appropriate open menu handler.
fn handle_menu_numeric_selection(
    ch: char,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    let idx = (ch as u8 - b'1') as usize; // '1' -> 0
    if app.options_menu_open {
        handle_options_menu_numeric(idx, app, details_tx)
    } else if app.panels_menu_open {
        Some(handle_panels_menu_numeric(idx, app))
    } else if app.config_menu_open {
        Some(handle_config_menu_numeric(idx, app))
    } else {
        None
    }
}

/// What: Handle global keybinds (help, theme reload, exit, PKGBUILD, sort).
///
/// Inputs:
/// - `ke`: Key event from crossterm
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
/// - `pkgb_tx`: Channel to request PKGBUILD content
/// - `query_tx`: Channel to send search queries
///
/// Output:
/// - `Some(true)` for exit, `Some(false)` if handled, `None` if not matched
///
/// Details:
/// - Checks key event against all global keybinds using a dispatch pattern.
fn handle_global_keybinds(
    ke: &KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
    query_tx: &mpsc::UnboundedSender<crate::state::QueryInput>,
) -> Option<bool> {
    let km = &app.keymap;

    // Help overlay (only if no modal is active, except Preflight which handles its own help)
    if !matches!(app.modal, crate::state::Modal::Preflight { .. })
        && matches_keybind(ke, &km.help_overlay)
    {
        return Some(handle_help_overlay(app));
    }

    // Configuration reload
    if matches_keybind(ke, &km.reload_config) {
        return Some(handle_reload_config(app, query_tx));
    }

    // Exit
    if matches_keybind(ke, &km.exit) {
        return Some(handle_exit());
    }

    // PKGBUILD toggle
    if matches_keybind(ke, &km.show_pkgbuild) {
        return Some(handle_toggle_pkgbuild(app, pkgb_tx));
    }

    // Sort change
    if matches_keybind(ke, &km.change_sort) {
        return Some(handle_change_sort(app, details_tx));
    }

    None
}

/// What: Handle config menu numeric selection.
///
/// Inputs:
/// - `idx`: Selected menu index (0=settings, 1=theme, 2=keybinds)
/// - `app`: Mutable application state
///
/// Details:
/// - Opens the selected config file in a terminal editor.
fn handle_config_menu_selection(idx: usize, app: &mut AppState) {
    let settings_path = crate::theme::config_dir().join("settings.conf");
    let theme_path = crate::theme::config_dir().join("theme.conf");
    let keybinds_path = crate::theme::config_dir().join("keybinds.conf");
    let target = match idx {
        0 => settings_path,
        1 => theme_path,
        2 => keybinds_path,
        _ => {
            app.config_menu_open = false;
            app.artix_filter_menu_open = false;
            return;
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
            "((command -v nvim >/dev/null 2>&1 || sudo pacman -Qi neovim >/dev/null 2>&1) && nvim '{path_str}') || \\
             ((command -v vim >/dev/null 2>&1 || sudo pacman -Qi vim >/dev/null 2>&1) && vim '{path_str}') || \\
             ((command -v hx >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && hx '{path_str}') || \\
             ((command -v helix >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && helix '{path_str}') || \\
             ((command -v emacsclient >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacsclient -t '{path_str}') || \\
             ((command -v emacs >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacs -nw '{path_str}') || \\
             ((command -v nano >/dev/null 2>&1 || sudo pacman -Qi nano >/dev/null 2>&1) && nano '{path_str}') || \\
             (echo 'No terminal editor found (nvim/vim/emacsclient/emacs/hx/helix/nano).'; echo 'File: {path_str}'; read -rn1 -s _ || true)",
        );
        let cmds = vec![editor_cmd];
        std::thread::spawn(move || {
            crate::install::spawn_shell_commands_in_terminal(&cmds);
        });
    }
    app.config_menu_open = false;
    app.artix_filter_menu_open = false;
}

/// What: Handle global shortcuts plus dropdown menus and optionally stop propagation.
///
/// Inputs:
/// - `ke`: Key event received from crossterm (code + modifiers)
/// - `app`: Mutable application state shared across panes and modals
/// - `details_tx`: Channel used to request package detail refreshes
/// - `pkgb_tx`: Channel used to request PKGBUILD content for the focused result
/// - `query_tx`: Channel to send search queries
///
/// Output:
/// - `Some(true)` when the caller should exit (e.g., global exit keybind triggered)
/// - `Some(false)` when a global keybind was handled (key should not be processed further)
/// - `None` when the key was not handled by global shortcuts
///
/// Details:
/// - Gives precedence to closing dropdown menus on `Esc` before other bindings.
/// - Routes configured global chords (help overlay, theme reload, exit, PKGBUILD toggle, sort cycle).
/// - When sort mode changes it persists the preference, re-sorts results, and refreshes details.
/// - Supports menu number shortcuts (1-9) for Options/Panels/Config dropdowns while they are open.
pub(super) fn handle_global_key(
    ke: KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
    query_tx: &mpsc::UnboundedSender<crate::state::QueryInput>,
) -> Option<bool> {
    // First: handle ESC to close dropdown menus
    if ke.code == KeyCode::Esc
        && let Some(result) = handle_escape(app)
    {
        return Some(result);
    }

    // Second: handle global keybinds (help, theme reload, exit, PKGBUILD, sort)
    if let Some(result) = handle_global_keybinds(&ke, app, details_tx, pkgb_tx, query_tx) {
        return Some(result);
    }

    // Third: handle numeric menu selection when dropdowns are open
    // Note: menu toggles (Shift+C/O/P) handled in Search Normal mode and not globally
    if let KeyCode::Char(ch) = ke.code
        && ch.is_ascii_digit()
        && ch != '0'
        && let Some(result) = handle_menu_numeric_selection(ch, app, details_tx)
    {
        return Some(result);
    }

    None // Key not handled by global shortcuts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Confirm pressing `Esc` while dropdowns are open closes them without exiting.
    ///
    /// Inputs:
    /// - App state with Options and Sort menus flagged open.
    /// - Synthetic `Esc` key event.
    ///
    /// Output:
    /// - Handler returns `false` and menu flags reset to `false`.
    ///
    /// Details:
    /// - Ensures the early escape branch short-circuits before other global shortcuts.
    fn global_escape_closes_dropdowns() {
        let mut app = new_app();
        app.sort_menu_open = true;
        app.options_menu_open = true;
        app.panels_menu_open = true;
        app.config_menu_open = true;

        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (query_tx, _query_rx) = mpsc::unbounded_channel::<crate::state::QueryInput>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &mut app,
            &details_tx,
            &pkgb_tx,
            &query_tx,
        );

        assert_eq!(exit, Some(false));
        assert!(!app.sort_menu_open);
        assert!(!app.options_menu_open);
        assert!(!app.panels_menu_open);
        assert!(!app.config_menu_open);
    }

    #[test]
    /// What: Verify the help overlay shortcut activates the Help modal.
    ///
    /// Inputs:
    /// - Default keymap (F1 assigned to help overlay).
    /// - `F1` key event with no modifiers.
    ///
    /// Output:
    /// - Handler returns `false` and sets `app.modal` to `Modal::Help`.
    ///
    /// Details:
    /// - Confirms `BackTab` normalization does not interfere with regular function keys.
    fn global_help_overlay_opens_modal() {
        let mut app = new_app();
        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (query_tx, _query_rx) = mpsc::unbounded_channel::<crate::state::QueryInput>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()),
            &mut app,
            &details_tx,
            &pkgb_tx,
            &query_tx,
        );

        assert_eq!(exit, Some(false));
        assert!(matches!(app.modal, crate::state::Modal::Help));
    }

    #[test]
    /// What: Ensure the PKGBUILD toggle opens the viewer and requests content.
    ///
    /// Inputs:
    /// - App state with a single selected result.
    /// - `Ctrl+X` key event matching the default `show_pkgbuild` chord.
    ///
    /// Output:
    /// - Handler returns `false`, sets `pkgb_visible`, and sends the selected item through `pkgb_tx`.
    ///
    /// Details:
    /// - Provides regression coverage for the channel send branch when the viewer becomes visible.
    fn global_show_pkgbuild_requests_content() {
        let mut app = new_app();
        app.results = vec![PackageItem {
            name: "ripgrep".into(),
            version: "14.0".into(),
            description: "fast search".into(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.selected = 0;

        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, mut pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (query_tx, _query_rx) = mpsc::unbounded_channel::<crate::state::QueryInput>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            &mut app,
            &details_tx,
            &pkgb_tx,
            &query_tx,
        );

        assert_eq!(exit, Some(false));
        assert!(app.pkgb_visible);
        let sent = pkgb_rx.try_recv().expect("pkgb request dispatched");
        assert_eq!(sent.name, "ripgrep");
    }

    #[test]
    /// What: Validate the exit key chord signals the application loop to terminate.
    ///
    /// Inputs:
    /// - Default keymap with `Ctrl+C` bound to exit.
    /// - `Ctrl+C` key event routed through the handler.
    ///
    /// Output:
    /// - Handler returns `true`, indicating the caller should stop processing events.
    ///
    /// Details:
    /// - Provides regression coverage so global exit handling keeps matching the configured chord.
    fn global_exit_chord_requests_shutdown() {
        let mut app = new_app();
        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (query_tx, _query_rx) = mpsc::unbounded_channel::<crate::state::QueryInput>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut app,
            &details_tx,
            &pkgb_tx,
            &query_tx,
        );

        assert_eq!(exit, Some(true));
    }
}
