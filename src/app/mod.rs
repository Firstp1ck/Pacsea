//! Pacsea application module (split from a single large file into submodules).
//!
//! This module organizes the TUI runtime into smaller files to improve
//! maintainability and keep individual files under 500 lines.

/// Shared helpers for the signature-validated install-list caches.
mod cache_common;
/// Dependency cache for storing resolved dependency information.
mod deps_cache;
/// File cache for storing package file information.
mod files_cache;
/// Persistence layer for saving and loading application state.
mod persist;
/// Recent queries and history management.
mod recent;
/// Runtime event loop and background workers.
pub(crate) mod runtime;
pub mod sandbox_cache;
pub mod services_cache;
/// Terminal setup and restoration utilities.
pub mod terminal;

// Re-export the public entrypoint so callers keep using `app::run(...)`.
pub use runtime::run;
/// Public Pi Scan worker contracts used by deterministic integration tests.
pub use runtime::workers::pi_scan::{
    PiScanRequestMessage, PiScanShutdownMessage, spawn_default_off_pi_scan_worker,
};
/// Public setup-wizard controller contracts used by integration and acceptance tests.
pub use runtime::workers::pi_scan_setup::{
    PiScanSetupControllerOptions, PiScanSetupEvent, PiScanSetupRequest, PiScanSetupStage,
    spawn_pi_scan_setup_controller,
};

// Re-export functions needed by event handlers
pub use runtime::init::{apply_settings_to_app_state, initialize_locale_system};
