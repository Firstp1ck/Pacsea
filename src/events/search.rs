use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::logic::{move_sel_cached, send_query};
use crate::state::{AppState, PackageItem, QueryInput};

use super::utils::{byte_index_for_char, char_count, refresh_install_details};

/// What: Handle key events while the Search pane is focused.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state (input, selection, sort, modes)
/// - `query_tx`: Channel to send debounced search queries
/// - `details_tx`: Channel to request details for the focused item
/// - `add_tx`: Channel to add items to the Install/Remove lists
/// - `preview_tx`: Channel to request preview details when moving focus
///
/// Output:
/// - `true` to request application exit (e.g., Ctrl+C); `false` to continue processing.
///
/// Details:
/// - Insert mode (default): typing edits the query, triggers debounced network/idx search, and
///   moves caret; Backspace edits; Space adds to list (Install by default, Remove in installed-only).
/// - Normal mode: toggled via configured chord; supports selection (h/l), deletion (d), navigation
///   (j/k, Ctrl+U/D), and list add/remove with Space/ Ctrl+Space (downgrade).
/// - Pane navigation: Left/Right and configured `pane_next` cycle focus across panes and subpanes,
///   differing slightly when installed-only mode is active.
/// - PKGBUILD reload is handled via debounced requests scheduled in the selection logic.
pub fn handle_search_key(
    ke: KeyEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    let km = &app.keymap;
    // Match helper that treats Shift+<char> from config as equivalent to uppercase char without Shift from terminal
    let matches_any = |list: &Vec<crate::theme::KeyChord>| {
        list.iter().any(|c| {
            if (c.code, c.mods) == (ke.code, ke.modifiers) {
                return true;
            }
            match (c.code, ke.code) {
                (
                    crossterm::event::KeyCode::Char(cfg_ch),
                    crossterm::event::KeyCode::Char(ev_ch),
                ) => {
                    let cfg_has_shift = c.mods.contains(crossterm::event::KeyModifiers::SHIFT);
                    if !cfg_has_shift {
                        return false;
                    }
                    // Accept uppercase event regardless of SHIFT flag
                    if ev_ch == cfg_ch.to_ascii_uppercase() {
                        return true;
                    }
                    // Accept lowercase char if terminal reports SHIFT in modifiers
                    if ke.modifiers.contains(crossterm::event::KeyModifiers::SHIFT)
                        && ev_ch.to_ascii_lowercase() == cfg_ch
                    {
                        return true;
                    }
                    false
                }
                _ => false,
            }
        })
    };

    // Toggle Normal mode (configurable)
    if matches_any(&km.search_normal_toggle) {
        app.search_normal_mode = !app.search_normal_mode;
        return false;
    }

    // Normal mode: Vim-like navigation without editing input
    if app.search_normal_mode {
        // If any dropdown is open, allow numeric selection 1..9 here as a fallback
        if let KeyCode::Char(ch) = ke.code
            && ch.is_ascii_digit()
            && ch != '0'
        {
            let idx = (ch as u8 - b'1') as usize;
            // Config/Lists menu numeric selection (rows 0..5)
            if app.config_menu_open {
                let settings_path = crate::theme::config_dir().join("settings.conf");
                let theme_path = crate::theme::config_dir().join("theme.conf");
                let keybinds_path = crate::theme::config_dir().join("keybinds.conf");
                let install_path = app.install_path.clone();
                let recent_path = app.recent_path.clone();
                let installed_list_path = crate::theme::config_dir().join("installed_packages.txt");
                if idx == 4 {
                    let mut names: Vec<String> =
                        crate::index::explicit_names().into_iter().collect();
                    names.sort();
                    let body = names.join("\n");
                    let _ = std::fs::write(&installed_list_path, body);
                }
                let target = match idx {
                    0 => settings_path,
                    1 => theme_path,
                    2 => keybinds_path,
                    3 => install_path,
                    4 => installed_list_path,
                    5 => recent_path,
                    _ => {
                        app.config_menu_open = false;
                        return false;
                    }
                };
                let path_str = target.display().to_string();
                let editor_cmd = format!(
                    "((command -v nvim >/dev/null 2>&1 || sudo pacman -Qi neovim >/dev/null 2>&1) && nvim '{path_str}') || \\\n                         ((command -v vim >/dev/null 2>&1 || sudo pacman -Qi vim >/dev/null 2>&1) && vim '{path_str}') || \\\n                         ((command -v hx >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && hx '{path_str}') || \\\n                         ((command -v helix >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && helix '{path_str}') || \\\n                         ((command -v emacsclient >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacsclient -t '{path_str}') || \\\n                         ((command -v emacs >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacs -nw '{path_str}') || \\\n                         ((command -v nano >/dev/null 2>&1 || sudo pacman -Qi nano >/dev/null 2>&1) && nano '{path_str}') || \\\n                         (echo 'No terminal editor found (nvim/vim/emacsclient/emacs/hx/helix/nano).'; echo 'File: {path_str}'; read -rn1 -s _ || true)"
                );
                let cmds = vec![editor_cmd];
                std::thread::spawn(move || {
                    crate::install::spawn_shell_commands_in_terminal(&cmds);
                });
                app.config_menu_open = false;
                return false;
            }
        }

        match (ke.code, ke.modifiers) {
            // Menu toggles in Normal mode
            (c, m) if matches_any(&km.config_menu_toggle) && (c, m) == (ke.code, ke.modifiers) => {
                app.config_menu_open = !app.config_menu_open;
                if app.config_menu_open {
                    app.options_menu_open = false;
                    app.panels_menu_open = false;
                    app.sort_menu_open = false;
                    app.sort_menu_auto_close_at = None;
                }
            }
            (c, m) if matches_any(&km.options_menu_toggle) && (c, m) == (ke.code, ke.modifiers) => {
                app.options_menu_open = !app.options_menu_open;
                if app.options_menu_open {
                    app.config_menu_open = false;
                    app.panels_menu_open = false;
                    app.sort_menu_open = false;
                    app.sort_menu_auto_close_at = None;
                }
            }
            (c, m) if matches_any(&km.panels_menu_toggle) && (c, m) == (ke.code, ke.modifiers) => {
                app.panels_menu_open = !app.panels_menu_open;
                if app.panels_menu_open {
                    app.config_menu_open = false;
                    app.options_menu_open = false;
                    app.sort_menu_open = false;
                    app.sort_menu_auto_close_at = None;
                }
            }
            // Open Arch status page in default browser
            (c, m)
                if matches_any(&km.search_normal_open_status)
                    && (c, m) == (ke.code, ke.modifiers) =>
            {
                crate::util::open_url("https://status.archlinux.org");
            }
            // Normal mode: Import (Shift+I)
            (c, m)
                if matches_any(&km.search_normal_import) && (c, m) == (ke.code, ke.modifiers) =>
            {
                // Disabled while in installed-only mode to match UI (buttons hidden)
                if !app.installed_only_mode {
                    // Show ImportHelp modal first
                    app.modal = crate::state::Modal::ImportHelp;
                }
                return false;
            }
            // Normal mode: Export (Shift+E)
            (c, m)
                if matches_any(&km.search_normal_export) && (c, m) == (ke.code, ke.modifiers) =>
            {
                // Disabled while in installed-only mode to match UI (buttons hidden)
                if !app.installed_only_mode {
                    // Export current Install List package names to config export dir
                    let mut names: Vec<String> =
                        app.install_list.iter().map(|p| p.name.clone()).collect();
                    names.sort();
                    if names.is_empty() {
                        app.toast_message = Some("Install List is empty".to_string());
                        app.toast_expires_at =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                    } else {
                        // Build export directory and file name install_list_YYYYMMDD_serial
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
                                break export_dir
                                    .join(format!("install_list_{date_str}_fallback.txt"));
                            }
                        };
                        let body = names.join("\n");
                        match std::fs::write(&file_path, body) {
                            Ok(_) => {
                                app.toast_message =
                                    Some(format!("Exported to {}", file_path.display()));
                                app.toast_expires_at = Some(
                                    std::time::Instant::now() + std::time::Duration::from_secs(4),
                                );
                                tracing::info!(path = %file_path.display().to_string(), count = names.len(), "export: wrote install list");
                            }
                            Err(e) => {
                                app.toast_message = Some(format!("Export failed: {e}"));
                                app.toast_expires_at = Some(
                                    std::time::Instant::now() + std::time::Duration::from_secs(5),
                                );
                                tracing::error!(error = %e, path = %file_path.display().to_string(), "export: failed to write install list");
                            }
                        }
                    }
                }
            }
            (c, m)
                if matches_any(&km.search_normal_insert) && (c, m) == (ke.code, ke.modifiers) =>
            {
                // return to insert mode
                app.search_normal_mode = false;
                app.search_select_anchor = None;
            }
            // Selection with configured left/right (default: h/l)
            (c, m)
                if matches_any(&km.search_normal_select_left)
                    && (c, m) == (ke.code, ke.modifiers) =>
            {
                // Begin selection if not started
                if app.search_select_anchor.is_none() {
                    app.search_select_anchor = Some(app.search_caret);
                }
                let cc = char_count(&app.input);
                let cur = app.search_caret as isize - 1;
                let new_ci = if cur < 0 { 0 } else { cur as usize };
                app.search_caret = new_ci.min(cc);
            }
            (c, m)
                if matches_any(&km.search_normal_select_right)
                    && (c, m) == (ke.code, ke.modifiers) =>
            {
                if app.search_select_anchor.is_none() {
                    app.search_select_anchor = Some(app.search_caret);
                }
                let cc = char_count(&app.input);
                let cur = app.search_caret + 1;
                app.search_caret = cur.min(cc);
            }
            // Delete selected range (default: d)
            (c, m)
                if matches_any(&km.search_normal_delete) && (c, m) == (ke.code, ke.modifiers) =>
            {
                if let Some(anchor) = app.search_select_anchor.take() {
                    let a = anchor.min(app.search_caret);
                    let b = anchor.max(app.search_caret);
                    if a != b {
                        let bs = byte_index_for_char(&app.input, a);
                        let be = byte_index_for_char(&app.input, b);
                        let mut new_input = String::with_capacity(app.input.len());
                        new_input.push_str(&app.input[..bs]);
                        new_input.push_str(&app.input[be..]);
                        app.input = new_input;
                        app.search_caret = a;
                        app.last_input_change = std::time::Instant::now();
                        app.last_saved_value = None;
                        send_query(app, query_tx);
                    }
                }
            }
            // Clear entire input (default: Shift+Del)
            (c, m) if matches_any(&km.search_normal_clear) && (c, m) == (ke.code, ke.modifiers) => {
                if !app.input.is_empty() {
                    app.input.clear();
                    app.search_caret = 0;
                    app.search_select_anchor = None;
                    app.last_input_change = std::time::Instant::now();
                    app.last_saved_value = None;
                    send_query(app, query_tx);
                }
            }
            (KeyCode::Char('j'), _) => move_sel_cached(app, 1, details_tx),
            (KeyCode::Char('k'), _) => move_sel_cached(app, -1, details_tx),
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => move_sel_cached(app, 10, details_tx),
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => move_sel_cached(app, -10, details_tx),
            (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
                if app.installed_only_mode
                    && let Some(item) = app.results.get(app.selected).cloned()
                {
                    crate::logic::add_to_downgrade_list(app, item);
                    // Do not change focus; only update details to reflect the new selection
                    super::utils::refresh_downgrade_details(app, details_tx);
                }
            }
            (KeyCode::Char(' '), _) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    if app.installed_only_mode {
                        crate::logic::add_to_remove_list(app, item);
                        super::utils::refresh_remove_details(app, details_tx);
                    } else {
                        let _ = add_tx.send(item);
                    }
                }
            }
            // Open Preflight (or bypass if skip_preflight) using configured search_install key (default: Enter)
            (c, m) if matches_any(&km.search_install) && (c, m) == (ke.code, ke.modifiers) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    if crate::theme::settings().skip_preflight {
                        // Direct install of single item
                        crate::install::spawn_install_all(std::slice::from_ref(&item), app.dry_run);
                        app.toast_message = Some("Installing (preflight skipped)".to_string());
                    } else {
                        app.modal = crate::state::Modal::Preflight {
                            items: vec![item],
                            action: crate::state::PreflightAction::Install,
                            tab: crate::state::PreflightTab::Summary,
                            dependency_info: Vec::new(),
                            dep_selected: 0,
                            dep_tree_expanded: std::collections::HashSet::new(),
                            file_info: Vec::new(),
                            file_selected: 0,
                            file_tree_expanded: std::collections::HashSet::new(),
                            cascade_mode: app.remove_cascade_mode,
                        };
                        app.toast_message = Some("Preflight opened".to_string());
                    }
                }
            }
            // Fallback on raw Enter
            (KeyCode::Char('\n') | KeyCode::Enter, _) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    if crate::theme::settings().skip_preflight {
                        crate::install::spawn_install_all(std::slice::from_ref(&item), app.dry_run);
                        app.toast_message = Some("Installing (preflight skipped)".to_string());
                    } else {
                        app.modal = crate::state::Modal::Preflight {
                            items: vec![item],
                            action: crate::state::PreflightAction::Install,
                            tab: crate::state::PreflightTab::Summary,
                            dependency_info: Vec::new(),
                            dep_selected: 0,
                            dep_tree_expanded: std::collections::HashSet::new(),
                            file_info: Vec::new(),
                            file_selected: 0,
                            file_tree_expanded: std::collections::HashSet::new(),
                            cascade_mode: app.remove_cascade_mode,
                        };
                        app.toast_message = Some("Preflight opened".to_string());
                    }
                }
            }
            (c, m) if matches_any(&km.pane_next) && (c, m) == (ke.code, ke.modifiers) => {
                // Desired cycle: Recent -> Search -> Downgrade -> Remove -> Recent
                if app.installed_only_mode {
                    // From Search move to Downgrade first
                    app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                    if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                        app.downgrade_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    super::utils::refresh_downgrade_details(app, details_tx);
                } else {
                    if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                        app.install_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    refresh_install_details(app, details_tx);
                }
            }
            (KeyCode::Right, _) => {
                // Search -> Install (adjacent)
                if app.installed_only_mode {
                    // Target Downgrade first in installed-only mode
                    app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                    if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                        app.downgrade_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    super::utils::refresh_downgrade_details(app, details_tx);
                } else {
                    if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                        app.install_state.select(Some(0));
                    }
                    app.focus = crate::state::Focus::Install;
                    refresh_install_details(app, details_tx);
                }
            }
            (KeyCode::Left, _) => {
                if app.history_state.selected().is_none() && !app.recent.is_empty() {
                    app.history_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Recent;
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
            }
            _ => {}
        }
        return false;
    }

    // Insert mode (default for Search)
    match (ke.code, ke.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
        (c, m) if matches_any(&km.pane_next) && (c, m) == (ke.code, ke.modifiers) => {
            // Desired cycle: Recent -> Search -> Downgrade -> Remove -> Recent
            if app.installed_only_mode {
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                    app.downgrade_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::utils::refresh_downgrade_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                refresh_install_details(app, details_tx);
            }
        }
        (KeyCode::Right, _) => {
            // Search -> Install (adjacent)
            if app.installed_only_mode {
                // Always target Downgrade first from Search
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                    app.downgrade_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::utils::refresh_downgrade_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                refresh_install_details(app, details_tx);
            }
        }
        (KeyCode::Left, _) => {
            // Search -> Recent (adjacent)
            if app.history_state.selected().is_none() && !app.recent.is_empty() {
                app.history_state.select(Some(0));
            }
            app.focus = crate::state::Focus::Recent;
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
            if app.installed_only_mode
                && let Some(item) = app.results.get(app.selected).cloned()
            {
                crate::logic::add_to_downgrade_list(app, item);
                // Do not change focus; only update details to reflect the new selection
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        (KeyCode::Char(' '), _) => {
            if let Some(item) = app.results.get(app.selected).cloned() {
                if app.installed_only_mode {
                    crate::logic::add_to_remove_list(app, item);
                    super::utils::refresh_remove_details(app, details_tx);
                } else {
                    let _ = add_tx.send(item);
                }
            }
        }
        (KeyCode::Backspace, _) => {
            app.input.pop();
            app.last_input_change = std::time::Instant::now();
            app.last_saved_value = None;
            // Move caret to end and clear selection in insert mode
            app.search_caret = char_count(&app.input);
            app.search_select_anchor = None;
            send_query(app, query_tx);
        }
        (KeyCode::Char('\n') | KeyCode::Enter, _) => {
            if let Some(item) = app.results.get(app.selected).cloned() {
                if crate::theme::settings().skip_preflight {
                    crate::install::spawn_install_all(std::slice::from_ref(&item), app.dry_run);
                    app.toast_message = Some("Installing (preflight skipped)".to_string());
                } else {
                    app.modal = crate::state::Modal::Preflight {
                        items: vec![item],
                        action: crate::state::PreflightAction::Install,
                        tab: crate::state::PreflightTab::Summary,
                        dependency_info: Vec::new(),
                        dep_selected: 0,
                        dep_tree_expanded: std::collections::HashSet::new(),
                        file_info: Vec::new(),
                        file_selected: 0,
                        file_tree_expanded: std::collections::HashSet::new(),
                        cascade_mode: app.remove_cascade_mode,
                    };
                    app.toast_message = Some("Preflight opened".to_string());
                }
            }
        }
        (KeyCode::Char(ch), _) => {
            app.input.push(ch);
            app.last_input_change = std::time::Instant::now();
            app.last_saved_value = None;
            app.search_caret = char_count(&app.input);
            app.search_select_anchor = None;
            send_query(app, query_tx);
        }
        (KeyCode::Up, _) => move_sel_cached(app, -1, details_tx),
        (KeyCode::Down, _) => move_sel_cached(app, 1, details_tx),
        (KeyCode::PageUp, _) => move_sel_cached(app, -10, details_tx),
        (KeyCode::PageDown, _) => move_sel_cached(app, 10, details_tx),
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_app() -> AppState {
        AppState {
            ..Default::default()
        }
    }

    #[test]
    /// What: Insert mode typing updates input, caret, and sends query; Backspace updates too
    ///
    /// - Input: 'r','g', Backspace
    /// - Output: input transitions "r"->"rg"->"r"; query messages sent
    fn search_insert_typing_and_backspace() {
        let mut app = new_app();
        let (qtx, mut qrx) = mpsc::unbounded_channel::<QueryInput>();
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

        let _ = handle_search_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &atx,
            &ptx,
        );
        let _ = handle_search_key(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &atx,
            &ptx,
        );
        let _ = handle_search_key(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &atx,
            &ptx,
        );
        assert_eq!(app.input, "r");
        // At least one query should have been sent
        assert!(qrx.try_recv().ok().is_some());
    }

    #[test]
    /// What: Normal mode selection commands set anchor and adjust caret
    ///
    /// - Input: Toggle normal mode, press select-right then select-left
    /// - Output: Anchor Some, caret stays within bounds
    fn search_normal_mode_selection() {
        let mut app = new_app();
        app.input = "rip".into();
        let (qtx, _qrx) = mpsc::unbounded_channel::<QueryInput>();
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();

        // Toggle into normal mode (Esc by default per KeyMap)
        let _ = handle_search_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &atx,
            &ptx,
        );
        // Select right (default 'l')
        let _ = handle_search_key(
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &atx,
            &ptx,
        );
        assert!(app.search_select_anchor.is_some());
        // Select left (default 'h')
        let _ = handle_search_key(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()),
            &mut app,
            &qtx,
            &dtx,
            &atx,
            &ptx,
        );
        assert!(app.search_caret <= crate::events::utils::char_count(&app.input));
    }
}
