//! Pacsea application module (split from a single large file into submodules).
//!
//! This module organizes the TUI runtime into smaller files to improve
//! maintainability and keep individual files under 500 lines.

mod deps_cache;
mod news;
mod persist;
mod recent;
mod runtime;
mod terminal;

// Re-export the public entrypoint so callers keep using `app::run(...)`.
pub use runtime::run;
