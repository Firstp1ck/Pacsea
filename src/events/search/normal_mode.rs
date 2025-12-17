use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::logic::{move_sel_cached, send_query};
use crate::state::{AppState, PackageItem, QueryInput};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::super::utils::matches_any;
use super::helpers::navigate_pane;
use super::preflight_helpers::open_preflight_modal;
use crate::events::utils::{byte_index_for_char, char_count, refresh_install_details};

/// What: Handle numeric selection (1-9) for config menu items.
///
/// Inputs:
/// - `idx`: Numeric index (0-8) from key press
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if handled, `false` otherwise
///
/// Details:
/// - Opens the selected config file in a terminal editor.
/// - Handles settings, theme, and keybinds.
fn handle_config_menu_numeric_selection(idx: usize, app: &mut AppState) -> bool {
    if !app.config_menu_open {
        return false;
    }

    let settings_path = crate::theme::config_dir().join("settings.conf");
    let theme_path = crate::theme::config_dir().join("theme.conf");
    let keybinds_path = crate::theme::config_dir().join("keybinds.conf");

    let target = match idx {
        0 => settings_path,
        1 => theme_path,
        2 => keybinds_path,
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
    true
}

/// What: Handle menu toggle key events.
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if a menu toggle was handled, `false` otherwise
///
/// Details:
/// - Toggles config, options, or panels menu.
/// - Ensures only one menu is open at a time.
pub fn handle_menu_toggles(ke: &KeyEvent, app: &mut AppState) -> bool {
    let km = &app.keymap;

    if matches_any(ke, &km.config_menu_toggle) {
        app.config_menu_open = !app.config_menu_open;
        if app.config_menu_open {
            app.options_menu_open = false;
            app.panels_menu_open = false;
            app.sort_menu_open = false;
            app.sort_menu_auto_close_at = None;
        }
        return true;
    }

    if matches_any(ke, &km.options_menu_toggle) {
        app.options_menu_open = !app.options_menu_open;
        if app.options_menu_open {
            app.config_menu_open = false;
            app.panels_menu_open = false;
            app.sort_menu_open = false;
            app.sort_menu_auto_close_at = None;
        }
        return true;
    }

    if matches_any(ke, &km.panels_menu_toggle) {
        app.panels_menu_open = !app.panels_menu_open;
        if app.panels_menu_open {
            app.config_menu_open = false;
            app.options_menu_open = false;
            app.sort_menu_open = false;
            app.sort_menu_auto_close_at = None;
        }
        return true;
    }

    false
}

/// What: Handle export of install list to file.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - Exports current Install List package names to config export dir.
/// - Creates files with format: `install_list_YYYYMMDD_serial.txt`
/// - Shows toast messages for success or failure.
pub fn handle_export(app: &mut AppState) {
    if app.installed_only_mode {
        return;
    }

    let mut names: Vec<String> = app.install_list.iter().map(|p| p.name.clone()).collect();
    names.sort();

    if names.is_empty() {
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.install_list_empty"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        return;
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
}

/// What: Handle text selection movement (left/right).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `direction`: Direction to move (-1 for left, 1 for right)
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - Begins selection if not already started.
/// - Moves caret in the specified direction within input bounds.
fn handle_selection_move(app: &mut AppState, direction: isize) {
    if app.search_select_anchor.is_none() {
        app.search_select_anchor = Some(app.search_caret);
    }

    let cc = char_count(&app.input);
    let cur = isize::try_from(app.search_caret).unwrap_or(0) + direction;
    let new_ci = if cur < 0 {
        0
    } else {
        usize::try_from(cur).unwrap_or(0)
    };
    app.search_caret = new_ci.min(cc);
}

/// What: Handle deletion of selected text range.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send debounced search queries
///
/// Output:
/// - `true` if deletion occurred, `false` otherwise
///
/// Details:
/// - Deletes the selected range between anchor and caret.
/// - Updates input and triggers query refresh.
fn handle_selection_delete(
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
) -> bool {
    let Some(anchor) = app.search_select_anchor.take() else {
        return false;
    };

    let a = anchor.min(app.search_caret);
    let b = anchor.max(app.search_caret);
    if a == b {
        return false;
    }

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
    true
}

/// What: Handle navigation key events (j/k, arrow keys, Ctrl+D/U).
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request details for the focused item
///
/// Output:
/// - `true` if navigation was handled, `false` otherwise
///
/// Details:
/// - Handles vim-like navigation: j/k for single line, arrow keys from keymap, Ctrl+D/U for page movement.
fn handle_navigation(
    ke: &KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
) -> bool {
    let km = &app.keymap;

    // Check keymap-based arrow keys first (works same in normal and insert mode)
    if matches_any(ke, &km.search_move_up) {
        if matches!(app.app_mode, crate::state::types::AppMode::News) {
            crate::events::utils::move_news_selection(app, -1);
        } else {
            move_sel_cached(app, -1, details_tx, comments_tx);
        }
        return true;
    }
    if matches_any(ke, &km.search_move_down) {
        if matches!(app.app_mode, crate::state::types::AppMode::News) {
            crate::events::utils::move_news_selection(app, 1);
        } else {
            move_sel_cached(app, 1, details_tx, comments_tx);
        }
        return true;
    }
    if matches_any(ke, &km.search_page_up) {
        if matches!(app.app_mode, crate::state::types::AppMode::News) {
            crate::events::utils::move_news_selection(app, -10);
        } else {
            move_sel_cached(app, -10, details_tx, comments_tx);
        }
        return true;
    }
    if matches_any(ke, &km.search_page_down) {
        if matches!(app.app_mode, crate::state::types::AppMode::News) {
            crate::events::utils::move_news_selection(app, 10);
        } else {
            move_sel_cached(app, 10, details_tx, comments_tx);
        }
        return true;
    }

    // Vim-like navigation (j/k, Ctrl+D/U)
    match (ke.code, ke.modifiers) {
        (KeyCode::Char('j'), _) => {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                crate::events::utils::move_news_selection(app, 1);
            } else {
                move_sel_cached(app, 1, details_tx, comments_tx);
            }
            true
        }
        (KeyCode::Char('k'), _) => {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                crate::events::utils::move_news_selection(app, -1);
            } else {
                move_sel_cached(app, -1, details_tx, comments_tx);
            }
            true
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            move_sel_cached(app, 10, details_tx, comments_tx);
            true
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            move_sel_cached(app, -10, details_tx, comments_tx);
            true
        }
        _ => false,
    }
}

