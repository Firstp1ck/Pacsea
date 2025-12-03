//! # Pacsea Crate Overview
//!
//! Pacsea bundles the core event loop, data pipelines, and UI helpers that power the
//! `pacsea` terminal application. Integration tests and downstream tooling can depend on this
//! crate to drive the runtime without going through the binary entrypoint.
//!
//! ## Why Pacsea?
//! > **TUI-first workflow:** Navigate Arch + AUR results with instant filtering, modal install
//! > previews, and keyboard-first ergonomics.
//! >
//! > **Complete ecosystem coverage:** Async workers query official repos, the AUR, mirrors, and
//! > Arch news so you can browse and act from one dashboard.
//! >
//! > **Aggressive caching & telemetry:** Persistent caches (`app::persist`) and ranked searches
//! > (`util::match_rank`) keep navigation snappy while structured tracing calls expose bottlenecks.
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
//! use pacsea::app;
//! use tracing_subscriber::EnvFilter;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("pacsea=info"));
//!     tracing_subscriber::fmt()
//!         .with_env_filter(filter)
//!         .with_target(false)
//!         .init();
//!
//!     // Drive the full TUI runtime (set `true` for dry-run install previews)
//!     app::run(false).await?;
//!     Ok(())
//! }
//! ```
//!
//! See `src/main.rs` for the full CLI wiring (argument parsing, log file setup, and mode flags).
//!
//! ## Subsystem Map
//! | Module | Jump points | Responsibilities |
//! | --- | --- | --- |
//! | [`app`] | `app::run`, `app::sandbox_cache`, `app::services_cache` | Terminal runtime orchestration, cache persistence, sandbox + service metadata. |
//! | [`events`] | `events::search`, `events::recent` | Keyboard/mouse dispatchers that mutate `state::AppState`.
//! | [`logic`] | `logic::send_query`, `logic::deps::resolve_dependencies` | Core business rules for querying indices, ranking, and dependency analysis. |
//! | [`install`] | `install::command`, `install::spawn_install`, `install::spawn_remove_all` | Batch + single install orchestration, scan integrations, terminal helpers. |
//! | [`index`] | `index::load_from_disk`, `index::all_official`, `index::save_to_disk` | Persistent Arch index management and enrichment queues. |
//! | [`state`] | `state::AppState`, `state::types::PackageItem` | Shared UI/runtime data model and domain structs. |
//! | [`theme`] & [`ui`] | `theme::settings`, `ui::middle`, `ui::details` | Theme resolution, keymaps, and ratatui component tree. |
//! | [`util`] | `util::match_rank`, `util::repo_order`, `util::ts_to_date` | Pure helpers for scoring, formatting, and sorting.
//!
//! ## Testing Hooks
//! - `pacsea::global_test_mutex()` / `pacsea::global_test_mutex_lock()` serialize tests that mutate
//!   global environment variables or touch shared caches.
//! - `state::test_mutex()` (private) is used inside state tests; prefer the crate-level guard for
//!   integration suites that spawn the runtime.
//!
//! ```rust,ignore
//! #[tokio::test]
//! async fn installs_are_serialized() {
//!     let _guard = pacsea::global_test_mutex_lock();
//!     std::env::set_var("PATH", "/tmp/pacsea-tests/bin");
//!     // run test body that mutates process globals
//! }
//! ```
//!
//! ## Common Tasks
//! **Kick off a search programmatically**
//! ```rust
//! use pacsea::logic::send_query;
//! use pacsea::state::{AppState, QueryInput};
//! use tokio::sync::mpsc;
//!
//! fn trigger_query(term: &str) {
//!     let mut app = AppState {
//!         input: term.to_string(),
//!         ..Default::default()
//!     };
//!     let (tx, _rx) = mpsc::unbounded_channel::<QueryInput>();
//!     send_query(&mut app, &tx);
//! }
//! ```
//!
//! **Inject a fake official index during tests**
//! ```rust
//! use pacsea::index::{load_from_disk, OfficialIndex, OfficialPkg};
//! use std::collections::HashMap;
//! use std::path::PathBuf;
//!
//! fn seed_index() {
//!     let mut tmp = PathBuf::from(std::env::temp_dir());
//!     tmp.push("pacsea_index_fixture.json");
//!     let snapshot = OfficialIndex {
//!         pkgs: vec![OfficialPkg {
//!             name: "pacsea-demo".into(),
//!             repo: "extra".into(),
//!             arch: "x86_64".into(),
//!             version: "1.0".into(),
//!             description: "fixture".into(),
//!         }],
//!         name_to_idx: HashMap::new(), // Skipped during serialization
//!     };
//!     std::fs::write(&tmp, serde_json::to_string(&snapshot).unwrap()).unwrap();
//!     load_from_disk(&tmp);
//!     let _ = std::fs::remove_file(tmp);
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

#[cfg(test)]
mod test_utils;

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
/// - All tests that modify PATH, `WAYLAND_DISPLAY`, or other global environment variables should use this mutex.
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
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
