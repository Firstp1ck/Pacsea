//! Helper functions for preflight integration tests.

use pacsea as crate_root;
use std::collections::HashMap;

/// What: Merge dependencies with the same name into a single entry.
///
/// Inputs:
/// - `deps`: Vector of dependencies that may contain duplicates
///
/// Output:
/// - Vector of merged dependencies (one per unique name)
///
/// Details:
/// - Combines `required_by` lists from duplicate dependencies
/// - Keeps the worst status (conflicts take precedence)
/// - Keeps the more restrictive version
fn merge_dependencies(
    deps: Vec<crate_root::state::modal::DependencyInfo>,
) -> Vec<crate_root::state::modal::DependencyInfo> {
    let mut merged: HashMap<String, crate_root::state::modal::DependencyInfo> = HashMap::new();

    for dep in deps {
        let dep_name = dep.name.clone();
        let entry = merged.entry(dep_name.clone()).or_insert_with(|| {
            crate_root::state::modal::DependencyInfo {
                name: dep_name.clone(),
                version: dep.version.clone(),
                status: dep.status.clone(),
                source: dep.source.clone(),
                required_by: dep.required_by.clone(),
                depends_on: Vec::new(),
                is_core: dep.is_core,
                is_system: dep.is_system,
            }
        });

        // Merge required_by lists (combine unique values)
        for req_by in dep.required_by {
            if !entry.required_by.contains(&req_by) {
                entry.required_by.push(req_by);
            }
        }

        // Merge status (keep worst - lower priority number = higher priority)
        // Conflicts take precedence
        if !matches!(
            entry.status,
            crate_root::state::modal::DependencyStatus::Conflict { .. }
        ) {
            let existing_priority = dependency_priority(&entry.status);
            let new_priority = dependency_priority(&dep.status);
            if new_priority < existing_priority {
                entry.status = dep.status.clone();
            }
        }

        // Merge version requirements (keep more restrictive)
        // But never overwrite a Conflict status
        if !dep.version.is_empty()
            && dep.version != entry.version
            && !matches!(
                entry.status,
                crate_root::state::modal::DependencyStatus::Conflict { .. }
            )
            && entry.version.is_empty()
        {
            entry.version = dep.version;
        }
    }

    let mut result: Vec<_> = merged.into_values().collect();
    // Sort dependencies: conflicts first, then missing, then to-install, then installed
    result.sort_by(|a, b| {
        let priority_a = dependency_priority(&a.status);
        let priority_b = dependency_priority(&b.status);
        priority_a
            .cmp(&priority_b)
            .then_with(|| a.name.cmp(&b.name))
    });
    result
}

/// What: Provide a numeric priority used to order dependency statuses.
///
/// Inputs:
/// - `status`: Dependency status variant subject to sorting.
///
/// Output:
/// - Returns a numeric priority where lower numbers represent higher urgency.
///
/// Details:
/// - Aligns the ordering logic with UI expectations (conflicts first, installed last).
const fn dependency_priority(status: &crate_root::state::modal::DependencyStatus) -> u8 {
    match status {
        crate_root::state::modal::DependencyStatus::Conflict { .. } => 0,
        crate_root::state::modal::DependencyStatus::Missing => 1,
        crate_root::state::modal::DependencyStatus::ToInstall => 2,
        crate_root::state::modal::DependencyStatus::ToUpgrade { .. } => 3,
        crate_root::state::modal::DependencyStatus::Installed { .. } => 4,
    }
}