/// What: Handle space key events (add to list or downgrade).
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request details for the focused item
/// - `add_tx`: Channel to add items to the Install/Remove lists
///
/// Output:
/// - `true` if space key was handled, `false` otherwise
///
/// Details:
/// - Ctrl+Space: Adds to downgrade list (installed-only mode only).
/// - Space: Adds to install list or remove list depending on mode.
fn handle_space_key(
    ke: &KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    match (ke.code, ke.modifiers) {
        (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
            if app.installed_only_mode
                && let Some(item) = app.results.get(app.selected).cloned()
            {
                crate::logic::add_to_downgrade_list(app, item);
                crate::events::utils::refresh_downgrade_details(app, details_tx);
            }
            true
        }
        (KeyCode::Char(' '), _) => {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                if let Some(item) = app.news_results.get(app.news_selected).cloned() {
                    let url_opt = item.url.clone();
                    let mut content = app.news_content.clone().or_else(|| {
                        url_opt
                            .as_ref()
                            .and_then(|u| app.news_content_cache.get(u).cloned())
                    });
                    let mut html_path = None;
                    if let Some(url) = &url_opt
                        && let Ok(html) = crate::util::curl::curl_text(url)
                    {
                        let dir = crate::theme::lists_dir().join("news_html");
                        let _ = std::fs::create_dir_all(&dir);
                        let mut hasher = DefaultHasher::new();
                        item.id.hash(&mut hasher);
                        let fname = format!("{:016x}.html", hasher.finish());
                        let path = dir.join(fname);
                        if std::fs::write(&path, &html).is_ok() {
                            html_path = Some(path.to_string_lossy().to_string());
                            if content.is_none() {
                                content = Some(crate::sources::parse_news_html(&html));
                            }
                        }
                    }
                    let bookmark = crate::state::types::NewsBookmark {
                        item,
                        content,
                        html_path,
                    };
                    app.add_news_bookmark(bookmark);
                    app.toast_message = Some(crate::i18n::t(
                        app,
                        "app.results.options_menu.news_management",
                    ));
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
                }
                return true;
            }
            if let Some(item) = app.results.get(app.selected).cloned() {
                if app.installed_only_mode {
                    crate::logic::add_to_remove_list(app, item);
                    crate::events::utils::refresh_remove_details(app, details_tx);
                } else {
                    let _ = add_tx.send(item);
                }
            }
            true
        }
        _ => false,
    }
}

