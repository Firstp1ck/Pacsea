//! Modularized state module.
//!
//! This splits the original monolithic `state.rs` into smaller files while
//! preserving the public API under `crate::state::*` via re-exports.

pub mod app_state;
pub mod modal;
pub mod types;

// Public re-exports to keep existing paths working
pub use app_state::AppState;
pub use modal::Modal;
pub use types::{
    ArchStatusColor, Focus, NewsItem, PackageDetails, PackageItem, QueryInput,
    RightPaneFocus, SearchResults, SortMode, Source,
};

#[cfg(test)]
static TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
