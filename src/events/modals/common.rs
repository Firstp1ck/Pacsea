//! Common modal handlers (Alert, Help, News, `PreflightExec`, `PostSummary`, `GnomeTerminalPrompt`).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::{AppState, PackageItem, Source};

/// What: Build startup setup steps in dependency-aware execution order.
#[must_use]
pub(super) fn startup_setup_steps_in_priority(
    selected: &std::collections::HashSet<crate::state::modal::StartupSetupTask>,
) -> VecDeque<crate::state::modal::StartupSetupTask> {
    let mut steps = VecDeque::new();
    let ordered = [
        crate::state::modal::StartupSetupTask::OptionalDepsMissing,
        crate::state::modal::StartupSetupTask::SudoTimestampSetup,
        crate::state::modal::StartupSetupTask::DoasPersistSetup,
        crate::state::modal::StartupSetupTask::SshAurSetup,
        crate::state::modal::StartupSetupTask::AurSleuthSetup,
        crate::state::modal::StartupSetupTask::VirusTotalSetup,
        crate::state::modal::StartupSetupTask::ArchNews,
    ];
    for step in ordered {
        if selected.contains(&step) {
            steps.push_back(step);
        }
    }
    steps
}

/// What: Advance first-startup selected setup steps one by one until a modal opens.
///
/// Inputs:
/// - `app`: Application state containing pending startup setup queue.
///
/// Output:
/// - Opens the next setup modal when available, or resumes normal startup popup sequence.
pub(super) fn show_next_startup_setup_step(app: &mut AppState) {
    while matches!(app.modal, crate::state::Modal::None) {
        let Some(next_step) = app.pending_startup_setup_steps.pop_front() else {
            show_next_pending_announcement(app);
            return;
        };
        match next_step {
            crate::state::modal::StartupSetupTask::ArchNews => {
                let prefs = crate::theme::settings();
                app.modal = crate::state::Modal::NewsSetup {
                    show_arch_news: prefs.startup_news_show_arch_news,
                    show_advisories: prefs.startup_news_show_advisories,
                    show_aur_updates: prefs.startup_news_show_aur_updates,
                    show_aur_comments: prefs.startup_news_show_aur_comments,
                    show_pkg_updates: prefs.startup_news_show_pkg_updates,
                    max_age_days: prefs.startup_news_max_age_days,
                    cursor: 0,
                };
            }
            crate::state::modal::StartupSetupTask::OptionalDepsMissing => {
                let rows: Vec<crate::state::types::OptionalDepRow> =
                    crate::events::mouse::menu_options::build_optional_deps_rows(app)
                        .into_iter()
                        .filter(|row| {
                            !row.installed
                                && row.selectable
                                && !matches!(
                                    row.package.as_str(),
                                    "aur-ssh-setup"
                                        | "virustotal-setup"
                                        | "aur-sleuth-setup"
                                        | "sudo-timestamp-setup"
                                        | "doas-persist-setup"
                                )
                        })
                        .collect();
                if rows.is_empty() {
                    continue;
                }
                app.modal = crate::state::Modal::OptionalDeps {
                    rows,
                    selected: 0,
                    selected_pkg_names: std::collections::HashSet::new(),
                };
            }
            crate::state::modal::StartupSetupTask::SshAurSetup => {
                if app.aur_ssh_help_ready.unwrap_or(false) {
                    continue;
                }
                super::optional_deps::open_setup_package(app, "aur-ssh-setup");
            }
            crate::state::modal::StartupSetupTask::AurSleuthSetup => {
                super::optional_deps::open_setup_package(app, "aur-sleuth-setup");
            }
            crate::state::modal::StartupSetupTask::VirusTotalSetup => {
                super::optional_deps::open_setup_package(app, "virustotal-setup");
            }
            crate::state::modal::StartupSetupTask::SudoTimestampSetup => {
                super::optional_deps::open_setup_package(app, "sudo-timestamp-setup");
            }
            crate::state::modal::StartupSetupTask::DoasPersistSetup => {
                super::optional_deps::open_setup_package(app, "doas-persist-setup");
            }
        }
    }
}

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
pub(super) fn show_next_pending_announcement(app: &mut AppState) {
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

    // Startup auto-popup for pending news is disabled by design.
    // Keep pending_news untouched so News mode can still consume/render data.
    tracing::debug!(
        pending_news_exists = app.pending_news.is_some(),
        news_loading = app.news_loading,
        "startup pending news auto-popup disabled"
    );
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
                super::repositories::reopen_repositories_modal_if_pending(app);
            }
            true // Stop propagation to prevent mode toggle
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
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
                super::repositories::reopen_repositories_modal_if_pending(app);
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

/// What: After a successful full repo apply, optionally open the foreign overlap wizard or an alert.
///
/// Inputs:
/// - `app`: Application state (consumes `pending_repo_apply_overlap_check` when set).
/// - `success`: `PreflightExec` completion flag.
/// - `items`: Packages associated with the run (empty for repo apply).
///
/// Output:
/// - `true` when this handler took over (`Alert` or `ForeignRepoOverlap`); caller should stop Enter handling.
///
/// Details:
/// - No-op unless `success == Some(true)` and `items` is empty with a pending overlap marker.
fn consume_repo_apply_overlap_on_preflight_exec_enter(
    app: &mut AppState,
    success: Option<bool>,
    items: &[PackageItem],
) -> bool {
    if success != Some(true) || !items.is_empty() {
        return false;
    }
    if !crate::logic::repos::repositories_linux_actions_supported() {
        return false;
    }
    let Some(pending) = app.pending_repo_apply_overlap_check.take() else {
        return false;
    };
    match crate::logic::repos::analyze_foreign_repo_overlap_with_qm_snapshot(
        &pending.repo_section,
        pending.pre_apply_foreign_snapshot.as_deref(),
    ) {
        Err(msg) => {
            app.modal = crate::state::Modal::Alert { message: msg };
            true
        }
        Ok(analysis) if analysis.entries.is_empty() => {
            if analysis.foreign_pkg_count > 0 {
                if analysis.sync_pkg_name_count == 0 {
                    app.toast_message = Some(crate::i18n::t_fmt1(
                        app,
                        "app.toasts.repo_overlap_no_sync_list",
                        &pending.repo_section,
                    ));
                } else {
                    app.toast_message = Some(crate::i18n::t_fmt1(
                        app,
                        "app.toasts.repo_overlap_no_matching_names",
                        &pending.repo_section,
                    ));
                }
                app.toast_expires_at = Some(Instant::now() + Duration::from_secs(10));
            }
            false
        }
        Ok(analysis) => {
            let repo_name = pending.repo_section;
            let entries: Vec<_> = analysis
                .entries
                .into_iter()
                .map(|e| (e.name, e.version))
                .collect();
            app.modal = crate::state::Modal::ForeignRepoOverlap {
                repo_name,
                entries,
                phase: crate::state::modal::ForeignRepoOverlapPhase::WarnAck {
                    step: 0,
                    list_scroll: 0,
                },
            };
            true
        }
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
            app.pending_repo_apply_overlap_check = None;
            let reopen_ok = success == Some(true);
            if !reopen_ok {
                app.pending_repositories_modal_resume = None;
            }
            app.modal = crate::state::Modal::None;
            if success == Some(false) && !app.pending_startup_setup_steps.is_empty() {
                show_next_startup_setup_step(app);
                return true;
            }
            if reopen_ok {
                super::repositories::reopen_repositories_modal_if_pending(app);
            }
            true // Stop propagation
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            if consume_repo_apply_overlap_on_preflight_exec_enter(app, success, items) {
                return true;
            }
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
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | '\n' | '\r') => {
            app.modal = crate::state::Modal::None;
            super::repositories::reopen_repositories_modal_if_pending(app);
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
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            app.modal = crate::state::Modal::None;
            false
        }
        _ => false,
    }
}

