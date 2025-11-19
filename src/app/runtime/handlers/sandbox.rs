use tokio::sync::mpsc;

use crate::state::*;

/// What: Log detailed dependency information for sandbox info entries.
///
/// Inputs:
/// - `sandbox_info`: Sandbox resolution results to log
///
/// Output: None (side effect: logging)
///
/// Details:
/// - Logs summary and per-package dependency counts
fn log_sandbox_info_details(sandbox_info: &[crate::logic::sandbox::SandboxInfo]) {
    if sandbox_info.is_empty() {
        tracing::warn!("[Runtime] handle_sandbox_result: Received empty sandbox info");
        return;
    }

    tracing::info!(
        "[Runtime] handle_sandbox_result: Received {} sandbox info entries",
        sandbox_info.len()
    );

    for info in sandbox_info {
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
}

/// What: Check if sandbox info is empty (all dependency vectors are empty).
///
/// Inputs:
/// - `info`: Sandbox info to check
///
/// Output: `true` if all dependency vectors are empty, `false` otherwise
fn is_empty_sandbox(info: &crate::logic::sandbox::SandboxInfo) -> bool {
    info.depends.is_empty()
        && info.makedepends.is_empty()
        && info.checkdepends.is_empty()
        && info.optdepends.is_empty()
}

/// What: Merge new sandbox info into existing cache, preserving valid entries.
///
/// Inputs:
/// - `current_cache`: Current cached sandbox info
/// - `new_info`: New sandbox resolution results
///
/// Output: Updated sandbox cache with merged entries
///
/// Details:
/// - Preserves entries for packages not in the new result
/// - Preserves existing valid entries if new entry is empty
/// - Replaces entries when new data is available
fn merge_sandbox_cache(
    current_cache: &[crate::logic::sandbox::SandboxInfo],
    new_info: &[crate::logic::sandbox::SandboxInfo],
) -> Vec<crate::logic::sandbox::SandboxInfo> {
    let mut updated_sandbox = current_cache.to_vec();
    let new_package_names: std::collections::HashSet<String> =
        new_info.iter().map(|s| s.package_name.clone()).collect();

    // Extract existing valid entries for packages that will be updated
    let mut existing_valid: std::collections::HashMap<String, crate::logic::sandbox::SandboxInfo> =
        updated_sandbox
            .iter()
            .filter(|s| new_package_names.contains(&s.package_name))
            .filter(|s| !is_empty_sandbox(s))
            .map(|s| (s.package_name.clone(), s.clone()))
            .collect();

    // Remove old entries for packages that are in the new result
    updated_sandbox.retain(|s| !new_package_names.contains(&s.package_name));

    // Add new entries, preserving existing valid data if new entry is empty
    for new_entry in new_info {
        if is_empty_sandbox(new_entry) {
            if let Some(existing) = existing_valid.remove(&new_entry.package_name) {
                tracing::debug!(
                    "[Runtime] handle_sandbox_result: Preserving existing valid sandbox info for '{}' (new entry is empty)",
                    new_entry.package_name
                );
                updated_sandbox.push(existing);
            } else {
                updated_sandbox.push(new_entry.clone());
            }
        } else {
            updated_sandbox.push(new_entry.clone());
        }
    }

    updated_sandbox
}

/// What: Sync sandbox info to preflight modal if open.
///
/// Inputs:
/// - `modal`: Preflight modal state
/// - `sandbox_info`: Sandbox resolution results
///
/// Output: None (side effect: updates modal state)
///
/// Details:
/// - Filters sandbox info to match modal items
/// - Handles empty results and mismatches gracefully
/// - Sets appropriate error messages when needed
fn sync_sandbox_to_modal(
    modal: &mut crate::state::Modal,
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
) {
    let crate::state::Modal::Preflight {
        items,
        sandbox_info: modal_sandbox,
        sandbox_loaded,
        sandbox_error,
        ..
    } = modal
    else {
        tracing::debug!("[Runtime] handle_sandbox_result: Preflight modal not open, skipping sync");
        return;
    };

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

    if !filtered_sandbox.is_empty() {
        sync_matching_sandbox(
            modal_sandbox,
            sandbox_loaded,
            sandbox_error,
            filtered_sandbox,
        );
        return;
    }

    sync_empty_or_mismatched_sandbox(
        sandbox_info,
        &item_names,
        aur_items.as_slice(),
        modal_sandbox,
        sandbox_loaded,
        sandbox_error,
    );
}

/// What: Sync matching sandbox info to modal.
///
/// Inputs:
/// - `modal_sandbox`: Modal sandbox info field to update
/// - `sandbox_loaded`: Modal loaded flag to update
/// - `sandbox_error`: Modal error field to update
/// - `filtered_sandbox`: Matching sandbox info to sync
///
/// Output: None (side effect: updates modal fields)
fn sync_matching_sandbox(
    modal_sandbox: &mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_loaded: &mut bool,
    sandbox_error: &mut Option<String>,
    filtered_sandbox: Vec<crate::logic::sandbox::SandboxInfo>,
) {
    tracing::info!(
        "[Runtime] handle_sandbox_result: Syncing {} sandbox infos to preflight modal",
        filtered_sandbox.len()
    );
    *modal_sandbox = filtered_sandbox;
    *sandbox_loaded = true;
    *sandbox_error = None;
    tracing::debug!(
        "[Runtime] handle_sandbox_result: Successfully synced sandbox info to modal, loaded={}",
        *sandbox_loaded
    );
}

/// What: Handle empty or mismatched sandbox info for modal sync.
///
/// Inputs:
/// - `sandbox_info`: All sandbox resolution results
/// - `item_names`: Names of items in the modal
/// - `aur_items`: AUR items in the modal
/// - `modal_sandbox`: Modal sandbox info field to update
/// - `sandbox_loaded`: Modal loaded flag to update
/// - `sandbox_error`: Modal error field to update
///
/// Output: None (side effect: updates modal fields)
///
/// Details:
/// - Handles cases where sandbox info doesn't match modal items
/// - Sets appropriate error messages for empty results
fn sync_empty_or_mismatched_sandbox(
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    item_names: &std::collections::HashSet<String>,
    aur_items: &[&crate::state::PackageItem],
    modal_sandbox: &mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_loaded: &mut bool,
    sandbox_error: &mut Option<String>,
) {
    if aur_items.is_empty() {
        *sandbox_loaded = true;
        *sandbox_error = None;
        return;
    }

    if !sandbox_info.is_empty() {
        handle_partial_match(
            sandbox_info,
            item_names,
            modal_sandbox,
            sandbox_loaded,
            sandbox_error,
        );
    } else {
        handle_empty_sandbox_result(aur_items, sandbox_loaded, sandbox_error);
    }
}

/// What: Handle partial match between sandbox info and modal items.
///
/// Inputs:
/// - `sandbox_info`: All sandbox resolution results
/// - `item_names`: Names of items in the modal
/// - `modal_sandbox`: Modal sandbox info field to update
/// - `sandbox_loaded`: Modal loaded flag to update
/// - `sandbox_error`: Modal error field to update
///
/// Output: None (side effect: updates modal fields)
fn handle_partial_match(
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    item_names: &std::collections::HashSet<String>,
    modal_sandbox: &mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_loaded: &mut bool,
    sandbox_error: &mut Option<String>,
) {
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
        *sandbox_loaded = true;
        *sandbox_error = None;
    }
}

