use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

use super::utils::{
    find_in_install, refresh_install_details, refresh_remove_details, refresh_selected_details,
};

/// Handle key events while the Install pane is focused.
///
/// Supports navigation, in-pane find, removal/clear actions, and opening the
/// batch install confirmation modal. Returns `true` to exit the app.
pub fn handle_install_key(
    ke: KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    _add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }

    // Pane-search mode first
    if app.pane_find.is_some() {
        match ke.code {
            KeyCode::Enter => {
                find_in_install(app, true);
                refresh_install_details(app, details_tx);
            }
            KeyCode::Esc => {
                app.pane_find = None;
            }
            KeyCode::Backspace => {
                if let Some(buf) = &mut app.pane_find {
                    buf.pop();
                }
            }
            KeyCode::Char(ch) => {
                if let Some(buf) = &mut app.pane_find {
                    buf.push(ch);
                }
            }
            _ => {}
        }
        return false;
    }

    let km = &app.keymap;
    let chord = (ke.code, ke.modifiers);
    let matches_any =
        |list: &Vec<crate::theme::KeyChord>| list.iter().any(|c| (c.code, c.mods) == chord);

    match ke.code {
        KeyCode::Char('j') => {
            // vim down
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui_helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                let sel = app.install_state.selected().unwrap_or(0);
                let max = inds.len().saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.install_state.select(Some(new));
                refresh_install_details(app, details_tx);
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                let len = app.remove_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.remove_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.remove_state.select(Some(new));
                refresh_remove_details(app, details_tx);
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            ) {
                let len = app.downgrade_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.downgrade_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.downgrade_state.select(Some(new));
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        KeyCode::Char('k') => {
            // vim up
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui_helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(sel) = app.install_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.install_state.select(Some(new));
                    refresh_install_details(app, details_tx);
                }
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                if let Some(sel) = app.remove_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.remove_state.select(Some(new));
                    refresh_remove_details(app, details_tx);
                }
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            )
                && let Some(sel) = app.downgrade_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.downgrade_state.select(Some(new));
                    super::utils::refresh_downgrade_details(app, details_tx);
                }
        }
        KeyCode::Char('/') => {
            app.pane_find = Some(String::new());
        }
        KeyCode::Enter => {
            if !app.installed_only_mode && !app.install_list.is_empty() {
                // Open confirmation modal listing all items to be installed
                app.modal = crate::state::Modal::ConfirmInstall {
                    items: app.install_list.clone(),
                };
            } else if app.installed_only_mode
                && matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove)
            {
                if !app.remove_list.is_empty() {
                    app.modal = crate::state::Modal::ConfirmRemove {
                        items: app.remove_list.clone(),
                    };
                }
            } else if app.installed_only_mode
                && matches!(
                    app.right_pane_focus,
                    crate::state::RightPaneFocus::Downgrade
                )
            {
                // Run the Arch 'downgrade' tool for all packages in the Downgrade List
                if !app.downgrade_list.is_empty() {
                    let names: Vec<String> =
                        app.downgrade_list.iter().map(|p| p.name.clone()).collect();
                    let joined = names.join(" ");
                    let cmd = if app.dry_run {
                        format!("echo DRY RUN: downgrade {joined}")
                    } else {
                        format!(
                            "(command -v downgrade >/dev/null 2>&1 && downgrade {joined}) || echo 'downgrade tool not found. Install \"downgrade\" from AUR.'"
                        )
                    };
                    crate::install::spawn_shell_commands_in_terminal(&[cmd]);
                    // Clear the list after triggering downgrade
                    app.downgrade_list.clear();
                    app.downgrade_state.select(None);
                }
            }
        }
        KeyCode::Esc => {
            app.focus = crate::state::Focus::Search;
            refresh_selected_details(app, details_tx);
        }
        code if matches_any(&km.pane_next) && code == ke.code => {
            // Desired cycle: Search -> Downgrade -> Remove -> Recent -> Search
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        // Downgrade -> Remove (stay in Install)
                        app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                        if app.remove_state.selected().is_none() && !app.remove_list.is_empty() {
                            app.remove_state.select(Some(0));
                        }
                        refresh_remove_details(app, details_tx);
                        return false;
                    }
                    crate::state::RightPaneFocus::Remove => {
                        // Remove -> Recent
                        if app.history_state.selected().is_none() && !app.recent.is_empty() {
                            app.history_state.select(Some(0));
                        }
                        app.focus = crate::state::Focus::Recent;
                        crate::ui_helpers::trigger_recent_preview(app, preview_tx);
                        return false;
                    }
                    crate::state::RightPaneFocus::Install => {}
                }
            }
            // Not in installed-only: Install -> Recent
            if app.history_state.selected().is_none() && !app.recent.is_empty() {
                app.history_state.select(Some(0));
            }
            app.focus = crate::state::Focus::Recent;
            crate::ui_helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Left => {
            // In installed-only mode, follow reverse: Remove -> Downgrade -> Search
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Remove => {
                        // Move to Downgrade subpane and keep Install focus
                        app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                        if app.downgrade_state.selected().is_none()
                            && !app.downgrade_list.is_empty()
                        {
                            app.downgrade_state.select(Some(0));
                        }
                        super::utils::refresh_downgrade_details(app, details_tx);
                    }
                    crate::state::RightPaneFocus::Downgrade => {
                        // Downgrade -> Search
                        app.focus = crate::state::Focus::Search;
                        refresh_selected_details(app, details_tx);
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Normal mode: Install -> Search
                        app.focus = crate::state::Focus::Search;
                        refresh_selected_details(app, details_tx);
                    }
                }
            } else {
                // Normal mode: Install -> Search
                app.focus = crate::state::Focus::Search;
                refresh_selected_details(app, details_tx);
            }
        }
        KeyCode::Right => {
            // In installed-only mode, follow: Downgrade -> Remove -> Recent
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                        if app.remove_state.selected().is_none() && !app.remove_list.is_empty() {
                            app.remove_state.select(Some(0));
                        }
                        refresh_remove_details(app, details_tx);
                    }
                    crate::state::RightPaneFocus::Remove => { /* no-op: rightmost subpane */ }
                    crate::state::RightPaneFocus::Install => {}
                }
            }
        }
        KeyCode::Delete if !ke.modifiers.contains(KeyModifiers::SHIFT) => {
            // Delete the selected item from the active list
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        if let Some(sel) = app.downgrade_state.selected()
                            && sel < app.downgrade_list.len() {
                                app.downgrade_list.remove(sel);
                                let len = app.downgrade_list.len();
                                if len == 0 {
                                    app.downgrade_state.select(None);
                                } else {
                                    let new_sel = sel.min(len - 1);
                                    app.downgrade_state.select(Some(new_sel));
                                    super::utils::refresh_downgrade_details(app, details_tx);
                                }
                            }
                    }
                    crate::state::RightPaneFocus::Remove => {
                        if let Some(sel) = app.remove_state.selected()
                            && sel < app.remove_list.len() {
                                app.remove_list.remove(sel);
                                let len = app.remove_list.len();
                                if len == 0 {
                                    app.remove_state.select(None);
                                } else {
                                    let new_sel = sel.min(len - 1);
                                    app.remove_state.select(Some(new_sel));
                                    refresh_remove_details(app, details_tx);
                                }
                            }
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Installed-only mode: when Install subpane is focused, delete from Install list
                        let inds = crate::ui_helpers::filtered_install_indices(app);
                        if inds.is_empty() {
                            return false;
                        }
                        if let Some(vsel) = app.install_state.selected() {
                            let i = inds.get(vsel).copied().unwrap_or(0);
                            if i < app.install_list.len() {
                                app.install_list.remove(i);
                                app.install_dirty = true;
                                let vis_len = inds.len().saturating_sub(1); // one less visible
                                if vis_len == 0 {
                                    app.install_state.select(None);
                                } else {
                                    let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                                    app.install_state.select(Some(new_sel));
                                    refresh_install_details(app, details_tx);
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal mode: delete from Install list
                let inds = crate::ui_helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(vsel) = app.install_state.selected() {
                    let i = inds.get(vsel).copied().unwrap_or(0);
                    if i < app.install_list.len() {
                        app.install_list.remove(i);
                        app.install_dirty = true;
                        let vis_len = inds.len().saturating_sub(1); // one less visible
                        if vis_len == 0 {
                            app.install_state.select(None);
                        } else {
                            let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                            app.install_state.select(Some(new_sel));
                            refresh_install_details(app, details_tx);
                        }
                    }
                }
            }
        }
        KeyCode::Delete if ke.modifiers.contains(KeyModifiers::SHIFT) => {
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        app.downgrade_list.clear();
                        app.downgrade_state.select(None);
                    }
                    crate::state::RightPaneFocus::Remove => {
                        app.remove_list.clear();
                        app.remove_state.select(None);
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Fallback to normal Install clear
                        app.install_list.clear();
                        app.install_state.select(None);
                        app.install_dirty = true;
                    }
                }
            } else {
                app.install_list.clear();
                app.install_state.select(None);
                app.install_dirty = true;
            }
        }
        code if matches_any(&km.install_remove) && code == ke.code => {
            // Support 'd' (and other configured keys) as an alternative to Delete everywhere
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        if let Some(sel) = app.downgrade_state.selected()
                            && sel < app.downgrade_list.len()
                        {
                            app.downgrade_list.remove(sel);
                            let len = app.downgrade_list.len();
                            if len == 0 {
                                app.downgrade_state.select(None);
                            } else {
                                let new_sel = sel.min(len - 1);
                                app.downgrade_state.select(Some(new_sel));
                                super::utils::refresh_downgrade_details(app, details_tx);
                            }
                        }
                    }
                    crate::state::RightPaneFocus::Remove => {
                        if let Some(sel) = app.remove_state.selected()
                            && sel < app.remove_list.len()
                        {
                            app.remove_list.remove(sel);
                            let len = app.remove_list.len();
                            if len == 0 {
                                app.remove_state.select(None);
                            } else {
                                let new_sel = sel.min(len - 1);
                                app.remove_state.select(Some(new_sel));
                                refresh_remove_details(app, details_tx);
                            }
                        }
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Fall through to normal install list removal logic below
                        let inds = crate::ui_helpers::filtered_install_indices(app);
                        if inds.is_empty() {
                            return false;
                        }
                        if let Some(vsel) = app.install_state.selected() {
                            let i = inds.get(vsel).copied().unwrap_or(0);
                            if i < app.install_list.len() {
                                app.install_list.remove(i);
                                app.install_dirty = true;
                                let vis_len = inds.len().saturating_sub(1); // one less visible
                                if vis_len == 0 {
                                    app.install_state.select(None);
                                } else {
                                    let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                                    app.install_state.select(Some(new_sel));
                                    refresh_install_details(app, details_tx);
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal mode: remove from Install list
                let inds = crate::ui_helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(vsel) = app.install_state.selected() {
                    let i = inds.get(vsel).copied().unwrap_or(0);
                    if i < app.install_list.len() {
                        app.install_list.remove(i);
                        app.install_dirty = true;
                        let vis_len = inds.len().saturating_sub(1); // one less visible
                        if vis_len == 0 {
                            app.install_state.select(None);
                        } else {
                            let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                            app.install_state.select(Some(new_sel));
                            refresh_install_details(app, details_tx);
                        }
                    }
                }
            }
        }
        KeyCode::Up => {
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui_helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(sel) = app.install_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.install_state.select(Some(new));
                    refresh_install_details(app, details_tx);
                }
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                if let Some(sel) = app.remove_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.remove_state.select(Some(new));
                    refresh_remove_details(app, details_tx);
                }
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            )
                && let Some(sel) = app.downgrade_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.downgrade_state.select(Some(new));
                    super::utils::refresh_downgrade_details(app, details_tx);
                }
        }
        KeyCode::Down => {
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui_helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                let sel = app.install_state.selected().unwrap_or(0);
                let max = inds.len().saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.install_state.select(Some(new));
                refresh_install_details(app, details_tx);
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                let len = app.remove_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.remove_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.remove_state.select(Some(new));
                refresh_remove_details(app, details_tx);
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            ) {
                let len = app.downgrade_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.downgrade_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.downgrade_state.select(Some(new));
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        _ => {}
    }
    false
}
