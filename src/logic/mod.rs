//! Core non-UI logic split into modular submodules.

pub mod deps;
pub mod distro;
pub mod faillock;
pub mod files;
pub mod filter;
pub mod gating;
pub mod lists;
pub mod password;
pub mod pkgbuild_checks;
pub mod prefetch;
pub mod preflight;
pub mod privilege;
pub mod query;
pub mod repos;
pub mod sandbox;
pub mod selection;
pub mod services;
pub mod sort;
pub mod ssh_setup;
pub mod summary;

// Re-export public APIs to preserve existing import paths (crate::logic::...)
pub use filter::apply_filters_and_sort_preserve_selection;
pub use gating::{is_allowed, set_allowed_only_selected, set_allowed_ring};
pub use lists::{add_to_downgrade_list, add_to_install_list, add_to_remove_list};
pub use pkgbuild_checks::{
    clear_stale_pkgbuild_checks_for_selection, pkgbuild_check_response_matches_selection,
};
pub use prefetch::ring_prefetch_from_selected;
pub use query::send_query;
pub use selection::move_sel_cached;
pub use services::resolve_service_impacts;
pub use sort::{invalidate_sort_caches, sort_results_preserve_selection};
pub use summary::compute_post_summary;
