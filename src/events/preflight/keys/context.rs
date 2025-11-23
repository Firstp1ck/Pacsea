//! Context structs for Preflight key handling.

use std::collections::{HashMap, HashSet};

use crate::state::PackageItem;

/// What: Context struct grouping parameters for Enter/Space key handling.
///
/// Details:
/// - Reduces function argument count to avoid clippy warnings.
pub struct EnterOrSpaceContext<'a> {
    pub(crate) tab: &'a crate::state::PreflightTab,
    pub(crate) items: &'a [PackageItem],
    pub(crate) dependency_info: &'a [crate::state::modal::DependencyInfo],
    pub(crate) dep_selected: usize,
    pub(crate) dep_tree_expanded: &'a mut HashSet<String>,
    pub(crate) file_info: &'a [crate::state::modal::PackageFileInfo],
    pub(crate) file_selected: usize,
    pub(crate) file_tree_expanded: &'a mut HashSet<String>,
    pub(crate) sandbox_info: &'a [crate::logic::sandbox::SandboxInfo],
    pub(crate) sandbox_selected: usize,
    pub(crate) sandbox_tree_expanded: &'a mut HashSet<String>,
    pub(crate) selected_optdepends: &'a mut HashMap<String, HashSet<String>>,
    pub(crate) service_info: &'a mut [crate::state::modal::ServiceImpact],
    pub(crate) service_selected: usize,
}

/// What: Context struct grouping all Preflight modal state for key handling.
///
/// Details:
/// - Reduces function argument count and cognitive complexity.
/// - Contains all mutable references needed by key handlers.
/// - Note: `app` is passed separately to avoid borrow checker issues.
pub struct PreflightKeyContext<'a> {
    pub(crate) tab: &'a mut crate::state::PreflightTab,
    pub(crate) items: &'a [PackageItem],
    pub(crate) action: &'a crate::state::PreflightAction,
    pub(crate) dependency_info: &'a mut Vec<crate::state::modal::DependencyInfo>,
    pub(crate) dep_selected: &'a mut usize,
    pub(crate) dep_tree_expanded: &'a mut HashSet<String>,
    pub(crate) deps_error: &'a mut Option<String>,
    pub(crate) file_info: &'a mut Vec<crate::state::modal::PackageFileInfo>,
    pub(crate) file_selected: &'a mut usize,
    pub(crate) file_tree_expanded: &'a mut HashSet<String>,
    pub(crate) files_error: &'a mut Option<String>,
    pub(crate) service_info: &'a mut Vec<crate::state::modal::ServiceImpact>,
    pub(crate) service_selected: &'a mut usize,
    pub(crate) services_loaded: &'a mut bool,
    pub(crate) services_error: &'a mut Option<String>,
    pub(crate) sandbox_info: &'a mut Vec<crate::logic::sandbox::SandboxInfo>,
    pub(crate) sandbox_selected: &'a mut usize,
    pub(crate) sandbox_tree_expanded: &'a mut HashSet<String>,
    pub(crate) selected_optdepends: &'a mut HashMap<String, HashSet<String>>,
}