/// What: Handle preflight modal opening.
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if preflight was opened, `false` otherwise
///
/// Details:
/// - Opens preflight modal for the selected package using configured key or Enter.
fn handle_preflight_open(ke: &KeyEvent, app: &mut AppState) -> bool {
    // Don't open preflight if Ctrl is held (might be Ctrl+M interpreted as Enter)
    if ke.modifiers.contains(KeyModifiers::CONTROL) {
        tracing::debug!(
            "[NormalMode] Key with Ctrl detected in handle_preflight_open, ignoring (likely Ctrl+M): code={:?}",
            ke.code
        );
        return false;
    }

    let should_open = matches_any(ke, &app.keymap.search_install)
        || matches!(ke.code, KeyCode::Char('\n') | KeyCode::Enter);

    if should_open && let Some(item) = app.results.get(app.selected).cloned() {
        tracing::debug!("[NormalMode] Opening preflight for package: {}", item.name);
        open_preflight_modal(app, vec![item], true);
        return true;
    }
    false
}

/// What: Handle pane navigation (Left/Right arrows and `pane_next`).
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request details for the focused item
/// - `preview_tx`: Channel to request preview details when moving focus
///
/// Output:
/// - `true` if pane navigation was handled, `false` otherwise
///
/// Details:
/// - Handles Left/Right arrow keys and configured `pane_next` key.
/// - Switches focus between panes and updates details accordingly.
fn handle_pane_navigation(
    ke: &KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    match ke.code {
        KeyCode::Right => {
            navigate_pane(app, "right", details_tx, preview_tx);
            true
        }
        KeyCode::Left => {
            navigate_pane(app, "left", details_tx, preview_tx);
            true
        }
        _ if matches_any(ke, &app.keymap.pane_next) => {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                app.focus = crate::state::Focus::Install;
                return true;
            }
            if app.installed_only_mode {
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                    app.downgrade_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                crate::events::utils::refresh_downgrade_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                refresh_install_details(app, details_tx);
            }
            true
        }
        _ => false,
    }
}

/// What: Handle input clearing.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send debounced search queries
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - Clears the entire search input and resets caret/selection.
fn handle_input_clear(app: &mut AppState, query_tx: &mpsc::UnboundedSender<QueryInput>) {
    if matches!(app.app_mode, crate::state::types::AppMode::News) {
        if !app.news_search_input.is_empty() {
            app.news_search_input.clear();
            app.input.clear();
            app.news_search_caret = 0;
            app.news_search_select_anchor = None;
            app.search_caret = 0;
            app.search_select_anchor = None;
            app.last_input_change = std::time::Instant::now();
            app.last_saved_value = None;
            app.refresh_news_results();
        }
    } else if !app.input.is_empty() {
        app.input.clear();
        app.search_caret = 0;
        app.search_select_anchor = None;
        app.last_input_change = std::time::Instant::now();
        app.last_saved_value = None;
        send_query(app, query_tx);
    }
}

/// What: Mark or unmark the selected News Feed item as read.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `mark_read`: Whether to mark as read (`true`) or unread (`false`)
///
/// Output:
/// - `true` if state changed (set updated), `false` otherwise
///
/// Details:
/// - Updates both the ID-based read set and the legacy URL set when available.
/// - Refreshes news results to honor read/unread filtering.
fn mark_news_feed_item(app: &mut AppState, mark_read: bool) -> bool {
    let Some(item) = app.news_results.get(app.news_selected).cloned() else {
        return false;
    };

    let is_read_before = app.news_read_ids.contains(&item.id)
        || item
            .url
            .as_ref()
            .is_some_and(|u| app.news_read_urls.contains(u));

    let mut changed = false;

    if mark_read {
        if !is_read_before {
            app.news_read_ids_dirty = true;
            app.news_read_dirty = app.news_read_dirty || item.url.is_some();
            changed = true;
        }
        app.news_read_ids.insert(item.id.clone());
        if let Some(url) = item.url.as_ref() {
            app.news_read_urls.insert(url.clone());
        }
    } else {
        if is_read_before {
            app.news_read_ids_dirty = true;
            app.news_read_dirty = app.news_read_dirty || item.url.is_some();
            changed = true;
        }
        app.news_read_ids.remove(&item.id);
        if let Some(url) = item.url.as_ref() {
            app.news_read_urls.remove(url);
        }
    }

    if changed {
        app.refresh_news_results();
    }
    changed
}

