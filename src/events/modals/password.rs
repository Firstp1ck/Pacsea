//! Password prompt modal event handling.

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::AppState;

/// What: Handle key events for password prompt modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `input`: Mutable reference to password input buffer
/// - `cursor`: Mutable reference to cursor position
///
/// Output:
/// - `true` if Enter was pressed (password submitted), `false` otherwise
///
/// Details:
/// - Handles text input, navigation, and Enter/Esc keys.
/// - Returns `true` on Enter to indicate password should be submitted.
pub(super) fn handle_password_prompt(
    ke: KeyEvent,
    app: &mut AppState,
    input: &mut String,
    cursor: &mut usize,
) -> bool {
    match ke.code {
        KeyCode::Esc => {
            app.modal = crate::state::Modal::None;
            false
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            // Password submitted - caller will handle transition (numpad Enter sends \n or \r)
            true
        }
        KeyCode::Backspace => {
            if *cursor > 0 && *cursor <= input.len() {
                input.remove(*cursor - 1);
                *cursor -= 1;
            }
            false
        }
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
            false
        }
        KeyCode::Right => {
            if *cursor < input.len() {
                *cursor += 1;
            }
            false
        }
        KeyCode::Home => {
            *cursor = 0;
            false
        }
        KeyCode::End => {
            *cursor = input.len();
            false
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
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use crate::state::{AppState, Modal};

    use super::handle_password_prompt;

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        let mut ke = KeyEvent::new(code, modifiers);
        ke.kind = KeyEventKind::Press;
        ke
    }

    #[test]
    fn key_numpad_enter_carriage_return_returns_true() {
        let mut app = AppState::default();
        let mut input = String::new();
        let mut cursor = 0_usize;
        let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(result, "numpad Enter as \\r should return true");
        assert!(input.is_empty(), "input must be unchanged");
        assert_eq!(cursor, 0, "cursor must be unchanged");
    }

    #[test]
    fn key_numpad_enter_newline_returns_true() {
        let mut app = AppState::default();
        let mut input = String::new();
        let mut cursor = 0_usize;
        let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(result, "numpad Enter as \\n should return true");
        assert!(input.is_empty(), "input must be unchanged");
        assert_eq!(cursor, 0, "cursor must be unchanged");
    }

    #[test]
    fn key_main_enter_returns_true() {
        let mut app = AppState::default();
        let mut input = String::new();
        let mut cursor = 0_usize;
        let ke = key_event(KeyCode::Enter, KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(result, "main Enter should return true");
    }

    #[test]
    fn key_esc_closes_modal_returns_false() {
        let mut app = AppState {
            modal: Modal::PasswordPrompt {
                purpose: crate::state::modal::PasswordPurpose::Install,
                items: vec![],
                input: String::new(),
                cursor: 0,
                error: None,
            },
            ..Default::default()
        };
        let mut input = String::new();
        let mut cursor = 0_usize;
        let ke = key_event(KeyCode::Esc, KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(!result, "Esc should return false");
        assert!(matches!(app.modal, Modal::None), "Esc should close modal");
    }

    #[test]
    fn key_backspace_returns_false_and_edits() {
        let mut app = AppState::default();
        let mut input = "ab".to_string();
        let mut cursor = 1_usize;
        let ke = key_event(KeyCode::Backspace, KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(!result, "Backspace should return false");
        assert_eq!(input, "b", "Backspace should remove char before cursor");
        assert_eq!(cursor, 0, "cursor should decrement");
    }

    #[test]
    fn key_left_right_home_end_return_false() {
        let mut app = AppState::default();
        let mut input = "ab".to_string();
        let mut cursor = 1_usize;

        let ke = key_event(KeyCode::Left, KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(!result);
        assert_eq!(cursor, 0);

        let ke = key_event(KeyCode::Right, KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(!result);
        assert_eq!(cursor, 1);

        let ke = key_event(KeyCode::End, KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(!result);
        assert_eq!(cursor, 2);

        let ke = key_event(KeyCode::Home, KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(!result);
        assert_eq!(cursor, 0);
    }

    #[test]
    fn key_char_inserts_and_returns_false() {
        let mut app = AppState::default();
        let mut input = String::new();
        let mut cursor = 0_usize;
        let ke = key_event(KeyCode::Char('x'), KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(!result, "Char should return false");
        assert_eq!(input, "x", "Char should insert");
        assert_eq!(cursor, 1, "cursor should advance");
    }

    #[test]
    fn submit_with_empty_input() {
        let mut app = AppState::default();
        for code in [KeyCode::Enter, KeyCode::Char('\r'), KeyCode::Char('\n')] {
            let mut input = String::new();
            let mut cursor = 0_usize;
            let ke = key_event(code, KeyModifiers::empty());
            let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
            assert!(
                result,
                "submit with empty input for {code:?} should return true"
            );
        }
    }

    #[test]
    fn submit_with_non_empty_input() {
        let mut app = AppState::default();
        for code in [KeyCode::Enter, KeyCode::Char('\r'), KeyCode::Char('\n')] {
            let mut input = "pass".to_string();
            let mut cursor = 4_usize;
            let ke = key_event(code, KeyModifiers::empty());
            let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
            assert!(
                result,
                "submit with non-empty input for {code:?} should return true"
            );
            assert_eq!(input, "pass", "input must be unchanged");
            assert_eq!(cursor, 4, "cursor must be unchanged");
        }
    }

    #[test]
    fn control_chars_not_inserted_and_no_submit() {
        let mut app = AppState::default();
        let mut input = String::new();
        let mut cursor = 0_usize;
        for code in [KeyCode::Char('\t'), KeyCode::Char('\x00')] {
            let ke = key_event(code, KeyModifiers::empty());
            let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
            assert!(!result, "control char {code:?} should not submit");
            assert!(input.is_empty(), "control char should not be inserted");
        }
    }

    #[test]
    fn other_key_codes_return_false() {
        let mut app = AppState::default();
        let mut input = "x".to_string();
        let mut cursor = 1_usize;
        for code in [KeyCode::Tab, KeyCode::Down, KeyCode::Up] {
            let ke = key_event(code, KeyModifiers::empty());
            let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
            assert!(!result, "{code:?} should return false");
            assert_eq!(input, "x", "input must be unchanged");
            assert_eq!(cursor, 1, "cursor must be unchanged");
        }
    }

    #[test]
    fn cursor_at_start_submit() {
        let mut app = AppState::default();
        let mut input = "pwd".to_string();
        let mut cursor = 0_usize;
        let ke = key_event(KeyCode::Char('\r'), KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(result);
        assert_eq!(input, "pwd");
        assert_eq!(cursor, 0);
    }

    #[test]
    fn cursor_in_middle_submit() {
        let mut app = AppState::default();
        let mut input = "pwd".to_string();
        let mut cursor = 2_usize;
        let ke = key_event(KeyCode::Char('\n'), KeyModifiers::empty());
        let result = handle_password_prompt(ke, &mut app, &mut input, &mut cursor);
        assert!(result);
        assert_eq!(input, "pwd");
        assert_eq!(cursor, 2);
    }
}
