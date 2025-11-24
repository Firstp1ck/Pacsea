//! Common modal handlers (Alert, Help, News, `PreflightExec`, `PostSummary`, `GnomeTerminalPrompt`).

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::{AppState, PackageItem, Source};

/// What: Handle key events for Alert modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `message`: Alert message content
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), `false` otherwise
///
/// Details:
/// - Handles Enter/Esc to close, Up/Down for help scrolling
/// - Returns `true` for Esc to prevent mode toggling in search handler
pub(super) fn handle_alert(ke: KeyEvent, app: &mut AppState, message: &str) -> bool {
    let is_help = message.contains("Help") || message.contains("Tab Help");
    match ke.code {
        KeyCode::Esc => {
            if is_help {
                app.help_scroll = 0; // Reset scroll when closing
            }
            // Restore previous modal if it was Preflight, otherwise close
            if let Some(prev_modal) = app.previous_modal.take() {
                app.modal = prev_modal;
            } else {
                app.modal = crate::state::Modal::None;
            }
            true // Stop propagation to prevent mode toggle
        }
        KeyCode::Enter => {
            if is_help {
                app.help_scroll = 0; // Reset scroll when closing
            }
            // Restore previous modal if it was Preflight, otherwise close
            if let Some(prev_modal) = app.previous_modal.take() {
                app.modal = prev_modal;
            } else {
                app.modal = crate::state::Modal::None;
            }
            false
        }
        KeyCode::Up if is_help => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
            false
        }
        KeyCode::Down if is_help => {
            app.help_scroll = app.help_scroll.saturating_add(1);
            false
        }
        _ => false,
    }
}

/// What: Handle key events for `PreflightExec` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `verbose`: Mutable reference to verbose flag
/// - `abortable`: Whether execution can be aborted
/// - `items`: Package items being processed
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles Esc/q to close, Enter to show summary, l to toggle verbose, x to abort
pub(super) fn handle_preflight_exec(
    ke: KeyEvent,
    app: &mut AppState,
    verbose: &mut bool,
    abortable: bool,
    items: &[crate::state::PackageItem],
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q') => app.modal = crate::state::Modal::None,
        KeyCode::Enter => {
            // Compute real counts best-effort and show summary
            let data = crate::logic::compute_post_summary(items);
            app.modal = crate::state::Modal::PostSummary {
                success: data.success,
                changed_files: data.changed_files,
                pacnew_count: data.pacnew_count,
                pacsave_count: data.pacsave_count,
                services_pending: data.services_pending,
                snapshot_label: data.snapshot_label,
            };
        }
        KeyCode::Char('l') => {
            *verbose = !*verbose;
            let verbose_status = if *verbose { "ON" } else { "OFF" };
            app.toast_message = Some(format!("Verbose: {verbose_status}"));
        }
        // TODO: implement Logic for aborting the transaction
        KeyCode::Char('x') => {
            if abortable {
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.abort_requested"));
            }
        }
        _ => {}
    }
    false
}

/// What: Handle key events for `PostSummary` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `services_pending`: List of services pending restart
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles Esc/Enter/q to close, r for rollback, s for service restart
pub(super) fn handle_post_summary(
    ke: KeyEvent,
    app: &mut AppState,
    services_pending: &[String],
) -> bool {
    match ke.code {
        // TODO: implement Logic for aborting the transaction
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => app.modal = crate::state::Modal::None,
        KeyCode::Char('r') => {
            app.toast_message = Some(crate::i18n::t(app, "app.toasts.rollback"));
        }
        // TODO: implement Logic for restarting the services
        KeyCode::Char('s') => {
            if services_pending.is_empty() {
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.no_services_to_restart"));
            } else {
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.restart_services"));
            }
        }
        _ => {}
    }
    false
}

/// What: Handle key events for Help modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Handles Esc/Enter to close
/// - Returns `true` for Esc to prevent mode toggling in search handler
pub(super) fn handle_help(ke: KeyEvent, app: &mut AppState) -> bool {
    match ke.code {
        KeyCode::Esc => {
            app.modal = crate::state::Modal::None;
            true // Stop propagation to prevent mode toggle
        }
        KeyCode::Enter => {
            app.modal = crate::state::Modal::None;
            false
        }
        _ => false,
    }
}

/// What: Handle key events for News modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `items`: News items
/// - `selected`: Currently selected item index
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Handles Esc/q to close, navigation, Enter to open URL, keymap shortcuts for marking read
pub(super) fn handle_news(
    ke: KeyEvent,
    app: &mut AppState,
    items: &[crate::state::NewsItem],
    selected: &mut usize,
) -> bool {
    let km = &app.keymap;
    if crate::events::utils::matches_any(&ke, &km.news_mark_read) {
        if let Some(it) = items.get(*selected) {
            app.news_read_urls.insert(it.url.clone());
            app.news_read_dirty = true;
        }
        return false;
    }
    if crate::events::utils::matches_any(&ke, &km.news_mark_all_read) {
        for it in items {
            app.news_read_urls.insert(it.url.clone());
        }
        app.news_read_dirty = true;
        return false;
    }
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            return true; // Stop propagation to prevent global Esc handler from running
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected > 0 {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected + 1 < items.len() {
                *selected += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(it) = items.get(*selected) {
                crate::util::open_url(&it.url);
            }
        }
        _ => {}
    }
    false
}

