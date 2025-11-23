use ratatui::prelude::Rect;
use ratatui::text::Line;

use crate::logic::sandbox::SandboxInfo;
use crate::state::modal::{
    CascadeMode, DependencyInfo, PackageFileInfo, PreflightAction, PreflightHeaderChips,
    PreflightSummaryData, PreflightTab, ServiceImpact,
};
use crate::state::{AppState, PackageItem};
use std::collections::{HashMap, HashSet};

use super::super::tabs::{
    SandboxTabContext, render_deps_tab, render_files_tab, render_sandbox_tab, render_services_tab,
    render_summary_tab,
};

/// What: Render content lines for the currently active tab.
///
/// Inputs:
/// - `app`: Application state for i18n and other context.
/// - `tab`: Current active tab.
/// - `items`: Packages being reviewed.
/// - `action`: Whether install or remove.
/// - `summary`: Summary data option.
/// - `header_chips`: Header chip data.
/// - `dependency_info`: Dependency information.
/// - `dep_selected`: Mutable reference to selected dependency index.
/// - `dep_tree_expanded`: Set of expanded dependency tree nodes.
/// - `deps_error`: Dependency resolution error if any.
/// - `file_info`: File information.
/// - `file_selected`: Mutable reference to selected file index.
/// - `file_tree_expanded`: Set of expanded file tree nodes.
/// - `files_error`: File resolution error if any.
/// - `service_info`: Service impact information.
/// - `service_selected`: Mutable reference to selected service index.
/// - `services_loaded`: Whether services are loaded.
/// - `services_error`: Service resolution error if any.
/// - `sandbox_info`: Sandbox information.
/// - `sandbox_selected`: Mutable reference to selected sandbox index.
/// - `sandbox_tree_expanded`: Set of expanded sandbox tree nodes.
/// - `sandbox_loaded`: Whether sandbox is loaded.
/// - `sandbox_error`: Sandbox resolution error if any.
/// - `selected_optdepends`: Selected optional dependencies map.
/// - `cascade_mode`: Cascade removal mode.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns vector of lines to render for the active tab.
///
/// Details:
/// - Delegates to tab-specific render functions based on the active tab.
/// - Each tab receives only the data it needs for rendering.
#[allow(clippy::too_many_arguments)]
pub fn render_tab_content(
    app: &AppState,
    tab: PreflightTab,
    items: &[PackageItem],
    action: PreflightAction,
    summary: Option<&Box<PreflightSummaryData>>,
    header_chips: &PreflightHeaderChips,
    dependency_info: &[DependencyInfo],
    dep_selected: &mut usize,
    dep_tree_expanded: &HashSet<String>,
    deps_error: Option<&String>,
    file_info: &[PackageFileInfo],
    file_selected: &mut usize,
    file_tree_expanded: &HashSet<String>,
    files_error: Option<&String>,
    service_info: &[ServiceImpact],
    service_selected: &mut usize,
    services_loaded: bool,
    services_error: Option<&String>,
    sandbox_info: &[SandboxInfo],
    sandbox_selected: &mut usize,
    sandbox_tree_expanded: &HashSet<String>,
    sandbox_loaded: bool,
    sandbox_error: Option<&String>,
    selected_optdepends: &HashMap<String, HashSet<String>>,
    cascade_mode: CascadeMode,
    content_rect: Rect,
) -> Vec<Line<'static>> {
    match tab {
        PreflightTab::Summary => render_summary_tab(
            app,
            items,
            action,
            summary,
            header_chips,
            dependency_info,
            cascade_mode,
            content_rect,
        ),
        PreflightTab::Deps => render_deps_tab(
            app,
            items,
            action,
            dependency_info,
            dep_selected,
            dep_tree_expanded,
            deps_error,
            content_rect,
        ),
        PreflightTab::Files => render_files_tab(
            app,
            items,
            file_info,
            file_selected,
            file_tree_expanded,
            files_error,
            content_rect,
        ),
        PreflightTab::Services => render_services_tab(
            app,
            service_info,
            service_selected,
            services_loaded,
            services_error,
            content_rect,
        ),
        PreflightTab::Sandbox => {
            let ctx = SandboxTabContext {
                items,
                sandbox_info,
                sandbox_tree_expanded,
                sandbox_loaded,
                sandbox_error,
                selected_optdepends,
                content_rect,
            };
            render_sandbox_tab(app, &ctx, sandbox_selected)
        }
    }
}
