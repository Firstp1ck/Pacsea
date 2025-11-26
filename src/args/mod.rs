//! Command-line argument parsing and handling.

pub mod cache;
pub mod definition;
pub mod i18n;
pub mod install;
pub mod list;
pub mod news;
pub mod package;
pub mod remove;
pub mod search;
pub mod update;
pub mod utils;

// Re-export commonly used items
pub use definition::{Args, process_args};
pub use utils::determine_log_level;