/// What: Create a test package item with specified properties.
///
/// Inputs:
/// - `name`: Package name
/// - `version`: Package version
/// - `source`: Package source (Official or AUR)
///
/// Output:
/// - A `PackageItem` with the specified properties
///
/// Details:
/// - Creates a minimal package item suitable for testing
pub fn create_test_package(
    name: impl Into<String>,
    version: impl Into<String>,
    source: crate_root::state::Source,
) -> crate_root::state::PackageItem {
    crate_root::state::PackageItem {
        name: name.into(),
        version: version.into(),
        description: String::new(),
        source,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

/// What: Create a default preflight modal state for testing.
///
/// Inputs:
/// - `packages`: Vector of packages to include
/// - `action`: Preflight action (Install or Remove)
/// - `initial_tab`: Initial tab to show
///
/// Output:
/// - A `Modal::Preflight` variant with default values
///
/// Details:
/// - Initializes all modal fields with sensible defaults
/// - Sets up empty collections and default flags
pub fn create_preflight_modal(
    packages: Vec<crate_root::state::PackageItem>,
    action: crate_root::state::PreflightAction,
    initial_tab: crate_root::state::PreflightTab,
) -> crate_root::state::Modal {
    let package_count = packages.len();
    let aur_count = packages
        .iter()
        .filter(|p| matches!(p.source, crate_root::state::Source::Aur))
        .count();

    crate_root::state::Modal::Preflight {
        items: packages,
        action,
        tab: initial_tab,
        summary: None,
        summary_scroll: 0,
        header_chips: crate_root::state::modal::PreflightHeaderChips {
            package_count,
            download_bytes: 0,
            install_delta_bytes: 0,
            aur_count,
            risk_score: 0,
            risk_level: crate_root::state::modal::RiskLevel::Low,
        },
        dependency_info: Vec::new(),
        dep_selected: 0,
        dep_tree_expanded: std::collections::HashSet::new(),
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: std::collections::HashSet::new(),
        files_error: None,
        service_info: Vec::new(),
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: crate_root::state::modal::CascadeMode::Basic,
        cached_reverse_deps_report: None,
    }
}

/// What: Switch to a preflight tab and sync data from app cache.
///
/// Inputs:
/// - `app`: Application state with cached data
/// - `tab`: Tab to switch to
///
/// Output:
/// - Updates modal state with synced data from cache
///
/// Details:
/// - Mirrors the sync logic from `src/ui/modals/preflight/helpers/sync.rs`
/// - Only syncs data relevant to the target tab
pub fn switch_preflight_tab(
    app: &mut crate_root::state::AppState,
    tab: crate_root::state::PreflightTab,
) {
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        tab: current_tab,
        dependency_info,
        dep_selected,
        file_info,
        file_selected,
        service_info,
        service_selected,
        services_loaded,
        sandbox_info,
        sandbox_loaded,
        ..
    } = &mut app.modal
    {
        *current_tab = tab;

        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();

        // Sync dependencies
        if matches!(*action, crate_root::state::PreflightAction::Install)
            && (matches!(tab, crate_root::state::PreflightTab::Deps)
                || matches!(tab, crate_root::state::PreflightTab::Summary)
                || dependency_info.is_empty())
        {
            let filtered: Vec<_> = app
                .install_list_deps
                .iter()
                .filter(|dep| {
                    dep.required_by
                        .iter()
                        .any(|req_by| item_names.contains(req_by))
                })
                .cloned()
                .collect();
            if !filtered.is_empty() {
                let was_empty = dependency_info.is_empty();
                // Merge dependencies with the same name
                *dependency_info = merge_dependencies(filtered);
                if was_empty {
                    *dep_selected = 0;
                }
            }
        }

        // Sync files
        if matches!(tab, crate_root::state::PreflightTab::Files) {
            let cached_files: Vec<_> = app
                .install_list_files
                .iter()
                .filter(|file_info| item_names.contains(&file_info.name))
                .cloned()
                .collect();
            if !cached_files.is_empty() {
                *file_info = cached_files;
                *file_selected = 0;
            }
        }

        // Sync services
        if matches!(*action, crate_root::state::PreflightAction::Install)
            && matches!(tab, crate_root::state::PreflightTab::Services)
        {
            let cached_services: Vec<_> = app
                .install_list_services
                .iter()
                .filter(|s| s.providers.iter().any(|p| item_names.contains(p)))
                .cloned()
                .collect();
            if !cached_services.is_empty() {
                *service_info = cached_services;
                *services_loaded = true;
                *service_selected = 0;
            }
        }

        // Sync sandbox
        if matches!(*action, crate_root::state::PreflightAction::Install)
            && matches!(tab, crate_root::state::PreflightTab::Sandbox)
        {
            let cached_sandbox: Vec<_> = app
                .install_list_sandbox
                .iter()
                .filter(|s| item_names.contains(&s.package_name))
                .cloned()
                .collect();
            if !cached_sandbox.is_empty() {
                *sandbox_info = cached_sandbox;
                *sandbox_loaded = true;
            }
        }
    }
}

/// What: Assert that the modal is a Preflight variant and return its fields.
///
/// Inputs:
/// - `app`: Application state
///
/// Output:
/// - Tuple of all Preflight modal fields for verification
///
/// Details:
/// - Panics if modal is not Preflight variant
/// - Useful for assertions and verification
/// - Returns references to all fields for easy access
#[allow(clippy::type_complexity)]
pub fn assert_preflight_modal(
    app: &crate_root::state::AppState,
) -> (
    &Vec<crate_root::state::PackageItem>,
    &crate_root::state::PreflightAction,
    &crate_root::state::PreflightTab,
    &Vec<crate_root::state::modal::DependencyInfo>,
    &Vec<crate_root::state::modal::PackageFileInfo>,
    &Vec<crate_root::state::modal::ServiceImpact>,
    &Vec<crate_root::logic::sandbox::SandboxInfo>,
    &bool,
    &bool,
) {
    if let crate_root::state::Modal::Preflight {
        items,
        action,
        tab,
        dependency_info,
        file_info,
        service_info,
        sandbox_info,
        services_loaded,
        sandbox_loaded,
        ..
    } = &app.modal
    {
        (
            items,
            action,
            tab,
            dependency_info,
            file_info,
            service_info,
            sandbox_info,
            services_loaded,
            sandbox_loaded,
        )
    } else {
        panic!("Expected Preflight modal");
    }
}
