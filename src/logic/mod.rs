//! Core non-UI logic split into modular submodules.

pub mod filter;
pub mod gating;
pub mod lists;
pub mod prefetch;
pub mod query;
pub mod selection;
pub mod sort;
pub mod summary;

// Re-export public APIs to preserve existing import paths (crate::logic::...)
pub use filter::apply_filters_and_sort_preserve_selection;
pub use gating::{is_allowed, set_allowed_only_selected, set_allowed_ring};
pub use lists::{add_to_downgrade_list, add_to_install_list, add_to_remove_list};
pub use prefetch::ring_prefetch_from_selected;
pub use query::send_query;
pub use selection::move_sel_cached;
pub use sort::sort_results_preserve_selection;
pub use summary::compute_post_summary;
