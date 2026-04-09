//! Individual modal handler functions that encapsulate field extraction and restoration.

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use super::restore;
use crate::install::ExecutorRequest;
use crate::state::{AppState, Modal, PackageItem};

/// Startup selector item count.
const STARTUP_SETUP_SELECTOR_ITEMS: usize = 7;

/// What: Check whether a startup selector task can be toggled by the user.
#[must_use]
fn startup_selector_task_selectable(
    task: crate::state::modal::StartupSetupTask,
    app: &AppState,
    active_tool: Option<crate::logic::privilege::PrivilegeTool>,
) -> bool {
    match task {
        crate::state::modal::StartupSetupTask::SshAurSetup => {
            !app.aur_ssh_help_ready.unwrap_or(false)
        }
        crate::state::modal::StartupSetupTask::SudoTimestampSetup => {
            matches!(
                active_tool,
                Some(crate::logic::privilege::PrivilegeTool::Sudo)
            )
        }
        crate::state::modal::StartupSetupTask::DoasPersistSetup => {
            matches!(
                active_tool,
                Some(crate::logic::privilege::PrivilegeTool::Doas)
            )
        }
        _ => true,
    }
}

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
/// - Defers Enter to post-summary when a repository overlap check is pending but the executor has
///   not yet set `success`, so the overlap step can still run after completion
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
        // Defer Enter until the executor sets `success`: overlap runs only when
        // `success == Some(true)`; an early Enter would open post-summary with `success` still
        // `None` and drop `Finished` output because the modal is no longer `PreflightExec`.
        if matches!(ke.code, KeyCode::Enter | KeyCode::Char('\n' | '\r'))
            && app.pending_repo_apply_overlap_check.is_some()
            && items.is_empty()
            && success.is_none()
        {
            app.toast_message = Some(crate::i18n::t(
                app,
                "app.toasts.repo_apply_wait_exec_finish",
            ));
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
            app.modal = modal;
            return true;
        }
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
            &[
                KeyCode::Esc,
                KeyCode::Enter,
                KeyCode::Char('q'),
                KeyCode::Char('\n'),
                KeyCode::Char('\r'),
            ],
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
            KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
                // Continue with batch update - use executor pattern instead of spawning terminal
                let items_clone = items.clone();
                let dry_run_clone = *dry_run;
                app.dry_run = dry_run_clone;

                // Get header_chips if available from pending_exec_header_chips, otherwise use default
                let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();

                if crate::events::install::try_open_warn_aur_repo_duplicate_modal(
                    app,
                    &items_clone,
                    header_chips.clone(),
                ) {
                    return true;
                }

                let settings = crate::theme::settings();
                if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
                    match crate::events::try_interactive_auth_handoff() {
                        Ok(true) => crate::events::preflight::start_execution(
                            app,
                            &items_clone,
                            crate::state::PreflightAction::Install,
                            header_chips,
                            None,
                        ),
                        Ok(false) => {
                            app.modal = crate::state::Modal::Alert {
                                message: crate::i18n::t(app, "app.errors.authentication_failed"),
                            };
                        }
                        Err(e) => {
                            app.modal = crate::state::Modal::Alert { message: e };
                        }
                    }
                } else if crate::logic::password::resolve_auth_mode(&settings)
                    == crate::logic::privilege::AuthMode::PasswordlessOnly
                    && crate::logic::password::should_use_passwordless_sudo(&settings)
                {
                    crate::events::preflight::start_execution(
                        app,
                        &items_clone,
                        crate::state::PreflightAction::Install,
                        header_chips,
                        None,
                    );
                } else {
                    app.modal = crate::state::Modal::PasswordPrompt {
                        purpose: crate::state::modal::PasswordPurpose::Install,
                        items: items_clone,
                        input: crate::state::SecureString::default(),
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
            KeyCode::Enter | KeyCode::Char('\n' | '\r' | 'y' | 'Y') => {
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

/// What: Handle keys for `WarnAurRepoDuplicate`, restoring modal when the key is not consumed.
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Application state.
/// - `modal`: Taken modal reference (original state before `mem::take`).
///
/// Output:
/// - `true` when the event was consumed.
///
/// Details:
/// - Delegates to [`super::foreign_overlap::handle_warn_aur_repo_duplicate_modal`].
pub(super) fn handle_warn_aur_repo_duplicate_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: &Modal,
) -> bool {
    if let Modal::WarnAurRepoDuplicate {
        dup_names,
        packages,
        header_chips,
    } = modal
    {
        let consumed = super::foreign_overlap::handle_warn_aur_repo_duplicate_modal(
            ke,
            app,
            dup_names,
            packages,
            header_chips,
        );
        if !consumed {
            app.modal = modal.clone();
        }
        return consumed;
    }
    false
}

/// What: Handle keys for `ForeignRepoOverlap`, restoring modal when the key is not consumed.
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Application state.
/// - `modal`: Taken modal reference.
///
/// Output:
/// - `true` when the event was consumed.
pub(super) fn handle_foreign_repo_overlap_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: &Modal,
) -> bool {
    if let Modal::ForeignRepoOverlap {
        repo_name,
        entries,
        phase,
    } = modal
    {
        let consumed = super::foreign_overlap::handle_foreign_repo_overlap_modal(
            ke,
            app,
            repo_name,
            entries,
            phase.clone(),
        );
        if !consumed {
            app.modal = modal.clone();
        }
        return consumed;
    }
    false
}

/// What: Handle key events for `ConfirmAurVote` modal.
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Mutable application state.
/// - `modal`: `ConfirmAurVote` modal variant.
///
/// Output:
/// - `true` if modal was closed or transitioned, `false` otherwise.
///
/// Details:
/// - Enter/y confirms and queues the request for tick-handler dispatch.
/// - Esc/q/n cancels the intent and closes the modal.
pub(super) fn handle_confirm_aur_vote_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: &Modal,
) -> bool {
    if let Modal::ConfirmAurVote {
        pkgbase, action, ..
    } = modal
    {
        match ke.code {
            KeyCode::Esc | KeyCode::Char('q' | 'Q' | 'n' | 'N') => {
                app.pending_aur_vote_intent = None;
                app.modal = crate::state::Modal::None;
                app.toast_message =
                    Some(format!("Cancelled AUR {action} request for '{pkgbase}'."));
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                return true;
            }
            KeyCode::Enter | KeyCode::Char('\n' | '\r' | 'y' | 'Y') => {
                app.pending_aur_vote_intent = None;
                app.pending_aur_vote_request = Some((pkgbase.clone(), *action));
                let action_label = match action {
                    crate::sources::VoteAction::Vote => "vote",
                    crate::sources::VoteAction::Unvote => "unvote",
                };
                app.toast_message = Some(format!(
                    "Queued AUR {action_label} request for '{pkgbase}'."
                ));
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                app.modal = crate::state::Modal::None;
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
            KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
                // Proceed with reinstall - use executor pattern
                // Use all_items (all packages) instead of just installed ones
                let items_clone = all_items.clone();
                let header_chips_clone = header_chips.clone();
                if crate::events::install::try_open_warn_aur_repo_duplicate_modal(
                    app,
                    &items_clone,
                    header_chips_clone.clone(),
                ) {
                    return true;
                }
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

                    let settings = crate::theme::settings();
                    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
                        match crate::events::try_interactive_auth_handoff() {
                            Ok(true) => crate::events::preflight::start_execution(
                                app,
                                &items_clone,
                                crate::state::PreflightAction::Install,
                                header_chips_clone,
                                None,
                            ),
                            Ok(false) => {
                                app.modal = crate::state::Modal::Alert {
                                    message: crate::i18n::t(
                                        app,
                                        "app.errors.authentication_failed",
                                    ),
                                };
                            }
                            Err(e) => {
                                app.modal = crate::state::Modal::Alert { message: e };
                            }
                        }
                    } else if crate::logic::password::resolve_auth_mode(&settings)
                        == crate::logic::privilege::AuthMode::PasswordlessOnly
                        && crate::logic::password::should_use_passwordless_sudo(&settings)
                    {
                        crate::events::preflight::start_execution(
                            app,
                            &items_clone,
                            crate::state::PreflightAction::Install,
                            header_chips_clone,
                            None,
                        );
                    } else {
                        app.modal = crate::state::Modal::PasswordPrompt {
                            purpose: crate::state::modal::PasswordPurpose::Install,
                            items: items_clone,
                            input: crate::state::SecureString::default(),
                            cursor: 0,
                            error: None,
                        };
                        app.pending_exec_header_chips = Some(header_chips_clone);
                    }
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
        ref mut filter_active,
        ref mut filter_query,
        ref mut filter_caret,
        ref mut last_selected_pkg_name,
        ref mut filtered_indices,
        ref mut selected_pkg_names,
    } = modal
    {
        let result = super::common::handle_updates(
            ke,
            app,
            entries,
            scroll,
            selected,
            filter_active,
            filter_query,
            filter_caret,
            last_selected_pkg_name,
            filtered_indices,
            selected_pkg_names,
        );
        return restore::restore_if_not_closed_with_bool_result(
            app,
            result,
            Modal::Updates {
                entries: entries.clone(),
                scroll: *scroll,
                selected: *selected,
                filter_active: *filter_active,
                filter_query: filter_query.clone(),
                filter_caret: *filter_caret,
                last_selected_pkg_name: last_selected_pkg_name.clone(),
                filtered_indices: filtered_indices.clone(),
                selected_pkg_names: selected_pkg_names.clone(),
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
        ref mut selected_pkg_names,
    } = modal
    {
        let should_stop =
            super::optional_deps::handle_optional_deps(ke, app, rows, selected, selected_pkg_names);
        return restore::restore_if_not_closed_with_option_result(
            app,
            &ke,
            should_stop,
            Modal::OptionalDeps {
                rows: rows.clone(),
                selected: *selected,
                selected_pkg_names: selected_pkg_names.clone(),
            },
        );
    }
    false
}

/// What: Handle key events for the read-only Repositories modal.
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Mutable application state.
/// - `modal`: `Repositories` modal variant.
///
/// Output:
/// - `true` when Esc/q should stop propagation, as with other list modals.
///
/// Details:
/// - Restores modal state after navigation unless the user closed it.
pub(super) fn handle_repositories_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::Repositories {
        ref rows,
        ref mut selected,
        ref mut scroll,
        ref repos_conf_error,
        ref pacman_warnings,
    } = modal
    {
        if matches!(ke.code, KeyCode::Char(' ')) {
            match super::repositories::toggle_selected_repo_enabled_and_apply(
                app,
                rows,
                *selected,
                *scroll,
                repos_conf_error.as_deref(),
            ) {
                Ok(()) => return true,
                Err(msg) => {
                    app.toast_message = Some(msg);
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
                    return restore::restore_if_not_closed_with_option_result(
                        app,
                        &ke,
                        Some(false),
                        Modal::Repositories {
                            rows: rows.clone(),
                            selected: *selected,
                            scroll: *scroll,
                            repos_conf_error: repos_conf_error.clone(),
                            pacman_warnings: pacman_warnings.clone(),
                        },
                    );
                }
            }
        }
        if matches!(ke.code, KeyCode::Enter | KeyCode::Char('\n' | '\r')) {
            match super::repositories::enter_repo_apply(
                app,
                rows,
                *selected,
                *scroll,
                repos_conf_error.as_deref(),
            ) {
                Ok(()) => return true,
                Err(msg) => {
                    app.modal = Modal::Alert { message: msg };
                    return true;
                }
            }
        }
        if matches!(ke.code, KeyCode::Char('r' | 'R')) {
            match super::repositories::enter_repo_key_refresh(
                app,
                rows,
                *selected,
                repos_conf_error.as_deref(),
            ) {
                Ok(()) => return true,
                Err(msg) => {
                    app.modal = Modal::Alert { message: msg };
                    return true;
                }
            }
        }
        if matches!(ke.code, KeyCode::Char('s' | 'S')) {
            super::repositories::open_repos_conf_example_in_editor(app);
            return restore::restore_if_not_closed_with_option_result(
                app,
                &ke,
                Some(false),
                Modal::Repositories {
                    rows: rows.clone(),
                    selected: *selected,
                    scroll: *scroll,
                    repos_conf_error: repos_conf_error.clone(),
                    pacman_warnings: pacman_warnings.clone(),
                },
            );
        }
        let should_stop = super::repositories::handle_repositories_modal_keys(
            ke,
            app,
            rows.len(),
            selected,
            scroll,
        );
        return restore::restore_if_not_closed_with_option_result(
            app,
            &ke,
            should_stop,
            Modal::Repositories {
                rows: rows.clone(),
                selected: *selected,
                scroll: *scroll,
                repos_conf_error: repos_conf_error.clone(),
                pacman_warnings: pacman_warnings.clone(),
            },
        );
    }
    false
}

/// What: Handle key events for `SshAurSetup` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Mutable application state.
/// - `modal`: `SshAurSetup` modal variant.
///
/// Output:
/// - `true` if event propagation should stop, otherwise `false`.
pub(super) fn handle_ssh_setup_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::SshAurSetup {
        ref mut step,
        ref mut status_lines,
        ref mut existing_host_block,
    } = modal
    {
        let result = super::optional_deps::handle_ssh_setup_modal(
            ke,
            app,
            step,
            status_lines,
            existing_host_block,
        );
        return restore::restore_if_not_closed_with_option_result(
            app,
            &ke,
            result,
            Modal::SshAurSetup {
                step: *step,
                status_lines: status_lines.clone(),
                existing_host_block: existing_host_block.clone(),
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
                // Cancel startup-news setup and continue startup flow.
                // Do not restore previous modal here: previous_modal is used by
                // unrelated flows (e.g. scan/preflight) and can be stale.
                app.previous_modal = None;
                app.modal = crate::state::Modal::None;
                if app.pending_startup_setup_steps.is_empty() {
                    super::common::show_next_pending_announcement(app);
                } else {
                    super::common::show_next_startup_setup_step(app);
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
            KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
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
                if app.pending_startup_setup_steps.is_empty() {
                    super::common::show_next_pending_announcement(app);
                } else {
                    super::common::show_next_startup_setup_step(app);
                }
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
/// - `true` (always stops propagation while this modal is active)
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
        let should_advance = matches!(ke.code, KeyCode::Esc)
            || (matches!(ke.code, KeyCode::Enter | KeyCode::Char('\n' | '\r'))
                && !input.trim().is_empty());
        super::scan::handle_virustotal_setup(ke, app, input, cursor);
        if should_advance
            && matches!(app.modal, Modal::None)
            && !app.pending_startup_setup_steps.is_empty()
        {
            super::common::show_next_startup_setup_step(app);
        }
        if !(should_advance && matches!(app.modal, Modal::None)) {
            restore::restore_if_not_closed_with_esc(
                app,
                &ke,
                Modal::VirusTotalSetup {
                    input: input.clone(),
                    cursor: *cursor,
                },
            );
        }
        return true;
    }
    false
}

/// What: Handle key events for `SudoTimestampSetup` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `SudoTimestampSetup` modal variant
///
/// Output:
/// - `true` (always stops propagation while this modal is active)
///
/// Details:
/// - Advances the first-startup queue when the wizard completes while a queue is pending.
#[allow(clippy::needless_pass_by_value)] // Matches `handle_modal_key` ownership pattern (`std::mem::take`).
pub(super) fn handle_sudo_timestamp_setup_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: Modal,
) -> bool {
    let Modal::SudoTimestampSetup { mut setup } = modal else {
        return false;
    };
    let finished =
        super::sudo_timestamp_setup::handle_sudo_timestamp_setup_key(ke, app, &mut setup);
    if finished {
        app.modal = Modal::None;
        if !app.pending_startup_setup_steps.is_empty() {
            super::common::show_next_startup_setup_step(app);
        }
    } else {
        app.modal = Modal::SudoTimestampSetup { setup };
    }
    true
}

/// What: Handle key events for `DoasPersistSetup` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `DoasPersistSetup` modal variant
///
/// Output:
/// - `true` (always stops propagation while this modal is active)
#[allow(clippy::needless_pass_by_value)]
pub(super) fn handle_doas_persist_setup_modal(
    ke: KeyEvent,
    app: &mut AppState,
    modal: Modal,
) -> bool {
    let Modal::DoasPersistSetup { mut setup } = modal else {
        return false;
    };
    let finished = super::doas_persist_setup::handle_doas_persist_setup_key(ke, app, &mut setup);
    if finished {
        app.modal = Modal::None;
        if !app.pending_startup_setup_steps.is_empty() {
            super::common::show_next_startup_setup_step(app);
        }
    } else {
        app.modal = Modal::DoasPersistSetup { setup };
    }
    true
}

/// What: Handle key events for `StartupSetupSelector` modal.
pub(super) fn handle_startup_setup_selector_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::StartupSetupSelector {
        ref mut cursor,
        ref mut selected,
        active_privilege_tool,
    } = modal
    {
        match ke.code {
            KeyCode::Esc => {
                app.pending_startup_setup_steps.clear();
                app.modal = Modal::None;
                super::common::show_next_pending_announcement(app);
                return true;
            }
            KeyCode::Char('r' | 'R') => {
                // Never show startup selector again.
                crate::theme::save_startup_news_configured(true);
                app.pending_startup_setup_steps.clear();
                app.modal = Modal::None;
                super::common::show_next_pending_announcement(app);
                return true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if *cursor + 1 < STARTUP_SETUP_SELECTOR_ITEMS {
                    *cursor += 1;
                }
            }
            KeyCode::Char(' ') => {
                let max_cursor = STARTUP_SETUP_SELECTOR_ITEMS.saturating_sub(1);
                *cursor = (*cursor).min(max_cursor);
                let task = match *cursor {
                    0 => crate::state::modal::StartupSetupTask::ArchNews,
                    1 => crate::state::modal::StartupSetupTask::SshAurSetup,
                    2 => crate::state::modal::StartupSetupTask::OptionalDepsMissing,
                    3 => crate::state::modal::StartupSetupTask::SudoTimestampSetup,
                    4 => crate::state::modal::StartupSetupTask::DoasPersistSetup,
                    5 => crate::state::modal::StartupSetupTask::AurSleuthSetup,
                    _ => crate::state::modal::StartupSetupTask::VirusTotalSetup,
                };
                if !startup_selector_task_selectable(task, app, active_privilege_tool) {
                    app.modal = Modal::StartupSetupSelector {
                        cursor: *cursor,
                        selected: selected.clone(),
                        active_privilege_tool,
                    };
                    return false;
                }
                if selected.contains(&task) {
                    selected.remove(&task);
                } else {
                    selected.insert(task);
                }
            }
            KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
                if app.aur_ssh_help_ready.unwrap_or(false) {
                    selected.remove(&crate::state::modal::StartupSetupTask::SshAurSetup);
                }
                app.pending_startup_setup_steps =
                    super::common::startup_setup_steps_in_priority(selected);
                app.modal = Modal::None;
                super::common::show_next_startup_setup_step(app);
                return true;
            }
            _ => {}
        }
        app.modal = Modal::StartupSetupSelector {
            cursor: *cursor,
            selected: selected.clone(),
            active_privilege_tool,
        };
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
        if !submitted && matches!(ke.code, KeyCode::Esc) {
            match purpose {
                crate::state::modal::PasswordPurpose::RepoApply => {
                    app.pending_repo_apply_commands = None;
                    app.pending_repo_apply_summary = None;
                    app.pending_repo_apply_overlap_check = None;
                    app.pending_repositories_modal_resume = None;
                }
                crate::state::modal::PasswordPurpose::RepoForeignMigrate => {
                    app.pending_foreign_migrate_commands = None;
                    app.pending_foreign_migrate_summary = None;
                }
                crate::state::modal::PasswordPurpose::Update => {
                    app.pending_update_commands = None;
                }
                crate::state::modal::PasswordPurpose::Install
                | crate::state::modal::PasswordPurpose::Remove
                | crate::state::modal::PasswordPurpose::Downgrade
                | crate::state::modal::PasswordPurpose::FileSync => {}
            }
        }
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
                match crate::logic::password::validate_sudo_password(pass.as_str()) {
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
                            app.pending_repo_apply_commands = None;
                            app.pending_repo_apply_summary = None;
                            app.pending_repo_apply_overlap_check = None;
                            app.pending_repositories_modal_resume = None;
                            app.pending_foreign_migrate_commands = None;
                            app.pending_foreign_migrate_summary = None;
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
                            input: crate::state::SecureString::default(), // Clear input field
                            cursor: 0,                                    // Reset cursor position
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

                let tool = match crate::logic::privilege::active_tool() {
                    Ok(t) => t,
                    Err(msg) => {
                        app.modal = crate::state::Modal::Alert { message: msg };
                        return true;
                    }
                };
                let cmd = if app.dry_run {
                    let downgrade_cmd = crate::logic::privilege::build_privilege_command(
                        tool,
                        &format!("downgrade {joined}"),
                    );
                    let quoted = crate::install::shell_single_quote(&downgrade_cmd);
                    format!("echo DRY RUN: {quoted}")
                } else {
                    let downgrade_cmd = password.as_ref().map_or_else(
                        || {
                            crate::logic::privilege::build_privilege_command(
                                tool,
                                &format!("downgrade {joined}"),
                            )
                        },
                        |pass| {
                            crate::logic::privilege::build_password_pipe(
                                tool,
                                pass,
                                &format!("downgrade {joined}"),
                            )
                            .unwrap_or_else(|| {
                                crate::logic::privilege::build_privilege_command(
                                    tool,
                                    &format!("downgrade {joined}"),
                                )
                            })
                        },
                    );

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
                    // Store password and header_chips so AUR update can run after pacman succeeds,
                    // or so ConfirmAurUpdate can run AUR if pacman fails
                    match (&mut app.pending_executor_password, &password) {
                        (Some(p), Some(pass)) => p.clone_from(pass),
                        (None, Some(pass)) => app.pending_executor_password = Some(pass.clone()),
                        (_, None) => app.pending_executor_password = None,
                    }
                    app.pending_exec_header_chips = Some(header_chips.clone());

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

            if matches!(purpose, crate::state::modal::PasswordPurpose::RepoApply) {
                if let Some(commands) = app.pending_repo_apply_commands.take() {
                    match (&mut app.pending_executor_password, &password) {
                        (Some(p), Some(pass)) => p.clone_from(pass),
                        (None, Some(pass)) => app.pending_executor_password = Some(pass.clone()),
                        (_, None) => app.pending_executor_password = None,
                    }
                    app.pending_exec_header_chips = Some(header_chips.clone());
                    let log_lines = app.pending_repo_apply_summary.take().unwrap_or_default();
                    app.modal = Modal::PreflightExec {
                        items: Vec::new(),
                        action: crate::state::PreflightAction::Install,
                        tab: crate::state::PreflightTab::Summary,
                        verbose: false,
                        log_lines,
                        abortable: false,
                        header_chips,
                        success: None,
                    };
                    app.pending_executor_request = Some(ExecutorRequest::Update {
                        commands,
                        password,
                        dry_run: app.dry_run,
                    });
                    return true;
                }
                app.modal = Modal::Alert {
                    message: crate::i18n::t(app, "app.modals.repositories.apply.missing_commands"),
                };
                return true;
            }

            if matches!(
                purpose,
                crate::state::modal::PasswordPurpose::RepoForeignMigrate
            ) {
                if let Some(commands) = app.pending_foreign_migrate_commands.take() {
                    match (&mut app.pending_executor_password, &password) {
                        (Some(p), Some(pass)) => p.clone_from(pass),
                        (None, Some(pass)) => app.pending_executor_password = Some(pass.clone()),
                        (_, None) => app.pending_executor_password = None,
                    }
                    app.pending_exec_header_chips = Some(header_chips.clone());
                    let log_lines = app
                        .pending_foreign_migrate_summary
                        .take()
                        .unwrap_or_default();
                    app.modal = Modal::PreflightExec {
                        items: Vec::new(),
                        action: crate::state::PreflightAction::Install,
                        tab: crate::state::PreflightTab::Summary,
                        verbose: false,
                        log_lines,
                        abortable: false,
                        header_chips,
                        success: None,
                    };
                    app.pending_executor_request = Some(ExecutorRequest::Update {
                        commands,
                        password,
                        dry_run: app.dry_run,
                    });
                    return true;
                }
                app.modal = Modal::Alert {
                    message: crate::i18n::t(
                        app,
                        "app.modals.foreign_overlap.missing_migrate_commands",
                    ),
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
                | crate::state::modal::PasswordPurpose::Update
                | crate::state::modal::PasswordPurpose::RepoApply
                | crate::state::modal::PasswordPurpose::RepoForeignMigrate => {
                    // This should never be reached due to the check above
                    unreachable!(
                        "Install/Update/RepoApply/RepoForeignMigrate should be handled above"
                    )
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
                | crate::state::modal::PasswordPurpose::Update
                | crate::state::modal::PasswordPurpose::RepoApply
                | crate::state::modal::PasswordPurpose::RepoForeignMigrate => {
                    // This should never be reached due to the check above
                    unreachable!(
                        "Install/Update/RepoApply/RepoForeignMigrate should be handled above"
                    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::collections::VecDeque;

    #[test]
    fn startup_selector_enter_builds_and_starts_queue() {
        let mut app = AppState::default();
        let mut selected = std::collections::HashSet::new();
        selected.insert(crate::state::modal::StartupSetupTask::ArchNews);
        selected.insert(crate::state::modal::StartupSetupTask::VirusTotalSetup);
        let modal = Modal::StartupSetupSelector {
            cursor: 0,
            selected,
            active_privilege_tool: None,
        };
        let handled = handle_startup_setup_selector_modal(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            modal,
        );
        assert!(handled);
        assert!(matches!(app.modal, Modal::VirusTotalSetup { .. }));
        assert_eq!(
            app.pending_startup_setup_steps,
            VecDeque::from([crate::state::modal::StartupSetupTask::ArchNews])
        );
    }

    #[test]
    fn startup_selector_esc_skips_all() {
        let mut app = AppState {
            pending_startup_setup_steps: VecDeque::from([
                crate::state::modal::StartupSetupTask::ArchNews,
            ]),
            ..AppState::default()
        };
        let modal = Modal::StartupSetupSelector {
            cursor: 0,
            selected: std::collections::HashSet::new(),
            active_privilege_tool: None,
        };
        let handled = handle_startup_setup_selector_modal(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &mut app,
            modal,
        );
        assert!(handled);
        assert!(matches!(app.modal, Modal::None));
        assert!(app.pending_startup_setup_steps.is_empty());
    }

    #[test]
    fn startup_selector_space_with_out_of_range_cursor_keeps_modal_and_clamps() {
        let mut app = AppState::default();
        let selected = std::collections::HashSet::new();
        let modal = Modal::StartupSetupSelector {
            cursor: usize::MAX,
            selected,
            active_privilege_tool: None,
        };

        let handled = handle_startup_setup_selector_modal(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
            &mut app,
            modal,
        );

        assert!(!handled);
        match &app.modal {
            Modal::StartupSetupSelector { cursor, .. } => {
                assert_eq!(*cursor, STARTUP_SETUP_SELECTOR_ITEMS - 1);
            }
            _ => panic!("startup selector modal should remain active"),
        }
    }

    #[test]
    fn sudo_setup_finish_consumes_enter_key() {
        let mut app = AppState::default();
        let modal = Modal::SudoTimestampSetup {
            setup: crate::state::modal::SudoTimestampSetupModalState {
                phase: crate::state::modal::SudoTimestampSetupPhase::Select,
                select_cursor: crate::state::modal::SUDO_TIMESTAMP_SELECT_ROWS - 1,
            },
        };
        let handled = handle_sudo_timestamp_setup_modal(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            modal,
        );
        assert!(handled);
        assert!(matches!(app.modal, Modal::None));
    }

    #[test]
    fn doas_setup_finish_consumes_enter_key() {
        let mut app = AppState::default();
        let modal = Modal::DoasPersistSetup {
            setup: crate::state::modal::DoasPersistSetupModalState {
                phase: crate::state::modal::DoasPersistSetupPhase::Select,
                select_cursor: crate::state::modal::DOAS_PERSIST_SELECT_ROWS - 1,
            },
        };
        let handled = handle_doas_persist_setup_modal(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            modal,
        );
        assert!(handled);
        assert!(matches!(app.modal, Modal::None));
    }

    #[test]
    fn virustotal_setup_enter_consumes_key_when_closing_modal() {
        let mut app = AppState::default();
        app.pending_startup_setup_steps.clear();
        let modal = Modal::VirusTotalSetup {
            input: "dummy-api-key".to_string(),
            cursor: 12,
        };
        let handled = handle_virustotal_setup_modal(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            modal,
        );
        assert!(handled);
        assert!(
            !matches!(app.modal, Modal::VirusTotalSetup { .. }),
            "virustotal setup modal should close on Enter with non-empty key"
        );
    }

    #[test]
    fn news_setup_esc_does_not_restore_stale_previous_modal() {
        let mut app = AppState::default();
        app.pending_startup_setup_steps.clear();
        app.previous_modal = Some(Modal::News {
            items: vec![crate::state::types::NewsFeedItem {
                id: "news-1".to_string(),
                date: "2026-01-01".to_string(),
                title: "Old news".to_string(),
                summary: None,
                url: Some("https://example.com/news-1".to_string()),
                source: crate::state::types::NewsFeedSource::ArchNews,
                severity: None,
                packages: Vec::new(),
            }],
            selected: 0,
            scroll: 0,
        });
        let modal = Modal::NewsSetup {
            show_arch_news: true,
            show_advisories: true,
            show_aur_updates: true,
            show_aur_comments: true,
            show_pkg_updates: true,
            max_age_days: Some(30),
            cursor: 0,
        };

        let handled = handle_news_setup_modal(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &mut app,
            modal,
        );

        assert!(handled);
        assert!(
            !matches!(app.modal, Modal::News { .. }),
            "Esc in NewsSetup must not resurrect stale News modal"
        );
        assert!(
            app.previous_modal.is_none(),
            "stale previous_modal should be cleared on cancel"
        );
    }
}
