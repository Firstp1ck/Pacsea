//! Individual modal handler functions that encapsulate field extraction and restoration.

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use super::restore;
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
    } = modal
    {
        let should_stop = super::common::handle_preflight_exec(ke, app, verbose, *abortable, items);
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
                // Continue with batch update
                let items_clone = items.clone();
                let dry_run_clone = *dry_run;
                app.modal = crate::state::Modal::None;
                // Proceed with the actual install
                crate::install::spawn_install_all(&items_clone, dry_run_clone);
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
    } = modal
    {
        let result = super::common::handle_news(ke, app, items, selected);
        return restore::restore_if_not_closed_with_bool_result(
            app,
            result,
            Modal::News {
                items: items.clone(),
                selected: *selected,
            },
        );
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
            // Password submitted - transition to PreflightExec and store executor request
            use crate::install::ExecutorRequest;

            let password = if input.trim().is_empty() {
                None
            } else {
                Some(input.clone())
            };

            let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();

            // Transition to PreflightExec
            let action = match purpose {
                crate::state::modal::PasswordPurpose::Install
                | crate::state::modal::PasswordPurpose::Update => {
                    crate::state::PreflightAction::Install
                }
                crate::state::modal::PasswordPurpose::Remove => {
                    crate::state::PreflightAction::Remove
                }
            };
            app.modal = Modal::PreflightExec {
                items: items.clone(),
                action,
                tab: crate::state::PreflightTab::Summary,
                verbose: false,
                log_lines: Vec::new(),
                abortable: false,
                header_chips,
            };

            // Store executor request
            app.pending_executor_request = Some(match purpose {
                crate::state::modal::PasswordPurpose::Install
                | crate::state::modal::PasswordPurpose::Update => ExecutorRequest::Install {
                    items: items.clone(),
                    password,
                    dry_run: app.dry_run,
                },
                crate::state::modal::PasswordPurpose::Remove => {
                    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
                    ExecutorRequest::Remove {
                        names,
                        password,
                        cascade: app.remove_cascade_mode,
                        dry_run: app.dry_run,
                    }
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
