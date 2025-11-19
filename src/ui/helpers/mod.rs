//! UI helper utilities for formatting and pane-specific behaviors.
//!
//! This module contains small, focused helpers used by the TUI layer:
//!
//! - Formatting package details into rich `ratatui` lines
//! - Human-readable byte formatting
//! - In-pane filtering for Recent and Install panes
//! - Triggering background preview fetches for Recent selections
//! - Resolving a query string to a best-effort first matching package

pub mod filter;
pub mod format;
pub mod preflight;
pub mod query;

pub use filter::{filtered_install_indices, filtered_recent_indices};
pub use format::{format_bytes, format_details_lines, format_signed_bytes, human_bytes};
pub use preflight::is_package_loading_preflight;
pub use query::{fetch_first_match_for_query, trigger_recent_preview};

#[cfg(test)]
mod tests;