/// What: Handle key events for Updates modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `entries`: Update entries list (name, `old_version`, `new_version`)
/// - `scroll`: Mutable scroll offset
/// - `selected`: Mutable selected index
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Handles Esc/q to close, Enter to install/update selected package
/// - Handles j/k and arrow keys for selection navigation
/// - Auto-scrolls to keep selected item visible
pub(super) fn handle_updates(
    ke: KeyEvent,
    app: &mut AppState,
    entries: &[(String, String, String)],
    scroll: &mut u16,
    selected: &mut usize,
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            return true; // Stop propagation
        }
        KeyCode::Enter => {
            // Install/update the selected package
            if let Some((pkg_name, _, new_version)) = entries.get(*selected) {
                // Try to find the package in the official index first
                let pkg_item = if let Some(mut pkg_item) = crate::index::find_package_by_name(pkg_name) {
                    // Update the version to the new version
                    pkg_item.version = new_version.clone();
                    pkg_item
                } else {
                    // If not found in official repos, assume it's an AUR package
                    PackageItem {
                        name: pkg_name.clone(),
                        version: new_version.clone(),
                        description: String::new(),
                        source: Source::Aur,
                        popularity: None,
                    }
                };
                
                // Spawn the install command (which works for updates too)
                #[cfg(not(target_os = "windows"))]
                crate::install::spawn_install(&pkg_item, None, false);
                #[cfg(target_os = "windows")]
                crate::install::spawn_install(&pkg_item, None, false);
                
                tracing::info!(
                    package = %pkg_name,
                    source = match pkg_item.source {
                        Source::Official { .. } => "official",
                        Source::Aur => "aur",
                    },
                    "Update triggered from Updates modal"
                );
            }
            app.modal = crate::state::Modal::None;
            return true; // Stop propagation
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected > 0 {
                *selected -= 1;
                // Auto-scroll to keep selected item visible
                update_scroll_for_selection(scroll, *selected);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected + 1 < entries.len() {
                *selected += 1;
                // Auto-scroll to keep selected item visible
                update_scroll_for_selection(scroll, *selected);
            }
        }
        KeyCode::PageUp => {
            if *selected >= 10 {
                *selected -= 10;
            } else {
                *selected = 0;
            }
            update_scroll_for_selection(scroll, *selected);
        }
        KeyCode::PageDown => {
            let max_idx = entries.len().saturating_sub(1);
            *selected = (*selected + 10).min(max_idx);
            update_scroll_for_selection(scroll, *selected);
        }
        KeyCode::Char('d')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Ctrl+D: page down (25 lines)
            let max_idx = entries.len().saturating_sub(1);
            *selected = (*selected + 25).min(max_idx);
            update_scroll_for_selection(scroll, *selected);
        }
        KeyCode::Char('u')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Ctrl+U: page up (20 lines)
            if *selected >= 20 {
                *selected -= 20;
            } else {
                *selected = 0;
            }
            update_scroll_for_selection(scroll, *selected);
        }
        _ => {}
    }
    false
}

/// What: Update scroll offset to keep the selected item visible.
///
/// Inputs:
/// - `scroll`: Mutable scroll offset
/// - `selected`: Selected index
///
/// Output:
/// - Updates scroll to ensure selected item is visible
///
/// Details:
/// - Estimates visible lines based on modal height
/// - Adjusts scroll so selected item is within visible range
fn update_scroll_for_selection(scroll: &mut u16, selected: usize) {
    // Estimate visible content lines (modal height minus header/footer/borders)
    // Header: 2 lines, borders: 2 lines, footer: 0 lines = ~4 lines overhead
    // Assume ~20 visible content lines as a reasonable default
    const VISIBLE_LINES: u16 = 20;

    let selected_line = u16::try_from(selected).unwrap_or(u16::MAX);

    // If selected item is above visible area, scroll up
    if selected_line < *scroll {
        *scroll = selected_line;
    }
    // If selected item is below visible area, scroll down
    else if selected_line >= *scroll + VISIBLE_LINES {
        *scroll = selected_line.saturating_sub(VISIBLE_LINES.saturating_sub(1));
    }
}

/// What: Handle key events for `GnomeTerminalPrompt` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles Enter to install terminal, Esc to show warning
pub(super) fn handle_gnome_terminal_prompt(ke: KeyEvent, app: &mut AppState) -> bool {
    match ke.code {
        KeyCode::Enter => {
            // Install GNOME Terminal, then close the prompt

            let cmd = "(sudo pacman -S --needed --noconfirm gnome-terminal) || (sudo pacman -S --needed --noconfirm gnome-console) || (sudo pacman -S --needed --noconfirm kgx)".to_string();

            if app.dry_run {
                crate::install::spawn_shell_commands_in_terminal(&[format!("echo DRY RUN: {cmd}")]);
            } else {
                crate::install::spawn_shell_commands_in_terminal(&[cmd]);
            }
            app.modal = crate::state::Modal::None;
        }

        KeyCode::Esc => {
            // Warn user about potential unexpected behavior and close the prompt
            app.toast_message = Some(crate::i18n::t(app, "app.toasts.gnome_terminal_warning"));
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(6));
            app.modal = crate::state::Modal::None;
        }
        _ => {}
    }
    false
}
