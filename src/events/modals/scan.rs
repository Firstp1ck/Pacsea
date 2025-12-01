//! Scan configuration and `VirusTotal` setup modal handling.

use crossterm::event::{KeyCode, KeyEvent};

use crate::install::ExecutorRequest;
use crate::state::{AppState, PackageItem, Source};

/// What: Handle key events for `ScanConfig` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `do_clamav`: Mutable reference to `ClamAV` flag
/// - `do_trivy`: Mutable reference to Trivy flag
/// - `do_semgrep`: Mutable reference to Semgrep flag
/// - `do_shellcheck`: Mutable reference to `Shellcheck` flag
/// - `do_virustotal`: Mutable reference to `VirusTotal` flag
/// - `do_custom`: Mutable reference to custom scan flag
/// - `do_sleuth`: Mutable reference to sleuth flag
/// - `cursor`: Mutable reference to cursor position
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles navigation, toggles, and Enter to confirm scan configuration
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_scan_config(
    ke: KeyEvent,
    app: &mut AppState,
    do_clamav: &mut bool,
    do_trivy: &mut bool,
    do_semgrep: &mut bool,
    do_shellcheck: &mut bool,
    do_virustotal: &mut bool,
    do_custom: &mut bool,
    do_sleuth: &mut bool,
    cursor: &mut usize,
) -> bool {
    match ke.code {
        KeyCode::Esc => {
            // Restore previous modal if it was Preflight, otherwise close
            if let Some(prev_modal) = app.previous_modal.take() {
                app.modal = prev_modal;
            } else {
                app.modal = crate::state::Modal::None;
            }
        }
        KeyCode::Up => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Down => {
            if *cursor < 6 {
                *cursor += 1;
            }
        }
        KeyCode::Char(' ') => match *cursor {
            0 => *do_clamav = !*do_clamav,
            1 => *do_trivy = !*do_trivy,
            2 => *do_semgrep = !*do_semgrep,
            3 => *do_shellcheck = !*do_shellcheck,
            4 => *do_virustotal = !*do_virustotal,
            5 => *do_custom = !*do_custom,
            6 => *do_sleuth = !*do_sleuth,
            _ => {}
        },
        KeyCode::Enter => {
            let pending_names = app.pending_install_names.clone();
            let new_modal = handle_scan_config_confirm(
                app,
                pending_names.as_ref(),
                app.dry_run,
                *do_clamav,
                *do_trivy,
                *do_semgrep,
                *do_shellcheck,
                *do_virustotal,
                *do_custom,
                *do_sleuth,
            );
            app.modal = new_modal;
        }
        _ => {}
    }
    false
}

/// What: Handle key events for `VirusTotalSetup` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `input`: Mutable reference to input string
/// - `cursor`: Mutable reference to cursor position
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles text input, navigation, and Enter to save API key
pub(super) fn handle_virustotal_setup(
    ke: KeyEvent,
    app: &mut AppState,
    input: &mut String,
    cursor: &mut usize,
) -> bool {
    match ke.code {
        KeyCode::Esc => {
            app.modal = crate::state::Modal::None;
        }
        KeyCode::Enter => {
            let key = input.trim().to_string();
            if key.is_empty() {
                let url = "https://www.virustotal.com/gui/my-apikey";
                crate::util::open_url(url);
                // Keep the setup modal open so the user can paste the key after opening the link
            } else {
                crate::theme::save_virustotal_api_key(&key);
                app.modal = crate::state::Modal::None;
            }
        }
        KeyCode::Backspace => {
            if *cursor > 0 && *cursor <= input.len() {
                input.remove(*cursor - 1);
                *cursor -= 1;
            }
        }
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Right => {
            if *cursor < input.len() {
                *cursor += 1;
            }
        }
        KeyCode::Home => {
            *cursor = 0;
        }
        KeyCode::End => {
            *cursor = input.len();
        }
        KeyCode::Char(ch) => {
            if !ch.is_control() {
                if *cursor <= input.len() {
                    input.insert(*cursor, ch);
                    *cursor += 1;
                } else {
                    input.push(ch);
                    *cursor = input.len();
                }
            }
        }
        _ => {}
    }
    false
}