/// What: Toggle read/unread state for the selected News Feed item.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if state changed (toggled), `false` otherwise
///
/// Details:
/// - Considers both ID-based and legacy URL-based read state for determining current status.
fn toggle_news_feed_item(app: &mut AppState) -> bool {
    let Some(item) = app.news_results.get(app.news_selected).cloned() else {
        return false;
    };
    let is_read = app.news_read_ids.contains(&item.id)
        || item
            .url
            .as_ref()
            .is_some_and(|u| app.news_read_urls.contains(u));
    if is_read {
        mark_news_feed_item(app, false)
    } else {
        mark_news_feed_item(app, true)
    }
}

/// What: Handle news mode keybindings.
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if a news keybinding was handled, `false` otherwise
///
/// Details:
/// - Handles mark read, mark unread, and toggle read keybindings in News mode.
fn handle_news_mode_keybindings(ke: &KeyEvent, app: &mut AppState) -> bool {
    if !matches!(app.app_mode, crate::state::types::AppMode::News) {
        return false;
    }

    if matches_any(ke, &app.keymap.news_mark_read_feed) {
        if mark_news_feed_item(app, true) {
            return true;
        }
    } else if matches_any(ke, &app.keymap.news_mark_unread_feed) {
        if mark_news_feed_item(app, false) {
            return true;
        }
    } else if matches_any(ke, &app.keymap.news_toggle_read_feed) && toggle_news_feed_item(app) {
        return true;
    }

    false
}

/// What: Handle keymap-based action keybindings.
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send debounced search queries
///
/// Output:
/// - `true` if a keymap action was handled, `false` otherwise
///
/// Details:
/// - Handles status, import, export, updates, insert mode, selection, delete, and clear actions.
fn handle_keymap_actions(
    ke: &KeyEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
) -> bool {
    if matches_any(ke, &app.keymap.search_normal_open_status) {
        crate::util::open_url("https://status.archlinux.org");
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_import) {
        if !app.installed_only_mode {
            app.modal = crate::state::Modal::ImportHelp;
        }
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_export) {
        handle_export(app);
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_updates) {
        if matches!(app.app_mode, crate::state::types::AppMode::News) {
            crate::events::mouse::handle_news_button(app);
        } else {
            crate::events::mouse::handle_updates_button(app);
        }
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_insert) {
        app.search_normal_mode = false;
        app.search_select_anchor = None;
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_select_left) {
        handle_selection_move(app, -1);
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_select_right) {
        handle_selection_move(app, 1);
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_delete) {
        handle_selection_delete(app, query_tx);
        return true;
    }

    if matches_any(ke, &app.keymap.search_normal_clear) {
        handle_input_clear(app, query_tx);
        return true;
    }

    false
}

