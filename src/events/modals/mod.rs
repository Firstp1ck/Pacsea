//! Modal event handling module (excluding Preflight which is in preflight.rs).

mod common;
mod handlers;
mod import;
mod install;
mod optional_deps;
mod restore;
mod scan;
mod system_update;

#[cfg(test)]
mod tests;

use crossterm::event::KeyEvent;
use tokio::sync::mpsc;

use crate::state::{AppState, Modal, PackageItem};

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
        Modal::Alert { .. } => handlers::handle_alert_modal(ke, app, modal),
        Modal::PreflightExec { .. } => handlers::handle_preflight_exec_modal(ke, app, modal),
        Modal::PostSummary { .. } => handlers::handle_post_summary_modal(ke, app, modal),
        Modal::SystemUpdate { .. } => handlers::handle_system_update_modal(ke, app, modal),
        Modal::ConfirmInstall { .. } => handlers::handle_confirm_install_modal(ke, app, modal),
        Modal::ConfirmRemove { .. } => handlers::handle_confirm_remove_modal(ke, app, modal),
        Modal::Help => handlers::handle_help_modal(ke, app, modal),
        Modal::News { .. } => handlers::handle_news_modal(ke, app, modal),
        Modal::OptionalDeps { .. } => handlers::handle_optional_deps_modal(ke, app, modal),
        Modal::ScanConfig { .. } => handlers::handle_scan_config_modal(ke, app, modal),
        Modal::VirusTotalSetup { .. } => handlers::handle_virustotal_setup_modal(ke, app, modal),
        Modal::GnomeTerminalPrompt => handlers::handle_gnome_terminal_prompt_modal(ke, app, modal),
        Modal::ImportHelp => handlers::handle_import_help_modal(ke, app, add_tx, modal),
        Modal::None => false,
        Modal::Preflight { .. } => {
            // Preflight is handled separately in preflight.rs
            // Restore it - we shouldn't have gotten here, but be safe
            app.modal = modal;
            false
        }
    }
}
