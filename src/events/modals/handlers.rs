//! Individual modal handler functions that encapsulate field extraction and restoration.

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use super::restore;
use crate::install::ExecutorRequest;
use crate::state::{AppState, Modal, PackageItem};

/// What: Handle key events for Alert modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: Alert modal variant with message
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler and handles restoration
/// - Returns the result from common handler to prevent event propagation when Esc is pressed
pub(super) fn handle_alert_modal(ke: KeyEvent, app: &mut AppState, modal: &Modal) -> bool {
    if let Modal::Alert { message } = modal {
        super::common::handle_alert(ke, app, message)
    } else {
        false
    }
}

/// What: Handle key events for `PreflightExec` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `PreflightExec` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to common handler, updates verbose flag, and restores modal if needed
/// - Returns `true` when modal is closed/transitioned to stop key propagation
pub(super) fn handle_preflight_exec_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::PreflightExec {
        ref mut verbose,
        ref log_lines,
        ref abortable,
        ref items,
        ref action,
        ref tab,
        ref header_chips,
        ref success,
    } = modal
    {
        // Pass success to the handler since app.modal is taken during dispatch
        let should_stop =
            super::common::handle_preflight_exec(ke, app, verbose, *abortable, items, *success);
        if should_stop {
            return true; // Modal was closed or transitioned, stop propagation
        }
        restore::restore_if_not_closed_with_excluded_keys(
            app,
            &ke,
            &[KeyCode::Esc, KeyCode::Char('q')],
            Modal::PreflightExec {
                verbose: *verbose,
                log_lines: log_lines.clone(),
                abortable: *abortable,
                items: items.clone(),
                action: *action,
                tab: *tab,
                success: *success,
                header_chips: header_chips.clone(),
            },
        );
    }
    false
}

/// What: Handle key events for `PostSummary` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `PostSummary` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to common handler and restores modal if needed
/// - Returns `true` when modal is closed to stop key propagation
pub(super) fn handle_post_summary_modal(ke: KeyEvent, app: &mut AppState, modal: &Modal) -> bool {
    if let Modal::PostSummary {
        success,
        changed_files,
        pacnew_count,
        pacsave_count,
        services_pending,
        snapshot_label,
    } = modal
    {
        let should_stop = super::common::handle_post_summary(ke, app, services_pending);
        if should_stop {
            return true; // Modal was closed, stop propagation
        }
        restore::restore_if_not_closed_with_excluded_keys(
            app,
            &ke,
            &[KeyCode::Esc, KeyCode::Enter, KeyCode::Char('q')],
            Modal::PostSummary {
                success: *success,
                changed_files: *changed_files,
                pacnew_count: *pacnew_count,
                pacsave_count: *pacsave_count,
                services_pending: services_pending.clone(),
                snapshot_label: snapshot_label.clone(),
            },
        );
    }
    false
}

/// What: Handle key events for `SystemUpdate` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `SystemUpdate` modal variant
///
/// Output:
/// - `true` if event propagation should stop, otherwise `false`
///
/// Details:
/// - Delegates to `system_update` handler and restores modal if needed
pub(super) fn handle_system_update_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::SystemUpdate {
        ref mut do_mirrors,
        ref mut do_pacman,
        ref mut force_sync,
        ref mut do_aur,
        ref mut do_cache,
        ref mut country_idx,
        ref countries,
        ref mut mirror_count,
        ref mut cursor,
    } = modal
    {
        let should_stop = super::system_update::handle_system_update(
            ke,
            app,
            do_mirrors,
            do_pacman,
            force_sync,
            do_aur,
            do_cache,
            country_idx,
            countries,
            mirror_count,
            cursor,
        );
        return restore::restore_if_not_closed_with_option_result(
            app,
            &ke,
            should_stop,
            Modal::SystemUpdate {
                do_mirrors: *do_mirrors,
                do_pacman: *do_pacman,
                force_sync: *force_sync,
                do_aur: *do_aur,
                do_cache: *do_cache,
                country_idx: *country_idx,
                countries: countries.clone(),
                mirror_count: *mirror_count,
                cursor: *cursor,
            },
        );
    }
    false
}

