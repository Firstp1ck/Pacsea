/// Common utilities for channel handlers.
mod common;

/// Handler for file analysis results.
pub mod files;
/// Handler for install list and dependency results.
pub mod install;
/// Handler for sandbox analysis results.
pub mod sandbox;
/// Handler for search results and details updates.
pub mod search;
/// Handler for service impact analysis results.
pub mod services;

pub use files::handle_file_result;
pub use install::{handle_add_to_install_list, handle_dependency_result};
pub use sandbox::handle_sandbox_result;
pub use search::{handle_details_update, handle_preview, handle_search_results};
pub use services::handle_service_result;
