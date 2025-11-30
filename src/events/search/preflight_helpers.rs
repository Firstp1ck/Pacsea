use crate::state::{AppState, PackageItem};

/// What: Filter cached dependencies for current items.
///
/// Inputs:
/// - `app`: Application state with cached dependencies.
/// - `item_names`: Set of current package names.
///
/// Output:
/// - Vector of matching cached dependencies.
fn filter_cached_dependencies(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
) -> Vec<crate::state::modal::DependencyInfo> {
    app.install_list_deps
        .iter()
        .filter(|dep| {
            dep.required_by
                .iter()
                .any(|req_by| item_names.contains(req_by))
        })
        .cloned()
        .collect()
}

/// What: Filter cached files for current items.
///
/// Inputs:
/// - `app`: Application state with cached files.
/// - `item_names`: Set of current package names.
///
/// Output:
/// - Vector of matching cached files.
fn filter_cached_files(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
) -> Vec<crate::state::modal::PackageFileInfo> {
    app.install_list_files
        .iter()
        .filter(|file_info| item_names.contains(&file_info.name))
        .cloned()
        .collect()
}

/// What: Trigger background resolution for preflight data.
///
/// Inputs:
/// - `app`: Application state.
/// - `items`: Packages to resolve.
/// - `dependency_info`: Current dependency info (empty triggers resolution).
/// - `cached_files`: Current cached files (empty triggers resolution).
///
/// Output:
/// - Updates app state with resolution flags and items.
fn trigger_background_resolution(
    app: &mut AppState,
    items: &[PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    cached_files: &[crate::state::modal::PackageFileInfo],
) {
    if dependency_info.is_empty() {
        app.preflight_deps_items = Some((
            items.to_vec(),
            crate::state::modal::PreflightAction::Install,
        ));
        app.preflight_deps_resolving = true;
    }
    if cached_files.is_empty() {
        app.preflight_files_items = Some(items.to_vec());
        app.preflight_files_resolving = true;
    }
    app.preflight_services_items = Some(items.to_vec());
    app.preflight_services_resolving = true;
    let aur_items: Vec<_> = items
        .iter()
        .filter(|p| matches!(p.source, crate::state::Source::Aur))
        .cloned()
        .collect();
    if !aur_items.is_empty() {
        app.preflight_sandbox_items = Some(aur_items);
        app.preflight_sandbox_resolving = true;
    }
}

/// What: Create preflight modal with cached data.
///
/// Inputs:
/// - `app`: Application state.
/// - `items`: Packages under review.
/// - `summary`: Preflight summary data.
/// - `header`: Header chips data.
/// - `dependency_info`: Dependency information.
/// - `cached_files`: Cached file information.
///
/// Output:
/// - Creates and sets the Preflight modal in app state.
fn create_preflight_modal_with_cache(
    app: &mut AppState,
    items: Vec<PackageItem>,
    summary: crate::state::modal::PreflightSummaryData,
    header: crate::state::modal::PreflightHeaderChips,
    dependency_info: Vec<crate::state::modal::DependencyInfo>,
    cached_files: Vec<crate::state::modal::PackageFileInfo>,
) {
    app.modal = crate::state::Modal::Preflight {
        items,
        action: crate::state::PreflightAction::Install,
        tab: crate::state::PreflightTab::Deps,
        summary: Some(Box::new(summary)),
        summary_scroll: 0,
        header_chips: header,
        dependency_info,
        dep_selected: 0,
        dep_tree_expanded: std::collections::HashSet::new(),
        deps_error: None,
        file_info: cached_files,
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
        cascade_mode: app.remove_cascade_mode,
        cached_reverse_deps_report: None,
    };
}

/// What: Create preflight modal for insert mode (background computation).
///
/// Inputs:
/// - `app`: Application state.
/// - `items`: Packages under review.
///
/// Output:
/// - Creates and sets the Preflight modal with background computation queued.
fn create_preflight_modal_insert_mode(app: &mut AppState, items: Vec<PackageItem>) {
    let items_clone = items.clone();
    app.preflight_cancelled
        .store(false, std::sync::atomic::Ordering::Relaxed);
    app.preflight_summary_items = Some((items_clone, crate::state::PreflightAction::Install));
    app.preflight_summary_resolving = true;
    app.pending_service_plan.clear();
    app.modal = crate::state::Modal::Preflight {
        items,
        action: crate::state::PreflightAction::Install,
        tab: crate::state::PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips {
            package_count: 1,
            download_bytes: 0,
            install_delta_bytes: 0,
            aur_count: 0,
            risk_score: 0,
            risk_level: crate::state::modal::RiskLevel::Low,
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
        cascade_mode: app.remove_cascade_mode,
        cached_reverse_deps_report: None,
    };
}

/// What: Open preflight modal with cached dependencies and files, or trigger background resolution.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Packages to open preflight for
/// - `use_cache`: Whether to use cached dependencies/files or trigger background resolution
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - If `use_cache` is true, checks cache and uses cached data if available, otherwise triggers background resolution.
/// - If `use_cache` is false, always triggers background resolution (used in insert mode).
/// - Sets up all preflight resolution flags and initializes the modal state.
pub fn open_preflight_modal(app: &mut AppState, items: Vec<PackageItem>, use_cache: bool) {
    if crate::theme::settings().skip_preflight {
        // Direct install - check for reinstalls first, then batch updates
        // First, check if we're installing packages that are already installed (reinstall scenario)
        // BUT exclude packages that have updates available (those should go through normal update flow)
        let installed_set = crate::logic::deps::get_installed_packages();
        let provided_set = crate::logic::deps::get_provided_packages(&installed_set);
        let upgradable_set = crate::logic::deps::get_upgradable_packages();

        let installed_packages: Vec<crate::state::PackageItem> = items
            .iter()
            .filter(|item| {
                // Check if package is installed or provided by an installed package
                let is_installed = crate::logic::deps::is_package_installed_or_provided(
                    &item.name,
                    &installed_set,
                    &provided_set,
                );

                if !is_installed {
                    return false;
                }

                // Check if package has an update available
                // For official packages: check if it's in upgradable_set
                // For AUR packages: check if target version is different/newer than installed version
                let has_update = if upgradable_set.contains(&item.name) {
                    // Official package with update available
                    true
                } else if matches!(item.source, crate::state::Source::Aur)
                    && !item.version.is_empty()
                {
                    // AUR package: compare target version with installed version
                    // Use simple string comparison for AUR packages
                    // If target version is different from installed, it's an update
                    crate::logic::deps::get_installed_version(&item.name)
                        .is_ok_and(|installed_version| item.version != installed_version)
                } else {
                    // No update available
                    false
                };

                // Only show reinstall confirmation if installed AND no update available
                // If update is available, it should go through normal update flow
                !has_update
            })
            .cloned()
            .collect();

        if !installed_packages.is_empty() {
            // Show reinstall confirmation modal
            app.modal = crate::state::Modal::ConfirmReinstall {
                items: installed_packages,
                header_chips: crate::state::modal::PreflightHeaderChips::default(),
            };
            return;
        }

        // Check if this is a batch update scenario requiring confirmation
        // Only show if there's actually an update available (package is upgradable)
        // AND the package has installed packages in its "Required By" field (dependency risk)
        let has_versions = items.iter().any(|item| {
            matches!(item.source, crate::state::Source::Official { .. }) && !item.version.is_empty()
        });
        let has_upgrade_available = items.iter().any(|item| {
            matches!(item.source, crate::state::Source::Official { .. })
                && upgradable_set.contains(&item.name)
        });

        // Only show warning if package has installed packages in "Required By" (dependency risk)
        let has_installed_required_by = items.iter().any(|item| {
            matches!(item.source, crate::state::Source::Official { .. })
                && crate::index::is_installed(&item.name)
                && crate::logic::deps::has_installed_required_by(&item.name)
        });

        if has_versions && has_upgrade_available && has_installed_required_by {
            // Show confirmation modal for batch updates (only if update is actually available
            // AND package has installed dependents that could be affected)
            app.modal = crate::state::Modal::ConfirmBatchUpdate {
                items,
                dry_run: app.dry_run,
            };
            return;
        }

        crate::install::start_integrated_install_all(app, &items, app.dry_run);
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.installing_skipped"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        return;
    }

    if use_cache {
        // Reset cancellation flag when opening new preflight
        app.preflight_cancelled
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let crate::logic::preflight::PreflightSummaryOutcome {
            summary,
            header,
            reverse_deps_report: _,
        } = crate::logic::preflight::compute_preflight_summary(
            &items,
            crate::state::PreflightAction::Install,
        );
        app.pending_service_plan.clear();

        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_deps = filter_cached_dependencies(app, &item_names);
        let cached_files = filter_cached_files(app, &item_names);

        let dependency_info = if cached_deps.is_empty() {
            tracing::debug!(
                "[Preflight] Cache empty, will trigger background dependency resolution for {} packages",
                items.len()
            );
            Vec::new()
        } else {
            cached_deps
        };

        trigger_background_resolution(app, &items, &dependency_info, &cached_files);
        create_preflight_modal_with_cache(
            app,
            items,
            summary,
            header,
            dependency_info,
            cached_files,
        );
    } else {
        create_preflight_modal_insert_mode(app, items);
    }
    app.toast_message = Some(if use_cache {
        crate::i18n::t(app, "app.toasts.preflight_opened")
    } else {
        "Preflight opened".to_string()
    });
    app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Provide a baseline `AppState` for preflight helper tests.
    ///
    /// Inputs: None
    ///
    /// Output: Fresh `AppState` with default values.
    fn new_app() -> AppState {
        AppState::default()
    }

    /// What: Create a test package item for testing.
    ///
    /// Inputs:
    /// - `name`: Package name
    ///
    /// Output: A test `PackageItem` with official source.
    fn test_package(name: &str) -> PackageItem {
        PackageItem {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "Test package".to_string(),
            source: crate::state::Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }
    }

    #[test]
    /// What: Verify that `trigger_background_resolution` sets resolution flags correctly.
    ///
    /// Inputs:
    /// - App state
    /// - Empty `dependency_info` and `cached_files`
    ///
    /// Output:
    /// - Resolution flags and items are set
    ///
    /// Details:
    /// - Tests that background resolution is properly triggered when caches are empty.
    fn trigger_background_resolution_sets_flags_when_cache_empty() {
        let mut app = new_app();
        let items = vec![test_package("test-pkg")];

        trigger_background_resolution(&mut app, &items, &[], &[]);

        // Flags should be set
        assert!(app.preflight_deps_resolving);
        assert!(app.preflight_files_resolving);
        assert!(app.preflight_services_resolving);
        // Items should be queued
        assert!(app.preflight_deps_items.is_some());
        assert!(app.preflight_files_items.is_some());
        assert!(app.preflight_services_items.is_some());
    }

    #[test]
    /// What: Verify that `trigger_background_resolution` does not set deps flag when cache has deps.
    ///
    /// Inputs:
    /// - App state
    /// - Non-empty `dependency_info`
    ///
    /// Output:
    /// - Deps resolution flag and items are not set
    ///
    /// Details:
    /// - Tests that existing cached deps prevent re-resolution.
    fn trigger_background_resolution_skips_deps_when_cached() {
        let mut app = new_app();
        let items = vec![test_package("test-pkg")];
        let cached_deps = vec![crate::state::modal::DependencyInfo {
            name: "cached-dep".to_string(),
            version: "1.0".to_string(),
            status: crate::state::modal::DependencyStatus::ToInstall,
            source: crate::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        }];

        trigger_background_resolution(&mut app, &items, &cached_deps, &[]);

        // Deps should not be triggered (cache has data)
        assert!(!app.preflight_deps_resolving);
        assert!(app.preflight_deps_items.is_none());
        // Files should still be triggered (cache empty)
        assert!(app.preflight_files_resolving);
        assert!(app.preflight_files_items.is_some());
    }

    #[test]
    /// What: Verify `create_preflight_modal_insert_mode` resets `preflight_cancelled` flag.
    ///
    /// Inputs:
    /// - App state with `preflight_cancelled` set to true
    ///
    /// Output:
    /// - `preflight_cancelled` is reset to false
    ///
    /// Details:
    /// - Tests the insert mode path resets the cancellation flag.
    fn create_preflight_modal_insert_mode_resets_cancelled() {
        let mut app = new_app();
        app.preflight_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let items = vec![test_package("test-pkg")];

        create_preflight_modal_insert_mode(&mut app, items);

        // Cancelled flag should be reset
        assert!(
            !app.preflight_cancelled
                .load(std::sync::atomic::Ordering::Relaxed)
        );
    }

    #[test]
    /// What: Verify `filter_cached_dependencies` returns only matching deps.
    ///
    /// Inputs:
    /// - App state with cached dependencies
    /// - Set of item names to filter by
    ///
    /// Output:
    /// - Only dependencies matching the item names are returned
    ///
    /// Details:
    /// - Tests that dependency filtering works correctly.
    fn filter_cached_dependencies_returns_matching() {
        let mut app = new_app();
        app.install_list_deps = vec![
            crate::state::modal::DependencyInfo {
                name: "dep-a".to_string(),
                version: "1.0".to_string(),
                status: crate::state::modal::DependencyStatus::ToInstall,
                source: crate::state::modal::DependencySource::Official {
                    repo: "extra".to_string(),
                },
                required_by: vec!["pkg-a".to_string()],
                depends_on: Vec::new(),
                is_core: false,
                is_system: false,
            },
            crate::state::modal::DependencyInfo {
                name: "dep-b".to_string(),
                version: "1.0".to_string(),
                status: crate::state::modal::DependencyStatus::ToInstall,
                source: crate::state::modal::DependencySource::Official {
                    repo: "extra".to_string(),
                },
                required_by: vec!["pkg-b".to_string()],
                depends_on: Vec::new(),
                is_core: false,
                is_system: false,
            },
        ];

        let mut item_names = std::collections::HashSet::new();
        item_names.insert("pkg-a".to_string());

        let result = filter_cached_dependencies(&app, &item_names);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "dep-a");
    }

    #[test]
    /// What: Verify `filter_cached_files` returns only matching files.
    ///
    /// Inputs:
    /// - App state with cached file info
    /// - Set of item names to filter by
    ///
    /// Output:
    /// - Only file info matching the item names are returned
    ///
    /// Details:
    /// - Tests that file filtering works correctly.
    fn filter_cached_files_returns_matching() {
        let mut app = new_app();
        app.install_list_files = vec![
            crate::state::modal::PackageFileInfo {
                name: "pkg-a".to_string(),
                files: vec![crate::state::modal::FileChange {
                    path: "/usr/bin/a".to_string(),
                    change_type: crate::state::modal::FileChangeType::New,
                    package: "pkg-a".to_string(),
                    is_config: false,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                }],
                total_count: 1,
                new_count: 1,
                changed_count: 0,
                removed_count: 0,
                config_count: 0,
                pacnew_candidates: 0,
                pacsave_candidates: 0,
            },
            crate::state::modal::PackageFileInfo {
                name: "pkg-b".to_string(),
                files: vec![crate::state::modal::FileChange {
                    path: "/usr/bin/b".to_string(),
                    change_type: crate::state::modal::FileChangeType::New,
                    package: "pkg-b".to_string(),
                    is_config: false,
                    predicted_pacnew: false,
                    predicted_pacsave: false,
                }],
                total_count: 1,
                new_count: 1,
                changed_count: 0,
                removed_count: 0,
                config_count: 0,
                pacnew_candidates: 0,
                pacsave_candidates: 0,
            },
        ];

        let mut item_names = std::collections::HashSet::new();
        item_names.insert("pkg-a".to_string());

        let result = filter_cached_files(&app, &item_names);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "pkg-a");
    }
}