/// What: Handle key events for `ConfirmInstall` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ConfirmInstall` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to install handler
pub(super) fn handle_confirm_install_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: &Modal,
) -> bool {
    if let Modal::ConfirmInstall { items } = modal {
        super::install::handle_confirm_install(ke, app, items);
    }
    false
}

/// What: Handle key events for `ConfirmRemove` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ConfirmRemove` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to install handler
pub(super) fn handle_confirm_remove_modal(ke: KeyEvent, app: &mut AppState, modal: &Modal) -> bool {
    if let Modal::ConfirmRemove { items } = modal {
        super::install::handle_confirm_remove(ke, app, items);
    }
    false
}

/// What: Handle key events for `ConfirmBatchUpdate` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ConfirmBatchUpdate` modal variant
///
/// Output:
/// - `true` if Esc/q was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Handles Esc/q to cancel, Enter to continue with batch update
/// - Uses executor pattern (PTY-based execution) instead of spawning terminal
pub(super) fn handle_confirm_batch_update_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: &Modal,
) -> bool {
    if let Modal::ConfirmBatchUpdate { items, dry_run } = modal {
        match ke.code {
            KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
                // Cancel update
                app.modal = crate::state::Modal::None;
                return true;
            }
            KeyCode::Enter => {
                // Continue with batch update - use executor pattern instead of spawning terminal
                let items_clone = items.clone();
                let dry_run_clone = *dry_run;
                app.dry_run = dry_run_clone;

                // Get header_chips if available from pending_exec_header_chips, otherwise use default
                let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();

                // Check passwordless sudo availability (requires setting enabled AND system configured)
                // All installs need sudo (official and AUR both need sudo)
                // but password may not be needed if passwordless sudo is configured and enabled
                let settings = crate::theme::settings();
                if crate::logic::password::should_use_passwordless_sudo(&settings) {
                    // Passwordless sudo enabled and available - skip password prompt and proceed directly
                    crate::events::preflight::start_execution(
                        app,
                        &items_clone,
                        crate::state::PreflightAction::Install,
                        header_chips,
                        None, // No password needed
                    );
                } else {
                    // Passwordless sudo not enabled or not available - show password prompt
                    app.modal = crate::state::Modal::PasswordPrompt {
                        purpose: crate::state::modal::PasswordPurpose::Install,
                        items: items_clone,
                        input: String::new(),
                        cursor: 0,
                        error: None,
                    };
                    app.pending_exec_header_chips = Some(header_chips);
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

/// What: Handle key events for `ConfirmAurUpdate` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ConfirmAurUpdate` modal variant
///
/// Output:
/// - `true` if modal was closed/transitioned, `false` otherwise
///
/// Details:
/// - Enter (Y) continues with AUR update
/// - Esc/q (N) cancels and closes modal
pub(super) fn handle_confirm_aur_update_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: &Modal,
) -> bool {
    if let Modal::ConfirmAurUpdate { .. } = modal {
        match ke.code {
            KeyCode::Esc | KeyCode::Char('q' | 'Q' | 'n' | 'N') => {
                // Cancel AUR update
                app.pending_aur_update_command = None;
                app.modal = crate::state::Modal::None;
                return true;
            }
            KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                // Continue with AUR update
                if let Some(aur_command) = app.pending_aur_update_command.take() {
                    let password = app.pending_executor_password.clone();
                    let dry_run = app.dry_run;

                    // Transition back to PreflightExec for AUR update
                    app.modal = Modal::PreflightExec {
                        items: Vec::new(),
                        action: crate::state::PreflightAction::Install,
                        tab: crate::state::PreflightTab::Summary,
                        verbose: false,
                        log_lines: Vec::new(),
                        abortable: true,
                        header_chips: app.pending_exec_header_chips.take().unwrap_or_default(),
                        success: None,
                    };

                    // Execute AUR update command
                    app.pending_executor_request = Some(ExecutorRequest::Update {
                        commands: vec![aur_command],
                        password,
                        dry_run,
                    });
                } else {
                    app.modal = crate::state::Modal::None;
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

/// What: Handle key events for `ConfirmReinstall` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ConfirmReinstall` modal variant
///
/// Output:
/// - `true` if modal was closed/transitioned, `false` otherwise
///
/// Details:
/// - Handles Esc/q to cancel, Enter to proceed with reinstall
pub(super) fn handle_confirm_reinstall_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: &Modal,
) -> bool {
    if let Modal::ConfirmReinstall {
        items: _installed_items,
        all_items,
        header_chips,
    } = modal
    {
        match ke.code {
            KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
                // Cancel reinstall
                app.modal = crate::state::Modal::None;
                return true;
            }
            KeyCode::Enter => {
                // Proceed with reinstall - use executor pattern
                // Use all_items (all packages) instead of just installed ones
                let items_clone = all_items.clone();
                let header_chips_clone = header_chips.clone();
                // Retrieve password that was stored when reinstall confirmation was shown
                let password = app.pending_executor_password.take();

                // All installs need sudo (official and AUR both need sudo)
                if password.is_none() {
                    // Check faillock status before proceeding
                    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
                    if let Some(lockout_msg) =
                        crate::logic::faillock::get_lockout_message_if_locked(&username, app)
                    {
                        // User is locked out - show warning
                        app.modal = crate::state::Modal::Alert {
                            message: lockout_msg,
                        };
                        return true;
                    }

                    // Check passwordless sudo availability (requires setting enabled AND system configured)
                    let settings = crate::theme::settings();
                    if crate::logic::password::should_use_passwordless_sudo(&settings) {
                        // Passwordless sudo enabled and available - skip password prompt and proceed directly
                        crate::events::preflight::start_execution(
                            app,
                            &items_clone,
                            crate::state::PreflightAction::Install,
                            header_chips_clone,
                            None, // No password needed
                        );
                    } else {
                        // Passwordless sudo not enabled or not available - show password prompt
                        app.modal = crate::state::Modal::PasswordPrompt {
                            purpose: crate::state::modal::PasswordPurpose::Install,
                            items: items_clone,
                            input: String::new(),
                            cursor: 0,
                            error: None,
                        };
                        app.pending_exec_header_chips = Some(header_chips_clone);
                    }
                } else {
                    // Password already obtained, go directly to execution
                    crate::events::preflight::start_execution(
                        app,
                        &items_clone,
                        crate::state::PreflightAction::Install,
                        header_chips_clone,
                        password,
                    );
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

/// What: Handle key events for Help modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: Help modal variant (unit type)
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler
pub(super) fn handle_help_modal(ke: KeyEvent, app: &mut AppState, _modal: Modal) -> bool {
    super::common::handle_help(ke, app)
}

/// What: Handle key events for News modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: News modal variant
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler and restores modal if needed
pub(super) fn handle_news_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::News {
        ref items,
        ref mut selected,
        ref mut scroll,
    } = modal
    {
        let result = super::common::handle_news(ke, app, items, selected, scroll);
        return restore::restore_if_not_closed_with_bool_result(
            app,
            result,
            Modal::News {
                items: items.clone(),
                selected: *selected,
                scroll: *scroll,
            },
        );
    }
    false
}

/// What: Handle key events for Announcement modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: Announcement modal variant
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler and restores modal if needed
pub(super) fn handle_announcement_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::Announcement {
        ref title,
        ref content,
        ref id,
        ref mut scroll,
    } = modal
    {
        let old_id = id.clone();
        let result = super::common::handle_announcement(ke, app, id, scroll);
        // Only restore if modal wasn't closed AND it's still the same announcement
        // (don't restore if a new pending announcement was shown)
        match &app.modal {
            Modal::Announcement { id: new_id, .. } if *new_id == old_id => {
                // Same announcement, restore scroll state
                app.modal = Modal::Announcement {
                    title: title.clone(),
                    content: content.clone(),
                    id: old_id,
                    scroll: *scroll,
                };
            }
            _ => {
                // Modal was closed or different modal (e.g., pending announcement was shown), don't restore
            }
        }
        return result;
    }
    false
}

/// What: Handle key events for Updates modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: Updates modal variant
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler and restores modal if needed
pub(super) fn handle_updates_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::Updates {
        ref entries,
        ref mut scroll,
        ref mut selected,
    } = modal
    {
        let result = super::common::handle_updates(ke, app, entries, scroll, selected);
        return restore::restore_if_not_closed_with_bool_result(
            app,
            result,
            Modal::Updates {
                entries: entries.clone(),
                scroll: *scroll,
                selected: *selected,
            },
        );
    }
    false
}

/// What: Handle key events for `OptionalDeps` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `OptionalDeps` modal variant
///
/// Output:
/// - `true` if event propagation should stop, otherwise `false`
///
/// Details:
/// - Delegates to `optional_deps` handler and restores modal if needed
pub(super) fn handle_optional_deps_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::OptionalDeps {
        ref rows,
        ref mut selected,
    } = modal
    {
        let should_stop = super::optional_deps::handle_optional_deps(ke, app, rows, selected);
        return restore::restore_if_not_closed_with_option_result(
            app,
            &ke,
            should_stop,
            Modal::OptionalDeps {
                rows: rows.clone(),
                selected: *selected,
            },
        );
    }
    false
}

/// What: Handle key events for `ScanConfig` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ScanConfig` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to scan handler and restores modal if needed
pub(super) fn handle_scan_config_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::ScanConfig {
        ref mut do_clamav,
        ref mut do_trivy,
        ref mut do_semgrep,
        ref mut do_shellcheck,
        ref mut do_virustotal,
        ref mut do_custom,
        ref mut do_sleuth,
        ref mut cursor,
    } = modal
    {
        super::scan::handle_scan_config(
            ke,
            app,
            do_clamav,
            do_trivy,
            do_semgrep,
            do_shellcheck,
            do_virustotal,
            do_custom,
            do_sleuth,
            cursor,
        );
        restore::restore_if_not_closed_with_esc(
            app,
            &ke,
            Modal::ScanConfig {
                do_clamav: *do_clamav,
                do_trivy: *do_trivy,
                do_semgrep: *do_semgrep,
                do_shellcheck: *do_shellcheck,
                do_virustotal: *do_virustotal,
                do_custom: *do_custom,
                do_sleuth: *do_sleuth,
                cursor: *cursor,
            },
        );
    }
    false
}

/// What: Handle key events for `NewsSetup` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `NewsSetup` modal variant
///
/// Output:
/// - `true` if modal was closed (to stop propagation), otherwise `false`
///
/// Details:
/// - Handles navigation, toggles, date selection, and Enter to save settings
/// - On save, persists settings and triggers startup news fetch
pub(super) fn handle_news_setup_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::NewsSetup {
        ref mut show_arch_news,
        ref mut show_advisories,
        ref mut show_aur_updates,
        ref mut show_aur_comments,
        ref mut show_pkg_updates,
        ref mut max_age_days,
        ref mut cursor,
    } = modal
    {
        match ke.code {
            KeyCode::Esc => {
                // Cancel - restore previous modal or close
                if let Some(prev_modal) = app.previous_modal.take() {
                    app.modal = prev_modal;
                } else {
                    app.modal = crate::state::Modal::None;
                }
                return true;
            }
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            KeyCode::Down => {
                // Max cursor is 7 (0-4 for toggles, 5-7 for date buttons)
                if *cursor < 7 {
                    *cursor += 1;
                }
            }
            KeyCode::Left => {
                // Navigate between date buttons when on date row (cursor 5-7)
                if *cursor >= 5 && *cursor <= 7 && *cursor > 5 {
                    *cursor -= 1;
                }
            }
            KeyCode::Right => {
                // Navigate between date buttons when on date row (cursor 5-7)
                if *cursor >= 5 && *cursor <= 7 && *cursor < 7 {
                    *cursor += 1;
                }
            }
            KeyCode::Char(' ') => match *cursor {
                0 => *show_arch_news = !*show_arch_news,
                1 => *show_advisories = !*show_advisories,
                2 => *show_aur_updates = !*show_aur_updates,
                3 => *show_aur_comments = !*show_aur_comments,
                4 => *show_pkg_updates = !*show_pkg_updates,
                5 => *max_age_days = Some(7),
                6 => *max_age_days = Some(30),
                7 => *max_age_days = Some(90),
                _ => {}
            },
            KeyCode::Enter => {
                // Save all settings
                crate::theme::save_startup_news_show_arch_news(*show_arch_news);
                crate::theme::save_startup_news_show_advisories(*show_advisories);
                crate::theme::save_startup_news_show_aur_updates(*show_aur_updates);
                crate::theme::save_startup_news_show_aur_comments(*show_aur_comments);
                crate::theme::save_startup_news_show_pkg_updates(*show_pkg_updates);
                crate::theme::save_startup_news_max_age_days(*max_age_days);
                crate::theme::save_startup_news_configured(true);

                // Mark that we need to trigger startup news fetch
                app.trigger_startup_news_fetch = true;

                // Close modal
                app.modal = crate::state::Modal::None;
                return true;
            }
            _ => {}
        }
        restore::restore_if_not_closed_with_esc(
            app,
            &ke,
            Modal::NewsSetup {
                show_arch_news: *show_arch_news,
                show_advisories: *show_advisories,
                show_aur_updates: *show_aur_updates,
                show_aur_comments: *show_aur_comments,
                show_pkg_updates: *show_pkg_updates,
                max_age_days: *max_age_days,
                cursor: *cursor,
            },
        );
    }
    false
}

/// What: Handle key events for `VirusTotalSetup` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `VirusTotalSetup` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to scan handler and restores modal if needed
pub(super) fn handle_virustotal_setup_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::VirusTotalSetup {
        ref mut input,
        ref mut cursor,
    } = modal
    {
        super::scan::handle_virustotal_setup(ke, app, input, cursor);
        restore::restore_if_not_closed_with_esc(
            app,
            &ke,
            Modal::VirusTotalSetup {
                input: input.clone(),
                cursor: *cursor,
            },
        );
    }
    false
}

/// What: Handle key events for `GnomeTerminalPrompt` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `GnomeTerminalPrompt` modal variant (unit type)
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to common handler
pub(super) fn handle_gnome_terminal_prompt_modal(
    ke: KeyEvent,
    app: &mut AppState,
    _modal: Modal,
) -> bool {
    super::common::handle_gnome_terminal_prompt(ke, app);
    false
}

/// What: Handle key events for `PasswordPrompt` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `PasswordPrompt` modal variant
///
/// Output:
/// - `true` if Enter was pressed (password submitted), `false` otherwise
///
/// Details:
/// - Delegates to password handler and restores modal if needed
/// - Returns `true` on Enter to indicate password should be submitted
#[allow(clippy::too_many_lines)] // Complex password validation and execution flow requires many lines (function has 327 lines)
pub(super) fn handle_password_prompt_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::PasswordPrompt {
        ref mut input,
        ref mut cursor,
        ref purpose,
        ref items,
        ref mut error,
    } = modal
    {
        let submitted = super::password::handle_password_prompt(ke, app, input, cursor);
        if submitted {
            // Password submitted - validate before starting execution
            let password = if input.trim().is_empty() {
                None
            } else {
                Some(input.clone())
            };

            // Validate password if provided (skip validation for passwordless sudo)
            if let Some(ref pass) = password {
                // Validate password before starting execution
                // Always validate - don't skip even if passwordless sudo might be configured
                match crate::logic::password::validate_sudo_password(pass) {
                    Ok(true) => {
                        // Password is valid, continue with execution
                    }
                    Ok(false) => {
                        // Password is invalid - check faillock status and show error
                        let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());

                        // Check if user is now locked out (this may have just happened)
                        let (is_locked, lockout_until, remaining_minutes) =
                            crate::logic::faillock::get_lockout_info(&username);

                        // Update AppState immediately with lockout status
                        app.faillock_locked = is_locked;
                        app.faillock_lockout_until = lockout_until;
                        app.faillock_remaining_minutes = remaining_minutes;

                        if is_locked {
                            // User is locked out - show alert modal with lockout message
                            let lockout_msg = remaining_minutes.map_or_else(
                                || {
                                    crate::i18n::t_fmt1(
                                        app,
                                        "app.modals.alert.account_locked",
                                        &username,
                                    )
                                },
                                |remaining| {
                                    if remaining > 0 {
                                        crate::i18n::t_fmt(
                                            app,
                                            "app.modals.alert.account_locked_with_time",
                                            &[&username as &dyn std::fmt::Display, &remaining],
                                        )
                                    } else {
                                        crate::i18n::t_fmt1(
                                            app,
                                            "app.modals.alert.account_locked",
                                            &username,
                                        )
                                    }
                                },
                            );

                            // Close password prompt and show alert
                            // Clear any pending executor state to abort the process
                            app.pending_executor_password = None;
                            app.pending_exec_header_chips = None;
                            app.pending_executor_request = None;
                            app.modal = crate::state::Modal::Alert {
                                message: lockout_msg,
                            };
                            return true;
                        }

                        // Not locked out, check status for remaining attempts
                        let error_msg = crate::logic::faillock::check_faillock_status(&username)
                            .map_or_else(
                                |_| {
                                    // Couldn't check faillock status, just show generic error
                                    crate::i18n::t(
                                        app,
                                        "app.modals.password_prompt.incorrect_password",
                                    )
                                },
                                |status| {
                                    let remaining =
                                        status.max_attempts.saturating_sub(status.attempts_used);
                                    crate::i18n::t_fmt1(
                                        app,
                                        "app.modals.password_prompt.incorrect_password_attempts",
                                        remaining,
                                    )
                                },
                            );
                        // Update modal with error message and keep it open for retry
                        // Clear the input field so user can immediately type a new password
                        app.modal = crate::state::Modal::PasswordPrompt {
                            purpose: *purpose,
                            items: items.clone(),
                            input: String::new(), // Clear input field
                            cursor: 0,            // Reset cursor position
                            error: Some(error_msg),
                        };
                        // Don't start execution, keep modal open for retry
                        // Return true to stop event propagation and prevent restore from overwriting
                        return true;
                    }
                    Err(e) => {
                        // Error validating password (e.g., sudo not available)
                        // Update modal with error message and keep it open
                        app.modal = crate::state::Modal::PasswordPrompt {
                            purpose: *purpose,
                            items: items.clone(),
                            input: input.clone(),
                            cursor: *cursor,
                            error: Some(crate::i18n::t_fmt1(
                                app,
                                "app.modals.password_prompt.validation_failed",
                                &e,
                            )),
                        };
                        // Return true to stop event propagation and prevent restore from overwriting
                        return true;
                    }
                }
            }

            // Handle downgrade specially - it's an interactive tool that needs a terminal
            if matches!(purpose, crate::state::modal::PasswordPurpose::Downgrade) {
                // Downgrade tool is interactive and needs to run in a terminal
                // Close the modal and spawn downgrade in a terminal
                app.modal = crate::state::Modal::None;

                let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
                let joined = names.join(" ");

                let cmd = if app.dry_run {
                    // Properly quote the command to avoid syntax errors
                    let downgrade_cmd = format!("sudo downgrade {joined}");
                    let quoted = crate::install::shell_single_quote(&downgrade_cmd);
                    format!("echo DRY RUN: {quoted}")
                } else {
                    // Build command with password passed via sudo -S
                    let downgrade_cmd = password.as_ref().map_or_else(
                        || {
                            // No password (passwordless sudo)
                            format!("sudo downgrade {joined}")
                        },
                        |pass| {
                            // Use sudo -S to pass password via stdin
                            let pass_escaped = crate::install::shell_single_quote(pass);
                            format!("echo {pass_escaped} | sudo -S downgrade {joined}")
                        },
                    );

                    // Check if downgrade command exists or if package is installed (pacman -Qi works without sudo for installed packages)
                    format!(
                        "if (command -v downgrade >/dev/null 2>&1) || pacman -Qi downgrade >/dev/null 2>&1; then {downgrade_cmd}; else echo 'downgrade tool not found. Install \"downgrade\" package.'; fi"
                    )
                };

                // Clear downgrade list
                app.downgrade_list.clear();
                app.downgrade_list_names.clear();
                app.downgrade_state.select(None);

                // Spawn downgrade in a terminal (interactive tool needs full terminal)
                crate::install::spawn_shell_commands_in_terminal(&[cmd]);

                // Show toast message
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.downgrade_started"));
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));

                return true;
            }

            let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();

            // Check if this is a custom command (for special packages like paru/yay/semgrep-bin)
            if let Some(custom_cmd) = app.pending_custom_command.take() {
                // Transition to PreflightExec for custom command
                app.modal = Modal::PreflightExec {
                    items: items.clone(),
                    action: crate::state::PreflightAction::Install,
                    tab: crate::state::PreflightTab::Summary,
                    verbose: false,
                    log_lines: Vec::new(),
                    abortable: false,
                    header_chips,
                    success: None,
                };

                // Store executor request with password
                app.pending_executor_request = Some(ExecutorRequest::CustomCommand {
                    command: custom_cmd,
                    password,
                    dry_run: app.dry_run,
                });

                return true;
            }

            // Handle Update purpose with pending_update_commands
            if matches!(purpose, crate::state::modal::PasswordPurpose::Update) {
                if let Some(commands) = app.pending_update_commands.take() {
                    // Transition to PreflightExec for system update
                    app.modal = Modal::PreflightExec {
                        items: Vec::new(), // System update doesn't have package items
                        action: crate::state::PreflightAction::Install,
                        tab: crate::state::PreflightTab::Summary,
                        verbose: false,
                        log_lines: Vec::new(),
                        abortable: false,
                        header_chips,
                        success: None,
                    };

                    // Store executor request with password
                    app.pending_executor_request = Some(ExecutorRequest::Update {
                        commands,
                        password,
                        dry_run: app.dry_run,
                    });

                    return true;
                }
                // No pending commands, this shouldn't happen but handle gracefully
                app.modal = Modal::Alert {
                    message: "No update commands found".to_string(),
                };
                return true;
            }

            // For Install actions, use start_execution to check for reinstall scenarios
            // This ensures the reinstall confirmation modal is shown if needed
            if matches!(purpose, crate::state::modal::PasswordPurpose::Install) {
                use crate::events::preflight::keys;
                keys::start_execution(
                    app,
                    items,
                    crate::state::PreflightAction::Install,
                    header_chips,
                    password,
                );
                return true;
            }

            // For Remove actions, proceed directly (no reinstall check needed)
            let action = match purpose {
                crate::state::modal::PasswordPurpose::Install
                | crate::state::modal::PasswordPurpose::Update => {
                    // This should never be reached due to the check above
                    unreachable!("Install/Update should be handled above")
                }
                crate::state::modal::PasswordPurpose::Remove => {
                    crate::state::PreflightAction::Remove
                }
                crate::state::modal::PasswordPurpose::Downgrade => {
                    // This should never be reached due to the check above
                    unreachable!("Downgrade should be handled above")
                }
                crate::state::modal::PasswordPurpose::FileSync => {
                    // This should never be reached - FileSync is handled via custom command above
                    unreachable!("FileSync should be handled via custom command above")
                }
            };
            app.modal = Modal::PreflightExec {
                items: items.clone(),
                action,
                tab: crate::state::PreflightTab::Summary,
                verbose: false,
                log_lines: Vec::new(),
                success: None,
                abortable: false,
                header_chips,
            };

            // Store executor request for remove
            app.pending_executor_request = Some(match purpose {
                crate::state::modal::PasswordPurpose::Install
                | crate::state::modal::PasswordPurpose::Update => {
                    // This should never be reached due to the check above
                    unreachable!("Install/Update should be handled above")
                }
                crate::state::modal::PasswordPurpose::Remove => {
                    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
                    ExecutorRequest::Remove {
                        names,
                        password,
                        cascade: app.remove_cascade_mode,
                        dry_run: app.dry_run,
                    }
                }
                crate::state::modal::PasswordPurpose::Downgrade => {
                    // This should never be reached due to the check above, but included for exhaustiveness
                    unreachable!("Downgrade should be handled above")
                }
                crate::state::modal::PasswordPurpose::FileSync => {
                    // This should never be reached - FileSync is handled via custom command above
                    unreachable!("FileSync should be handled via custom command above")
                }
            });

            return true;
        }
        restore::restore_if_not_closed_with_esc(
            app,
            &ke,
            Modal::PasswordPrompt {
                purpose: *purpose,
                items: items.clone(),
                input: input.clone(),
                cursor: *cursor,
                error: error.clone(),
            },
        );
    }
    false
}

/// What: Handle key events for `ImportHelp` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `add_tx`: Channel for adding packages
/// - `modal`: `ImportHelp` modal variant (unit type)
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to import handler
pub(super) fn handle_import_help_modal(
    ke: KeyEvent,
    app: &mut AppState,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    _modal: Modal,
) -> bool {
    super::import::handle_import_help(ke, app, add_tx);
    false
}
