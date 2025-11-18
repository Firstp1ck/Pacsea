use tokio::sync::mpsc;

use crate::state::*;

/// What: Handle sandbox resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `sandbox_info`: Sandbox resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached sandbox info
/// - Syncs sandbox info to preflight modal if open
/// - Handles empty results and errors gracefully
/// - Respects cancellation flag
pub fn handle_sandbox_result(
    app: &mut AppState,
    sandbox_info: Vec<crate::logic::sandbox::SandboxInfo>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_sandbox_resolving;
    app.sandbox_resolving = false;
    app.preflight_sandbox_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached sandbox info
        tracing::info!(
            stage = "sandbox",
            result_count = sandbox_info.len(),
            was_preflight = was_preflight,
            "[Runtime] Sandbox resolution worker completed"
        );

        // Log detailed dependency information for each sandbox info entry
        if !sandbox_info.is_empty() {
            tracing::info!(
                "[Runtime] handle_sandbox_result: Received {} sandbox info entries",
                sandbox_info.len()
            );
            for info in &sandbox_info {
                let total_deps = info.depends.len()
                    + info.makedepends.len()
                    + info.checkdepends.len()
                    + info.optdepends.len();
                let installed_deps = info.depends.iter().filter(|d| d.is_installed).count()
                    + info.makedepends.iter().filter(|d| d.is_installed).count()
                    + info.checkdepends.iter().filter(|d| d.is_installed).count()
                    + info.optdepends.iter().filter(|d| d.is_installed).count();
                tracing::info!(
                    "[Runtime] handle_sandbox_result: Package '{}' - total_deps={}, installed_deps={}, depends={}, makedepends={}, checkdepends={}, optdepends={}",
                    info.package_name,
                    total_deps,
                    installed_deps,
                    info.depends.len(),
                    info.makedepends.len(),
                    info.checkdepends.len(),
                    info.optdepends.len()
                );
            }
        } else {
            tracing::warn!(
                "[Runtime] handle_sandbox_result: Received empty sandbox info (was_preflight={})",
                was_preflight
            );
        }

        tracing::debug!(
            "[Runtime] handle_sandbox_result: Updating install_list_sandbox with {} entries (current cache has {})",
            sandbox_info.len(),
            app.install_list_sandbox.len()
        );

        // Merge/update entries instead of replacing entire list
        // This preserves entries for packages not in the current resolution result
        let mut updated_sandbox = app.install_list_sandbox.clone();
        let new_package_names: std::collections::HashSet<String> = sandbox_info
            .iter()
            .map(|s| s.package_name.clone())
            .collect();

        // Helper to check if sandbox info is empty (all dependency vectors are empty)
        let is_empty_sandbox = |info: &crate::logic::sandbox::SandboxInfo| -> bool {
            info.depends.is_empty()
                && info.makedepends.is_empty()
                && info.checkdepends.is_empty()
                && info.optdepends.is_empty()
        };

        // Remove old entries for packages that are in the new result
        // But preserve existing valid entries if the new entry is empty
        let mut existing_valid: std::collections::HashMap<
            String,
            crate::logic::sandbox::SandboxInfo,
        > = updated_sandbox
            .iter()
            .filter(|s| new_package_names.contains(&s.package_name))
            .filter(|s| !is_empty_sandbox(s))
            .map(|s| (s.package_name.clone(), s.clone()))
            .collect();

        updated_sandbox.retain(|s| !new_package_names.contains(&s.package_name));

        // Add new entries, but preserve existing valid data if new entry is empty
        for new_entry in &sandbox_info {
            if is_empty_sandbox(new_entry) {
                // If new entry is empty, check if we have existing valid data
                if let Some(existing) = existing_valid.remove(&new_entry.package_name) {
                    tracing::debug!(
                        "[Runtime] handle_sandbox_result: Preserving existing valid sandbox info for '{}' (new entry is empty)",
                        new_entry.package_name
                    );
                    updated_sandbox.push(existing);
                } else {
                    // No existing valid data, add empty entry
                    updated_sandbox.push(new_entry.clone());
                }
            } else {
                // New entry has valid data, use it
                updated_sandbox.push(new_entry.clone());
            }
        }

        app.install_list_sandbox = updated_sandbox;
        tracing::debug!(
            "[Runtime] handle_sandbox_result: install_list_sandbox now has {} entries: {:?}",
            app.install_list_sandbox.len(),
            app.install_list_sandbox
                .iter()
                .map(|s| &s.package_name)
                .collect::<Vec<_>>()
        );
        // Sync sandbox info to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items,
            sandbox_info: modal_sandbox,
            sandbox_loaded,
            sandbox_error,
            ..
        } = &mut app.modal
        {
            // Filter sandbox info to only those for current modal items (AUR only)
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let aur_items: Vec<_> = items
                .iter()
                .filter(|p| matches!(p.source, crate::state::Source::Aur))
                .collect();
            let filtered_sandbox: Vec<_> = sandbox_info
                .iter()
                .filter(|sb| item_names.contains(&sb.package_name))
                .cloned()
                .collect();

            tracing::info!(
                "[Runtime] handle_sandbox_result: Modal open - items={}, aur_items={}, filtered_sandbox={}, modal_current={}",
                items.len(),
                aur_items.len(),
                filtered_sandbox.len(),
                modal_sandbox.len()
            );

            // Always sync sandbox info if we have matching entries, even if dependency lists are empty
            // (empty lists mean all dependencies are already installed, which is still useful info)
            if !filtered_sandbox.is_empty() {
                tracing::info!(
                    "[Runtime] handle_sandbox_result: Syncing {} sandbox infos to preflight modal (was_preflight={})",
                    filtered_sandbox.len(),
                    was_preflight
                );
                *modal_sandbox = filtered_sandbox;
                *sandbox_loaded = true;
                *sandbox_error = None; // Clear any previous errors
                tracing::debug!(
                    "[Runtime] handle_sandbox_result: Successfully synced sandbox info to modal, loaded={}",
                    *sandbox_loaded
                );
            } else {
                // Check if we have AUR packages but no sandbox info
                if aur_items.is_empty() {
                    // No AUR packages, mark as loaded
                    *sandbox_loaded = true;
                    *sandbox_error = None;
                } else if !sandbox_info.is_empty() {
                    // We have sandbox info but it doesn't match current items
                    // This could happen if items changed between resolution start and completion
                    // Try to sync anyway - maybe some packages match
                    let partial_match: Vec<_> = sandbox_info
                        .iter()
                        .filter(|sb| item_names.contains(&sb.package_name))
                        .cloned()
                        .collect();
                    if !partial_match.is_empty() {
                        tracing::debug!(
                            "[Runtime] Partial sandbox sync: {} of {} packages matched",
                            partial_match.len(),
                            item_names.len()
                        );
                        *modal_sandbox = partial_match;
                        *sandbox_loaded = true;
                        *sandbox_error = None;
                    } else {
                        tracing::warn!(
                            "[Runtime] Sandbox info exists but doesn't match modal items. Modal items: {:?}, Sandbox packages: {:?}",
                            item_names,
                            sandbox_info
                                .iter()
                                .map(|s| &s.package_name)
                                .collect::<Vec<_>>()
                        );
                        // Still mark as loaded to prevent infinite loading state
                        *sandbox_loaded = true;
                        *sandbox_error = None;
                    }
                } else {
                    // sandbox_info is empty but we have AUR packages - resolution likely failed
                    // This could happen if AUR is down or network issues
                    tracing::warn!(
                        "[Runtime] handle_sandbox_result: Sandbox resolution returned empty results for {} AUR packages (AUR may be down or network issues). Modal items: {:?}",
                        aur_items.len(),
                        aur_items.iter().map(|i| &i.name).collect::<Vec<_>>()
                    );
                    *sandbox_loaded = true; // Mark as loaded so UI can show error/empty state
                    *sandbox_error = Some(format!(
                        "Failed to fetch sandbox information for {} AUR package(s). AUR may be temporarily unavailable.",
                        aur_items.len()
                    ));
                }
            }
        } else {
            tracing::debug!(
                "[Runtime] handle_sandbox_result: Preflight modal not open, skipping sync"
            );
        }
        if was_preflight {
            app.preflight_sandbox_items = None;
        }
        app.sandbox_cache_dirty = true; // Mark cache as dirty for persistence
        tracing::debug!(
            "[Runtime] handle_sandbox_result: Marked sandbox_cache_dirty=true, install_list_sandbox has {} entries: {:?}",
            app.install_list_sandbox.len(),
            app.install_list_sandbox
                .iter()
                .map(|s| &s.package_name)
                .collect::<Vec<_>>()
        );
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring sandbox result (preflight cancelled)");
        app.preflight_sandbox_items = None;
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
    /// What: Verify that handle_sandbox_result updates cache correctly.
    ///
    /// Inputs:
    /// - App state
    /// - Sandbox resolution results
    ///
    /// Output:
    /// - Sandbox info is cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that sandbox results are properly processed
    fn handle_sandbox_result_updates_cache() {
        let mut app = new_app();
        app.sandbox_resolving = true;

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let sandbox_info = vec![crate::logic::sandbox::SandboxInfo {
            package_name: "test-package".to_string(),
            depends: vec![],
            makedepends: vec![],
            checkdepends: vec![],
            optdepends: vec![],
        }];

        handle_sandbox_result(&mut app, sandbox_info.clone(), &tick_tx);

        // Sandbox info should be cached
        assert_eq!(app.install_list_sandbox.len(), 1);
        // Flags should be reset
        assert!(!app.sandbox_resolving);
        assert!(!app.preflight_sandbox_resolving);
        // Cache dirty flag should be set
        assert!(app.sandbox_cache_dirty);
    }
}
