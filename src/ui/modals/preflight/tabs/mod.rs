pub mod deps;
pub mod files;
pub mod sandbox;
pub mod services;
pub mod summary;

pub use deps::render_deps_tab;
pub use files::render_files_tab;
pub use sandbox::render_sandbox_tab;
pub use services::render_services_tab;
pub use summary::render_summary_tab;
