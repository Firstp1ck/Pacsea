use crate::logic::sandbox::SandboxInfo;
use crate::state::modal::{
    CascadeMode, DependencyInfo, PackageFileInfo, PreflightAction, PreflightHeaderChips,
    PreflightSummaryData, PreflightTab, ServiceImpact,
};
use crate::state::{Modal, PackageItem};
use std::collections::{HashMap, HashSet};

/// What: Holds all extracted fields from `Modal::Preflight` variant.
///
/// Inputs: None (struct definition).
///
/// Output: None (struct definition).
///
/// Details: Used to simplify field extraction and reduce pattern matching complexity.
pub struct PreflightFields<'a> {
    /// Package items for the operation.
    pub items: &'a [PackageItem],
    /// Preflight action (install, remove, downgrade).
    pub action: &'a PreflightAction,
    /// Currently active preflight tab.
    pub tab: &'a PreflightTab,
    /// Optional preflight summary data.
    pub summary: &'a Option<Box<PreflightSummaryData>>,
    /// Header chip metrics.
    pub header_chips: &'a PreflightHeaderChips,
    /// Dependency information.
    pub dependency_info: &'a mut Vec<DependencyInfo>,
    /// Currently selected dependency index.
    pub dep_selected: &'a mut usize,
    /// Set of expanded dependency names in the tree view.
    pub dep_tree_expanded: &'a HashSet<String>,
    /// Optional error message for dependencies tab.
    pub deps_error: &'a Option<String>,
    /// Package file information.
    pub file_info: &'a mut Vec<PackageFileInfo>,
    /// Currently selected file index.
    pub file_selected: &'a mut usize,
    /// Set of expanded file names in the tree view.
    pub file_tree_expanded: &'a HashSet<String>,
    /// Optional error message for files tab.
    pub files_error: &'a Option<String>,
    /// Service impact information.
    pub service_info: &'a mut Vec<ServiceImpact>,
    /// Currently selected service index.
    pub service_selected: &'a mut usize,
    /// Whether services data has been loaded.
    pub services_loaded: &'a mut bool,
    /// Optional error message for services tab.
    pub services_error: &'a Option<String>,
    /// Sandbox information.
    pub sandbox_info: &'a mut Vec<SandboxInfo>,
    /// Currently selected sandbox item index.
    pub sandbox_selected: &'a mut usize,
    /// Set of expanded sandbox names in the tree view.
    pub sandbox_tree_expanded: &'a HashSet<String>,
    /// Whether sandbox data has been loaded.
    pub sandbox_loaded: &'a mut bool,
    /// Optional error message for sandbox tab.
    pub sandbox_error: &'a Option<String>,
    /// Selected optional dependencies by package name.
    pub selected_optdepends: &'a HashMap<String, HashSet<String>>,
    /// Cascade removal mode.
    pub cascade_mode: &'a CascadeMode,
}

/// What: Extract all fields from `Modal::Preflight` variant into a struct.
///
/// Inputs:
/// - `modal`: Mutable reference to Modal enum.
///
/// Output:
/// - Returns `Some(PreflightFields)` if modal is Preflight variant, `None` otherwise.
///
/// Details:
/// - Extracts all 25+ fields from the Preflight variant into a more manageable struct.
/// - Returns None if modal is not the Preflight variant.
pub fn extract_preflight_fields(modal: &mut Modal) -> Option<PreflightFields<'_>> {
    let Modal::Preflight {
        items,
        action,
        tab,
        summary,
        summary_scroll: _,
        header_chips,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        deps_error,
        file_info,
        file_selected,
        file_tree_expanded,
        files_error,
        service_info,
        service_selected,
        services_loaded,
        services_error,
        sandbox_info,
        sandbox_selected,
        sandbox_tree_expanded,
        sandbox_loaded,
        sandbox_error,
        selected_optdepends,
        cascade_mode,
        cached_reverse_deps_report: _,
    } = modal
    else {
        return None;
    };

    Some(PreflightFields {
        items,
        action,
        tab,
        summary,
        header_chips,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        deps_error,
        file_info,
        file_selected,
        file_tree_expanded,
        files_error,
        service_info,
        service_selected,
        services_loaded,
        services_error,
        sandbox_info,
        sandbox_selected,
        sandbox_tree_expanded,
        sandbox_loaded,
        sandbox_error,
        selected_optdepends,
        cascade_mode,
    })
}