/// What: Calculate scroll offset to keep the selected item in the middle of the viewport.
///
/// Inputs:
/// - `selected`: Currently selected item index
/// - `total_items`: Total number of items in the list
/// - `visible_height`: Height of the visible content area (in lines)
///
/// Output:
/// - Scroll offset (lines) that centers the selected item
///
/// Details:
/// - Calculates scroll so selected item is in the middle of visible area
/// - Ensures scroll doesn't go negative or past the end
#[cfg_attr(test, allow(dead_code))]
fn calculate_news_scroll_for_selection(
    selected: usize,
    total_items: usize,
    visible_height: u16,
) -> u16 {
    if total_items == 0 || visible_height == 0 {
        return 0;
    }

    // Clamp values to u16::MAX to prevent overflow in calculations.
    // Note: If selected or total_items exceeds u16::MAX, the scroll calculation will be
    // performed for the clamped values, which may not match the actual selected item.
    // This is acceptable since u16::MAX (65535) is far beyond practical UI list sizes.
    let selected_line = u16::try_from(selected).unwrap_or(u16::MAX);
    let total_lines = u16::try_from(total_items).unwrap_or(u16::MAX);
    // Ensure selected doesn't exceed total after clamping to maintain valid calculations
    let selected_line = selected_line.min(total_lines);

    // Calculate middle position: we want selected item to be at visible_height / 2
    let middle_offset = visible_height / 2;

    // Calculate desired scroll to center the selection
    let desired_scroll = selected_line.saturating_sub(middle_offset);

    // Calculate maximum scroll (when last item is at the bottom)
    let max_scroll = total_lines.saturating_sub(visible_height);

    // Clamp scroll to valid range
    desired_scroll.min(max_scroll)
}

#[cfg(test)]
mod news_tests {
    use super::*;
    use crate::state::{AppState, types::NewsFeedItem, types::NewsFeedSource};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    /// What: Test `calculate_news_scroll_for_selection` centers selected item.
    ///
    /// Inputs:
    /// - Selected index, total items, visible height.
    ///
    /// Output:
    /// - Scroll offset that centers selection within viewport bounds.
    ///
    /// Details:
    /// - Verifies scroll calculation clamps to valid range.
    fn test_calculate_news_scroll_for_selection() {
        // Test: center item in middle of list
        let scroll = calculate_news_scroll_for_selection(5, 10, 5);
        assert!(scroll <= 5, "Scroll should not exceed max");

        // Test: first item (should scroll to 0)
        let scroll = calculate_news_scroll_for_selection(0, 10, 5);
        assert_eq!(scroll, 0, "First item should have scroll 0");

        // Test: empty list
        let scroll = calculate_news_scroll_for_selection(0, 0, 5);
        assert_eq!(scroll, 0, "Empty list should return 0");

        // Test: zero height
        let scroll = calculate_news_scroll_for_selection(5, 10, 0);
        assert_eq!(scroll, 0, "Zero height should return 0");
    }

    #[test]
    /// What: Test `handle_news` marks item as read when keymap chord is pressed.
    ///
    /// Inputs:
    /// - News modal with items, keymap chord for mark-read.
    ///
    /// Output:
    /// - Selected item added to `news_read_ids` and `news_read_urls`, dirty flags set.
    ///
    /// Details:
    /// - Verifies read-state mutation and dirty flag handling.
    /// - Only works in normal mode.
    #[allow(clippy::field_reassign_with_default)] // Field assignment in tests is acceptable for test setup
    fn test_handle_news_mark_read() {
        let mut app = AppState::default();
        app.search_normal_mode = true; // Must be in normal mode
        app.keymap.news_mark_read = [crate::theme::KeyChord {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::empty(),
        }]
        .into();

        let items = vec![NewsFeedItem {
            id: "test-id-1".to_string(),
            date: "2025-01-01".to_string(),
            title: "Test News".to_string(),
            summary: None,
            url: Some("https://example.com/news/1".to_string()),
            source: NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        }];

        let mut selected = 0;
        let mut scroll = 0;
        let mut ke = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty());
        ke.kind = crossterm::event::KeyEventKind::Press;

        let _ = handle_news(ke, &mut app, &items, &mut selected, &mut scroll);

