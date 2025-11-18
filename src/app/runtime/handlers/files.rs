use tokio::sync::mpsc;

use crate::state::*;

/// What: Handle file resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `files`: File resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached files
/// - Syncs files to preflight modal if open
/// - Respects cancellation flag
pub fn handle_file_result(
    app: &mut AppState,
    files: Vec<crate::state::modal::PackageFileInfo>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_files_resolving;
    app.files_resolving = false;
    app.preflight_files_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached files
        tracing::info!(
            stage = "files",
            result_count = files.len(),
            "[Runtime] File resolution worker completed"
        );
        tracing::debug!(
            "[Runtime] handle_file_result: Updating install_list_files with {} entries (current cache has {})",
            files.len(),
            app.install_list_files.len()
        );
        for file_info in &files {
            tracing::info!(
                "[Runtime] handle_file_result: Package '{}' - total={}, new={}, changed={}, removed={}, config={}, pacnew={}, pacsave={}",
                file_info.name,
                file_info.total_count,
                file_info.new_count,
                file_info.changed_count,
                file_info.removed_count,
                file_info.config_count,
                file_info.pacnew_candidates,
                file_info.pacsave_candidates
            );
        }
        app.install_list_files = files.clone();
        tracing::debug!(
            "[Runtime] handle_file_result: install_list_files now has {} entries: {:?}",
            app.install_list_files.len(),
            app.install_list_files
                .iter()
                .map(|f| &f.name)
                .collect::<Vec<_>>()
        );
        // Sync files to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items, file_info, ..
        } = &mut app.modal
        {
            // Filter files to only those for current modal items
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let filtered_files: Vec<_> = files
                .iter()
                .filter(|file_info| item_names.contains(&file_info.name))
                .cloned()
                .collect();
            tracing::info!(
                "[Runtime] handle_file_result: Modal open - items={}, filtered_files={}, modal_current={}",
                items.len(),
                filtered_files.len(),
                file_info.len()
            );
            if !filtered_files.is_empty() {
                tracing::info!(
                    "[Runtime] handle_file_result: Syncing {} file infos to preflight modal (was_preflight={})",
                    filtered_files.len(),
                    was_preflight
                );
                *file_info = filtered_files;
                tracing::debug!(
                    "[Runtime] handle_file_result: Successfully synced file info to modal, modal now has {} entries",
                    file_info.len()
                );
            } else {
                tracing::debug!(
                    "[Runtime] handle_file_result: No matching files to sync. Modal items: {:?}, File packages: {:?}",
                    item_names,
                    files.iter().map(|f| &f.name).collect::<Vec<_>>()
                );
            }
        } else {
            tracing::debug!(
                "[Runtime] handle_file_result: Preflight modal not open, skipping sync"
            );
        }
        if was_preflight {
            app.preflight_files_items = None;
        }
        app.files_cache_dirty = true; // Mark cache as dirty for persistence
        tracing::debug!(
            "[Runtime] handle_file_result: Marked files_cache_dirty=true, install_list_files has {} entries: {:?}",
            app.install_list_files.len(),
            app.install_list_files
                .iter()
                .map(|f| &f.name)
                .collect::<Vec<_>>()
        );
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring file result (preflight cancelled)");
        app.preflight_files_items = None;
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
    /// What: Verify that handle_file_result updates cache correctly.
    ///
    /// Inputs:
    /// - App state
    /// - File resolution results
    ///
    /// Output:
    /// - Files are cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that file results are properly processed
    fn handle_file_result_updates_cache() {
        let mut app = new_app();
        app.files_resolving = true;

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let files = vec![crate::state::modal::PackageFileInfo {
            name: "test-package".to_string(),
            files: vec![],
            total_count: 0,
            new_count: 0,
            changed_count: 0,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        }];

        handle_file_result(&mut app, files.clone(), &tick_tx);

        // Files should be cached
        assert_eq!(app.install_list_files.len(), 1);
        // Flags should be reset
        assert!(!app.files_resolving);
        assert!(!app.preflight_files_resolving);
        // Cache dirty flag should be set
        assert!(app.files_cache_dirty);
    }
}
