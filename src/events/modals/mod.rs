//! Modal event handling module (excluding Preflight which is in preflight.rs).

mod common;
mod import;
mod install;
mod optional_deps;
mod scan;
mod system_update;

#[cfg(test)]
mod tests;

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// What: Handle key events for every modal except Preflight, mutating UI state as needed.
///
/// Inputs:
/// - `ke`: Key event delivered while a non-Preflight modal is active
/// - `app`: Mutable application state holding the active modal and related data
/// - `add_tx`: Channel used to enqueue packages into the install list from modal actions
///
/// Output:
/// - `true` if the event is fully handled and should not propagate to other handlers; otherwise `false`.
///
/// Details:
/// - Covers Alert, PreflightExec, PostSummary, SystemUpdate, ConfirmInstall/Remove, Help, News,
///   OptionalDeps, VirusTotalSetup, ScanConfig, ImportHelp, and other lightweight modals.
/// - Each branch performs modal-specific mutations (toggles, list navigation, spawning commands) and
///   is responsible for clearing or restoring `app.modal` when exiting.
/// - When a modal should block further processing this function returns `true`, allowing callers to
///   short-circuit additional event handling.
pub(crate) fn handle_modal_key(
    ke: KeyEvent,
    app: &mut AppState,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    // Use a temporary to avoid borrow checker issues
    let modal = std::mem::take(&mut app.modal);
    match modal {
        crate::state::Modal::Alert { message } => {
            common::handle_alert(ke, app, &message);
            false
        }
        crate::state::Modal::PreflightExec {
            mut verbose,
            log_lines,
            abortable,
            items,
            action,
            tab,
            header_chips,
        } => {
            common::handle_preflight_exec(ke, app, &mut verbose, abortable, &items);
            // Restore modal with updated verbose if handler didn't change it and key wasn't Esc/q
            if matches!(app.modal, crate::state::Modal::None)
                && !matches!(ke.code, KeyCode::Esc | KeyCode::Char('q'))
            {
                app.modal = crate::state::Modal::PreflightExec {
                    verbose,
                    log_lines,
                    abortable,
                    items,
                    action,
                    tab,
                    header_chips,
                };
            }
            false
        }
        crate::state::Modal::PostSummary {
            success,
            changed_files,
            pacnew_count,
            pacsave_count,
            services_pending,
            snapshot_label,
        } => {
            common::handle_post_summary(ke, app, &services_pending);
            // Restore modal if handler didn't change it and key wasn't Esc/Enter/q
            if matches!(app.modal, crate::state::Modal::None)
                && !matches!(ke.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q'))
            {
                app.modal = crate::state::Modal::PostSummary {
                    success,
                    changed_files,
                    pacnew_count,
                    pacsave_count,
                    services_pending,
                    snapshot_label,
                };
            }
            false
        }
        crate::state::Modal::SystemUpdate {
            mut do_mirrors,
            mut do_pacman,
            mut do_aur,
            mut do_cache,
            mut country_idx,
            countries,
            mut mirror_count,
            mut cursor,
        } => {
            let should_stop = system_update::handle_system_update(
                ke,
                app,
                &mut do_mirrors,
                &mut do_pacman,
                &mut do_aur,
                &mut do_cache,
                &mut country_idx,
                &countries,
                &mut mirror_count,
                &mut cursor,
            );
            // Restore modal with updated state if handler didn't change modal type
            // (either handler didn't handle the key, or handled it but didn't close the modal)
            if matches!(app.modal, crate::state::Modal::None) {
                // Only restore if handler didn't intentionally close (Esc returns Some(false) but closes modal)
                // For navigation/toggle keys, handler returns Some(false) but doesn't close, so we restore
                if should_stop.is_none()
                    || (should_stop == Some(false) && !matches!(ke.code, KeyCode::Esc))
                {
                    app.modal = crate::state::Modal::SystemUpdate {
                        do_mirrors,
                        do_pacman,
                        do_aur,
                        do_cache,
                        country_idx,
                        countries,
                        mirror_count,
                        cursor,
                    };
                }
            }
            should_stop.unwrap_or(false)
        }
        crate::state::Modal::ConfirmInstall { items } => {
            install::handle_confirm_install(ke, app, &items);
            false
        }
        crate::state::Modal::ConfirmRemove { items } => {
            install::handle_confirm_remove(ke, app, &items);
            false
        }
        crate::state::Modal::Help => {
            common::handle_help(ke, app);
            false
        }
        crate::state::Modal::News {
            items,
            mut selected,
        } => {
            let result = common::handle_news(ke, app, &items, &mut selected);
            // Restore modal if handler didn't change it and Esc wasn't pressed (result != true)
            // Esc returns true to stop propagation, so we shouldn't restore in that case
            if !result && matches!(app.modal, crate::state::Modal::None) {
                app.modal = crate::state::Modal::News { items, selected };
            }
            result
        }
        crate::state::Modal::OptionalDeps { rows, mut selected } => {
            let should_stop = optional_deps::handle_optional_deps(ke, app, &rows, &mut selected);
            // Restore modal with updated state if handler didn't change modal type
            // (either handler didn't handle the key, or handled it but didn't close the modal)
            if matches!(app.modal, crate::state::Modal::None) {
                // Only restore if handler didn't intentionally close (Esc returns Some(false) but closes modal)
                // For navigation keys, handler returns Some(false) but doesn't close, so we restore
                if should_stop.is_none()
                    || (should_stop == Some(false) && !matches!(ke.code, KeyCode::Esc))
                {
                    app.modal = crate::state::Modal::OptionalDeps { rows, selected };
                }
            }
            should_stop.unwrap_or(false)
        }
        crate::state::Modal::ScanConfig {
            mut do_clamav,
            mut do_trivy,
            mut do_semgrep,
            mut do_shellcheck,
            mut do_virustotal,
            mut do_custom,
            mut do_sleuth,
            mut cursor,
        } => {
            scan::handle_scan_config(
                ke,
                app,
                &mut do_clamav,
                &mut do_trivy,
                &mut do_semgrep,
                &mut do_shellcheck,
                &mut do_virustotal,
                &mut do_custom,
                &mut do_sleuth,
                &mut cursor,
            );
            // Restore modal if handler didn't change it and key wasn't Esc
            if matches!(app.modal, crate::state::Modal::None) && !matches!(ke.code, KeyCode::Esc) {
                app.modal = crate::state::Modal::ScanConfig {
                    do_clamav,
                    do_trivy,
                    do_semgrep,
                    do_shellcheck,
                    do_virustotal,
                    do_custom,
                    do_sleuth,
                    cursor,
                };
            }
            false
        }
        crate::state::Modal::VirusTotalSetup {
            mut input,
            mut cursor,
        } => {
            scan::handle_virustotal_setup(ke, app, &mut input, &mut cursor);
            // Restore modal if handler didn't change it and key wasn't Esc
            if matches!(app.modal, crate::state::Modal::None) && !matches!(ke.code, KeyCode::Esc) {
                app.modal = crate::state::Modal::VirusTotalSetup { input, cursor };
            }
            false
        }
        crate::state::Modal::GnomeTerminalPrompt => {
            common::handle_gnome_terminal_prompt(ke, app);
            false
        }
        crate::state::Modal::ImportHelp => {
            import::handle_import_help(ke, app, add_tx);
            false
        }
        crate::state::Modal::None => false,
        crate::state::Modal::Preflight { .. } => {
            // Preflight is handled separately in preflight.rs
            // Restore it - we shouldn't have gotten here, but be safe
            app.modal = modal;
            false
        }
    }
}