/// What: Handle empty sandbox result when AUR packages are expected.
///
/// Inputs:
/// - `aur_items`: AUR items in the modal
/// - `sandbox_loaded`: Modal loaded flag to update
/// - `sandbox_error`: Modal error field to update
///
/// Output: None (side effect: updates modal fields)
fn handle_empty_sandbox_result(
    aur_items: &[&crate::state::PackageItem],
    sandbox_loaded: &mut bool,
    sandbox_error: &mut Option<String>,
) {
    tracing::warn!(
        "[Runtime] handle_sandbox_result: Sandbox resolution returned empty results for {} AUR packages (AUR may be down or network issues). Modal items: {:?}",
        aur_items.len(),
        aur_items.iter().map(|i| &i.name).collect::<Vec<_>>()
    );
    *sandbox_loaded = true;
    *sandbox_error = Some(format!(
        "Failed to fetch sandbox information for {} AUR package(s). AUR may be temporarily unavailable.",
        aur_items.len()
    ));
}

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
    tracing::debug!(
        "[Runtime] handle_sandbox_result: Clearing flags - was_preflight={}, sandbox_resolving={}, preflight_sandbox_resolving={}, cancelled={}",
        was_preflight,
        app.sandbox_resolving,
        app.preflight_sandbox_resolving,
        cancelled
    );
    app.sandbox_resolving = false;
    app.preflight_sandbox_resolving = false;

    if cancelled {
        if was_preflight {
            tracing::debug!("[Runtime] Ignoring sandbox result (preflight cancelled)");
            app.preflight_sandbox_items = None;
        }
        let _ = tick_tx.send(());
        return;
    }

    // Update cached sandbox info
    tracing::info!(
        stage = "sandbox",
        result_count = sandbox_info.len(),
        was_preflight = was_preflight,
        "[Runtime] Sandbox resolution worker completed"
    );

    log_sandbox_info_details(&sandbox_info);

    tracing::debug!(
        "[Runtime] handle_sandbox_result: Updating install_list_sandbox with {} entries (current cache has {})",
        sandbox_info.len(),
        app.install_list_sandbox.len()
    );

    app.install_list_sandbox = merge_sandbox_cache(&app.install_list_sandbox, &sandbox_info);

    tracing::debug!(
        "[Runtime] handle_sandbox_result: install_list_sandbox now has {} entries: {:?}",
        app.install_list_sandbox.len(),
        app.install_list_sandbox
            .iter()
            .map(|s| &s.package_name)
            .collect::<Vec<_>>()
    );

    sync_sandbox_to_modal(&mut app.modal, &sandbox_info);

    if was_preflight {
        app.preflight_sandbox_items = None;
    }
    app.sandbox_cache_dirty = true;
    tracing::debug!(
        "[Runtime] handle_sandbox_result: Marked sandbox_cache_dirty=true, install_list_sandbox has {} entries: {:?}",
        app.install_list_sandbox.len(),
        app.install_list_sandbox
            .iter()
            .map(|s| &s.package_name)
            .collect::<Vec<_>>()
    );

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
