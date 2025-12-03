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
        KeyCode::Enter => {
            // Password submitted - caller will handle transition
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
