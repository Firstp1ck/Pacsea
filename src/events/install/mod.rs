use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

use super::utils::{
    find_in_install, refresh_install_details, refresh_remove_details, refresh_selected_details,
};

/// Preflight modal opening functions for install operations.
mod preflight;

#[cfg(test)]
mod tests;

pub use preflight::{
    open_preflight_downgrade_modal, open_preflight_install_modal, open_preflight_remove_modal,
};

/// What: Handle `pane_next` navigation (cycles through panes).
fn handle_pane_next_navigation(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) {
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
                return;
            }
            crate::state::RightPaneFocus::Remove => {
                // Remove -> Recent
                if app.history_state.selected().is_none() && !app.recent.is_empty() {
                    app.history_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Recent;
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                return;
            }
            crate::state::RightPaneFocus::Install => {}
        }
    }
    // Not in installed-only: Install -> Recent
    if app.history_state.selected().is_none() && !app.recent.is_empty() {
        app.history_state.select(Some(0));
    }
    app.focus = crate::state::Focus::Recent;
    crate::ui::helpers::trigger_recent_preview(app, preview_tx);
}

/// What: Handle Left arrow navigation (moves focus left).
fn handle_left_arrow_navigation(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    // In installed-only mode, follow reverse: Remove -> Downgrade -> Search
    if app.installed_only_mode {
        match app.right_pane_focus {
            crate::state::RightPaneFocus::Remove => {
                // Move to Downgrade subpane and keep Install focus
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
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

/// What: Handle Right arrow navigation (moves focus right).
fn handle_right_arrow_navigation(
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    // In installed-only mode, follow: Downgrade -> Remove -> Recent; else wrap to Recent
    if app.installed_only_mode {
        match app.right_pane_focus {
            crate::state::RightPaneFocus::Downgrade => {
                app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                if app.remove_state.selected().is_none() && !app.remove_list.is_empty() {
                    app.remove_state.select(Some(0));
                }
                refresh_remove_details(app, details_tx);
            }
            crate::state::RightPaneFocus::Remove => {
                // Wrap-around to Recent from rightmost subpane
                if app.history_state.selected().is_none() && !app.recent.is_empty() {
                    app.history_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Recent;
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
            }
            crate::state::RightPaneFocus::Install => {
                // Normal Install subpane: wrap directly to Recent
                if app.history_state.selected().is_none() && !app.recent.is_empty() {
                    app.history_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Recent;
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
            }
        }
    } else {
        // Normal mode: Install -> Recent (wrap)
        if app.history_state.selected().is_none() && !app.recent.is_empty() {
            app.history_state.select(Some(0));
        }
        app.focus = crate::state::Focus::Recent;
        crate::ui::helpers::trigger_recent_preview(app, preview_tx);
    }
}

/// What: Handle key events while the Install pane (right column) is focused.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state (selection, lists, pane focus)
/// - `details_tx`: Channel to request package details for the focused item
/// - `preview_tx`: Channel to request preview details (used for some focus changes)
/// - `_add_tx`: Channel for adding items (not used directly in Install handler)
///
/// Output:
/// - `true` to request application exit (e.g., Ctrl+C); `false` to continue.
///
/// Details:
/// - In-pane find: `/` enters find mode; typing edits the pattern; Enter jumps to next match;
///   Esc cancels. Find matches against name/description (Install) or name-only (Remove/Downgrade).
/// - Navigation: `j/k` and `Down/Up` move selection in the active subpane. Behavior adapts to
///   installed-only mode (`app.installed_only_mode`) and current `right_pane_focus`:
///   - Normal mode: selection moves in Install list only.
///   - Installed-only: selection moves in Downgrade/Remove/Install subpane depending on focus.
/// - Pane cycling: Configured `pane_next` chord cycles focus across panes. In installed-only mode
///   it cycles Search → Downgrade → Remove → Recent → Search; otherwise Search → Install → Recent.
/// - Arrow focus: Left/Right move focus between Search/Install/Recent (and subpanes when installed-only).
/// - Deletion: `Delete` (or configured `install_remove`) removes the selected entry from the active
///   list (Install/Remove/Downgrade) and updates selection and details.
/// - Clear list: Configured `install_clear` clears the respective list (or all in normal mode),
///   and resets selection.
/// - Enter:
///   - Normal mode with non-empty Install list: opens `Modal::ConfirmInstall` for batch install.
///   - Installed-only Remove focus with non-empty list: opens `Modal::ConfirmRemove`.
///   - Installed-only Downgrade focus with non-empty list: runs `downgrade` tool (or dry-run).
/// - Esc: Returns focus to Search and refreshes the selected result's details.
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
    if app.pane_find.is_some() && handle_pane_find_mode(ke, app, details_tx) {
        return false;
    }

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
                    let ev_has_no_shift =
                        !ke.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
                    cfg_has_shift && ev_has_no_shift && ev_ch == cfg_ch.to_ascii_uppercase()
                }
                _ => false,
            }
        })
    };

    match ke.code {
        KeyCode::Char('j') => {
            handle_navigation_down(app, details_tx);
        }
        KeyCode::Char('k') => {
            handle_navigation_up(app, details_tx);
        }
        KeyCode::Char('/') => {
            app.pane_find = Some(String::new());
        }
        KeyCode::Enter => {
            handle_enter_key(app);
        }
        KeyCode::Esc => {
            app.focus = crate::state::Focus::Search;
            // Activate Search Normal mode when returning with Esc
            app.search_normal_mode = true;
            refresh_selected_details(app, details_tx);
        }
        code if matches_any(&km.pane_next) && code == ke.code => {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                // Bookmarks -> History
                app.focus = crate::state::Focus::Recent;
            } else {
                handle_pane_next_navigation(app, details_tx, preview_tx);
            }
        }
        KeyCode::Left => {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                app.focus = crate::state::Focus::Search;
            } else {
                handle_left_arrow_navigation(app, details_tx);
            }
        }
        KeyCode::Right => {
            if matches!(app.app_mode, crate::state::types::AppMode::News) {
                app.focus = crate::state::Focus::Recent;
            } else {
                handle_right_arrow_navigation(app, details_tx, preview_tx);
            }
        }
        KeyCode::Delete if !ke.modifiers.contains(KeyModifiers::SHIFT) => {
            handle_delete_item(app, details_tx);
        }
        code if matches_any(&km.install_clear) && code == ke.code => {
            handle_clear_list(app);
        }
        code if matches_any(&km.install_remove) && code == ke.code => {
            handle_delete_item(app, details_tx);
        }
        KeyCode::Up => {
            handle_navigation_up(app, details_tx);
        }
        KeyCode::Down => {
            handle_navigation_down(app, details_tx);
        }
        _ => {}
    }
    false
}

