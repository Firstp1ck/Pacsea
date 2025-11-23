//! Optional dependencies modal handling.

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::AppState;

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
        KeyCode::Enter => {
            if let Some(row) = rows.get(*selected) {
                match handle_optional_deps_enter(app.dry_run, row) {
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
/// - `dry_run`: Whether to run in dry-run mode
/// - `row`: Selected optional dependency row
///
/// Output:
/// - `(new_modal, should_stop_propagation)` tuple
///
/// Details:
/// - Handles setup for virustotal/aur-sleuth or installs optional dependencies
fn handle_optional_deps_enter(
    dry_run: bool,
    row: &crate::state::types::OptionalDepRow,
) -> (crate::state::Modal, bool) {
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
        let to_run = if dry_run {
            vec![format!("echo DRY RUN: {cmd}")]
        } else {
            vec![cmd]
        };
        crate::install::spawn_shell_commands_in_terminal(&to_run);
        return (crate::state::Modal::None, true);
    }
    if !row.installed && row.selectable {
        let pkg = row.package.clone();
        let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
        let cmd = if pkg == "paru" {
            "rm -rf paru && git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si"
                .to_string()
        } else if pkg == "yay" {
            "rm -rf yay && git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si"
                .to_string()
        } else if pkg == "semgrep-bin" {
            "rm -rf semgrep-bin && git clone https://aur.archlinux.org/semgrep-bin.git && cd semgrep-bin && makepkg -si".to_string()
        } else if pkg == "rate-mirrors" {
            format!(
                "{}{}",
                crate::install::command::aur_install_body("-S --needed --noconfirm", &pkg),
                hold_tail
            )
        } else {
            format!("sudo pacman -S --needed --noconfirm {pkg}")
        };
        let to_run = if dry_run {
            vec![format!("echo DRY RUN: {cmd}")]
        } else {
            vec![cmd]
        };
        crate::install::spawn_shell_commands_in_terminal(&to_run);
        return (crate::state::Modal::None, true);
    }
    (crate::state::Modal::None, false)
}
