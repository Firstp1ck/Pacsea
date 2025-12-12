//! Context structs for Preflight key handling.

use std::collections::{HashMap, HashSet};

use crate::state::PackageItem;

/// What: Context struct grouping parameters for Enter/Space key handling.
///
/// Details:
/// - Reduces function argument count to avoid clippy warnings.
pub struct EnterOrSpaceContext<'a> {
    /// Currently active preflight tab.
    pub(crate) tab: &'a crate::state::PreflightTab,
    /// Package items being analyzed.
    pub(crate) items: &'a [PackageItem],
    /// Dependency information for packages.
    pub(crate) dependency_info: &'a [crate::state::modal::DependencyInfo],
    /// Currently selected dependency index.
    pub(crate) dep_selected: usize,
    /// Set of expanded dependency tree nodes.
    pub(crate) dep_tree_expanded: &'a mut HashSet<String>,
    /// File information for packages.
    pub(crate) file_info: &'a [crate::state::modal::PackageFileInfo],
    /// Currently selected file index.
    pub(crate) file_selected: usize,
    /// Set of expanded file tree nodes.
    pub(crate) file_tree_expanded: &'a mut HashSet<String>,
    /// Sandbox analysis information.
    pub(crate) sandbox_info: &'a [crate::logic::sandbox::SandboxInfo],
    /// Currently selected sandbox item index.
    pub(crate) sandbox_selected: usize,
    /// Set of expanded sandbox tree nodes.
    pub(crate) sandbox_tree_expanded: &'a mut HashSet<String>,
    /// Map of selected optional dependencies by package.
    pub(crate) selected_optdepends: &'a mut HashMap<String, HashSet<String>>,
    /// Service impact information.
    pub(crate) service_info: &'a mut [crate::state::modal::ServiceImpact],
    /// Currently selected service index.
    pub(crate) service_selected: usize,
}

/// What: Context struct grouping all Preflight modal state for key handling.
///
/// Details:
/// - Reduces function argument count and cognitive complexity.
/// - Contains all mutable references needed by key handlers.
/// - Note: `app` is passed separately to avoid borrow checker issues.
pub struct PreflightKeyContext<'a> {
    /// Currently active preflight tab.
    pub(crate) tab: &'a mut crate::state::PreflightTab,
    /// Package items being analyzed.
    pub(crate) items: &'a [PackageItem],
    /// Preflight action (install/remove/downgrade).
    pub(crate) action: &'a crate::state::PreflightAction,
    /// Dependency information for packages.
    pub(crate) dependency_info: &'a mut Vec<crate::state::modal::DependencyInfo>,
    /// Currently selected dependency index.
    pub(crate) dep_selected: &'a mut usize,
    /// Set of expanded dependency tree nodes.
    pub(crate) dep_tree_expanded: &'a mut HashSet<String>,
    /// Error message for dependency resolution, if any.
    pub(crate) deps_error: &'a mut Option<String>,
    /// File information for packages.
    pub(crate) file_info: &'a mut Vec<crate::state::modal::PackageFileInfo>,
    /// Currently selected file index.
    pub(crate) file_selected: &'a mut usize,
    /// Set of expanded file tree nodes.
    pub(crate) file_tree_expanded: &'a mut HashSet<String>,
    /// Error message for file analysis, if any.
    pub(crate) files_error: &'a mut Option<String>,
    /// Service impact information.
    pub(crate) service_info: &'a mut Vec<crate::state::modal::ServiceImpact>,
    /// Currently selected service index.
    pub(crate) service_selected: &'a mut usize,
    /// Whether service information has been loaded.
    pub(crate) services_loaded: &'a mut bool,
    /// Error message for service analysis, if any.
    pub(crate) services_error: &'a mut Option<String>,
    /// Sandbox analysis information.
    pub(crate) sandbox_info: &'a mut Vec<crate::logic::sandbox::SandboxInfo>,
    /// Currently selected sandbox item index.
    pub(crate) sandbox_selected: &'a mut usize,
    /// Set of expanded sandbox tree nodes.
    pub(crate) sandbox_tree_expanded: &'a mut HashSet<String>,
    /// Map of selected optional dependencies by package.
    pub(crate) selected_optdepends: &'a mut HashMap<String, HashSet<String>>,
}
