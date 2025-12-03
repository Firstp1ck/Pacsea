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
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_system_update(
    ke: KeyEvent,
    app: &mut AppState,
    do_mirrors: &mut bool,
    do_pacman: &mut bool,
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
            if *cursor == 4 && !countries.is_empty() {
                if *country_idx == 0 {
                    *country_idx = countries.len() - 1;
                } else {
                    *country_idx -= 1;
                }
            }
            Some(false)
        }
        KeyCode::Right => {
            if *cursor == 4 && !countries.is_empty() {
                *country_idx = (*country_idx + 1) % countries.len();
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
        KeyCode::Enter => {
            handle_system_update_enter(
                app,
                *do_mirrors,
                *do_pacman,
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
/// - `do_aur`: Whether to update AUR packages
/// - `do_cache`: Whether to clean cache
/// - `country_idx`: Selected country index
/// - `countries`: Available countries list
/// - `mirror_count`: Number of mirrors to use
///
/// Output:
/// - Sets `app.pending_executor_request` and transitions to `PreflightExec` modal, or `Modal::Alert` if no actions selected
///
/// Details:
/// - Builds command list based on selected options
/// - Uses `ExecutorRequest::Update` to execute commands via PTY executor
/// - Transitions to `PreflightExec` modal for in-TUI execution
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn handle_system_update_enter(
    app: &mut AppState,
    do_mirrors: bool,
    do_pacman: bool,
    do_aur: bool,
    do_cache: bool,
    country_idx: usize,
    countries: &[String],
    mirror_count: u16,
) {
    use crate::install::ExecutorRequest;

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
        cmds.push(distro::mirror_update_command(countries_arg, mirror_count));
    }
    if do_pacman {
        cmds.push("sudo pacman -Syu --noconfirm".to_string());
    }
    if do_aur {
        cmds.push(
            "if command -v paru >/dev/null 2>&1; then \
                paru -Syu --noconfirm; \
            elif command -v yay >/dev/null 2>&1; then \
                yay -Syu --noconfirm; \
            else \
                echo 'No AUR helper (paru/yay) found.'; \
            fi"
            .to_string(),
        );
    }
    if do_cache {
        cmds.push("sudo pacman -Sc --noconfirm".to_string());
        cmds.push("((command -v paru >/dev/null 2>&1 || sudo pacman -Qi paru >/dev/null 2>&1) && paru -Sc --noconfirm) || ((command -v yay >/dev/null 2>&1 || sudo pacman -Qi yay >/dev/null 2>&1) && yay -Sc --noconfirm) || true".to_string());
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

    // Store executor request for processing in tick handler
    app.pending_executor_request = Some(ExecutorRequest::Update {
        commands: cmds,
        password: None, // Password will be handled via PasswordPrompt modal if needed
        dry_run: app.dry_run,
    });

    // Transition to PreflightExec modal
    app.modal = crate::state::Modal::PreflightExec {
        items: Vec::new(), // System update doesn't have package items
        action: crate::state::PreflightAction::Install, // Use Install action for display purposes
        tab: crate::state::PreflightTab::Summary,
        verbose: false,
        log_lines: Vec::new(),
        abortable: false,
        header_chips: crate::state::modal::PreflightHeaderChips::default(),
        success: None,
    };
}
