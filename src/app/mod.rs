//! Pacsea application module (split from a single large file into submodules).
//!
//! This module organizes the TUI runtime into smaller files to improve
//! maintainability and keep individual files under 500 lines.

mod deps_cache;
mod files_cache;
mod persist;
mod recent;
mod runtime;
pub mod sandbox_cache;
pub mod services_cache;
mod terminal;

// Re-export the public entrypoint so callers keep using `app::run(...)`.
pub use runtime::run;

// Re-export functions needed by event handlers
pub use runtime::init::{apply_settings_to_app_state, initialize_locale_system};
