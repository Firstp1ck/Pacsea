//! Preflight modal event handling.

pub(crate) mod display;
pub(crate) mod keys;
mod modal;

#[cfg(test)]
mod tests;

pub(crate) use display::{build_file_display_items, compute_file_display_items_len};
pub(crate) use keys::handle_preflight_key;
