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

pub use config::{
    ensure_settings_keys_present, save_show_install_pane, save_show_keybinds_footer,
    save_show_recent_pane, save_sort_mode,
};
pub use paths::{config_dir, lists_dir, logs_dir};
pub use settings::settings;
pub use store::{reload_theme, theme};
pub use types::{KeyChord, KeyMap, Settings, Theme};
