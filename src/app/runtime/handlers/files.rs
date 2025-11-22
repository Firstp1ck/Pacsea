use tokio::sync::mpsc;

use crate::app::runtime::handlers::common::{HandlerConfig, handle_result};
use crate::state::*;

/// What: Handler configuration for file results.
struct FileHandlerConfig;

impl HandlerConfig for FileHandlerConfig {
    type Result = crate::state::modal::PackageFileInfo;

    fn get_resolving(&self, app: &AppState) -> bool {
        app.files_resolving
    }

    fn set_resolving(&self, app: &mut AppState, value: bool) {
        app.files_resolving = value;
    }

    fn get_preflight_resolving(&self, app: &AppState) -> bool {
        app.preflight_files_resolving
    }

    fn set_preflight_resolving(&self, app: &mut AppState, value: bool) {
        app.preflight_files_resolving = value;
    }

    fn stage_name(&self) -> &'static str {
        "files"
    }

    fn update_cache(&self, app: &mut AppState, results: &[Self::Result]) {
        tracing::debug!(
            "[Runtime] handle_file_result: Updating install_list_files with {} entries (current cache has {})",
            results.len(),
            app.install_list_files.len()
        );
        for file_info in results {
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
        app.install_list_files = results.to_vec();
        tracing::debug!(
            "[Runtime] handle_file_result: install_list_files now has {} entries: {:?}",
            app.install_list_files.len(),
            app.install_list_files
                .iter()
                .map(|f| &f.name)
                .collect::<Vec<_>>()
        );
    }

    fn set_cache_dirty(&self, app: &mut AppState) {
        app.files_cache_dirty = true;
        tracing::debug!(
            "[Runtime] handle_file_result: Marked files_cache_dirty=true, install_list_files has {} entries: {:?}",
            app.install_list_files.len(),
            app.install_list_files
                .iter()
                .map(|f| &f.name)
                .collect::<Vec<_>>()
        );
    }

    fn clear_preflight_items(&self, app: &mut AppState) {
        app.preflight_files_items = None;
    }

    fn sync_to_modal(&self, app: &mut AppState, results: &[Self::Result], was_preflight: bool) {
        // Sync files to preflight modal if it's open (whether preflight or install list resolution)
        if let crate::state::Modal::Preflight {
            items, file_info, ..
        } = &mut app.modal
        {
            // Filter files to only those for current modal items
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let filtered_files: Vec<_> = results
                .iter()
                .filter(|file_info| item_names.contains(&file_info.name))
                .cloned()
                .collect();
            let old_files_len = file_info.len();
            tracing::info!(
                "[Runtime] handle_file_result: Modal open - items={}, filtered_files={}, modal_current={} (was {})",
                items.len(),
                filtered_files.len(),
                file_info.len(),
                old_files_len
            );
            if filtered_files.is_empty() {
                tracing::debug!(
                    "[Runtime] handle_file_result: No matching files to sync. Modal items: {:?}, File packages: {:?}",
                    item_names,
                    results.iter().map(|f| &f.name).collect::<Vec<_>>()
                );
            } else {
                tracing::info!(
                    "[Runtime] handle_file_result: Syncing {} file infos to preflight modal (was_preflight={}, modal had {} before)",
                    filtered_files.len(),
                    was_preflight,
                    old_files_len
                );
                // Merge new file data into existing entries instead of replacing
                // This preserves empty entries for packages that don't have data yet
                let filtered_files_map: std::collections::HashMap<String, _> = filtered_files
                    .iter()
                    .map(|f| (f.name.clone(), f.clone()))
                    .collect();

                // Update existing entries with new data, or add new entries
                for file_info_entry in file_info.iter_mut() {
                    if let Some(new_data) = filtered_files_map.get(&file_info_entry.name) {
                        let old_total = file_info_entry.total_count;
                        *file_info_entry = new_data.clone();
                        if old_total != new_data.total_count {
                            tracing::info!(
                                "[Runtime] handle_file_result: Updated modal entry for '{}' from total={} to total={}",
                                file_info_entry.name,
                                old_total,
                                new_data.total_count
                            );
                        }
                    }
                }

                // Add any new entries that weren't in the modal yet
                for new_file in &filtered_files {
                    if !file_info.iter().any(|f| f.name == new_file.name) {
                        file_info.push(new_file.clone());
                    }
                }

                tracing::info!(
                    "[Runtime] handle_file_result: Successfully synced file info to modal, modal now has {} entries (was {})",
                    file_info.len(),
                    old_files_len
                );
            }
        } else {
            tracing::debug!(
                "[Runtime] handle_file_result: Preflight modal not open, skipping sync"
            );
        }
    }

    fn log_flag_clear(&self, app: &AppState, was_preflight: bool, cancelled: bool) {
        tracing::debug!(
            "[Runtime] handle_file_result: Clearing flags - was_preflight={}, files_resolving={}, preflight_files_resolving={}, cancelled={}",
            was_preflight,
            self.get_resolving(app),
            app.preflight_files_resolving,
            cancelled
        );
    }

    fn is_resolution_complete(&self, app: &AppState, results: &[Self::Result]) -> bool {
        // Check if all items have file info
        // If preflight modal is open, check modal items
        if let crate::state::Modal::Preflight { items, .. } = &app.modal {
            let item_names: std::collections::HashSet<String> =
                items.iter().map(|i| i.name.clone()).collect();
            let result_names: std::collections::HashSet<String> =
                results.iter().map(|f| f.name.clone()).collect();
            let cache_names: std::collections::HashSet<String> = app
                .install_list_files
                .iter()
                .map(|f| f.name.clone())
                .collect();

            // Check if all items are in results or cache
            let all_have_data = item_names
                .iter()
                .all(|name| result_names.contains(name) || cache_names.contains(name));

            if !all_have_data {
                let missing: Vec<String> = item_names
                    .iter()
                    .filter(|name| !result_names.contains(*name) && !cache_names.contains(*name))
                    .cloned()
                    .collect();
                tracing::debug!(
                    "[Runtime] handle_file_result: Resolution incomplete - missing files for: {:?}",
                    missing
                );
            }

            return all_have_data;
        }

        // If no preflight modal, check install list items
        if let Some(ref install_items) = app.preflight_files_items {
            let item_names: std::collections::HashSet<String> =
                install_items.iter().map(|i| i.name.clone()).collect();
            let result_names: std::collections::HashSet<String> =
                results.iter().map(|f| f.name.clone()).collect();
            let cache_names: std::collections::HashSet<String> = app
                .install_list_files
                .iter()
                .map(|f| f.name.clone())
                .collect();

            // Check if all items are in results or cache
            let all_have_data = item_names
                .iter()
                .all(|name| result_names.contains(name) || cache_names.contains(name));

            if !all_have_data {
                let missing: Vec<String> = item_names
                    .iter()
                    .filter(|name| !result_names.contains(*name) && !cache_names.contains(*name))
                    .cloned()
                    .collect();
                tracing::debug!(
                    "[Runtime] handle_file_result: Resolution incomplete - missing files for: {:?}",
                    missing
                );
            }

            return all_have_data;
        }

        // No items to check, resolution is complete
        true
    }
}

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
    handle_result(app, &files, tick_tx, &FileHandlerConfig);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::new_app;

    #[test]
    /// What: Verify that `handle_file_result` updates cache correctly.
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

        handle_file_result(&mut app, files, &tick_tx);

        // Files should be cached
        assert_eq!(app.install_list_files.len(), 1);
        // Flags should be reset
        assert!(!app.files_resolving);
        assert!(!app.preflight_files_resolving);
        // Cache dirty flag should be set
        assert!(app.files_cache_dirty);
    }
}
