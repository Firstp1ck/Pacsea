//! Common modal handlers (Alert, Help, News, `PreflightExec`, `PostSummary`, `GnomeTerminalPrompt`).

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::{AppState, PackageItem, Source};

/// What: Show next pending announcement from queue if available.
///
/// Inputs:
/// - `app`: Application state to update
///
/// Output: None (modifies app state in place)
///
/// Details:
/// - Checks if there are pending announcements in the queue
/// - Shows the first valid announcement if modal is currently None
fn show_next_pending_announcement(app: &mut AppState) {
    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

    // Only show if no modal is currently displayed
    if !matches!(app.modal, crate::state::Modal::None) {
        tracing::debug!("skipping pending announcement check (modal still open)");
        return;
    }

    tracing::debug!(
        queue_size = app.pending_announcements.len(),
        "checking for pending announcements"
    );

    // Find next valid announcement in queue
    while let Some(announcement) = app.pending_announcements.first() {
        // Check version range
        if !crate::announcements::version_matches(
            CURRENT_VERSION,
            announcement.min_version.as_deref(),
            announcement.max_version.as_deref(),
        ) {
            tracing::debug!(
                id = %announcement.id,
                "pending announcement version range mismatch, removing from queue"
            );
            app.pending_announcements.remove(0);
            continue;
        }

        // Check expiration
        if crate::announcements::is_expired(announcement.expires.as_deref()) {
            tracing::debug!(
                id = %announcement.id,
                "pending announcement expired, removing from queue"
            );
            app.pending_announcements.remove(0);
            continue;
        }

        // Check if already read
        if app.announcements_read_ids.contains(&announcement.id) {
            tracing::debug!(
                id = %announcement.id,
                "pending announcement already read, removing from queue"
            );
            app.pending_announcements.remove(0);
            continue;
        }

        // Show this announcement
        let announcement = app.pending_announcements.remove(0);
        let announcement_id = announcement.id.clone();
        app.modal = crate::state::Modal::Announcement {
            title: announcement.title,
            content: announcement.content,
            id: announcement_id.clone(),
            scroll: 0,
        };
        tracing::info!(id = %announcement_id, "showing pending announcement");
        return;
    }

    tracing::debug!(
        queue_empty = app.pending_announcements.is_empty(),
        "no more pending announcements"
    );

    // After all announcements are shown, check for pending news
    if let Some(news_items) = app.pending_news.take()
        && !news_items.is_empty()
    {
        app.modal = crate::state::Modal::News {
            items: news_items,
            selected: 0,
        };
        tracing::info!("showing pending news after announcements");
    }
}

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
    let is_lockout = message.contains("locked") || message.contains("lockout");
    match ke.code {
        KeyCode::Esc => {
            if is_help {
                app.help_scroll = 0; // Reset scroll when closing
            }
            // For lockout alerts, clear any pending executor state to abort the process
            if is_lockout {
                app.pending_executor_password = None;
                app.pending_exec_header_chips = None;
                app.pending_executor_request = None;
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
            // For lockout alerts, clear any pending executor state to abort the process
            if is_lockout {
                app.pending_executor_password = None;
                app.pending_exec_header_chips = None;
                app.pending_executor_request = None;
            }
            // Restore previous modal if it was Preflight, otherwise close
            if let Some(prev_modal) = app.previous_modal.take() {
                app.modal = prev_modal;
            } else {
                app.modal = crate::state::Modal::None;
            }
            // Return true for lockout alerts to stop propagation and abort the process
            is_lockout
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
/// - `success`: Execution success status from the modal
///
/// Output:
/// - `true` when modal is closed/transitioned to stop propagation, `false` otherwise
///
/// Details:
/// - Handles Esc/q to close, Enter to show summary, l to toggle verbose, x to abort
/// - Success must be passed in since app.modal is taken during dispatch
pub(super) fn handle_preflight_exec(
    ke: KeyEvent,
    app: &mut AppState,
    verbose: &mut bool,
    abortable: bool,
    items: &[crate::state::PackageItem],
    success: Option<bool>,
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            true // Stop propagation
        }
        KeyCode::Enter => {
            // Check if this is a scan (items have names starting with "scan:")
            let is_scan = items.iter().any(|item| item.name.starts_with("scan:"));

            if is_scan {
                // For scans, skip PostSummary and go directly back to Preflight
                if let Some(prev_modal) = app.previous_modal.take() {
                    if matches!(prev_modal, crate::state::Modal::Preflight { .. }) {
                        app.modal = prev_modal;
                        return true; // Stop propagation
                    }
                    // If it's not Preflight, put it back and close normally
                    app.previous_modal = Some(prev_modal);
                }
                app.modal = crate::state::Modal::None;
                return true; // Stop propagation
            }

            // For regular installs, show loading modal and queue background computation
            // Use the success flag passed in from the modal (app.modal is taken during dispatch)
            app.pending_post_summary_items = Some((items.to_vec(), success));
            app.modal = crate::state::Modal::Loading {
                message: "Computing summary...".to_string(),
            };
            true // Stop propagation - transitioning to Loading
        }
        KeyCode::Char('l') => {
            *verbose = !*verbose;
            let verbose_status = if *verbose { "ON" } else { "OFF" };
            app.toast_message = Some(format!("Verbose: {verbose_status}"));
            false
        }
        // TODO: implement Logic for aborting the transaction
        KeyCode::Char('x') => {
            if abortable {
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.abort_requested"));
            }
            false
        }
        _ => false,
    }
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
/// - Returns `true` when closing modal to stop key propagation
pub(super) fn handle_post_summary(
    ke: KeyEvent,
    app: &mut AppState,
    services_pending: &[String],
) -> bool {
    match ke.code {
        // Close modal and stop propagation to prevent key from reaching other handlers
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            true // Stop propagation - prevents Enter from opening preflight again
        }
        KeyCode::Char('r') => {
            app.toast_message = Some(crate::i18n::t(app, "app.toasts.rollback"));
            false
        }
        // TODO: implement Logic for restarting the services
        KeyCode::Char('s') => {
            if services_pending.is_empty() {
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.no_services_to_restart"));
            } else {
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.restart_services"));
            }
            false
        }
        _ => false,
    }
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

/// What: Handle key events for Announcement modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `id`: Unique identifier for this announcement (version string or remote ID)
/// - `scroll`: Mutable scroll offset
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - "r" key: Mark as read (store ID, set dirty flag, close modal - won't show again)
/// - Enter/Esc/q: Dismiss temporarily (close modal without marking read - will show again)
/// - Arrow keys: Scroll content
pub(super) fn handle_announcement(
    ke: crossterm::event::KeyEvent,
    app: &mut AppState,
    id: &str,
    scroll: &mut u16,
) -> bool {
    match ke.code {
        crossterm::event::KeyCode::Char('r') => {
            // Mark as read - won't show again
            app.announcements_read_ids.insert(id.to_string());
            app.announcement_dirty = true;
            tracing::debug!(id = %id, "marked announcement as read, closing modal");
            app.modal = crate::state::Modal::None;
            // Show next pending announcement if any
            show_next_pending_announcement(app);
            return true; // Stop propagation
        }
        crossterm::event::KeyCode::Enter | crossterm::event::KeyCode::Esc => {
            // Dismiss temporarily - will show again on next startup
            tracing::debug!(id = %id, "dismissed announcement temporarily, closing modal");
            app.modal = crate::state::Modal::None;
            // Show next pending announcement if any
            show_next_pending_announcement(app);
            return true; // Stop propagation for both Enter and Esc
        }
        crossterm::event::KeyCode::Char('q') => {
            // Dismiss temporarily
            tracing::debug!(id = %id, "dismissed announcement temporarily, closing modal");
            app.modal = crate::state::Modal::None;
            // Show next pending announcement if any
            show_next_pending_announcement(app);
            return true; // Stop propagation
        }
        crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
            if *scroll > 0 {
                *scroll -= 1;
            }
        }
        crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
            *scroll = scroll.saturating_add(1);
        }
        crossterm::event::KeyCode::PageUp => {
            *scroll = scroll.saturating_sub(10);
        }
        crossterm::event::KeyCode::PageDown => {
            *scroll = scroll.saturating_add(10);
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
                let pkg_item =
                    if let Some(mut pkg_item) = crate::index::find_package_by_name(pkg_name) {
                        // Update the version to the new version
                        pkg_item.version.clone_from(new_version);
                        pkg_item
                    } else {
                        // If not found in official repos, assume it's an AUR package
                        PackageItem {
                            name: pkg_name.clone(),
                            version: new_version.clone(),
                            description: String::new(),
                            source: Source::Aur,
                            popularity: None,
                            out_of_date: None,
                            orphaned: false,
                        }
                    };

                // Close Updates modal
                app.modal = crate::state::Modal::None;

                // Open Preflight modal for this package (respects skip_preflight setting)
                crate::events::search::open_preflight_modal(app, vec![pkg_item], true);
            }
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
                // Properly quote the command to avoid syntax errors with complex shell constructs
                use crate::install::shell_single_quote;
                let quoted = shell_single_quote(&cmd);
                crate::install::spawn_shell_commands_in_terminal(&[format!(
                    "echo DRY RUN: {quoted}"
                )]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::announcements::RemoteAnnouncement;
    use crate::state::types::NewsItem;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// What: Create a test `KeyEvent` for testing.
    ///
    /// Inputs:
    /// - `code`: `KeyCode` to create event for.
    ///
    /// Output:
    /// - `KeyEvent` with the specified code and no modifiers.
    ///
    /// Details:
    /// - Helper function to create test key events.
    fn test_key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    #[test]
    /// What: Verify `show_next_pending_announcement` shows next valid announcement.
    ///
    /// Inputs:
    /// - `AppState` with pending announcements in queue.
    ///
    /// Output:
    /// - Shows the first valid announcement and sets modal.
    ///
    /// Details:
    /// - Should show the first announcement when modal is None.
    #[allow(clippy::field_reassign_with_default)]
    fn test_show_next_pending_announcement_shows_valid() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::None;

        let announcement = RemoteAnnouncement {
            id: "test-1".to_string(),
            title: "Test Title".to_string(),
            content: "Test content".to_string(),
            min_version: None,
            max_version: None,
            expires: None,
        };
        app.pending_announcements.push(announcement);

        show_next_pending_announcement(&mut app);

        match &app.modal {
            crate::state::Modal::Announcement { id, title, .. } => {
                assert_eq!(id, "test-1");
                assert_eq!(title, "Test Title");
            }
            _ => panic!("Expected Announcement modal"),
        }
        assert!(app.pending_announcements.is_empty());
    }

    #[test]
    /// What: Verify `show_next_pending_announcement` skips expired announcements.
    ///
    /// Inputs:
    /// - `AppState` with expired announcement in queue.
    ///
    /// Output:
    /// - Skips expired announcement and removes it from queue.
    ///
    /// Details:
    /// - Expired announcements should be removed without showing.
    #[allow(clippy::field_reassign_with_default)]
    fn test_show_next_pending_announcement_skips_expired() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::None;

        let expired = RemoteAnnouncement {
            id: "expired-1".to_string(),
            title: "Expired".to_string(),
            content: "Content".to_string(),
            min_version: None,
            max_version: None,
            expires: Some("2020-01-01".to_string()), // Past date
        };
        app.pending_announcements.push(expired);

        let valid = RemoteAnnouncement {
            id: "valid-1".to_string(),
            title: "Valid".to_string(),
            content: "Content".to_string(),
            min_version: None,
            max_version: None,
            expires: None,
        };
        app.pending_announcements.push(valid);

        show_next_pending_announcement(&mut app);

        // Should skip expired and show valid
        match &app.modal {
            crate::state::Modal::Announcement { id, .. } => {
                assert_eq!(id, "valid-1");
            }
            _ => panic!("Expected Announcement modal"),
        }
        assert!(app.pending_announcements.is_empty());
    }

    #[test]
    /// What: Verify `show_next_pending_announcement` skips already-read announcements.
    ///
    /// Inputs:
    /// - `AppState` with already-read announcement in queue.
    ///
    /// Output:
    /// - Skips read announcement and removes it from queue.
    ///
    /// Details:
    /// - Already-read announcements should be removed without showing.
    #[allow(clippy::field_reassign_with_default)]
    fn test_show_next_pending_announcement_skips_read() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::None;

        let read = RemoteAnnouncement {
            id: "read-1".to_string(),
            title: "Read".to_string(),
            content: "Content".to_string(),
            min_version: None,
            max_version: None,
            expires: None,
        };
        app.announcements_read_ids.insert("read-1".to_string());
        app.pending_announcements.push(read);

        let unread = RemoteAnnouncement {
            id: "unread-1".to_string(),
            title: "Unread".to_string(),
            content: "Content".to_string(),
            min_version: None,
            max_version: None,
            expires: None,
        };
        app.pending_announcements.push(unread);

        show_next_pending_announcement(&mut app);

        // Should skip read and show unread
        match &app.modal {
            crate::state::Modal::Announcement { id, .. } => {
                assert_eq!(id, "unread-1");
            }
            _ => panic!("Expected Announcement modal"),
        }
        assert!(app.pending_announcements.is_empty());
    }

    #[test]
    /// What: Verify `show_next_pending_announcement` skips version-mismatched announcements.
    ///
    /// Inputs:
    /// - `AppState` with version-mismatched announcement in queue.
    ///
    /// Output:
    /// - Skips mismatched announcement and removes it from queue.
    ///
    /// Details:
    /// - Announcements outside version range should be removed without showing.
    #[allow(clippy::field_reassign_with_default)]
    fn test_show_next_pending_announcement_skips_version_mismatch() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::None;

        // Assuming current version is something like "0.6.0", this should be out of range
        let mismatched = RemoteAnnouncement {
            id: "mismatch-1".to_string(),
            title: "Mismatch".to_string(),
            content: "Content".to_string(),
            min_version: Some("999.0.0".to_string()), // Future version
            max_version: None,
            expires: None,
        };
        app.pending_announcements.push(mismatched);

        let valid = RemoteAnnouncement {
            id: "valid-1".to_string(),
            title: "Valid".to_string(),
            content: "Content".to_string(),
            min_version: None,
            max_version: None,
            expires: None,
        };
        app.pending_announcements.push(valid);

        show_next_pending_announcement(&mut app);

        // Should skip mismatched and show valid
        match &app.modal {
            crate::state::Modal::Announcement { id, .. } => {
                assert_eq!(id, "valid-1");
            }
            _ => panic!("Expected Announcement modal"),
        }
        assert!(app.pending_announcements.is_empty());
    }

    #[test]
    /// What: Verify `show_next_pending_announcement` shows pending news after all announcements.
    ///
    /// Inputs:
    /// - `AppState` with no pending announcements but pending news.
    ///
    /// Output:
    /// - Shows News modal after announcements are exhausted.
    ///
    /// Details:
    /// - After all announcements are shown, pending news should be displayed.
    #[allow(clippy::field_reassign_with_default)]
    fn test_show_next_pending_announcement_shows_news_after() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::None;

        let news_items = vec![NewsItem {
            date: "2025-01-01".to_string(),
            title: "Test News".to_string(),
            url: "https://example.com/news".to_string(),
        }];
        app.pending_news = Some(news_items);

        show_next_pending_announcement(&mut app);

        // Should show news modal
        match &app.modal {
            crate::state::Modal::News { items, .. } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].title, "Test News");
            }
            _ => panic!("Expected News modal"),
        }
        assert!(app.pending_news.is_none());
    }

    #[test]
    /// What: Verify `handle_announcement` 'r' key marks as read and closes.
    ///
    /// Inputs:
    /// - `KeyEvent` with 'r' key.
    /// - `AppState` with announcement modal open.
    ///
    /// Output:
    /// - Marks announcement as read, closes modal, shows next pending.
    ///
    /// Details:
    /// - `r` key should mark announcement as read permanently.
    #[allow(clippy::field_reassign_with_default)]
    fn test_handle_announcement_mark_read() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::Announcement {
            title: "Test".to_string(),
            content: "Content".to_string(),
            id: "test-1".to_string(),
            scroll: 0,
        };

        let mut scroll = 0u16;
        let ke = test_key_event(KeyCode::Char('r'));
        let result = handle_announcement(ke, &mut app, "test-1", &mut scroll);

        assert!(result); // Should stop propagation
        assert!(app.announcements_read_ids.contains("test-1"));
        assert!(app.announcement_dirty);
        assert!(matches!(app.modal, crate::state::Modal::None));
    }

    #[test]
    /// What: Verify `handle_announcement` Enter/Esc/q dismisses temporarily.
    ///
    /// Inputs:
    /// - `KeyEvent` with Enter, Esc, or 'q' key.
    /// - `AppState` with announcement modal open.
    ///
    /// Output:
    /// - Closes modal without marking as read, shows next pending.
    ///
    /// Details:
    /// - `Enter`, `Esc`, and `q` should dismiss temporarily (will show again on next startup).
    #[allow(clippy::field_reassign_with_default)]
    fn test_handle_announcement_dismiss_temporary() {
        for key_code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Char('q')] {
            let mut app = crate::state::AppState::default();
            app.modal = crate::state::Modal::Announcement {
                title: "Test".to_string(),
                content: "Content".to_string(),
                id: "test-1".to_string(),
                scroll: 0,
            };

            let mut scroll = 0u16;
            let ke = test_key_event(key_code);
            let result = handle_announcement(ke, &mut app, "test-1", &mut scroll);

            assert!(result); // Should stop propagation
            assert!(!app.announcements_read_ids.contains("test-1")); // Not marked as read
            assert!(matches!(app.modal, crate::state::Modal::None));
        }
    }

    #[test]
    /// What: Verify `handle_announcement` Up/Down/j/k scrolls content.
    ///
    /// Inputs:
    /// - `KeyEvent` with Up, Down, 'j', or 'k' key.
    /// - `AppState` with announcement modal open and scroll position.
    ///
    /// Output:
    /// - Scroll position is updated correctly.
    ///
    /// Details:
    /// - `Up`/`k` should decrease scroll, `Down`/`j` should increase scroll.
    #[allow(clippy::field_reassign_with_default)]
    fn test_handle_announcement_scroll() {
        // Test Up/k decreases scroll
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::Announcement {
            title: "Test".to_string(),
            content: "Content".to_string(),
            id: "test-1".to_string(),
            scroll: 5,
        };

        let mut scroll = 5u16;
        let ke_up = test_key_event(KeyCode::Up);
        handle_announcement(ke_up, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 4);

        let ke_k = test_key_event(KeyCode::Char('k'));
        handle_announcement(ke_k, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 3);

        // Test scroll doesn't go below 0
        scroll = 0;
        let ke_up_zero = test_key_event(KeyCode::Up);
        handle_announcement(ke_up_zero, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 0);

        // Test Down/j increases scroll
        scroll = 0;
        let ke_down = test_key_event(KeyCode::Down);
        handle_announcement(ke_down, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 1);

        let ke_j = test_key_event(KeyCode::Char('j'));
        handle_announcement(ke_j, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 2);
    }

    #[test]
    /// What: Verify `handle_announcement` PageUp/PageDown scrolls by 10.
    ///
    /// Inputs:
    /// - `KeyEvent` with `PageUp` or `PageDown` key.
    /// - `AppState` with announcement modal open and scroll position.
    ///
    /// Output:
    /// - Scroll position is updated by 10 lines.
    ///
    /// Details:
    /// - `PageUp` should decrease scroll by 10, `PageDown` should increase by 10.
    #[allow(clippy::field_reassign_with_default)]
    fn test_handle_announcement_page_scroll() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::Announcement {
            title: "Test".to_string(),
            content: "Content".to_string(),
            id: "test-1".to_string(),
            scroll: 20,
        };

        let mut scroll = 20u16;

        // Test PageUp decreases by 10
        let ke_page_up = test_key_event(KeyCode::PageUp);
        handle_announcement(ke_page_up, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 10);

        // Test PageDown increases by 10
        let ke_page_down = test_key_event(KeyCode::PageDown);
        handle_announcement(ke_page_down, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 20);

        // Test PageUp doesn't go below 0 (saturating_sub)
        scroll = 5;
        let ke_page_up_saturate = test_key_event(KeyCode::PageUp);
        handle_announcement(ke_page_up_saturate, &mut app, "test-1", &mut scroll);
        assert_eq!(scroll, 0); // Should saturate at 0
    }

    #[test]
    /// What: Verify `handle_announcement` shows next pending after marking as read.
    ///
    /// Inputs:
    /// - `KeyEvent` with 'r' key.
    /// - `AppState` with announcement modal and pending announcements.
    ///
    /// Output:
    /// - Shows next pending announcement after marking current as read.
    ///
    /// Details:
    /// - After marking as read, should automatically show next pending announcement.
    #[allow(clippy::field_reassign_with_default)]
    fn test_handle_announcement_shows_next_after_read() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::Announcement {
            title: "Test 1".to_string(),
            content: "Content".to_string(),
            id: "test-1".to_string(),
            scroll: 0,
        };

        let next = RemoteAnnouncement {
            id: "test-2".to_string(),
            title: "Test 2".to_string(),
            content: "Content".to_string(),
            min_version: None,
            max_version: None,
            expires: None,
        };
        app.pending_announcements.push(next);

        let mut scroll = 0u16;
        let ke = test_key_event(KeyCode::Char('r'));
        handle_announcement(ke, &mut app, "test-1", &mut scroll);

        // Should show next announcement
        match &app.modal {
            crate::state::Modal::Announcement { id, .. } => {
                assert_eq!(id, "test-2");
            }
            _ => panic!("Expected Announcement modal with next announcement"),
        }
    }
}