/// What: Handle key events in Normal mode for the Search pane.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send debounced search queries
/// - `details_tx`: Channel to request details for the focused item
/// - `add_tx`: Channel to add items to the Install/Remove lists
/// - `preview_tx`: Channel to request preview details when moving focus
///
/// Output:
/// - `true` if the key was handled and should stop further processing; `false` otherwise
///
/// Details:
/// - Handles vim-like navigation, selection, deletion, menu toggles, import/export, and preflight opening.
/// - Supports numeric selection for config menu items.
pub fn handle_normal_mode(
    ke: KeyEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    comments_tx: &mpsc::UnboundedSender<String>,
) -> bool {
    // Handle numeric selection for config menu (1-9)
    if let KeyCode::Char(ch) = ke.code
        && ch.is_ascii_digit()
        && ch != '0'
    {
        let idx = (ch as u8 - b'1') as usize;
        if handle_config_menu_numeric_selection(idx, app) {
            return false;
        }
    }

    // Handle news mode keybindings
    if handle_news_mode_keybindings(&ke, app) {
        return false;
    }

    // Handle menu toggles
    let menu_toggled = {
        let km = &app.keymap;
        matches_any(&ke, &km.config_menu_toggle)
            || matches_any(&ke, &km.options_menu_toggle)
            || matches_any(&ke, &km.panels_menu_toggle)
    };
    if menu_toggled && handle_menu_toggles(&ke, app) {
        return false;
    }

    // Handle keymap-based actions
    if handle_keymap_actions(&ke, app, query_tx) {
        return false;
    }

    // Handle navigation keys
    if handle_navigation(&ke, app, details_tx, comments_tx) {
        return false;
    }

    // Handle space key
    if handle_space_key(&ke, app, details_tx, add_tx) {
        return false;
    }

    // Handle preflight opening
    if handle_preflight_open(&ke, app) {
        return false;
    }

    // Handle pane navigation
    if handle_pane_navigation(&ke, app, details_tx, preview_tx) {
        return false;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::{AppMode, NewsFeedItem, NewsFeedSource};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use tokio::sync::mpsc;

    fn make_news_item(id: &str, url: &str) -> NewsFeedItem {
        NewsFeedItem {
            id: id.to_string(),
            date: "2025-01-01".to_string(),
            title: format!("Item {id}"),
            summary: None,
            url: Some(url.to_string()),
            source: NewsFeedSource::ArchNews,
            severity: None,
            packages: vec![],
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn mark_news_feed_item_sets_read_state() {
        let item = make_news_item("one", "https://example.com/one");
        let mut app = AppState {
            app_mode: AppMode::News,
            news_items: vec![item.clone()],
            news_results: vec![item],
            news_selected: 0,
            news_max_age_days: None,
            ..AppState::default()
        };

        let changed = mark_news_feed_item(&mut app, true);
        assert!(changed);
        assert!(app.news_read_ids.contains("one"));
        assert!(app.news_read_ids_dirty);
        assert!(app.news_read_urls.contains("https://example.com/one"));
        assert!(app.news_read_dirty);

        let changed_unread = mark_news_feed_item(&mut app, false);
        assert!(changed_unread);
        assert!(!app.news_read_ids.contains("one"));
        assert!(app.news_read_ids_dirty);
    }

    #[test]
    fn toggle_news_feed_item_respects_legacy_url_state() {
        let item = make_news_item("two", "https://example.com/two");
        let mut app = AppState {
            app_mode: AppMode::News,
            news_items: vec![item.clone()],
            news_results: vec![item],
            news_selected: 0,
            news_max_age_days: None,
            ..AppState::default()
        };
        app.news_read_urls.insert("https://example.com/two".into());
        app.news_read_dirty = true;

        let toggled = toggle_news_feed_item(&mut app);
        assert!(toggled);
        assert!(!app.news_read_ids.contains("two"));
        assert!(!app.news_read_urls.contains("https://example.com/two"));
        assert!(app.news_read_ids_dirty);
        assert!(app.news_read_dirty);
    }

    #[test]
    fn handle_normal_mode_marks_read_via_keybinding() {
        let item = make_news_item("three", "https://example.com/three");
        let mut app = AppState {
            app_mode: AppMode::News,
            news_items: vec![item.clone()],
            news_results: vec![item],
            news_selected: 0,
            ..AppState::default()
        };
        let (query_tx, _query_rx) = mpsc::unbounded_channel();
        let (details_tx, _details_rx) = mpsc::unbounded_channel();
        let (add_tx, _add_rx) = mpsc::unbounded_channel();
        let (preview_tx, _preview_rx) = mpsc::unbounded_channel();
        let (comments_tx, _comments_rx) = mpsc::unbounded_channel();

        let ke = key(KeyCode::Char('r'));
        let handled = handle_normal_mode(
            ke,
            &mut app,
            &query_tx,
            &details_tx,
            &add_tx,
            &preview_tx,
            &comments_tx,
        );
        assert!(!handled);
        assert!(app.news_read_ids.contains("three"));
    }
}
