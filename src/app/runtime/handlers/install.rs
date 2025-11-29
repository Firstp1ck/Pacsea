use tokio::sync::mpsc;

use crate::app::runtime::handlers::common::{HandlerConfig, handle_result};
use crate::logic::add_to_install_list;
use crate::state::{AppState, PackageItem};

/// What: Handle add to install list event (single item).
///
/// Inputs:
/// - `app`: Application state
/// - `item`: Package item to add
/// - `deps_req_tx`: Channel sender for dependency resolution requests (with action)
/// - `files_req_tx`: Channel sender for file resolution requests (with action)
/// - `services_req_tx`: Channel sender for service resolution requests
/// - `sandbox_req_tx`: Channel sender for sandbox resolution requests
///
/// Details:
/// - Adds item to install list
/// - Triggers background resolution for dependencies, files, services, and sandbox
pub fn handle_add_to_install_list(
    app: &mut AppState,
    item: PackageItem,
    deps_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    files_req_tx: &mpsc::UnboundedSender<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    services_req_tx: &mpsc::UnboundedSender<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    add_to_install_list(app, item);
    // Trigger background dependency resolution for updated install list (Install action)
    if !app.install_list.is_empty() {
        app.deps_resolving = true;
        let _ = deps_req_tx.send((
            app.install_list.clone(),
            crate::state::modal::PreflightAction::Install,
        ));
        // Trigger background file resolution for updated install list (Install action)
        app.files_resolving = true;
        let _ = files_req_tx.send((
            app.install_list.clone(),
            crate::state::modal::PreflightAction::Install,
        ));
        // Trigger background service resolution for updated install list
        app.services_resolving = true;
        let _ = services_req_tx.send((
            app.install_list.clone(),
            crate::state::modal::PreflightAction::Install,
        ));
        // Trigger background sandbox resolution for updated install list
        app.sandbox_resolving = true;
        let _ = sandbox_req_tx.send(app.install_list.clone());
    }
}

/// What: Handler configuration for dependency results.
struct DependencyHandlerConfig;

impl HandlerConfig for DependencyHandlerConfig {
    type Result = crate::state::modal::DependencyInfo;

    fn get_resolving(&self, app: &AppState) -> bool {
        app.deps_resolving
    }

    fn set_resolving(&self, app: &mut AppState, value: bool) {
        app.deps_resolving = value; // CRITICAL: Always reset this flag when we receive ANY result
    }

    fn get_preflight_resolving(&self, app: &AppState) -> bool {
        app.preflight_deps_resolving
    }

    fn set_preflight_resolving(&self, app: &mut AppState, value: bool) {
        app.preflight_deps_resolving = value;
    }

