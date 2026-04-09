//! System update modal handling.

use crossterm::event::{KeyCode, KeyEvent};

use crate::events::distro;
use crate::state::AppState;

/// What: Handle key events for `SystemUpdate` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `do_mirrors`: Mutable reference to mirrors flag
/// - `do_pacman`: Mutable reference to pacman flag
/// - `force_sync`: Mutable reference to force sync flag (toggled with Left/Right on pacman row)
/// - `do_aur`: Mutable reference to AUR flag
/// - `do_cache`: Mutable reference to cache flag
/// - `country_idx`: Mutable reference to selected country index
/// - `countries`: Available countries list
/// - `mirror_count`: Mutable reference to mirror count
/// - `cursor`: Mutable reference to cursor position
///
/// Output:
/// - `Some(true)` if Enter was pressed and commands were executed, `Some(false)` otherwise, `None` if not handled
///
/// Details:
/// - Handles Esc/q to close, navigation, toggles, and Enter to execute update commands
/// - Left/Right on row 1 (pacman) toggles between Normal (-Syu) and Force Sync (-Syyu)
/// - Left/Right on row 4 (country) cycles through countries
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_system_update(
    ke: KeyEvent,
    app: &mut AppState,
    do_mirrors: &mut bool,
    do_pacman: &mut bool,
    force_sync: &mut bool,
    do_aur: &mut bool,
    do_cache: &mut bool,
    country_idx: &mut usize,
    countries: &[String],
    mirror_count: &mut u16,
    cursor: &mut usize,
) -> Option<bool> {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            Some(false)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *cursor > 0 {
                *cursor -= 1;
            }
            Some(false)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = 4; // 4 options (0..3) + country row (index 4)
            if *cursor < max {
                *cursor += 1;
            }
            Some(false)
        }
        KeyCode::Left => {
            match *cursor {
                // Toggle sync mode on pacman row
                1 => *force_sync = !*force_sync,
                // Cycle countries on country row
                4 if !countries.is_empty() => {
                    if *country_idx == 0 {
                        *country_idx = countries.len() - 1;
                    } else {
                        *country_idx -= 1;
                    }
                }
                _ => {}
            }
            Some(false)
        }
        KeyCode::Right | KeyCode::Tab => {
            match *cursor {
                // Toggle sync mode on pacman row
                1 => *force_sync = !*force_sync,
                // Cycle countries on country row
                4 if !countries.is_empty() => {
                    *country_idx = (*country_idx + 1) % countries.len();
                }
                _ => {}
            }
            Some(false)
        }
        KeyCode::Char(' ') => {
            match *cursor {
                0 => *do_mirrors = !*do_mirrors,
                1 => *do_pacman = !*do_pacman,
                2 => *do_aur = !*do_aur,
                3 => *do_cache = !*do_cache,
                _ => {}
            }
            Some(false)
        }
        KeyCode::Char('-') => {
            // Decrease mirror count when focused on the country/count row
            if *cursor == 4 && *mirror_count > 1 {
                *mirror_count -= 1;
                crate::theme::save_mirror_count(*mirror_count);
            }
            Some(false)
        }
        KeyCode::Char('+') => {
            // Increase mirror count when focused on the country/count row
            if *cursor == 4 && *mirror_count < 200 {
                *mirror_count += 1;
                crate::theme::save_mirror_count(*mirror_count);
            }
            Some(false)
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            handle_system_update_enter(
                app,
                *do_mirrors,
                *do_pacman,
                *force_sync,
                *do_aur,
                *do_cache,
                *country_idx,
                countries,
                *mirror_count,
            );
            Some(true)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;

/// What: Build and execute system update commands using executor pattern.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `do_mirrors`: Whether to update mirrors
/// - `do_pacman`: Whether to update pacman packages
/// - `force_sync`: Whether to force sync databases (use -Syyu instead of -Syu)
/// - `do_aur`: Whether to update AUR packages
/// - `do_cache`: Whether to clean cache
/// - `country_idx`: Selected country index
/// - `countries`: Available countries list
/// - `mirror_count`: Number of mirrors to use
///
/// Output:
/// - Stores commands in `pending_update_commands` and transitions to `PasswordPrompt` modal,
///   or `Modal::Alert` if no actions selected
///
/// Details:
/// - Builds command list based on selected options
/// - Shows `PasswordPrompt` modal to get sudo password
/// - Actual execution happens after password is validated in password handler
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn handle_system_update_enter(
    app: &mut AppState,
    do_mirrors: bool,
    do_pacman: bool,
    force_sync: bool,
    do_aur: bool,
    do_cache: bool,
    country_idx: usize,
    countries: &[String],
    mirror_count: u16,
) {
    maybe_show_long_run_auth_preflight_warning(app);

    let mut cmds: Vec<String> = Vec::new();
    if do_mirrors {
        let sel = if country_idx < countries.len() {
            countries[country_idx].as_str()
        } else {
            "Worldwide"
        };
        let prefs = crate::theme::settings();
        let countries_arg = if sel == "Worldwide" {
            prefs.selected_countries.as_str()
        } else {
            sel
        };
        crate::theme::save_selected_countries(countries_arg);
        crate::theme::save_mirror_count(mirror_count);
        match distro::mirror_update_command(countries_arg, mirror_count) {
            Ok(cmd) => cmds.push(cmd),
            Err(msg) => {
                app.modal = crate::state::Modal::Alert { message: msg };
                return;
            }
        }
    }
    if do_pacman {
        // Use -Syyu (force sync) or -Syu (normal sync) based on user selection
        let sync_flag = if force_sync { "-Syyu" } else { "-Syu" };
        let tool = match crate::logic::privilege::active_tool() {
            Ok(t) => t,
            Err(msg) => {
                app.modal = crate::state::Modal::Alert { message: msg };
                return;
            }
        };
        cmds.push(crate::logic::privilege::build_privilege_command(
            tool,
            &format!("pacman {sync_flag} --noconfirm"),
        ));
    }

    // Build AUR command separately - will be executed conditionally if pacman fails
    let aur_command = if do_aur {
        // Always use -Sua (AUR only) to update only AUR packages
        // AUR helpers (paru/yay) will automatically handle dependency resolution:
        // - If AUR packages require newer official packages, the helper will report this
        // - Users can then also select pacman update if dependency issues occur
        // - This follows Arch Linux best practices: update official packages first, then AUR
        let sync_flag = "-Sua";
        Some(format!(
            "if command -v paru >/dev/null 2>&1; then \
                paru {sync_flag} --noconfirm; \
            elif command -v yay >/dev/null 2>&1; then \
                yay {sync_flag} --noconfirm; \
            else \
                echo 'No AUR helper (paru/yay) found.'; \
            fi"
        ))
    } else {
        None
    };

    // If both pacman and AUR are selected, store AUR command separately for conditional execution
    // If only AUR is selected, add it to the main command list
    if let Some(aur_cmd) = aur_command {
        if do_pacman {
            // Store AUR command separately - will be executed after pacman if user confirms
            app.pending_aur_update_command = Some(aur_cmd);
        } else {
            // Only AUR selected, add to main command list
            cmds.push(aur_cmd);
        }
    }
    if do_cache {
        let tool = match crate::logic::privilege::active_tool() {
            Ok(t) => t,
            Err(msg) => {
                app.modal = crate::state::Modal::Alert { message: msg };
                return;
            }
        };
        let bin = tool.binary_name();
        cmds.push(crate::logic::privilege::build_privilege_command(
            tool,
            "pacman -Sc --noconfirm",
        ));
        cmds.push(format!("((command -v paru >/dev/null 2>&1 || {bin} pacman -Qi paru >/dev/null 2>&1) && paru -Sc --noconfirm) || ((command -v yay >/dev/null 2>&1 || {bin} pacman -Qi yay >/dev/null 2>&1) && yay -Sc --noconfirm) || true"));
    }
    if cmds.is_empty() {
        app.modal = crate::state::Modal::Alert {
            message: "No actions selected".to_string(),
        };
        return;
    }

    // In test mode with PACSEA_TEST_OUT, spawn terminal directly to allow tests to verify terminal argument shapes
    // This bypasses the executor pattern which runs commands in PTY
    if std::env::var("PACSEA_TEST_OUT").is_ok() {
        crate::install::spawn_shell_commands_in_terminal(&cmds);
        app.modal = crate::state::Modal::None;
        return;
    }

    let settings = crate::theme::settings();
    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
        match crate::events::try_interactive_auth_handoff() {
            Ok(true) => {
                app.modal = crate::state::Modal::PreflightExec {
                    items: Vec::new(),
                    action: crate::state::PreflightAction::Install,
                    tab: crate::state::PreflightTab::Summary,
                    verbose: false,
                    log_lines: Vec::new(),
                    abortable: false,
                    header_chips: crate::state::modal::PreflightHeaderChips::default(),
                    success: None,
                };
                app.pending_executor_request = Some(crate::install::ExecutorRequest::Update {
                    commands: cmds,
                    password: None,
                    dry_run: app.dry_run,
                });
            }
            Ok(false) => {
                app.modal = crate::state::Modal::Alert {
                    message: crate::i18n::t(app, "app.errors.authentication_failed"),
                };
            }
            Err(e) => {
                app.modal = crate::state::Modal::Alert { message: e };
            }
        }
        return;
    }

    if crate::logic::password::resolve_auth_mode(&settings)
        == crate::logic::privilege::AuthMode::PasswordlessOnly
        && crate::logic::password::should_use_passwordless_sudo(&settings)
    {
        app.modal = crate::state::Modal::PreflightExec {
            items: Vec::new(),
            action: crate::state::PreflightAction::Install,
            tab: crate::state::PreflightTab::Summary,
            verbose: false,
            log_lines: Vec::new(),
            abortable: false,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            success: None,
        };
        app.pending_executor_request = Some(crate::install::ExecutorRequest::Update {
            commands: cmds,
            password: None,
            dry_run: app.dry_run,
        });
        return;
    }

    // Store update commands for processing after password prompt.
    app.pending_update_commands = Some(cmds);

    // Prompt mode with stdin-capable tools still uses in-app password entry.
    app.modal = crate::state::Modal::PasswordPrompt {
        purpose: crate::state::modal::PasswordPurpose::Update,
        items: Vec::new(), // System update doesn't have package items
        input: crate::state::SecureString::default(),
        cursor: 0,
        error: None,
    };
}

/// What: Show one-time long-run auth preflight guidance for system update flows.
///
/// Inputs:
/// - `app`: Mutable application state.
///
/// Output:
/// - Sets a warning toast and latches the session guard when guidance should be shown.
///
/// Details:
/// - Uses long-run auth readiness evaluation from current settings.
/// - Warning is only shown once per session via `long_run_auth_preflight_warned`.
fn maybe_show_long_run_auth_preflight_warning(app: &mut AppState) {
    let settings = crate::theme::settings();
    let readiness = crate::logic::long_run_auth::evaluate_long_run_auth_readiness(&settings);
    if readiness.should_warn && !app.long_run_auth_preflight_warned {
        app.long_run_auth_preflight_warned = true;
        app.toast_message = Some(crate::logic::long_run_auth::build_long_run_warning_message(
            app,
        ));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
    }
}