        assert!(app.news_read_ids.contains("test-id-1"));
        assert!(app.news_read_urls.contains("https://example.com/news/1"));
        assert!(app.news_read_ids_dirty);
        assert!(app.news_read_dirty);
    }

    #[test]
    /// What: Test `handle_news` marks all items as read when mark-all-read chord is pressed.
    ///
    /// Inputs:
    /// - News modal with multiple items, keymap chord for mark-all-read.
    ///
    /// Output:
    /// - All items added to read sets, dirty flags set.
    ///
    /// Details:
    /// - Verifies bulk read-state mutation.
    /// - Only works in normal mode.
    #[allow(clippy::field_reassign_with_default)] // Field assignment in tests is acceptable for test setup
    fn test_handle_news_mark_all_read() {
        let mut app = AppState::default();
        app.search_normal_mode = true; // Must be in normal mode
        app.keymap.news_mark_all_read = [crate::theme::KeyChord {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::CONTROL,
        }]
        .into();

        let items = vec![
            NewsFeedItem {
                id: "test-id-1".to_string(),
                date: "2025-01-01".to_string(),
                title: "Test News 1".to_string(),
                summary: None,
                url: Some("https://example.com/news/1".to_string()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
            NewsFeedItem {
                id: "test-id-2".to_string(),
                date: "2025-01-02".to_string(),
                title: "Test News 2".to_string(),
                summary: None,
                url: Some("https://example.com/news/2".to_string()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
        ];

        let mut selected = 0;
        let mut scroll = 0;
        let mut ke = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        ke.kind = crossterm::event::KeyEventKind::Press;

        let _ = handle_news(ke, &mut app, &items, &mut selected, &mut scroll);

        assert!(app.news_read_ids.contains("test-id-1"));
        assert!(app.news_read_ids.contains("test-id-2"));
        assert!(app.news_read_urls.contains("https://example.com/news/1"));
        assert!(app.news_read_urls.contains("https://example.com/news/2"));
        assert!(app.news_read_ids_dirty);
        assert!(app.news_read_dirty);
    }

    #[test]
    /// What: Test `handle_news` navigation updates selection and scroll.
    ///
    /// Inputs:
    /// - News modal with items, navigation keys (Up/Down).
    ///
    /// Output:
    /// - Selection index updated, scroll recalculated.
    ///
    /// Details:
    /// - Verifies navigation updates selection and scroll centering.
    #[allow(clippy::field_reassign_with_default)] // Field assignment in tests is acceptable for test setup
    fn test_handle_news_navigation() {
        let mut app = AppState::default();
        app.news_list_rect = Some((0, 0, 50, 10)); // visible height = 10

        let items = vec![
            NewsFeedItem {
                id: "test-id-1".to_string(),
                date: "2025-01-01".to_string(),
                title: "Test News 1".to_string(),
                summary: None,
                url: None,
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
            NewsFeedItem {
                id: "test-id-2".to_string(),
                date: "2025-01-02".to_string(),
                title: "Test News 2".to_string(),
                summary: None,
                url: None,
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
        ];

        let mut selected = 0;
        let mut scroll = 0;

        // Test Down key
        let mut ke = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
        ke.kind = crossterm::event::KeyEventKind::Press;
        let _ = handle_news(ke, &mut app, &items, &mut selected, &mut scroll);
        assert_eq!(selected, 1, "Down should increment selection");

        // Test Up key
        let mut ke = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
        ke.kind = crossterm::event::KeyEventKind::Press;
        let _ = handle_news(ke, &mut app, &items, &mut selected, &mut scroll);
        assert_eq!(selected, 0, "Up should decrement selection");
    }

    #[test]
    /// What: Test `handle_news` does not mark item as read when in insert mode.
    ///
    /// Inputs:
    /// - News modal with items, keymap chord for mark-read, but in insert mode.
    ///
    /// Output:
    /// - Item should NOT be added to read sets when in insert mode.
    ///
    /// Details:
    /// - Verifies that mark read only works in normal mode, not insert mode.
    #[allow(clippy::field_reassign_with_default)] // Field assignment in tests is acceptable for test setup
    fn test_handle_news_mark_read_insert_mode() {
        let mut app = AppState::default();
        app.search_normal_mode = false; // Insert mode
        app.keymap.news_mark_read = [crate::theme::KeyChord {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::empty(),
        }]
        .into();

        let items = vec![NewsFeedItem {
            id: "test-id-1".to_string(),
            date: "2025-01-01".to_string(),
            title: "Test News".to_string(),
            summary: None,
            url: Some("https://example.com/news/1".to_string()),
            source: NewsFeedSource::ArchNews,
            severity: None,
            packages: Vec::new(),
        }];

        let mut selected = 0;
        let mut scroll = 0;
        let mut ke = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty());
        ke.kind = crossterm::event::KeyEventKind::Press;

        let _ = handle_news(ke, &mut app, &items, &mut selected, &mut scroll);

        // In insert mode, 'r' should not mark as read
        assert!(!app.news_read_ids.contains("test-id-1"));
        assert!(!app.news_read_urls.contains("https://example.com/news/1"));
        assert!(!app.news_read_ids_dirty);
        assert!(!app.news_read_dirty);
    }

    #[test]
    /// What: Test `handle_news` does not mark all items as read when in insert mode.
    ///
    /// Inputs:
    /// - News modal with multiple items, keymap chord for mark-all-read, but in insert mode.
    ///
    /// Output:
    /// - Items should NOT be added to read sets when in insert mode.
    ///
    /// Details:
    /// - Verifies that mark all read only works in normal mode, not insert mode.
    #[allow(clippy::field_reassign_with_default)] // Field assignment in tests is acceptable for test setup
    fn test_handle_news_mark_all_read_insert_mode() {
        let mut app = AppState::default();
        app.search_normal_mode = false; // Insert mode
        app.keymap.news_mark_all_read = [crate::theme::KeyChord {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::CONTROL,
        }]
        .into();

        let items = vec![
            NewsFeedItem {
                id: "test-id-1".to_string(),
                date: "2025-01-01".to_string(),
                title: "Test News 1".to_string(),
                summary: None,
                url: Some("https://example.com/news/1".to_string()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
            NewsFeedItem {
                id: "test-id-2".to_string(),
                date: "2025-01-02".to_string(),
                title: "Test News 2".to_string(),
                summary: None,
                url: Some("https://example.com/news/2".to_string()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            },
        ];

        let mut selected = 0;
        let mut scroll = 0;
        let mut ke = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        ke.kind = crossterm::event::KeyEventKind::Press;

        let _ = handle_news(ke, &mut app, &items, &mut selected, &mut scroll);

        // In insert mode, Ctrl+R should not mark as read
        assert!(!app.news_read_ids.contains("test-id-1"));
        assert!(!app.news_read_ids.contains("test-id-2"));
        assert!(!app.news_read_urls.contains("https://example.com/news/1"));
        assert!(!app.news_read_urls.contains("https://example.com/news/2"));
        assert!(!app.news_read_ids_dirty);
        assert!(!app.news_read_dirty);
    }
}

/// What: Handle key events for News modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `items`: News feed items
/// - `selected`: Currently selected item index
/// - `scroll`: Mutable scroll offset
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Handles Esc/q to close, navigation, Enter to open URL, keymap shortcuts for marking read
/// - Updates scroll to keep selection centered
/// - Mark read actions only work in normal mode (not insert mode)
pub(super) fn handle_news(
    ke: KeyEvent,
    app: &mut AppState,
    items: &[crate::state::types::NewsFeedItem],
    selected: &mut usize,
    scroll: &mut u16,
) -> bool {
    let km = &app.keymap;
    if crate::events::utils::matches_any(&ke, &km.news_mark_read) {
        // Mark as read only works in normal mode
        if !app.search_normal_mode {
            return false;
        }
        if let Some(it) = items.get(*selected) {
            // Mark as read using id (primary) and url if available
            app.news_read_ids.insert(it.id.clone());
            app.news_read_ids_dirty = true;
            if let Some(url) = &it.url {
                app.news_read_urls.insert(url.clone());
                app.news_read_dirty = true;
            }
        }
        return false;
    }
    if crate::events::utils::matches_any(&ke, &km.news_mark_all_read) {
        // Mark all as read only works in normal mode
        if !app.search_normal_mode {
            return false;
        }
        for it in items {
            app.news_read_ids.insert(it.id.clone());
            if let Some(url) = &it.url {
                app.news_read_urls.insert(url.clone());
            }
        }
        app.news_read_ids_dirty = true;
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
                // Update scroll to keep selection centered
                if let Some((_, _, _, visible_h)) = app.news_list_rect {
                    *scroll =
                        calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected + 1 < items.len() {
                *selected += 1;
                // Update scroll to keep selection centered
                if let Some((_, _, _, visible_h)) = app.news_list_rect {
                    *scroll =
                        calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
                }
            }
        }
        KeyCode::PageUp => {
            if *selected >= 10 {
                *selected -= 10;
            } else {
                *selected = 0;
            }
            if let Some((_, _, _, visible_h)) = app.news_list_rect {
                *scroll = calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
            }
        }
        KeyCode::PageDown => {
            let max_idx = items.len().saturating_sub(1);
            *selected = (*selected + 10).min(max_idx);
            if let Some((_, _, _, visible_h)) = app.news_list_rect {
                *scroll = calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
            }
        }
        KeyCode::Char('d')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Ctrl+D: page down (25 lines)
            let max_idx = items.len().saturating_sub(1);
            *selected = (*selected + 25).min(max_idx);
            if let Some((_, _, _, visible_h)) = app.news_list_rect {
                *scroll = calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
            }
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
            if let Some((_, _, _, visible_h)) = app.news_list_rect {
                *scroll = calculate_news_scroll_for_selection(*selected, items.len(), visible_h);
            }
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            if let Some(it) = items.get(*selected)
                && let Some(url) = &it.url
            {
                crate::util::open_url(url);
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
        crossterm::event::KeyCode::Enter
        | crossterm::event::KeyCode::Char('\n' | '\r')
        | crossterm::event::KeyCode::Esc => {
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
#[allow(clippy::too_many_arguments)]
// Updates modal needs coordinated mutable state (selection, scroll, and filter state) in one handler.
#[allow(clippy::too_many_lines)] // Keeps full keymap behavior in a single readable dispatch for modal navigation and filter mode.
#[allow(clippy::cognitive_complexity)] // Consolidating key dispatch here preserves modal behavior locality across filter, navigation, and batch actions.
pub(super) fn handle_updates(
    ke: KeyEvent,
    app: &mut AppState,
    entries: &[(String, String, String)],
    scroll: &mut u16,
    selected: &mut usize,
    filter_active: &mut bool,
    filter_query: &mut String,
    filter_caret: &mut usize,
    last_selected_pkg_name: &mut Option<String>,
    filtered_indices: &mut Vec<usize>,
    selected_pkg_names: &mut std::collections::HashSet<String>,
) -> bool {
    let sync_scroll =
        |app: &AppState, scroll: &mut u16, selected_original: usize, filtered: &[usize]| {
            let visible_selected =
                updates_visible_index_for_selected(selected_original, filtered).unwrap_or(0);
            let visible_count = filtered.len();
            if visible_count == 0 {
                *scroll = 0;
                return;
            }
            *scroll = crate::events::utils::compute_updates_modal_scroll_for_selection(
                &app.updates_modal_entry_line_starts,
                app.updates_modal_total_lines,
                app.updates_modal_content_rect,
                visible_selected,
                visible_count,
                *scroll,
            );
        };
    if filtered_indices.is_empty() && (!*filter_active || filter_query.trim().is_empty()) {
        *filtered_indices = (0..entries.len()).collect();
    }
    if *selected >= entries.len() {
        *selected = entries.len().saturating_sub(1);
    }
    if let Some((name, _, _)) = entries.get(*selected) {
        *last_selected_pkg_name = Some(name.clone());
    }

    if *filter_active {
        if handle_updates_filter_editing(
            ke,
            entries,
            scroll,
            selected,
            filter_active,
            filter_query,
            filter_caret,
            last_selected_pkg_name,
            filtered_indices,
            selected_pkg_names,
            &sync_scroll,
            app,
        ) {
            return false;
        }
    } else if matches!(ke.code, KeyCode::Char('/')) {
        *filter_active = true;
        *filter_caret = crate::events::utils::char_count(filter_query);
        return false;
    }

    let plain_lower_g = matches!(ke.code, KeyCode::Char('g')) && ke.modifiers.is_empty();
    if !plain_lower_g {
        clear_updates_pending_g(app);
    }

    match ke.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            return true; // Stop propagation
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            let selected_items = collect_selected_update_items(entries, selected_pkg_names);
            if !selected_items.is_empty() {
                app.modal = crate::state::Modal::None;
                crate::events::search::open_preflight_modal(app, selected_items, true);
                return true; // Stop propagation
            }

            // Install/update the focused package when no batch selection exists.
            if let Some((pkg_name, _, new_version)) = entries.get(*selected) {
                let pkg_item = package_item_for_update_entry(pkg_name, new_version);
                app.modal = crate::state::Modal::None;
                crate::events::search::open_preflight_modal(app, vec![pkg_item], true);
            }
            return true; // Stop propagation
        }
        KeyCode::Char(' ') => {
            if let Some((name, _, _)) = entries.get(*selected)
                && !selected_pkg_names.remove(name)
            {
                selected_pkg_names.insert(name.clone());
            }
        }
        KeyCode::Char('a') if ke.modifiers.is_empty() => {
            for &idx in filtered_indices.iter() {
                if let Some((name, _, _)) = entries.get(idx) {
                    selected_pkg_names.insert(name.clone());
                }
            }
        }
        KeyCode::Home => {
            *selected = filtered_indices.first().copied().unwrap_or(0);
            sync_scroll(app, scroll, *selected, filtered_indices);
        }
        KeyCode::End | KeyCode::Char('G') => {
            *selected = filtered_indices.last().copied().unwrap_or(0);
            sync_scroll(app, scroll, *selected, filtered_indices);
        }
        KeyCode::Char('g') if ke.modifiers.is_empty() => {
            let now = Instant::now();
            if is_updates_pending_g_active(app, now) {
                clear_updates_pending_g(app);
                *selected = filtered_indices.first().copied().unwrap_or(0);
                sync_scroll(app, scroll, *selected, filtered_indices);
            } else {
                app.updates_modal_pending_g_at = Some(now);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(visible) = updates_visible_index_for_selected(*selected, filtered_indices)
                && visible > 0
            {
                *selected = filtered_indices[visible - 1];
                // Auto-scroll to keep selected item visible
                sync_scroll(app, scroll, *selected, filtered_indices);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(visible) = updates_visible_index_for_selected(*selected, filtered_indices)
                && visible + 1 < filtered_indices.len()
            {
                *selected = filtered_indices[visible + 1];
                // Auto-scroll to keep selected item visible
                sync_scroll(app, scroll, *selected, filtered_indices);
            }
        }
        KeyCode::PageUp => {
            if let Some(visible) = updates_visible_index_for_selected(*selected, filtered_indices) {
                let next_visible = visible.saturating_sub(10);
                *selected = filtered_indices[next_visible];
                sync_scroll(app, scroll, *selected, filtered_indices);
            }
        }
        KeyCode::PageDown => {
            if let Some(visible) = updates_visible_index_for_selected(*selected, filtered_indices) {
                let max_visible = filtered_indices.len().saturating_sub(1);
                let next_visible = (visible + 10).min(max_visible);
                *selected = filtered_indices[next_visible];
                sync_scroll(app, scroll, *selected, filtered_indices);
            }
        }
        KeyCode::Char('d')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Ctrl+D: page down (25 lines)
            if let Some(visible) = updates_visible_index_for_selected(*selected, filtered_indices) {
                let max_visible = filtered_indices.len().saturating_sub(1);
                let next_visible = (visible + 25).min(max_visible);
                *selected = filtered_indices[next_visible];
                sync_scroll(app, scroll, *selected, filtered_indices);
            }
        }
        KeyCode::Char('u')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            // Ctrl+U: page up (20 lines)
            if let Some(visible) = updates_visible_index_for_selected(*selected, filtered_indices) {
                let next_visible = visible.saturating_sub(20);
                *selected = filtered_indices[next_visible];
                sync_scroll(app, scroll, *selected, filtered_indices);
            }
        }
        _ => {}
    }
    if let Some((name, _, _)) = entries.get(*selected) {
        *last_selected_pkg_name = Some(name.clone());
    }
    false
}

/// What: Build an update preflight `PackageItem` for a selected updates row.
fn package_item_for_update_entry(pkg_name: &str, new_version: &str) -> PackageItem {
    if let Some(mut pkg_item) = crate::index::find_package_by_name(pkg_name) {
        pkg_item.version = new_version.to_string();
        pkg_item
    } else {
        PackageItem {
            name: pkg_name.to_string(),
            version: new_version.to_string(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }
    }
}

/// What: Collect selected updates rows as preflight items in original entries order.
fn collect_selected_update_items(
    entries: &[(String, String, String)],
    selected_pkg_names: &std::collections::HashSet<String>,
) -> Vec<PackageItem> {
    entries
        .iter()
        .filter(|(name, _, _)| selected_pkg_names.contains(name))
        .map(|(name, _, new_version)| package_item_for_update_entry(name, new_version))
        .collect()
}

/// What: Return visible filtered position for a selected original updates index.
fn updates_visible_index_for_selected(
    selected: usize,
    filtered_indices: &[usize],
) -> Option<usize> {
    filtered_indices.iter().position(|&idx| idx == selected)
}

/// What: Recompute updates filter result and restore selection with identity-first strategy.
#[allow(clippy::too_many_arguments)]
fn recompute_updates_filter_state(
    entries: &[(String, String, String)],
    scroll: &mut u16,
    selected: &mut usize,
    filter_query: &str,
    last_selected_pkg_name: &mut Option<String>,
    filtered_indices: &mut Vec<usize>,
    sync_scroll: &impl Fn(&AppState, &mut u16, usize, &[usize]),
    app: &AppState,
) {
    let previous_visible =
        updates_visible_index_for_selected(*selected, filtered_indices).unwrap_or(0);
    *filtered_indices =
        crate::events::utils::compute_updates_filtered_indices(entries, filter_query);
    if filtered_indices.is_empty() {
        *scroll = 0;
        return;
    }

    let restored_by_name = last_selected_pkg_name.as_ref().and_then(|name| {
        filtered_indices.iter().copied().find(|&original_idx| {
            entries
                .get(original_idx)
                .is_some_and(|(entry_name, _, _)| entry_name == name)
        })
    });
    let fallback_visible = previous_visible.min(filtered_indices.len().saturating_sub(1));
    *selected = restored_by_name.unwrap_or(filtered_indices[fallback_visible]);

    if let Some((name, _, _)) = entries.get(*selected) {
        *last_selected_pkg_name = Some(name.clone());
    }
    sync_scroll(app, scroll, *selected, filtered_indices);
}

/// What: Handle key input while updates slash-filter text mode is active.
///
/// Inputs:
/// - `ke`: Key event.
/// - `entries`, `scroll`, `selected`, `filter_*`, `last_selected_pkg_name`, `filtered_indices`:
///   Same roles as in [`handle_updates`].
/// - `selected_pkg_names`: Multi-select marks for batch preflight.
/// - `sync_scroll`, `app`: Scroll sync closure and app state for filter recomputation.
///
/// Output:
/// - `true` when the key was consumed by filter editing (including Space toggle); `false` to fall
///   through to normal navigation when applicable.
///
/// Details:
/// - Space toggles the mark on the focused row (same as outside filter mode) and does not insert
///   into the query string.
/// - Arrow and page keys fall through (`_ => false`) to list navigation in [`handle_updates`].
/// - Unmodified and Shift+letter `KeyCode::Char` inputs (the full Latin alphabet for search) insert
///   into the query; `Home` / `End` move the filter caret.
#[allow(clippy::too_many_arguments)]
fn handle_updates_filter_editing(
    ke: KeyEvent,
    entries: &[(String, String, String)],
    scroll: &mut u16,
    selected: &mut usize,
    filter_active: &mut bool,
    filter_query: &mut String,
    filter_caret: &mut usize,
    last_selected_pkg_name: &mut Option<String>,
    filtered_indices: &mut Vec<usize>,
    selected_pkg_names: &mut std::collections::HashSet<String>,
    sync_scroll: &impl Fn(&AppState, &mut u16, usize, &[usize]),
    app: &AppState,
) -> bool {
    match ke.code {
        KeyCode::Esc => {
            filter_query.clear();
            *filter_caret = 0;
            *filter_active = false;
            *filtered_indices = (0..entries.len()).collect();
            recompute_updates_filter_state(
                entries,
                scroll,
                selected,
                filter_query,
                last_selected_pkg_name,
                filtered_indices,
                sync_scroll,
                app,
            );
            true
        }
        KeyCode::Left => {
            *filter_caret = filter_caret.saturating_sub(1);
            true
        }
        KeyCode::Right => {
            let chars = crate::events::utils::char_count(filter_query);
            *filter_caret = (*filter_caret + 1).min(chars);
            true
        }
        KeyCode::Home => {
            *filter_caret = 0;
            true
        }
        KeyCode::End => {
            *filter_caret = crate::events::utils::char_count(filter_query);
            true
        }
        KeyCode::Backspace => {
            if *filter_caret > 0 {
                let start_ci = filter_caret.saturating_sub(1);
                let start_b = crate::events::utils::byte_index_for_char(filter_query, start_ci);
                let end_b = crate::events::utils::byte_index_for_char(filter_query, *filter_caret);
                filter_query.replace_range(start_b..end_b, "");
                *filter_caret = start_ci;
                recompute_updates_filter_state(
                    entries,
                    scroll,
                    selected,
                    filter_query,
                    last_selected_pkg_name,
                    filtered_indices,
                    sync_scroll,
                    app,
                );
            }
            true
        }
        KeyCode::Delete => {
            let chars = crate::events::utils::char_count(filter_query);
            if *filter_caret < chars {
                let start_b =
                    crate::events::utils::byte_index_for_char(filter_query, *filter_caret);
                let end_b =
                    crate::events::utils::byte_index_for_char(filter_query, *filter_caret + 1);
                filter_query.replace_range(start_b..end_b, "");
                recompute_updates_filter_state(
                    entries,
                    scroll,
                    selected,
                    filter_query,
                    last_selected_pkg_name,
                    filtered_indices,
                    sync_scroll,
                    app,
                );
            }
            true
        }
        KeyCode::Char(' ') => {
            if let Some((name, _, _)) = entries.get(*selected)
                && !selected_pkg_names.remove(name)
            {
                selected_pkg_names.insert(name.clone());
            }
            true
        }
        KeyCode::Char(ch)
            if ke.modifiers.is_empty() || ke.modifiers == crossterm::event::KeyModifiers::SHIFT =>
        {
            let insert_at = crate::events::utils::byte_index_for_char(filter_query, *filter_caret);
            filter_query.insert(insert_at, ch);
            *filter_caret += 1;
            recompute_updates_filter_state(
                entries,
                scroll,
                selected,
                filter_query,
                last_selected_pkg_name,
                filtered_indices,
                sync_scroll,
                app,
            );
            true
        }
        KeyCode::Enter | KeyCode::Tab => true,
        KeyCode::Char('k' | 'j' | 'g' | 'G')
            if ke
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL) =>
        {
            true
        }
        _ => false,
    }
}

/// What: Timeout window used to complete the `g g` chord in Updates modal navigation.
const UPDATES_GG_TIMEOUT: Duration = Duration::from_millis(500);

/// What: Clear any pending `g` chord state for Updates modal navigation.
///
/// Inputs:
/// - `app`: Mutable application state.
///
/// Output:
/// - Resets `updates_modal_pending_g_at` to `None`.
///
/// Details:
/// - Called on non-`g` keypresses and after successful `g g`.
const fn clear_updates_pending_g(app: &mut AppState) {
    app.updates_modal_pending_g_at = None;
}

/// What: Check whether a pending `g` chord is still valid for `g g`.
///
/// Inputs:
/// - `app`: Application state containing pending chord timestamp.
/// - `now`: Current monotonic timestamp.
///
/// Output:
/// - `true` when a prior `g` exists and is within `UPDATES_GG_TIMEOUT`.
///
/// Details:
/// - Uses `saturating_duration_since` to avoid panics if clocks appear reordered.
fn is_updates_pending_g_active(app: &AppState, now: Instant) -> bool {
    app.updates_modal_pending_g_at
        .is_some_and(|pending| now.saturating_duration_since(pending) <= UPDATES_GG_TIMEOUT)
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
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            // Install GNOME Terminal, then close the prompt

            let bin = match crate::logic::privilege::active_tool() {
                Ok(tool) => tool.binary_name(),
                Err(msg) => {
                    app.toast_message = Some(msg);
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
                    app.modal = crate::state::Modal::None;
                    return false;
                }
            };
            let cmd = format!(
                "({bin} pacman -S --needed --noconfirm gnome-terminal) || ({bin} pacman -S --needed --noconfirm gnome-console) || ({bin} pacman -S --needed --noconfirm kgx)"
            );

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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::time::{Duration, Instant};

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
    /// What: Verify startup queue skips `SshAurSetup` when SSH is already ready.
    ///
    /// Inputs:
    /// - `AppState` with startup queue containing `SshAurSetup` then `ArchNews`.
    /// - `aur_ssh_help_ready` set to `Some(true)`.
    ///
    /// Output:
    /// - Opens the next eligible setup modal (`NewsSetup`) instead of `SshAurSetup`.
    ///
    /// Details:
    /// - Revalidates queue entries at open time so stale queued SSH setup cannot open.
    fn test_show_next_startup_setup_step_skips_ssh_step_when_ready() {
        let mut app = crate::state::AppState {
            aur_ssh_help_ready: Some(true),
            pending_startup_setup_steps: std::collections::VecDeque::from([
                crate::state::modal::StartupSetupTask::SshAurSetup,
                crate::state::modal::StartupSetupTask::ArchNews,
            ]),
            ..Default::default()
        };

        show_next_startup_setup_step(&mut app);

        assert!(matches!(app.modal, crate::state::Modal::NewsSetup { .. }));
        assert!(app.pending_startup_setup_steps.is_empty());
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
    /// What: Verify startup flow does not auto-show pending news modal.
    ///
    /// Inputs:
    /// - `AppState` with no pending announcements but pending news.
    ///
    /// Output:
    /// - Does not show News modal automatically.
    ///
    /// Details:
    /// - Pending news remains queued for News mode consumption.
    #[allow(clippy::field_reassign_with_default)]
    fn test_show_next_pending_announcement_does_not_auto_show_news() {
        let mut app = crate::state::AppState::default();
        app.modal = crate::state::Modal::None;

        let news_items = vec![crate::state::NewsItem {
            date: "2025-01-01".to_string(),
            title: "Test News".to_string(),
            url: "https://example.com/news".to_string(),
        }];
        app.pending_news = Some(news_items);

        show_next_pending_announcement(&mut app);

        assert!(
            matches!(app.modal, crate::state::Modal::None),
            "startup flow must not auto-open News modal"
        );
        assert!(app.pending_news.is_some());
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
        for key_code in [
            KeyCode::Enter,
            KeyCode::Char('\n'),
            KeyCode::Char('\r'),
            KeyCode::Esc,
            KeyCode::Char('q'),
        ] {
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

    #[test]
    /// What: Verify updates modal scroll uses rendered-line mapping for wrapped rows.
    ///
    /// Inputs:
    /// - App state with synthetic entry->line starts and viewport height.
    ///
    /// Output:
    /// - Scroll offset follows rendered line starts instead of entry index.
    ///
    /// Details:
    /// - Protects against selection/scroll desync when entries wrap to multiple lines.
    fn test_update_scroll_for_selection_uses_rendered_line_starts() {
        let app = crate::state::AppState {
            updates_modal_entry_line_starts: vec![0, 3, 5],
            updates_modal_total_lines: 7,
            updates_modal_content_rect: Some((0, 0, 40, 3)),
            ..Default::default()
        };

        let mut scroll = crate::events::utils::compute_updates_modal_scroll_for_selection(
            &app.updates_modal_entry_line_starts,
            app.updates_modal_total_lines,
            app.updates_modal_content_rect,
            1,
            3,
            0,
        );
        assert_eq!(
            scroll, 1,
            "entry 1 starts at rendered line 3 with height 3 viewport"
        );

        scroll = crate::events::utils::compute_updates_modal_scroll_for_selection(
            &app.updates_modal_entry_line_starts,
            app.updates_modal_total_lines,
            app.updates_modal_content_rect,
            2,
            3,
            scroll,
        );
        assert_eq!(
            scroll, 3,
            "entry 2 should move viewport to include rendered line 5"
        );
    }

    #[test]
    /// What: Verify updates modal scroll clamps to computed max scroll.
    ///
    /// Inputs:
    /// - App state with small total rendered lines and larger desired scroll.
    ///
    /// Output:
    /// - Scroll value is clamped within valid range.
    ///
    /// Details:
    /// - Ensures dynamic viewport calculations cannot overscroll.
    fn test_update_scroll_for_selection_clamps_to_max_scroll() {
        let app = crate::state::AppState {
            updates_modal_entry_line_starts: vec![0, 2],
            updates_modal_total_lines: 4,
            updates_modal_content_rect: Some((0, 0, 40, 3)),
            ..Default::default()
        };

        let scroll = crate::events::utils::compute_updates_modal_scroll_for_selection(
            &app.updates_modal_entry_line_starts,
            app.updates_modal_total_lines,
            app.updates_modal_content_rect,
            0,
            2,
            10,
        );
        assert_eq!(scroll, 0, "first entry should clamp scroll back to start");
    }

    #[test]
    /// What: Verify `Home` and `End` jump to first/last updates entry.
    ///
    /// Inputs:
    /// - Updates list with three entries and selected index in the middle.
    ///
    /// Output:
    /// - `Home` moves selection to index 0; `End` moves to last index.
    ///
    /// Details:
    /// - Confirms parity navigation keys for absolute jumps.
    fn test_handle_updates_home_and_end_jump_bounds() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("a".to_string(), "1".to_string(), "2".to_string()),
            ("b".to_string(), "1".to_string(), "2".to_string()),
            ("c".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 1usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = (0..entries.len()).collect();
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Home),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 0);

        let _ = handle_updates(
            test_key_event(KeyCode::End),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 2);
    }

    #[test]
    /// What: Verify uppercase `G` jumps to the last updates entry.
    ///
    /// Inputs:
    /// - Updates list with three entries and selected index 0.
    ///
    /// Output:
    /// - Selection moves to final index.
    ///
    /// Details:
    /// - Mirrors common TUI navigation behavior.
    fn test_handle_updates_uppercase_g_jumps_last() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("a".to_string(), "1".to_string(), "2".to_string()),
            ("b".to_string(), "1".to_string(), "2".to_string()),
            ("c".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = (0..entries.len()).collect();
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Char('G')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 2);
    }

    #[test]
    /// What: Verify `g g` within timeout jumps to first entry.
    ///
    /// Inputs:
    /// - Updates list with three entries and selected index at end.
    ///
    /// Output:
    /// - First `g` arms pending chord; second `g` jumps selection to 0.
    ///
    /// Details:
    /// - Ensures chord behavior is active in normal navigation mode.
    fn test_handle_updates_gg_within_timeout_jumps_first() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("a".to_string(), "1".to_string(), "2".to_string()),
            ("b".to_string(), "1".to_string(), "2".to_string()),
            ("c".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 2usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = (0..entries.len()).collect();
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Char('g')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert!(app.updates_modal_pending_g_at.is_some());
        assert_eq!(selected, 2);

        let _ = handle_updates(
            test_key_event(KeyCode::Char('g')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 0);
        assert!(app.updates_modal_pending_g_at.is_none());
    }

    #[test]
    /// What: Verify expired pending `g` does not trigger a jump.
    ///
    /// Inputs:
    /// - Updates list with pending `g` timestamp older than timeout.
    ///
    /// Output:
    /// - Next `g` re-arms the chord and keeps selection unchanged.
    ///
    /// Details:
    /// - Prevents stale chord state from causing surprise jumps.
    fn test_handle_updates_expired_pending_g_does_not_jump() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("a".to_string(), "1".to_string(), "2".to_string()),
            ("b".to_string(), "1".to_string(), "2".to_string()),
            ("c".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 2usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = (0..entries.len()).collect();
        let mut selected_pkg_names = std::collections::HashSet::new();
        let now = Instant::now();
        let expired = now
            .checked_sub(UPDATES_GG_TIMEOUT + Duration::from_millis(50))
            .map_or(now, |instant| instant);
        app.updates_modal_pending_g_at = Some(expired);

        let _ = handle_updates(
            test_key_event(KeyCode::Char('g')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 2);
        assert!(app.updates_modal_pending_g_at.is_some());
    }

    #[test]
    /// What: Verify non-`g` key clears pending `g` and preserves navigation behavior.
    ///
    /// Inputs:
    /// - Pending `g` state followed by `Down`.
    ///
    /// Output:
    /// - `Down` moves selection normally and clears pending chord state.
    ///
    /// Details:
    /// - Ensures pending chord does not leak into unrelated key handling.
    fn test_handle_updates_non_g_clears_pending_chord() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("a".to_string(), "1".to_string(), "2".to_string()),
            ("b".to_string(), "1".to_string(), "2".to_string()),
            ("c".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = (0..entries.len()).collect();
        let mut selected_pkg_names = std::collections::HashSet::new();
        app.updates_modal_pending_g_at = Some(Instant::now());

        let _ = handle_updates(
            test_key_event(KeyCode::Down),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 1);
        assert!(app.updates_modal_pending_g_at.is_none());
    }

    #[test]
    /// What: Verify slash key enters updates filter text mode.
    ///
    /// Inputs:
    /// - Updates entries and slash keypress.
    ///
    /// Output:
    /// - Filter mode becomes active without mutating selection.
    ///
    /// Details:
    /// - Guards entrypoint behavior for phase-4 quick filter.
    fn test_handle_updates_slash_enters_filter_mode() {
        let mut app = crate::state::AppState::default();
        let entries = vec![("pkg-a".to_string(), "1".to_string(), "2".to_string())];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = vec![0];
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Char('/')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert!(filter_active);
        assert_eq!(selected, 0);
    }

    #[test]
    /// What: Verify Space in slash-filter mode toggles the multi-select mark for the focused row.
    ///
    /// Inputs:
    /// - Active filter with a matching row; Space keypresses.
    ///
    /// Output:
    /// - `selected_pkg_names` gains then loses the focused package name.
    ///
    /// Details:
    /// - Space must not append to `filter_query` (regression guard vs generic `Char` insertion).
    fn test_handle_updates_space_in_filter_mode_toggles_mark() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = "al".to_string();
        let mut filter_caret = 2usize;
        let mut last_selected_pkg_name = Some("alpha".to_string());
        let mut filtered_indices =
            crate::events::utils::compute_updates_filtered_indices(&entries, &filter_query);
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Char(' ')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(filter_query, "al");
        assert!(selected_pkg_names.contains("alpha"));

        let _ = handle_updates(
            test_key_event(KeyCode::Char(' ')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert!(!selected_pkg_names.contains("alpha"));
    }

    #[test]
    /// What: Verify Down/Up reach list navigation while slash-filter mode stays active.
    ///
    /// Inputs:
    /// - Active filter with empty query (all rows visible); Down then Up.
    ///
    /// Output:
    /// - Selection moves on the filtered list; filter remains active.
    ///
    /// Details:
    /// - Regression guard: filter editing must not swallow arrow keys as no-ops.
    fn test_handle_updates_filter_mode_arrow_navigates_list() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = Some("alpha".to_string());
        let mut filtered_indices = vec![0, 1];
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Down),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 1);
        assert!(filter_active);

        let _ = handle_updates(
            test_key_event(KeyCode::Up),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(selected, 0);
        assert!(filter_active);
    }

    #[test]
    /// What: Verify `j` in filter mode edits the query instead of moving list selection.
    ///
    /// Inputs:
    /// - Active filter with caret after `a`; `j` keypress.
    ///
    /// Output:
    /// - Query becomes `aj`; focused entry index unchanged.
    ///
    /// Details:
    /// - `j` / `k` are filter text in slash mode; arrows still move the list.
    fn test_handle_updates_filter_mode_j_inserts_into_query() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = "a".to_string();
        let mut filter_caret = 1usize;
        let mut last_selected_pkg_name = Some("alpha".to_string());
        let mut filtered_indices =
            crate::events::utils::compute_updates_filtered_indices(&entries, &filter_query);
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Char('j')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(filter_query, "aj");
        assert_eq!(filter_caret, 2);
        assert_eq!(selected, 0);
        assert!(filter_active);
    }

    #[test]
    /// What: Verify `g` and Shift+`g` (`G`) insert into the slash-filter query.
    ///
    /// Inputs:
    /// - Active filter with empty query; `g` then `G` (`Char('G')` + shift modifier).
    ///
    /// Output:
    /// - Query becomes `gG`; list navigation shortcuts stay disabled for those keys in filter mode.
    ///
    /// Details:
    /// - Regression guard: every ASCII letter must be available for package-name search.
    fn test_handle_updates_filter_mode_g_and_shift_g_insert_into_query() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("gamma".to_string(), "1".to_string(), "2".to_string()),
            ("other".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = Some("gamma".to_string());
        let mut filtered_indices =
            crate::events::utils::compute_updates_filtered_indices(&entries, &filter_query);
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Char('g')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(filter_query, "g");
        assert_eq!(filter_caret, 1);

        let shift_g = KeyEvent {
            code: KeyCode::Char('G'),
            modifiers: KeyModifiers::SHIFT,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        };
        let _ = handle_updates(
            shift_g,
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert_eq!(filter_query, "gG");
        assert_eq!(filter_caret, 2);
        assert_eq!(selected, 0);
        assert!(filter_active);
    }

    #[test]
    /// What: Verify Esc clears query and exits updates filter mode.
    ///
    /// Inputs:
    /// - Active filter mode with non-empty query.
    ///
    /// Output:
    /// - Filter mode exits and query is reset.
    ///
    /// Details:
    /// - Ensures reversible filtering UX.
    fn test_handle_updates_filter_esc_clears_and_exits() {
        let mut app = crate::state::AppState::default();
        let entries = vec![("pkg-a".to_string(), "1".to_string(), "2".to_string())];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = "pkg".to_string();
        let mut filter_caret = 3usize;
        let mut last_selected_pkg_name = Some("pkg-a".to_string());
        let mut filtered_indices = vec![0];
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Esc),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert!(!filter_active);
        assert!(filter_query.is_empty());
        assert_eq!(filter_caret, 0);
        assert_eq!(filtered_indices, vec![0]);
    }

    #[test]
    /// What: Verify updates filter restores selection by package identity when reintroduced.
    ///
    /// Inputs:
    /// - Entries with selected package hidden then restored by query edits.
    ///
    /// Output:
    /// - Selection returns to the same package once it matches again.
    ///
    /// Details:
    /// - Protects best-UX identity restoration requirement in phase 4.
    fn test_handle_updates_filter_restores_selection_by_identity() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
            ("gamma".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 1usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = "be".to_string();
        let mut filter_caret = 2usize;
        let mut last_selected_pkg_name = Some("beta".to_string());
        let mut filtered_indices =
            crate::events::utils::compute_updates_filtered_indices(&entries, "be");
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Backspace),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );

        assert_eq!(selected, 1);
        assert_eq!(last_selected_pkg_name, Some("beta".to_string()));
        assert!(!filtered_indices.is_empty());
    }

    #[test]
    /// What: Verify active non-empty filter preserves an empty match set.
    ///
    /// Inputs:
    /// - Active slash filter, query text with no matching entries, and navigation key press.
    ///
    /// Output:
    /// - Filtered indices stay empty and are not replaced with the unfiltered list.
    ///
    /// Details:
    /// - Prevents regression where "0 matches" becomes indistinguishable from "no filter".
    fn test_handle_updates_active_filter_does_not_refill_empty_results() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = "zzz".to_string();
        let mut filter_caret = 3usize;
        let mut last_selected_pkg_name = Some("alpha".to_string());
        let mut filtered_indices =
            crate::events::utils::compute_updates_filtered_indices(&entries, &filter_query);
        let mut selected_pkg_names = std::collections::HashSet::new();
        assert!(filtered_indices.is_empty());

        let _ = handle_updates(
            test_key_event(KeyCode::Down),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );

        assert!(filter_active);
        assert_eq!(filter_query, "zzz");
        assert!(filtered_indices.is_empty());
    }

    #[test]
    /// What: Verify Space toggles batch selection for focused updates row.
    fn test_handle_updates_space_toggles_selected_package() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 1usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = vec![0, 1];
        let mut selected_pkg_names = std::collections::HashSet::new();

        let _ = handle_updates(
            test_key_event(KeyCode::Char(' ')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert!(selected_pkg_names.contains("beta"));

        let _ = handle_updates(
            test_key_event(KeyCode::Char(' ')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );
        assert!(!selected_pkg_names.contains("beta"));
    }

    #[test]
    /// What: Verify `a` selects all currently visible filtered rows.
    fn test_handle_updates_a_selects_all_visible_rows() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
            ("gamma".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 0usize;
        let mut scroll = 0u16;
        let mut filter_active = false;
        let mut filter_query = String::new();
        let mut filter_caret = 0usize;
        let mut last_selected_pkg_name = None;
        let mut filtered_indices = vec![1, 2];
        let mut selected_pkg_names = std::collections::HashSet::from(["alpha".to_string()]);

        let _ = handle_updates(
            test_key_event(KeyCode::Char('a')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );

        assert!(selected_pkg_names.contains("alpha"));
        assert!(selected_pkg_names.contains("beta"));
        assert!(selected_pkg_names.contains("gamma"));
        assert_eq!(selected_pkg_names.len(), 3);
    }

    #[test]
    /// What: Verify selected packages remain intact while filter query changes.
    fn test_handle_updates_filter_changes_preserve_hidden_selection() {
        let mut app = crate::state::AppState::default();
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
            ("gamma".to_string(), "1".to_string(), "2".to_string()),
        ];
        let mut selected = 1usize;
        let mut scroll = 0u16;
        let mut filter_active = true;
        let mut filter_query = "b".to_string();
        let mut filter_caret = 1usize;
        let mut last_selected_pkg_name = Some("beta".to_string());
        let mut filtered_indices = vec![1];
        let mut selected_pkg_names = std::collections::HashSet::from(["gamma".to_string()]);

        let _ = handle_updates(
            test_key_event(KeyCode::Char('e')),
            &mut app,
            &entries,
            &mut scroll,
            &mut selected,
            &mut filter_active,
            &mut filter_query,
            &mut filter_caret,
            &mut last_selected_pkg_name,
            &mut filtered_indices,
            &mut selected_pkg_names,
        );

        assert!(selected_pkg_names.contains("gamma"));
    }

    #[test]
    /// What: Ensure selected-set collection preserves original entry order.
    fn test_collect_selected_update_items_preserves_entry_order() {
        let entries = vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
            ("gamma".to_string(), "1".to_string(), "2".to_string()),
        ];
        let selected_pkg_names =
            std::collections::HashSet::from(["gamma".to_string(), "alpha".to_string()]);

        let items = collect_selected_update_items(&entries, &selected_pkg_names);
        let names: Vec<String> = items.into_iter().map(|item| item.name).collect();
        assert_eq!(names, vec!["alpha".to_string(), "gamma".to_string()]);
    }
}