    fn stage_name(&self) -> &'static str {
        "dependencies"
    }

    fn update_cache(&self, app: &mut AppState, results: &[Self::Result]) {
        app.install_list_deps = results.to_vec();
    }

    fn set_cache_dirty(&self, app: &mut AppState) {
        app.deps_cache_dirty = true;
    }

    fn clear_preflight_items(&self, app: &mut AppState) {
        app.preflight_deps_items = None;
    }

    fn sync_to_modal(&self, app: &mut AppState, results: &[Self::Result], was_preflight: bool) {
        // Sync dependencies to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items,
            action,
            dependency_info,
            ..
        } = &mut app.modal
        {
            tracing::info!(
                "[Runtime] sync_to_modal: action={:?}, results={}, items={}, was_preflight={}",
                action,
                results.len(),
                items.len(),
                was_preflight
            );

            // Filter dependencies to only those required by current modal items
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();

            // Log first few results for debugging
            for (i, dep) in results.iter().take(3).enumerate() {
                tracing::info!(
                    "[Runtime] sync_to_modal: result[{}] name={}, required_by={:?}",
                    i,
                    dep.name,
                    dep.required_by
                );
            }

            let filtered_deps: Vec<_> = results
                .iter()
                .filter(|dep| {
                    let matches = dep
                        .required_by
                        .iter()
                        .any(|req_by| item_names.contains(req_by));
                    if !matches && results.len() <= 10 {
                        tracing::debug!(
                            "[Runtime] sync_to_modal: dep {} required_by={:?} doesn't match items={:?}",
                            dep.name,
                            dep.required_by,
                            item_names
                        );
                    }
                    matches
                })
                .cloned()
                .collect();
            let old_deps_len = dependency_info.len();
            tracing::info!(
                "[Runtime] sync_to_modal: filtered {} deps from {} results (items={:?})",
                filtered_deps.len(),
                results.len(),
                item_names
            );
            if filtered_deps.is_empty() {
                tracing::info!(
                    "[Runtime] No matching dependencies to sync (results={}, items={:?})",
                    results.len(),
                    item_names
                );
                // For Remove action with no matching deps, still update to empty
                // This indicates no reverse dependencies found
                if matches!(action, crate::state::PreflightAction::Remove) {
                    tracing::info!(
                        "[Runtime] Remove action: setting dependency_info to empty (no reverse deps)"
                    );
                    *dependency_info = Vec::new();
                }
            } else {
                tracing::info!(
                    "[Runtime] Syncing {} dependencies to preflight modal (was_preflight={}, modal had {} before)",
                    filtered_deps.len(),
                    was_preflight,
                    old_deps_len
                );
                *dependency_info = filtered_deps;
                tracing::info!(
                    "[Runtime] Modal dependency_info now has {} entries (was {})",
                    dependency_info.len(),
                    old_deps_len
                );
            }
        } else {
            tracing::debug!("[Runtime] sync_to_modal: Modal is not Preflight, skipping sync");
        }
    }

    fn log_flag_clear(&self, app: &AppState, was_preflight: bool, cancelled: bool) {
        tracing::debug!(
            "[Runtime] handle_dependency_result: Clearing flags - was_preflight={}, deps_resolving={}, preflight_deps_resolving={}, cancelled={}",
            was_preflight,
            self.get_resolving(app),
            app.preflight_deps_resolving,
            cancelled
        );
    }

    fn is_resolution_complete(&self, app: &AppState, results: &[Self::Result]) -> bool {
        // Check if preflight modal is open
        if let crate::state::Modal::Preflight { items, action, .. } = &app.modal {
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();

            if item_names.is_empty() {
                return true;
            }

            // For Remove action: reverse dependency resolution is always complete when we get
            // a result back. Having 0 results means no packages depend on the removal targets,
            // which is a valid complete state.
            if matches!(action, crate::state::PreflightAction::Remove) {
                tracing::debug!(
                    "[Runtime] handle_dependency_result: Remove action - resolution complete with {} results",
                    results.len()
                );
                return true;
            }

            // For Install action: check if all items have been processed
            // Collect all packages that appear in required_by fields
            let result_packages: std::collections::HashSet<String> = results
                .iter()
                .flat_map(|d| d.required_by.iter().cloned())
                .collect();
            let cache_packages: std::collections::HashSet<String> = app
                .install_list_deps
                .iter()
                .flat_map(|d| d.required_by.iter().cloned())
                .collect();

            // Check if all items appear in required_by (meaning they've been processed)
            // OR if they're in the cache from a previous resolution
            let all_processed = item_names
                .iter()
                .all(|name| result_packages.contains(name) || cache_packages.contains(name));

            if !all_processed {
                let missing: Vec<String> = item_names
                    .iter()
                    .filter(|name| {
                        !result_packages.contains(*name) && !cache_packages.contains(*name)
                    })
                    .cloned()
                    .collect();
                tracing::debug!(
                    "[Runtime] handle_dependency_result: Resolution incomplete - missing deps for: {:?}",
                    missing
                );
            }

            return all_processed;
        }

        // If no preflight modal, check preflight_deps_items
        if let Some((ref install_items, ref action)) = app.preflight_deps_items {
            let item_names: std::collections::HashSet<String> =
                install_items.iter().map(|i| i.name.clone()).collect();

            if item_names.is_empty() {
                return true;
            }

            // For Remove action: always complete when we get a result
            if matches!(action, crate::state::PreflightAction::Remove) {
                tracing::debug!(
                    "[Runtime] handle_dependency_result: Remove action (no modal) - resolution complete with {} results",
                    results.len()
                );
                return true;
            }

            // For Install action: check if all items have been processed
            // Collect all packages that appear in required_by fields
            let result_packages: std::collections::HashSet<String> = results
                .iter()
                .flat_map(|d| d.required_by.iter().cloned())
                .collect();
            let cache_packages: std::collections::HashSet<String> = app
                .install_list_deps
                .iter()
                .flat_map(|d| d.required_by.iter().cloned())
                .collect();

            // Check if all items appear in required_by (meaning they've been processed)
            let all_processed = item_names
                .iter()
                .all(|name| result_packages.contains(name) || cache_packages.contains(name));

            if !all_processed {
                let missing: Vec<String> = item_names
                    .iter()
                    .filter(|name| {
                        !result_packages.contains(*name) && !cache_packages.contains(*name)
                    })
                    .cloned()
                    .collect();
                tracing::debug!(
                    "[Runtime] handle_dependency_result: Resolution incomplete - missing deps for: {:?}",
                    missing
                );
            }

            return all_processed;
        }

        // No items to check, resolution is complete
        true
    }
}

