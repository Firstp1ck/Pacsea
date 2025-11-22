//! Install and remove modal handling.

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::{AppState, PackageItem};

/// What: Handle key events for `ConfirmInstall` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `items`: Package items to install
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles Esc to close, Enter to install, s to scan
pub(super) fn handle_confirm_install(
    ke: KeyEvent,
    app: &mut AppState,
    items: &[PackageItem],
) -> bool {
    match ke.code {
        KeyCode::Esc => {
            app.modal = crate::state::Modal::None;
        }
        KeyCode::Enter => {
            let new_modal = handle_confirm_install_enter(
                &mut app.refresh_installed_until,
                &mut app.next_installed_refresh_at,
                &mut app.pending_install_names,
                app.dry_run,
                items,
            );
            app.modal = new_modal;
        }
        KeyCode::Char('s' | 'S') => {
            let new_modal = handle_confirm_install_scan(&mut app.pending_install_names, items);
            app.modal = new_modal;
        }
        _ => {}
    }
    false
}

/// What: Handle key events for `ConfirmRemove` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `items`: Package items to remove
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles Esc/Enter to cancel (defaults to No)
/// - Only proceeds with removal if user explicitly presses 'y' or 'Y'
pub(super) fn handle_confirm_remove(
    ke: KeyEvent,
    app: &mut AppState,
    items: &[PackageItem],
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Enter => {
            // Cancel removal (defaults to No)
            app.modal = crate::state::Modal::None;
        }
        KeyCode::Char('y' | 'Y') => {
            // Explicit confirmation required - proceed with removal
            handle_confirm_remove_execute(
                &mut app.remove_list,
                &mut app.remove_state,
                &mut app.refresh_installed_until,
                &mut app.next_installed_refresh_at,
                &mut app.pending_remove_names,
                app.dry_run,
                app.remove_cascade_mode,
                items,
            );
            app.modal = crate::state::Modal::None;
        }
        _ => {}
    }
    false
}

/// What: Execute package installation.
///
/// Inputs:
/// - `refresh_installed_until`: Mutable reference to refresh timer
/// - `next_installed_refresh_at`: Mutable reference to next refresh time
/// - `pending_install_names`: Mutable reference to pending install names
/// - `dry_run`: Whether to run in dry-run mode
/// - `items`: Package items to install
///
/// Output: New modal state (always None after install)
///
/// Details:
/// - Spawns install command(s) and sets up refresh tracking
fn handle_confirm_install_enter(
    refresh_installed_until: &mut Option<std::time::Instant>,
    next_installed_refresh_at: &mut Option<std::time::Instant>,
    pending_install_names: &mut Option<Vec<String>>,
    dry_run: bool,
    items: &[PackageItem],
) -> crate::state::Modal {
    let list = items.to_vec();
    if list.len() <= 1 {
        if let Some(it) = list.first() {
            crate::install::spawn_install(it, None, dry_run);
            if !dry_run {
                *refresh_installed_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(12));
                *next_installed_refresh_at = None;
                *pending_install_names = Some(vec![it.name.clone()]);
            }
        }
    } else {
        crate::install::spawn_install_all(&list, dry_run);
        if !dry_run {
            *refresh_installed_until =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(12));
            *next_installed_refresh_at = None;
            *pending_install_names = Some(list.iter().map(|p| p.name.clone()).collect());
        }
    }
    crate::state::Modal::None
}

/// What: Setup scan configuration for AUR packages.
///
/// Inputs:
/// - `pending_install_names`: Mutable reference to pending install names
/// - `items`: Package items to scan
///
/// Output: New modal state (Alert or `ScanConfig`)
///
/// Details:
/// - Filters AUR packages and opens scan configuration modal
fn handle_confirm_install_scan(
    pending_install_names: &mut Option<Vec<String>>,
    items: &[PackageItem],
) -> crate::state::Modal {
    let list = items.to_vec();
    let mut names: Vec<String> = Vec::new();
    for it in list.iter() {
        if matches!(it.source, crate::state::Source::Aur) {
            names.push(it.name.clone());
        }
    }
    if names.is_empty() {
        crate::state::Modal::Alert {
            message: "No AUR packages selected to scan.\nSelect AUR results or add AUR packages to the Install list, then press 's'.".into(),
        }
    } else {
        *pending_install_names = Some(names);
        let prefs = crate::theme::settings();
        crate::state::Modal::ScanConfig {
            do_clamav: prefs.scan_do_clamav,
            do_trivy: prefs.scan_do_trivy,
            do_semgrep: prefs.scan_do_semgrep,
            do_shellcheck: prefs.scan_do_shellcheck,
            do_virustotal: prefs.scan_do_virustotal,
            do_custom: prefs.scan_do_custom,
            do_sleuth: prefs.scan_do_sleuth,
            cursor: 0,
        }
    }
}

/// What: Execute package removal.
///
/// Inputs:
/// - `remove_list`: Mutable reference to remove list
/// - `remove_state`: Mutable reference to remove state
/// - `refresh_installed_until`: Mutable reference to refresh timer
/// - `next_installed_refresh_at`: Mutable reference to next refresh time
/// - `pending_remove_names`: Mutable reference to pending remove names
/// - `dry_run`: Whether to run in dry-run mode
/// - `remove_cascade_mode`: Cascade mode for removal
/// - `items`: Package items to remove
///
/// Output: None (modifies state)
///
/// Details:
/// - Spawns removal command and updates UI state
#[allow(clippy::too_many_arguments)]
fn handle_confirm_remove_execute(
    remove_list: &mut Vec<PackageItem>,
    remove_state: &mut ratatui::widgets::ListState,
    refresh_installed_until: &mut Option<std::time::Instant>,
    next_installed_refresh_at: &mut Option<std::time::Instant>,
    pending_remove_names: &mut Option<Vec<String>>,
    dry_run: bool,
    remove_cascade_mode: crate::state::modal::CascadeMode,
    items: &[PackageItem],
) {
    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
    if dry_run {
        crate::install::spawn_remove_all(&names, true, remove_cascade_mode);
        remove_list.retain(|p| !names.iter().any(|n| n == &p.name));
        remove_state.select(None);
    } else {
        crate::install::spawn_remove_all(&names, false, remove_cascade_mode);
        remove_list.retain(|p| !names.iter().any(|n| n == &p.name));
        remove_state.select(None);
        *refresh_installed_until =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
        *next_installed_refresh_at = None;
        *pending_remove_names = Some(names);
    }
}
