//! Key handling for the optional `sudo` `timestamp_timeout` setup wizard.

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::AppState;
use crate::state::modal::{
    SUDO_TIMESTAMP_SELECT_ROWS, SudoTimestampChoice, SudoTimestampSetupModalState,
    SudoTimestampSetupPhase,
};

/// What: Handle key input for [`crate::state::Modal::SudoTimestampSetup`].
///
/// Inputs:
/// - `ke`: Terminal key event.
/// - `app`: Application state (toasts, dry-run, privilege terminal spawn).
/// - `setup`: Wizard state to mutate in place.
///
/// Output:
/// - `true` when the wizard finished and the UI should clear this modal (caller sets `app.modal`).
///
/// Details:
/// - Does not assign `app.modal`; the caller reconstructs `Modal::SudoTimestampSetup` or `None`.
/// - On finish, the caller runs the startup-setup queue when applicable.
#[must_use]
pub(super) fn handle_sudo_timestamp_setup_key(
    ke: KeyEvent,
    app: &mut AppState,
    setup: &mut SudoTimestampSetupModalState,
) -> bool {
    match &mut setup.phase {
        SudoTimestampSetupPhase::Select => match ke.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                return true;
            }
            KeyCode::Up | KeyCode::Char('k') if setup.select_cursor > 0 => {
                setup.select_cursor -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if setup.select_cursor + 1 < SUDO_TIMESTAMP_SELECT_ROWS =>
            {
                setup.select_cursor += 1;
            }
            KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
                if setup.select_cursor == SUDO_TIMESTAMP_SELECT_ROWS - 1 {
                    return true;
                }
                let choice = match setup.select_cursor {
                    0 => SudoTimestampChoice::TenMinutes,
                    1 => SudoTimestampChoice::ThirtyMinutes,
                    2 => SudoTimestampChoice::Infinity,
                    _ => {
                        return true;
                    }
                };
                setup.phase = SudoTimestampSetupPhase::Instructions { choice, scroll: 0 };
            }
            _ => {}
        },
        SudoTimestampSetupPhase::Instructions { choice, scroll } => {
            let lines =
                crate::logic::sudo_timestamp_setup::sudo_timestamp_instruction_lines(app, *choice);
            let max_scroll = lines.len().saturating_sub(
                crate::logic::sudo_timestamp_setup::SUDO_TIMESTAMP_INSTRUCTION_VIEWPORT_LINES,
            );
            let max_scroll_u16 = u16::try_from(max_scroll).unwrap_or(u16::MAX);
            match ke.code {
                KeyCode::Esc => {
                    setup.phase = SudoTimestampSetupPhase::Select;
                }
                KeyCode::Enter | KeyCode::Char('\n' | '\r' | 'q') => {
                    return true;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    *scroll = scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *scroll = (*scroll + 1).min(max_scroll_u16);
                }
                KeyCode::Char('t' | 'T') => {
                    let script =
                        crate::logic::sudo_timestamp_setup::apply_drop_in_shell_script(*choice);
                    if app.dry_run {
                        app.toast_message = Some(crate::i18n::t(
                            app,
                            "app.modals.sudo_timestamp_setup.dry_run_terminal_skipped",
                        ));
                        app.toast_expires_at =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    } else {
                        crate::install::spawn_shell_commands_in_terminal(&[script]);
                    }
                }
                _ => {}
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    #[test]
    fn select_enter_skip_closes() {
        let mut app = AppState::default();
        let mut setup = SudoTimestampSetupModalState {
            phase: SudoTimestampSetupPhase::Select,
            select_cursor: SUDO_TIMESTAMP_SELECT_ROWS - 1,
        };
        let closed = handle_sudo_timestamp_setup_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            &mut setup,
        );
        assert!(closed);
    }

    #[test]
    fn select_enter_choice_opens_instructions() {
        let mut app = AppState::default();
        let mut setup = SudoTimestampSetupModalState {
            phase: SudoTimestampSetupPhase::Select,
            select_cursor: 1,
        };
        let closed = handle_sudo_timestamp_setup_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            &mut setup,
        );
        assert!(!closed);
        assert!(matches!(
            setup.phase,
            SudoTimestampSetupPhase::Instructions {
                choice: SudoTimestampChoice::ThirtyMinutes,
                ..
            }
        ));
    }

    #[test]
    fn instructions_esc_returns_to_select() {
        let mut app = AppState::default();
        let mut setup = SudoTimestampSetupModalState {
            phase: SudoTimestampSetupPhase::Instructions {
                choice: SudoTimestampChoice::TenMinutes,
                scroll: 3,
            },
            select_cursor: 2,
        };
        let closed = handle_sudo_timestamp_setup_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &mut app,
            &mut setup,
        );
        assert!(!closed);
        assert!(matches!(setup.phase, SudoTimestampSetupPhase::Select));
        assert_eq!(setup.select_cursor, 2);
    }
}
