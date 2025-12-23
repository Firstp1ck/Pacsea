//! Tests for modal key event handling, particularly Esc key bug fixes.

mod alert;
mod common;
mod confirm;
mod global_keybinds;
mod help;
mod news;
mod optional_deps;
mod other;
mod post_summary;
mod preflight;
mod scan;
mod system_update;

// Re-export handle_modal_key_test for tests
pub(super) use crate::events::modals::handle_modal_key_test as handle_modal_key;
