//! Library entry for Pacsea exposing core logic for integration tests.

pub mod index;
pub mod logic;
pub mod sources;
pub mod state;
pub mod theme;
pub mod ui;
pub mod util;

// Backwards-compat shim: keep `crate::ui_helpers::*` working
pub use crate::ui::helpers as ui_helpers;
