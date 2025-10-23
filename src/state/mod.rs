//! Modularized state module.
//!
//! This splits the original monolithic `state.rs` into smaller files while
//! preserving the public API under `crate::state::*` via re-exports.

pub mod app_state;
pub mod modal;
pub mod types;

// Public re-exports to keep existing paths working
pub use app_state::AppState;
pub use modal::{Modal, PreflightAction, PreflightTab};
pub use types::{
    ArchStatusColor, Focus, NewsItem, PackageDetails, PackageItem, QueryInput, RightPaneFocus,
    SearchResults, SortMode, Source,
};
