//! Theme system for Pacsea.
//!
//! Split from a monolithic file into submodules for maintainability. Public
//! re-exports keep the `crate::theme::*` API stable.

mod config;
mod parsing;
mod paths;
mod settings;
mod store;
mod types;

pub use config::save_sort_mode;
pub use paths::{cache_dir, state_dir};
pub use settings::settings;
pub use store::{reload_theme, theme};
pub use types::{KeyChord, KeyMap, Settings, Theme};
