use tokio::sync::mpsc;

use crate::logic::add_to_install_list;
use crate::state::*;

/// What: Handle add to install list event (single item).
///
/// Inputs:
/// - `app`: Application state
/// - `item`: Package item to add
/// - `deps_req_tx`: Channel sender for dependency resolution requests
/// - `files_req_tx`: Channel sender for file resolution requests
/// - `services_req_tx`: Channel sender for service resolution requests
/// - `sandbox_req_tx`: Channel sender for sandbox resolution requests
///
/// Details:
/// - Adds item to install list
/// - Triggers background resolution for dependencies, files, services, and sandbox
pub fn handle_add_to_install_list(
    app: &mut AppState,
    item: PackageItem,
    deps_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    files_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    services_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
    sandbox_req_tx: &mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    add_to_install_list(app, item);
    // Trigger background dependency resolution for updated install list
    if !app.install_list.is_empty() {
        app.deps_resolving = true;
        let _ = deps_req_tx.send(app.install_list.clone());
        // Trigger background file resolution for updated install list
        app.files_resolving = true;
        let _ = files_req_tx.send(app.install_list.clone());
        // Trigger background service resolution for updated install list
        app.services_resolving = true;
        let _ = services_req_tx.send(app.install_list.clone());
        // Trigger background sandbox resolution for updated install list
        app.sandbox_resolving = true;
        let _ = sandbox_req_tx.send(app.install_list.clone());
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
    deps: Vec<crate::state::modal::DependencyInfo>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_deps_resolving;
    app.deps_resolving = false; // CRITICAL: Always reset this flag when we receive ANY result
    app.preflight_deps_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached dependencies
        tracing::info!(
            stage = "dependencies",
            result_count = deps.len(),
            "[Runtime] Dependency resolution worker completed"
        );
        app.install_list_deps = deps.clone();
        // Sync dependencies to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items,
            dependency_info,
            ..
        } = &mut app.modal
        {
            // Filter dependencies to only those required by current modal items
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let filtered_deps: Vec<_> = deps
                .iter()
                .filter(|dep| {
                    dep.required_by
                        .iter()
                        .any(|req_by| item_names.contains(req_by))
                })
                .cloned()
                .collect();
            if !filtered_deps.is_empty() {
                tracing::debug!(
                    "[Runtime] Synced {} dependencies to preflight modal (was_preflight={})",
                    filtered_deps.len(),
                    was_preflight
                );
                *dependency_info = filtered_deps;
            }
        }
        if was_preflight {
            app.preflight_deps_items = None;
        }
        app.deps_cache_dirty = true; // Mark cache as dirty for persistence
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring dependency result (preflight cancelled)");
        app.preflight_deps_items = None;
    }
    let _ = tick_tx.send(());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Provide a baseline `AppState` for handler tests.
    ///
    /// Inputs: None
    /// Output: Fresh `AppState` with default values
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Verify that handle_add_to_install_list adds item and triggers resolutions.
    ///
    /// Inputs:
    /// - App state with empty install list
    /// - PackageItem to add
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

        let (deps_tx, mut deps_rx) = mpsc::unbounded_channel();
        let (files_tx, mut files_rx) = mpsc::unbounded_channel();
        let (services_tx, mut services_rx) = mpsc::unbounded_channel();
        let (sandbox_tx, mut sandbox_rx) = mpsc::unbounded_channel();

        let item = PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: Source::Aur,
            popularity: None,
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
        // Requests should be sent
        assert!(deps_rx.try_recv().is_ok());
        assert!(files_rx.try_recv().is_ok());
        assert!(services_rx.try_recv().is_ok());
        assert!(sandbox_rx.try_recv().is_ok());
    }

    #[test]
    /// What: Verify that handle_dependency_result updates cache and respects cancellation.
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

        handle_dependency_result(&mut app, deps.clone(), &tick_tx);

        // Dependencies should be cached
        assert_eq!(app.install_list_deps.len(), 1);
        // Flags should be reset
        assert!(!app.deps_resolving);
        assert!(!app.preflight_deps_resolving);
        // Cache dirty flag should be set
        assert!(app.deps_cache_dirty);
    }

    #[test]
    /// What: Verify that handle_dependency_result ignores results when cancelled.
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

        handle_dependency_result(&mut app, deps, &tick_tx);

        // Dependencies should not be updated when cancelled
        assert_eq!(app.install_list_deps.len(), 0);
        // Flags should still be reset
        assert!(!app.deps_resolving);
        assert!(!app.preflight_deps_resolving);
    }
}
