//! Common modal handlers (Alert, Help, News, `PreflightExec`, `PostSummary`, `GnomeTerminalPrompt`).

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::AppState;

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
/// - Handles navigation, Enter to open URL, keymap shortcuts for marking read
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
        for it in items.iter() {
            app.news_read_urls.insert(it.url.clone());
        }
        app.news_read_dirty = true;
        return false;
    }
    match ke.code {
        KeyCode::Esc => {
            app.modal = crate::state::Modal::None;
            return true; // Stop propagation to prevent global Esc handler from running
        }
        KeyCode::Up => {
            if *selected > 0 {
                *selected -= 1;
            }
        }
        KeyCode::Down => {
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
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Handles Esc/Enter to close, Up/Down/PageUp/PageDown for scrolling
pub(super) fn handle_updates(
    ke: KeyEvent,
    app: &mut AppState,
    entries: &[(String, String, String)],
    scroll: &mut u16,
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.modal = crate::state::Modal::None;
            return true; // Stop propagation
        }
        KeyCode::Up => {
            *scroll = scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            // Calculate max scroll based on content height
            // Each entry is 1 line, plus header (1 line), blank (1 line), footer (1 line), blank (1 line) = 4 lines
            let content_lines = u16::try_from(entries.len())
                .unwrap_or(u16::MAX)
                .saturating_add(4);
            // Estimate visible lines (modal height minus borders and title/footer)
            let max_scroll = content_lines.saturating_sub(10);
            if *scroll < max_scroll {
                *scroll = scroll.saturating_add(1);
            }
        }
        KeyCode::PageUp => {
            *scroll = scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            let content_lines = u16::try_from(entries.len())
                .unwrap_or(u16::MAX)
                .saturating_add(4);
            let max_scroll = content_lines.saturating_sub(10);
            *scroll = (*scroll + 10).min(max_scroll);
        }
        KeyCode::Char('d')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Ctrl+D: page down (20 lines)
            let content_lines = u16::try_from(entries.len())
                .unwrap_or(u16::MAX)
                .saturating_add(4);
            let max_scroll = content_lines.saturating_sub(10);
            *scroll = (*scroll + 25).min(max_scroll);
        }
        KeyCode::Char('u')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Ctrl+U: page up (20 lines)
            *scroll = scroll.saturating_sub(20);
        }
        _ => {}
    }
    false
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
