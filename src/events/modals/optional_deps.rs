//! Optional dependencies modal handling.

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::AppState;

/// What: Open one of the setup pseudo-packages used by first-startup orchestration.
///
/// Inputs:
/// - `app`: Mutable application state.
/// - `package`: Pseudo-package id (`aur-ssh-setup`, `aur-sleuth-setup`, `virustotal-setup`).
///
/// Output:
/// - Sets `app.modal` or spawns setup terminal command depending on selected setup.
pub(super) fn open_setup_package(app: &mut AppState, package: &str) {
    let row = crate::state::types::OptionalDepRow {
        label: package.to_string(),
        package: package.to_string(),
        installed: false,
        selectable: true,
        note: Some("Setup".to_string()),
    };
    let (new_modal, _) = handle_optional_deps_enter(app, &row);
    app.modal = new_modal;
}

/// What: Handle key events for `OptionalDeps` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `rows`: Optional dependency rows
/// - `selected`: Currently selected row index
///
/// Output:
/// - `Some(true)` if Enter was pressed and should stop propagation, `Some(false)` otherwise, `None` if not handled
///
/// Details:
/// - Handles Esc/q to close, navigation and Enter to install/setup optional dependencies
pub(super) fn handle_optional_deps(
    ke: KeyEvent,
    app: &mut AppState,
    rows: &[crate::state::types::OptionalDepRow],
    selected: &mut usize,
) -> Option<bool> {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            if !app.pending_startup_setup_steps.is_empty() {
                super::common::show_next_startup_setup_step(app);
            }
            Some(false)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected > 0 {
                *selected -= 1;
            }
            Some(false)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected + 1 < rows.len() {
                *selected += 1;
            }
            Some(false)
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            if let Some(row) = rows.get(*selected) {
                match handle_optional_deps_enter(app, row) {
                    (new_modal, true) => {
                        app.modal = new_modal;
                        Some(true)
                    }
                    (new_modal, false) => {
                        app.modal = new_modal;
                        Some(false)
                    }
                }
            } else {
                Some(false)
            }
        }
        _ => None,
    }
}