/// What: Confirm and execute scan configuration.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `pending_install_names`: Mutable reference to pending install names
/// - `dry_run`: Whether to run in dry-run mode
/// - `do_clamav`: `ClamAV` scan flag
/// - `do_trivy`: Trivy scan flag
/// - `do_semgrep`: Semgrep scan flag
/// - `do_shellcheck`: `Shellcheck` scan flag
/// - `do_virustotal`: `VirusTotal` scan flag
/// - `do_custom`: Custom scan flag
/// - `do_sleuth`: Sleuth scan flag
///
/// Output: New modal state (`PreflightExec` for first package, `None` for subsequent)
///
/// Details:
/// - Persists scan settings and launches AUR scans via integrated process
/// - aur-sleuth runs in separate terminal simultaneously if enabled
#[allow(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    clippy::needless_pass_by_ref_mut
)]
fn handle_scan_config_confirm(
    app: &mut crate::state::AppState,
    pending_install_names: Option<&Vec<String>>,
    dry_run: bool,
    do_clamav: bool,
    do_trivy: bool,
    do_semgrep: bool,
    do_shellcheck: bool,
    do_virustotal: bool,
    do_custom: bool,
    do_sleuth: bool,
) -> crate::state::Modal {
    tracing::info!(
        event = "scan_config_confirm",
        dry_run,
        do_clamav,
        do_trivy,
        do_semgrep,
        do_shellcheck,
        do_virustotal,
        do_custom,
        pending_count = pending_install_names.map_or(0, Vec::len),
        "Scan Configuration confirmed"
    );
    crate::theme::save_scan_do_clamav(do_clamav);
    crate::theme::save_scan_do_trivy(do_trivy);
    crate::theme::save_scan_do_semgrep(do_semgrep);
    crate::theme::save_scan_do_shellcheck(do_shellcheck);
    crate::theme::save_scan_do_virustotal(do_virustotal);
    crate::theme::save_scan_do_custom(do_custom);
    crate::theme::save_scan_do_sleuth(do_sleuth);

    #[cfg(not(target_os = "windows"))]
    if let Some(names) = pending_install_names {
        if names.is_empty() {
            tracing::warn!("Scan confirmed but no pending AUR package names were found");
            return crate::state::Modal::None;
        }

        tracing::info!(
            names = ?names,
            count = names.len(),
            dry_run,
            "Launching AUR scans via integrated process"
        );

        // Handle each package sequentially (for now, just first package)
        // TODO: Add support for sequential multi-package scans
        let first_pkg = &names[0];

        // Create a dummy `PackageItem` for display in `PreflightExec` modal
        let scan_item = PackageItem {
            name: format!("scan:{first_pkg}"),
            version: String::new(),
            description: format!("Security scan for {first_pkg}"),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };

        // Transition to PreflightExec modal for scan
        app.modal = crate::state::Modal::PreflightExec {
            items: vec![scan_item],
            action: crate::state::PreflightAction::Install, // Use Install as placeholder
            tab: crate::state::PreflightTab::Summary,
            verbose: false,
            log_lines: Vec::new(),
            abortable: false,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            success: None,
        };

        // Store executor request for scan
        app.pending_executor_request = Some(ExecutorRequest::Scan {
            package: first_pkg.clone(),
            do_clamav,
            do_trivy,
            do_semgrep,
            do_shellcheck,
            do_virustotal,
            do_custom,
            dry_run,
        });

        // If sleuth is enabled, spawn it in a separate terminal (runs simultaneously)
        if do_sleuth && !dry_run {
            let sleuth_cmd = crate::install::build_sleuth_command_for_terminal(first_pkg);
            crate::install::spawn_shell_commands_in_terminal(&[sleuth_cmd]);
        }

        // If there are multiple packages, log a warning (sequential scans not yet implemented)
        if names.len() > 1 {
            tracing::warn!(
                "Multiple packages requested for scan, but only scanning first package: {}",
                first_pkg
            );
        }

        return crate::state::Modal::PreflightExec {
            items: vec![PackageItem {
                name: format!("scan:{first_pkg}"),
                version: String::new(),
                description: format!("Security scan for {first_pkg}"),
                source: Source::Aur,
                popularity: None,
                out_of_date: None,
                orphaned: false,
            }],
            action: crate::state::PreflightAction::Install,
            tab: crate::state::PreflightTab::Summary,
            verbose: false,
            log_lines: Vec::new(),
            abortable: false,
            success: None,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
        };
    }

    tracing::warn!("Scan confirmed but no pending AUR package names were found");
    crate::state::Modal::None
}

#[cfg(test)]
mod tests;
