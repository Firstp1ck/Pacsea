use crossterm::event::{KeyCode, KeyEvent};

use crate::state::modal::{
    DOAS_PERSIST_SELECT_ROWS, DoasPersistChoice, DoasPersistSetupModalState, DoasPersistSetupPhase,
};

/// What: Handle key input for [`crate::state::Modal::DoasPersistSetup`].
///
/// Inputs:
/// - `ke`: Key event to apply.
/// - `app`: App state (for dry-run-aware terminal spawn and i18n strings).
/// - `setup`: Mutable wizard state.
///
/// Output:
/// - `true` when wizard should close, otherwise `false`.
///
/// Details:
/// - Enter on a selection opens instructions (or closes on Skip).
/// - Enter in instructions launches validation guidance in terminal (`dry_run` aware).
/// - Esc/q closes from select phase or returns from instructions.
pub(super) fn handle_doas_persist_setup_key(
    ke: KeyEvent,
    app: &crate::state::AppState,
    setup: &mut DoasPersistSetupModalState,
) -> bool {
    match &mut setup.phase {
        DoasPersistSetupPhase::Select => match ke.code {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Up | KeyCode::Char('k') if setup.select_cursor > 0 => {
                setup.select_cursor -= 1;
            }
            KeyCode::Down | KeyCode::Char('j')
                if setup.select_cursor + 1 < DOAS_PERSIST_SELECT_ROWS =>
            {
                setup.select_cursor += 1;
            }
            KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
                let choice = match setup.select_cursor {
                    0 => DoasPersistChoice::WheelScoped,
                    1 => DoasPersistChoice::UserScoped,
                    _ => DoasPersistChoice::Skip,
                };
                if choice == DoasPersistChoice::Skip {
                    return true;
                }
                setup.phase = DoasPersistSetupPhase::Instructions { choice, scroll: 0 };
            }
            _ => {}
        },
        DoasPersistSetupPhase::Instructions { choice, scroll } => {
            let lines =
                crate::logic::doas_persist_setup::doas_persist_instruction_lines(app, *choice);
            let max_scroll = lines.len().saturating_sub(
                crate::logic::doas_persist_setup::DOAS_PERSIST_INSTRUCTION_VIEWPORT_LINES,
            );
            let max_scroll_u16 = u16::try_from(max_scroll).unwrap_or(u16::MAX);
            match ke.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    setup.phase = DoasPersistSetupPhase::Select;
                }
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = (*scroll + 1).min(max_scroll_u16),
                KeyCode::Enter | KeyCode::Char('\n' | '\r' | 't' | 'T') => {
                    let cmds = if app.dry_run {
                        vec![format!(
                            "echo {}",
                            crate::i18n::t(
                                app,
                                "app.modals.doas_persist_setup.dry_run_terminal_skipped"
                            )
                        )]
                    } else {
                        crate::logic::doas_persist_setup::validation_commands()
                            .into_iter()
                            .map(ToString::to_string)
                            .collect()
                    };
                    crate::install::spawn_shell_commands_in_terminal(&cmds);
                }
                _ => {}
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::handle_doas_persist_setup_key;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn enter_select_opens_instructions() {
        let app = crate::state::AppState::default();
        let mut setup = crate::state::modal::DoasPersistSetupModalState {
            phase: crate::state::modal::DoasPersistSetupPhase::Select,
            select_cursor: 0,
        };
        let closed = handle_doas_persist_setup_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &app,
            &mut setup,
        );
        assert!(!closed);
        assert!(matches!(
            setup.phase,
            crate::state::modal::DoasPersistSetupPhase::Instructions { .. }
        ));
    }

    #[test]
    fn esc_select_closes_modal() {
        let app = crate::state::AppState::default();
        let mut setup = crate::state::modal::DoasPersistSetupModalState {
            phase: crate::state::modal::DoasPersistSetupPhase::Select,
            select_cursor: 0,
        };
        let closed = handle_doas_persist_setup_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &app,
            &mut setup,
        );
        assert!(closed);
    }

    #[test]
    fn instruction_scroll_clamps_to_max() {
        let app = crate::state::AppState::default();
        let mut setup = crate::state::modal::DoasPersistSetupModalState {
            phase: crate::state::modal::DoasPersistSetupPhase::Instructions {
                choice: crate::state::modal::DoasPersistChoice::WheelScoped,
                scroll: 0,
            },
            select_cursor: 0,
        };
        let max_scroll = u16::try_from(
            crate::logic::doas_persist_setup::doas_persist_instruction_lines(
                &app,
                crate::state::modal::DoasPersistChoice::WheelScoped,
            )
            .len()
            .saturating_sub(
                crate::logic::doas_persist_setup::DOAS_PERSIST_INSTRUCTION_VIEWPORT_LINES,
            ),
        )
        .unwrap_or(u16::MAX);
        for _ in 0..64 {
            let _ = handle_doas_persist_setup_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
                &app,
                &mut setup,
            );
        }
        match setup.phase {
            crate::state::modal::DoasPersistSetupPhase::Instructions { scroll, .. } => {
                assert_eq!(scroll, max_scroll);
            }
            crate::state::modal::DoasPersistSetupPhase::Select => {
                panic!("expected instructions phase")
            }
        }
    }
}
