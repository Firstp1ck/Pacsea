//! # Pacsea Crate Overview
//!
//! Pacsea bundles the core event loop, data pipelines, and UI helpers that power the
//! `pacsea` terminal application. Integration tests and downstream tooling can depend on this
//! crate to drive the runtime without going through the binary entrypoint.
//!
//! ## Highlights
//! - TUI runtime (`app::runtime`) orchestrating async tasks, caches, and rendering.
//! - Modular subsystems for install flows, package index querying, and translation loading.
//! - Reusable helpers for theme paths, serialization, and UI composition.
//!
//! ## Crate Layout
//! - [`app`]: runtime, caches, and persistence glue for the interactive TUI.
//! - [`events`], [`logic`], [`install`]: event handling and command execution pipelines.
//! - [`index`], [`sources`]: Arch/AUR metadata fetchers plus enrichment.
//! - [`state`], [`theme`], [`ui`], [`util`]: configuration, rendering, and misc helpers.
//!
//! ## Quick Start
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     pacsea::app::runtime::run(false).await?;
//!     Ok(())
//! }
//! ```
//!
//! The modules listed below link to detailed documentation for each subsystem.

pub mod app;

pub mod events;
pub mod i18n;
pub mod index;
pub mod install;
pub mod logic;
pub mod sources;
pub mod state;
pub mod theme;
pub mod ui;
pub mod util;

// Backwards-compat shim: keep `crate::ui_helpers::*` working
#[doc(hidden)]
pub use crate::ui::helpers as ui_helpers;

#[cfg(test)]
static GLOBAL_TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
/// What: Provide a global mutex to serialize all tests that mutate PATH or other global environment variables.
///
/// Input: None.
/// Output: `&'static Mutex<()>` guard to synchronize tests touching global environment state.
///
/// Details:
/// - Lazily initializes a global `Mutex` via `OnceLock` for cross-test coordination.
/// - All tests that modify PATH, WAYLAND_DISPLAY, or other global environment variables should use this mutex.
/// - This ensures tests run serially even when --test-threads=1 is used, preventing race conditions.
/// - Handles poisoned mutexes gracefully by recovering from panics in previous tests.
pub fn global_test_mutex() -> &'static std::sync::Mutex<()> {
    GLOBAL_TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}

#[cfg(test)]
/// What: Lock the global test mutex, handling poisoned mutexes gracefully.
///
/// Input: None.
/// Output: `MutexGuard<()>` that will be released when dropped.
///
/// Details:
/// - If the mutex is poisoned (from a previous test panic), recovers by acquiring the lock anyway.
/// - This allows tests to continue running even if a previous test panicked while holding the lock.
pub fn global_test_mutex_lock() -> std::sync::MutexGuard<'static, ()> {
    global_test_mutex()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
