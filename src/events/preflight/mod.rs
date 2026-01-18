//! Preflight modal event handling.

pub mod display;
pub mod keys;
mod modal;

#[cfg(test)]
mod tests;

pub use display::{build_file_display_items, compute_file_display_items_len};
pub use keys::handle_preflight_key;
pub use keys::start_execution;