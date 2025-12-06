//! Cache-related default initialization helpers for `AppState`.

use std::path::PathBuf;

use crate::state::modal::{CascadeMode, PreflightAction, ServiceImpact};
use crate::state::types::PackageItem;

/// Type alias for default services cache state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultServicesCacheState = (
    Vec<ServiceImpact>,
    bool,
    PathBuf,
    bool,
    bool,
    Option<u64>,
    u64,
    Option<(PreflightAction, Vec<String>)>,
    Vec<ServiceImpact>,
);

/// Type alias for default cache refresh state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultCacheRefreshState = (
    Option<std::time::Instant>,
    Option<std::time::Instant>,
    Option<Vec<String>>,
    Option<Vec<String>>,
);

/// Type alias for default preflight state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultPreflightState = (
    Option<(Vec<PackageItem>, PreflightAction)>,
    Option<(Vec<PackageItem>, PreflightAction)>,
    Option<Vec<PackageItem>>,
    Option<Vec<PackageItem>>,
    Option<Vec<PackageItem>>,
    bool,
    bool,
    bool,
    bool,
    bool,
    Option<(usize, bool, bool)>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
);

/// What: Create default cache refresh state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of cache refresh fields: `refresh_installed_until`, `next_installed_refresh_at`, `pending_install_names`, `pending_remove_names`.
///
/// Details:
/// - Cache refresh is inactive by default, no pending installs or removals.
pub(super) const fn default_cache_refresh_state() -> DefaultCacheRefreshState {
    (None, None, None, None)
}

/// What: Create default dependency cache state.
///
/// Inputs:
/// - `deps_cache_path`: Path where the dependency cache is persisted.
///
/// Output:
/// - Tuple of dependency cache fields: `install_list_deps`, `remove_preflight_summary`, `remove_cascade_mode`, `deps_resolving`, `deps_cache_path`, `deps_cache_dirty`.
///
/// Details:
/// - Empty dependency cache, basic cascade mode, resolution not in progress.
#[allow(clippy::missing_const_for_fn)]
pub(super) fn default_deps_cache_state(
    deps_cache_path: PathBuf,
) -> (
    Vec<crate::state::modal::DependencyInfo>,
    Vec<crate::state::modal::ReverseRootSummary>,
    CascadeMode,
    bool,
    PathBuf,
    bool,
) {
    (
        Vec::new(),
        Vec::new(),
        CascadeMode::Basic,
        false,
        deps_cache_path,
        false,
    )
}

/// What: Create default file cache state.
///
/// Inputs:
/// - `files_cache_path`: Path where the file cache is persisted.
///
/// Output:
/// - Tuple of file cache fields: `install_list_files`, `files_resolving`, `files_cache_path`, `files_cache_dirty`.
///
/// Details:
/// - Empty file cache, resolution not in progress.
#[allow(clippy::missing_const_for_fn)]
pub(super) fn default_files_cache_state(
    files_cache_path: PathBuf,
) -> (
    Vec<crate::state::modal::PackageFileInfo>,
    bool,
    PathBuf,
    bool,
) {
    (Vec::new(), false, files_cache_path, false)
}

/// What: Create default service cache state.
///
/// Inputs:
/// - `services_cache_path`: Path where the service cache is persisted.
///
/// Output:
/// - Tuple of service cache fields: `install_list_services`, `services_resolving`, `services_cache_path`, `services_cache_dirty`, `service_resolve_now`, `active_service_request`, `next_service_request_id`, `services_pending_signature`, `pending_service_plan`.
///
/// Details:
/// - Empty service cache, resolution not in progress, next request ID is 1.
#[allow(clippy::missing_const_for_fn)]
pub(super) fn default_services_cache_state(
    services_cache_path: PathBuf,
) -> DefaultServicesCacheState {
    (
        Vec::new(),
        false,
        services_cache_path,
        false,
        false,
        None,
        1,
        None,
        Vec::new(),
    )
}

/// What: Create default sandbox cache state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of sandbox cache fields: `install_list_sandbox`, `sandbox_resolving`, `sandbox_cache_path`, `sandbox_cache_dirty`.
///
/// Details:
/// - Empty sandbox cache, resolution not in progress, path is under lists directory.
pub(super) fn default_sandbox_cache_state()
-> (Vec<crate::logic::sandbox::SandboxInfo>, bool, PathBuf, bool) {
    (
        Vec::new(),
        false,
        crate::theme::lists_dir().join("sandbox_cache.json"),
        false,
    )
}

/// What: Create default preflight modal state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of preflight fields: `preflight_summary_items`, `preflight_deps_items`, `preflight_files_items`, `preflight_services_items`, `preflight_sandbox_items`, `preflight_summary_resolving`, `preflight_deps_resolving`, `preflight_files_resolving`, `preflight_services_resolving`, `preflight_sandbox_resolving`, `last_logged_preflight_deps_state`, `preflight_cancelled`.
///
/// Details:
/// - No preflight items to resolve, all resolution flags false, cancellation flag initialized.
pub(super) fn default_preflight_state() -> DefaultPreflightState {
    (
        None,
        None,
        None,
        None,
        None,
        false,
        false,
        false,
        false,
        false,
        None,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
}