/// What: Handle Enter key in `OptionalDeps` modal.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `row`: Selected optional dependency row
///
/// Output:
/// - `(new_modal, should_stop_propagation)` tuple
///
/// Details:
/// - Handles setup for virustotal/aur-sleuth (keeps terminal spawn for interactive setup)
/// - Shows reinstall confirmation for already installed dependencies
/// - Installs optional dependencies using executor pattern
#[allow(clippy::too_many_lines)] // Complex function handling multiple installation paths (function has 227 lines)
fn handle_optional_deps_enter(
    app: &mut AppState,
    row: &crate::state::types::OptionalDepRow,
) -> (crate::state::Modal, bool) {
    use crate::install::ExecutorRequest;
    use crate::state::{PackageItem, Source};

    // Setup flows need interactive terminal, keep as-is
    if row.package == "aur-ssh-setup" {
        return (
            crate::state::Modal::SshAurSetup {
                step: crate::state::SshSetupStep::Intro,
                status_lines: vec![
                    "This setup will create '~/.ssh/aur_key' if missing.".to_string(),
                    "It will add/update Host aur.archlinux.org in '~/.ssh/config'.".to_string(),
                    "Then it validates with: ssh aur@aur.archlinux.org help".to_string(),
                    format!(
                        "After setup, upload '~/.ssh/aur_key.pub' to {}",
                        crate::logic::ssh_setup::AUR_ACCOUNT_URL
                    ),
                    "Press Enter to run, O to open account page, Esc to cancel.".to_string(),
                ],
                existing_host_block: None,
            },
            false,
        );
    }
    if row.package == "virustotal-setup" {
        let current = crate::theme::settings().virustotal_api_key;
        let cur_len = current.len();
        return (
            crate::state::Modal::VirusTotalSetup {
                input: current,
                cursor: cur_len,
            },
            false,
        );
    }
    if row.package == "aur-sleuth-setup" {
        let cmd = r##"(set -e
            if ! command -v aur-sleuth >/dev/null 2>&1; then
            echo "aur-sleuth not found."
            echo
            echo "Install aur-sleuth:"
            echo "  1) system (/usr/local) requires sudo"
            echo "  2) user (~/.local)"
            echo "  3) cancel"
            read -rp "Choose [1/2/3]: " choice
            case "$choice" in
            1)
            tmp="$(mktemp -d)"; cd "$tmp"
            git clone https://github.com/mgalgs/aur-sleuth.git
            cd aur-sleuth
            sudo make install
            ;;
            2)
            tmp="$(mktemp -d)"; cd "$tmp"
            git clone https://github.com/mgalgs/aur-sleuth.git
            cd aur-sleuth
            make install PREFIX="$HOME/.local"
            ;;
            *)
            echo "Cancelled."; echo "Press any key to close..."; read -rn1 -s _; exit 0;;
            esac
            else
            echo "aur-sleuth already installed; continuing to setup"
            fi
            conf="${XDG_CONFIG_HOME:-$HOME/.config}/aur-sleuth.conf"
            mkdir -p "$(dirname "$conf")"
            echo "# aur-sleuth configuration" > "$conf"
            echo "[default]" >> "$conf"
            read -rp "OPENAI_BASE_URL (e.g. https://openrouter.ai/api/v1 or http://localhost:11434/v1): " base
            read -rp "OPENAI_MODEL (e.g. qwen/qwen3-30b-a3b-instruct-2507 or llama3.1:8b): " model
            read -rp "OPENAI_API_KEY: " key
            read -rp "MAX_LLM_JOBS (default 3): " jobs
            read -rp "AUDIT_FAILURE_FATAL (true/false) [true]: " fatal
            jobs=${jobs:-3}
            fatal=${fatal:-true}
            [ -n "$base" ] && echo "OPENAI_BASE_URL = $base" >> "$conf"
            [ -n "$model" ] && echo "OPENAI_MODEL = $model" >> "$conf"
            echo "OPENAI_API_KEY = $key" >> "$conf"
            echo "MAX_LLM_JOBS = $jobs" >> "$conf"
            echo "AUDIT_FAILURE_FATAL = $fatal" >> "$conf"
            echo; echo "Wrote $conf"
            echo "Tip: You can run 'aur-sleuth package-name' or audit a local pkgdir with '--pkgdir .'"
            echo; echo "Press any key to close..."; read -rn1 -s _)"##
            .to_string();
        let to_run = if app.dry_run {
            // Properly quote the command to avoid syntax errors with complex shell constructs
            use crate::install::shell_single_quote;
            let quoted = shell_single_quote(&cmd);
            vec![format!("echo DRY RUN: {quoted}")]
        } else {
            vec![cmd]
        };
        crate::install::spawn_shell_commands_in_terminal(&to_run);
        return (crate::state::Modal::None, true);
    }

    // Handle reinstall for already installed dependencies
    if row.installed {
        let pkg = row.package.clone();

        // Determine if official or AUR to create proper PackageItem
        let package_item = crate::index::find_package_by_name(&pkg).unwrap_or_else(|| {
            // Assume AUR if not found in official index
            PackageItem {
                name: pkg.clone(),
                version: String::new(),
                description: String::new(),
                source: Source::Aur,
                popularity: None,
                out_of_date: None,
                orphaned: false,
            }
        });

        // Show reinstall confirmation modal
        // For optional deps, it's a single package, so items and all_items are the same
        return (
            crate::state::Modal::ConfirmReinstall {
                items: vec![package_item.clone()],
                all_items: vec![package_item],
                header_chips: crate::state::modal::PreflightHeaderChips::default(),
            },
            false,
        );
    }

    // Install optional dependencies using executor pattern
    if !row.installed && row.selectable {
        let pkg = row.package.clone();

        // Special packages that need custom installation commands (can't use AUR helpers)
        // paru and yay can't install themselves via AUR helpers (chicken-and-egg problem)
        if pkg == "paru" || pkg == "yay" {
            let cmd = if pkg == "paru" {
                // Use temporary directory to avoid conflicts with existing directories
                "tmp=$(mktemp -d) && cd \"$tmp\" && git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si"
                    .to_string()
            } else {
                // yay
                // Use temporary directory to avoid conflicts with existing directories
                "tmp=$(mktemp -d) && cd \"$tmp\" && git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si"
                    .to_string()
            };

            // Create a dummy PackageItem for display in PreflightExec modal
            let item = PackageItem {
                name: pkg,
                version: String::new(),
                description: String::new(),
                source: Source::Aur,
                popularity: None,
                out_of_date: None,
                orphaned: false,
            };

            // These commands need sudo (makepkg -si)
            // Check faillock status before proceeding
            let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
            if let Some(lockout_msg) =
                crate::logic::faillock::get_lockout_message_if_locked(&username, app)
            {
                // User is locked out - show warning
                app.modal = crate::state::Modal::Alert {
                    message: lockout_msg,
                };
                return (crate::state::Modal::None, false);
            }

            let header_chips = crate::state::modal::PreflightHeaderChips {
                package_count: 1,
                download_bytes: 0,
                install_delta_bytes: 0,
                aur_count: 1,
                risk_score: 0,
                risk_level: crate::state::modal::RiskLevel::Low,
            };

            let settings = crate::theme::settings();
            let proceed_no_password =
                |app: &mut AppState,
                 item: PackageItem,
                 cmd: String,
                 header_chips: crate::state::modal::PreflightHeaderChips| {
                    app.pending_custom_command = Some(cmd);
                    app.modal = crate::state::Modal::PreflightExec {
                        items: vec![item],
                        action: crate::state::PreflightAction::Install,
                        tab: crate::state::PreflightTab::Summary,
                        verbose: false,
                        log_lines: Vec::new(),
                        abortable: false,
                        header_chips,
                        success: None,
                    };
                    app.pending_executor_request =
                        Some(crate::install::ExecutorRequest::CustomCommand {
                            command: app.pending_custom_command.take().unwrap_or_default(),
                            password: None,
                            dry_run: app.dry_run,
                        });
                };

            if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
                match crate::events::try_interactive_auth_handoff() {
                    Ok(true) => proceed_no_password(app, item, cmd, header_chips),
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
                proceed_no_password(app, item, cmd, header_chips);
            } else {
                app.modal = crate::state::Modal::PasswordPrompt {
                    purpose: crate::state::modal::PasswordPurpose::Install,
                    items: vec![item],
                    input: String::new(),
                    cursor: 0,
                    error: None,
                };
                app.pending_custom_command = Some(cmd);
                app.pending_exec_header_chips = Some(header_chips);
            }

            return (app.modal.clone(), false);
        }

        // Regular packages: determine if official or AUR
        let (package_item, is_aur) = crate::index::find_package_by_name(&pkg).map_or_else(
            || {
                // Assume AUR if not found in official index
                (
                    PackageItem {
                        name: pkg.clone(),
                        version: String::new(),
                        description: String::new(),
                        source: Source::Aur,
                        popularity: None,
                        out_of_date: None,
                        orphaned: false,
                    },
                    true,
                )
            },
            |official_item| (official_item, false),
        );

        // For rate-mirrors and semgrep-bin, use AUR helper if available
        let use_aur_helper = is_aur || pkg == "rate-mirrors" || pkg == "semgrep-bin";

        // Transition to PreflightExec modal
        app.modal = crate::state::Modal::PreflightExec {
            items: vec![package_item.clone()],
            action: crate::state::PreflightAction::Install,
            tab: crate::state::PreflightTab::Summary,
            verbose: false,
            log_lines: Vec::new(),
            success: None,
            abortable: false,
            header_chips: crate::state::modal::PreflightHeaderChips {
                package_count: 1,
                download_bytes: 0,
                install_delta_bytes: 0,
                aur_count: usize::from(use_aur_helper),
                risk_score: 0,
                risk_level: crate::state::modal::RiskLevel::Low,
            },
        };

        // Store executor request for processing in tick handler
        app.pending_executor_request = Some(ExecutorRequest::Install {
            items: vec![package_item],
            password: None, // Password will be prompted if needed for official packages
            dry_run: app.dry_run,
        });

        return (app.modal.clone(), true);
    }

    (crate::state::Modal::None, false)
}