/// What: Handle dependency resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `deps`: Dependency resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached dependencies
/// - Syncs dependencies to preflight modal if open
/// - Respects cancellation flag
pub fn handle_dependency_result(
    app: &mut AppState,
    deps: &[crate::state::modal::DependencyInfo],
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    handle_result(app, deps, tick_tx, &DependencyHandlerConfig);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Source;

    /// What: Provide a baseline `AppState` for handler tests.
    ///
    /// Inputs: None
    /// Output: Fresh `AppState` with default values
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Verify that `handle_add_to_install_list` adds item and triggers resolutions.
    ///
    /// Inputs:
    /// - App state with empty install list
    /// - `PackageItem` to add
    /// - Channel senders
    ///
    /// Output:
    /// - Item is added to install list
    /// - Resolution flags are set
    /// - Requests are sent to resolution channels
    ///
    /// Details:
    /// - Tests that adding items triggers background resolution
    fn handle_add_to_install_list_adds_and_triggers_resolution() {
        let mut app = new_app();
        app.install_list.clear();

        let (deps_tx, mut deps_rx) =
            mpsc::unbounded_channel::<(Vec<PackageItem>, crate::state::modal::PreflightAction)>();
        let (files_tx, mut files_rx) = mpsc::unbounded_channel();
        let (services_tx, mut services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, mut sandbox_rx) = mpsc::unbounded_channel();

        let item = PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };

        handle_add_to_install_list(
            &mut app,
            item,
            &deps_tx,
            &files_tx,
            &services_tx,
            &sandbox_tx,
        );

        // Item should be added
        assert_eq!(app.install_list.len(), 1);
        assert_eq!(app.install_list[0].name, "test-package");
        // Flags should be set
        assert!(app.deps_resolving);
        assert!(app.files_resolving);
        assert!(app.services_resolving);
        assert!(app.sandbox_resolving);
        // Requests should be sent (with Install action)
        let (items, action) = deps_rx.try_recv().expect("deps request should be sent");
        assert_eq!(items.len(), 1);
        assert!(matches!(
            action,
            crate::state::modal::PreflightAction::Install
        ));
        assert!(files_rx.try_recv().is_ok());
        assert!(services_rx.try_recv().is_ok());
        assert!(sandbox_rx.try_recv().is_ok());
    }

    #[test]
    /// What: Verify that `handle_dependency_result` updates cache and respects cancellation.
    ///
    /// Inputs:
    /// - App state
    /// - Dependency resolution results
    /// - Cancellation flag not set
    ///
    /// Output:
    /// - Dependencies are cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that dependency results are properly processed
    fn handle_dependency_result_updates_cache() {
        let mut app = new_app();
        app.deps_resolving = true;
        app.preflight_deps_resolving = false;
        app.preflight_cancelled
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let deps = vec![crate::state::modal::DependencyInfo {
            name: "dep-package".to_string(),
            version: "1.0.0".to_string(),
            status: crate::state::modal::DependencyStatus::ToInstall,
            source: crate::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        }];

        handle_dependency_result(&mut app, &deps, &tick_tx);

        // Dependencies should be cached
        assert_eq!(app.install_list_deps.len(), 1);
        // Flags should be reset
        assert!(!app.deps_resolving);
        assert!(!app.preflight_deps_resolving);
        // Cache dirty flag should be set
        assert!(app.deps_cache_dirty);
    }

    #[test]
    /// What: Verify that `handle_dependency_result` ignores results when cancelled.
    ///
    /// Inputs:
    /// - App state with cancellation flag set
    /// - Dependency resolution results
    ///
    /// Output:
    /// - Results are ignored
    /// - Flags are still reset
    ///
    /// Details:
    /// - Tests that cancellation is properly respected
    fn handle_dependency_result_respects_cancellation() {
        let mut app = new_app();
        app.preflight_deps_resolving = true;
        app.preflight_cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        app.install_list_deps = vec![]; // Empty before

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let deps = vec![crate::state::modal::DependencyInfo {
            name: "dep-package".to_string(),
            version: "1.0.0".to_string(),
            status: crate::state::modal::DependencyStatus::ToInstall,
            source: crate::state::modal::DependencySource::Official {
                repo: "extra".to_string(),
            },
            required_by: vec!["test-package".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        }];

        handle_dependency_result(&mut app, &deps, &tick_tx);

        // Dependencies should not be updated when cancelled
        assert_eq!(app.install_list_deps.len(), 0);
        // Flags should still be reset
        assert!(!app.deps_resolving);
        assert!(!app.preflight_deps_resolving);
    }
}