/// What: Handle key events while in pane-find mode.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - `true` if the event was handled and should return early; `false` otherwise
///
/// Details:
/// - Handles Enter (jump to next match), Esc (cancel), Backspace (delete char), and Char (append char).
fn handle_pane_find_mode(
    ke: KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
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
    true
}

/// What: Handle Enter key to trigger install/remove/downgrade actions.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - No return value; modifies app state to open modals or trigger actions
///
/// Details:
/// - Normal mode with non-empty Install list: opens Preflight modal or skips to direct install
/// - Installed-only Remove focus: opens Preflight modal or skips to direct remove
/// - Installed-only Downgrade focus: runs downgrade tool
fn handle_enter_key(app: &mut AppState) {
    // Never trigger preflight when SystemUpdate or OptionalDeps modals are active
    // These modals use spawn_shell_commands_in_terminal which bypasses preflight
    let skip_preflight_for_modals = matches!(
        app.modal,
        crate::state::Modal::SystemUpdate { .. } | crate::state::Modal::OptionalDeps { .. }
    );
    let skip = crate::theme::settings().skip_preflight || skip_preflight_for_modals;
    if !app.installed_only_mode && !app.install_list.is_empty() {
        if skip {
            // Direct install - check for reinstalls first, then batch updates
            // First, check if we're installing packages that are already installed (reinstall scenario)
            // BUT exclude packages that have updates available (those should go through normal update flow)
            let installed_set = crate::logic::deps::get_installed_packages();
            let provided_set = crate::logic::deps::get_provided_packages(&installed_set);
            let upgradable_set = crate::logic::deps::get_upgradable_packages();

            let installed_packages: Vec<crate::state::PackageItem> = app
                .install_list
                .iter()
                .filter(|item| {
                    // Check if package is installed or provided by an installed package
                    let is_installed = crate::logic::deps::is_package_installed_or_provided(
                        &item.name,
                        &installed_set,
                        &provided_set,
                    );

                    if !is_installed {
                        return false;
                    }

                    // Check if package has an update available
                    // For official packages: check if it's in upgradable_set OR version differs from installed
                    // For AUR packages: check if target version is different from installed version
                    let has_update = if upgradable_set.contains(&item.name) {
                        // Package is in upgradable set (pacman -Qu)
                        true
                    } else if !item.version.is_empty() {
                        // Normalize target version by removing revision suffix (same as installed version normalization)
                        let normalized_target_version =
                            item.version.split('-').next().unwrap_or(&item.version);
                        // Compare normalized target version with normalized installed version
                        // This works for both official and AUR packages
                        crate::logic::deps::get_installed_version(&item.name).is_ok_and(
                            |installed_version| normalized_target_version != installed_version,
                        )
                    } else {
                        // No version info available, no update
                        false
                    };

                    // Only show reinstall confirmation if installed AND no update available
                    // If update is available, it should go through normal update flow
                    !has_update
                })
                .cloned()
                .collect();

            if installed_packages.is_empty() {
                // Check if this is a batch update scenario requiring confirmation
                // Only show if there's actually an update available (package is upgradable)
                // AND the package has installed packages in its "Required By" field (dependency risk)
                let has_versions = app.install_list.iter().any(|item| {
                    matches!(item.source, crate::state::Source::Official { .. })
                        && !item.version.is_empty()
                });
                let has_upgrade_available = app.install_list.iter().any(|item| {
                    matches!(item.source, crate::state::Source::Official { .. })
                        && upgradable_set.contains(&item.name)
                });

                // Only show warning if package has installed packages in "Required By" (dependency risk)
                let has_installed_required_by = app.install_list.iter().any(|item| {
                    matches!(item.source, crate::state::Source::Official { .. })
                        && crate::index::is_installed(&item.name)
                        && crate::logic::deps::has_installed_required_by(&item.name)
                });

                if has_versions && has_upgrade_available && has_installed_required_by {
                    // Show confirmation modal for batch updates (only if update is actually available
                    // AND package has installed dependents that could be affected)
                    app.modal = crate::state::Modal::ConfirmBatchUpdate {
                        items: app.install_list.clone(),
                        dry_run: app.dry_run,
                    };
                } else {
                    let items = app.install_list.clone();
                    crate::install::start_integrated_install_all(app, &items, app.dry_run);
                    app.toast_message = Some(crate::i18n::t(
                        app,
                        "app.toasts.installing_preflight_skipped",
                    ));
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                }
            } else {
                // Show reinstall confirmation modal
                // Store both installed packages (for display) and all packages (for installation)
                app.modal = crate::state::Modal::ConfirmReinstall {
                    items: installed_packages,
                    all_items: app.install_list.clone(),
                    header_chips: crate::state::modal::PreflightHeaderChips::default(),
                };
            }
        } else {
            open_preflight_install_modal(app);
        }
    } else if app.installed_only_mode
        && matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove)
    {
        if !app.remove_list.is_empty() {
            if skip {
                let names: Vec<String> = app.remove_list.iter().map(|p| p.name.clone()).collect();
                crate::install::start_integrated_remove_all(
                    app,
                    &names,
                    app.dry_run,
                    app.remove_cascade_mode,
                );
                app.toast_message =
                    Some(crate::i18n::t(app, "app.toasts.removing_preflight_skipped"));
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                app.remove_list.clear();
                app.remove_list_names.clear();
                app.remove_state.select(None);
            } else {
                open_preflight_remove_modal(app);
            }
        }
    } else if app.installed_only_mode
        && matches!(
            app.right_pane_focus,
            crate::state::RightPaneFocus::Downgrade
        )
        && !app.downgrade_list.is_empty()
    {
        if skip {
            let names: Vec<String> = app.downgrade_list.iter().map(|p| p.name.clone()).collect();
            let joined = names.join(" ");
            let cmd = if app.dry_run {
                format!("echo DRY RUN: sudo downgrade {joined}")
            } else {
                format!(
                    "if (command -v downgrade >/dev/null 2>&1) || sudo pacman -Qi downgrade >/dev/null 2>&1; then sudo downgrade {joined}; else echo 'downgrade tool not found. Install \"downgrade\" package.'; fi"
                )
            };
            crate::install::spawn_shell_commands_in_terminal(&[cmd]);
            app.downgrade_list.clear();
            app.downgrade_list_names.clear();
            app.downgrade_state.select(None);
        } else {
            open_preflight_downgrade_modal(app);
        }
    }
}