/// What: Handle key events for `SshAurSetup` modal.
///
/// Inputs:
/// - `ke`: Key event from terminal.
/// - `app`: Mutable application state.
/// - `step`: Current setup step.
/// - `status_lines`: Mutable status lines shown in modal.
/// - `existing_host_block`: Mutable optional conflicting host block text.
///
/// Output:
/// - `Some(true)` when event is handled and should stop propagation.
/// - `Some(false)` when handled without stop.
/// - `None` when event is not handled.
pub(super) fn handle_ssh_setup_modal(
    ke: KeyEvent,
    app: &mut AppState,
    step: &mut crate::state::SshSetupStep,
    status_lines: &mut Vec<String>,
    existing_host_block: &mut Option<String>,
) -> Option<bool> {
    match (*step, ke.code) {
        (_, KeyCode::Char('o' | 'O')) => {
            crate::util::open_url(crate::logic::ssh_setup::AUR_ACCOUNT_URL);
            Some(false)
        }
        (crate::state::SshSetupStep::Intro, KeyCode::Esc | KeyCode::Char('q'))
        | (
            crate::state::SshSetupStep::Result,
            KeyCode::Esc | KeyCode::Char('q' | '\n' | '\r') | KeyCode::Enter,
        ) => {
            app.modal = crate::state::Modal::None;
            if !app.pending_startup_setup_steps.is_empty() {
                super::common::show_next_startup_setup_step(app);
            }
            Some(false)
        }
        (crate::state::SshSetupStep::Intro, KeyCode::Enter | KeyCode::Char('\n' | '\r')) => {
            let ssh_command = crate::theme::settings().aur_vote_ssh_command;
            match crate::logic::ssh_setup::run_aur_ssh_setup(false, &ssh_command) {
                crate::logic::ssh_setup::AurSshSetupResult::Completed(report) => {
                    *step = crate::state::SshSetupStep::Result;
                    *status_lines = report.lines;
                    *existing_host_block = None;
                }
                crate::logic::ssh_setup::AurSshSetupResult::NeedsOverwrite {
                    existing_block,
                    lines,
                } => {
                    *step = crate::state::SshSetupStep::ConfirmOverwrite;
                    *status_lines = lines;
                    *existing_host_block = Some(existing_block);
                }
            }
            Some(true)
        }
        (
            crate::state::SshSetupStep::ConfirmOverwrite,
            KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc,
        ) => {
            app.modal = crate::state::Modal::None;
            app.toast_message = Some("SSH setup cancelled (existing config kept).".to_string());
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
            if !app.pending_startup_setup_steps.is_empty() {
                super::common::show_next_startup_setup_step(app);
            }
            Some(false)
        }
        (
            crate::state::SshSetupStep::ConfirmOverwrite,
            KeyCode::Char('y' | 'Y' | '\n' | '\r') | KeyCode::Enter,
        ) => {
            let ssh_command = crate::theme::settings().aur_vote_ssh_command;
            let report = match crate::logic::ssh_setup::run_aur_ssh_setup(true, &ssh_command) {
                crate::logic::ssh_setup::AurSshSetupResult::Completed(report) => report,
                crate::logic::ssh_setup::AurSshSetupResult::NeedsOverwrite { lines, .. } => {
                    crate::logic::ssh_setup::AurSshSetupReport {
                        success: false,
                        lines,
                    }
                }
            };
            *step = crate::state::SshSetupStep::Result;
            *status_lines = report.lines;
            *existing_host_block = None;
            Some(true)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
