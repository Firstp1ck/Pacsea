//! Pacsea application module (split from a single large file into submodules).
//!
//! This module organizes the TUI runtime into smaller files to improve
//! maintainability and keep individual files under 500 lines.

/// Dependency cache for storing resolved dependency information.
mod deps_cache;
/// File cache for storing package file information.
mod files_cache;
/// Persistence layer for saving and loading application state.
mod persist;
/// Recent queries and history management.
mod recent;
/// Runtime event loop and background workers.
mod runtime;
pub mod sandbox_cache;
pub mod services_cache;
/// Terminal setup and restoration utilities.
mod terminal;

// Re-export the public entrypoint so callers keep using `app::run(...)`.
pub use runtime::run;

// Re-export functions needed by event handlers
pub use runtime::init::{apply_settings_to_app_state, initialize_locale_system};