/// What: Handle navigation down (j/Down) in the active pane.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - No return value; updates selection in the active pane
///
/// Details:
/// - Moves selection down in Install/Remove/Downgrade based on mode and focus
fn handle_navigation_down(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    if !app.installed_only_mode
        || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
    {
        let inds = crate::ui::helpers::filtered_install_indices(app);
        if inds.is_empty() {
            return;
        }
        let sel = app.install_state.selected().unwrap_or(0);
        let max = inds.len().saturating_sub(1);
        let new = std::cmp::min(sel + 1, max);
        app.install_state.select(Some(new));
        refresh_install_details(app, details_tx);
    } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
        let len = app.remove_list.len();
        if len == 0 {
            return;
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
            return;
        }
        let sel = app.downgrade_state.selected().unwrap_or(0);
        let max = len.saturating_sub(1);
        let new = std::cmp::min(sel + 1, max);
        app.downgrade_state.select(Some(new));
        super::utils::refresh_downgrade_details(app, details_tx);
    }
}

/// What: Handle navigation up (k/Up) in the active pane.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - No return value; updates selection in the active pane
///
/// Details:
/// - Moves selection up in Install/Remove/Downgrade based on mode and focus
fn handle_navigation_up(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    if !app.installed_only_mode
        || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
    {
        let inds = crate::ui::helpers::filtered_install_indices(app);
        if inds.is_empty() {
            return;
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
    ) && let Some(sel) = app.downgrade_state.selected()
    {
        let new = sel.saturating_sub(1);
        app.downgrade_state.select(Some(new));
        super::utils::refresh_downgrade_details(app, details_tx);
    }
}

/// What: Delete the selected item from the active list.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - No return value; removes item and updates selection
///
/// Details:
/// - Handles deletion from Install/Remove/Downgrade lists based on mode and focus
fn handle_delete_item(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    if app.installed_only_mode {
        match app.right_pane_focus {
            crate::state::RightPaneFocus::Downgrade => {
                if let Some(sel) = app.downgrade_state.selected()
                    && sel < app.downgrade_list.len()
                {
                    if let Some(removed_item) = app.downgrade_list.get(sel) {
                        app.downgrade_list_names
                            .remove(&removed_item.name.to_lowercase());
                    }
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
                    if let Some(removed_item) = app.remove_list.get(sel) {
                        app.remove_list_names
                            .remove(&removed_item.name.to_lowercase());
                    }
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
                delete_from_install_list(app, details_tx);
            }
        }
    } else {
        delete_from_install_list(app, details_tx);
    }
}

/// What: Delete selected item from Install list.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `details_tx`: Channel to request package details
///
/// Output:
/// - No return value; removes item and updates selection
///
/// Details:
/// - Handles deletion from Install list with filtered indices
fn delete_from_install_list(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    let inds = crate::ui::helpers::filtered_install_indices(app);
    if inds.is_empty() {
        return;
    }
    if let Some(vsel) = app.install_state.selected() {
        let i = inds.get(vsel).copied().unwrap_or(0);
        if i < app.install_list.len() {
            if let Some(removed_item) = app.install_list.get(i) {
                app.install_list_names
                    .remove(&removed_item.name.to_lowercase());
            }
            app.install_list.remove(i);
            app.install_dirty = true;
            // Clear dependency cache when list changes
            app.install_list_deps.clear();
            app.install_list_files.clear();
            app.deps_resolving = false;
            app.files_resolving = false;
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

/// What: Clear the active list based on mode and focus.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - No return value; clears the appropriate list and resets selection
///
/// Details:
/// - Clears Install/Remove/Downgrade list based on mode and focus
fn handle_clear_list(app: &mut AppState) {
    if app.installed_only_mode {
        match app.right_pane_focus {
            crate::state::RightPaneFocus::Downgrade => {
                app.downgrade_list.clear();
                app.downgrade_list_names.clear();
                app.downgrade_state.select(None);
            }
            crate::state::RightPaneFocus::Remove => {
                app.remove_list.clear();
                app.remove_list_names.clear();
                app.remove_state.select(None);
            }
            crate::state::RightPaneFocus::Install => {
                app.install_list.clear();
                app.install_list_names.clear();
                app.install_state.select(None);
                app.install_dirty = true;
                // Clear dependency cache when list is cleared
                app.install_list_deps.clear();
                app.install_list_files.clear();
                app.deps_resolving = false;
                app.files_resolving = false;
            }
        }
    } else {
        app.install_list.clear();
        app.install_list_names.clear();
        app.install_state.select(None);
        app.install_dirty = true;
        // Clear dependency cache when list is cleared
        app.install_list_deps.clear();
        app.deps_resolving = false;
    }
}
