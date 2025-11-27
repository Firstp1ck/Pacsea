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
    pub items: &'a [PackageItem],
    pub action: &'a PreflightAction,
    pub tab: &'a PreflightTab,
    pub summary: &'a Option<Box<PreflightSummaryData>>,
    pub summary_scroll: u16,
    pub header_chips: &'a PreflightHeaderChips,
    pub dependency_info: &'a mut Vec<DependencyInfo>,
    pub dep_selected: &'a mut usize,
    pub dep_tree_expanded: &'a HashSet<String>,
    pub deps_error: &'a Option<String>,
    pub file_info: &'a mut Vec<PackageFileInfo>,
    pub file_selected: &'a mut usize,
    pub file_tree_expanded: &'a HashSet<String>,
    pub files_error: &'a Option<String>,
    pub service_info: &'a mut Vec<ServiceImpact>,
    pub service_selected: &'a mut usize,
    pub services_loaded: &'a mut bool,
    pub services_error: &'a Option<String>,
    pub sandbox_info: &'a mut Vec<SandboxInfo>,
    pub sandbox_selected: &'a mut usize,
    pub sandbox_tree_expanded: &'a HashSet<String>,
    pub sandbox_loaded: &'a mut bool,
    pub sandbox_error: &'a Option<String>,
    pub selected_optdepends: &'a HashMap<String, HashSet<String>>,
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
        summary_scroll,
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
        summary_scroll: *summary_scroll,
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
