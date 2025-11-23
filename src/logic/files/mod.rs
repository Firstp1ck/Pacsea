//! File list resolution and diff computation for preflight checks.

mod backup;
mod db_sync;
mod lists;
mod pkgbuild_fetch;
mod pkgbuild_parse;
mod resolution;

pub use backup::{get_backup_files, get_backup_files_from_installed};
pub use db_sync::{
    ensure_file_db_synced, get_file_db_sync_info, get_file_db_sync_timestamp, is_file_db_stale,
};
pub use lists::{get_installed_file_list, get_remote_file_list};
pub use pkgbuild_fetch::{fetch_pkgbuild_sync, fetch_srcinfo_sync, get_pkgbuild_from_cache};
pub use pkgbuild_parse::{
    parse_backup_array_content, parse_backup_from_pkgbuild, parse_backup_from_srcinfo,
    parse_install_paths_from_pkgbuild,
};
pub use resolution::{
    batch_get_remote_file_lists, resolve_install_files, resolve_install_files_with_remote_list,
    resolve_package_files, resolve_remove_files,
};

use crate::state::modal::PackageFileInfo;
use crate::state::types::PackageItem;

/// What: Determine file-level changes for a set of packages under a specific preflight action.
///
/// Inputs:
/// - `items`: Package descriptors under consideration.
/// - `action`: Preflight action (install or remove) influencing the comparison strategy.
///
/// Output:
/// - Returns a vector of `PackageFileInfo` entries describing per-package file deltas.
///
/// Details:
/// - Invokes pacman commands to compare remote and installed file lists while preserving package order.
#[allow(clippy::missing_const_for_fn)]
pub fn resolve_file_changes(
    items: &[PackageItem],
    action: crate::state::modal::PreflightAction,
) -> Vec<PackageFileInfo> {
    // Check if file database is stale, but don't force sync (let user decide)
    // Only sync if database doesn't exist or is very old (>30 days)
    const MAX_AUTO_SYNC_AGE_DAYS: u64 = 30;
    let _span = tracing::info_span!(
        "resolve_file_changes",
        stage = "files",
        item_count = items.len()
    )
    .entered();
    let start_time = std::time::Instant::now();

    if items.is_empty() {
        tracing::warn!("No packages provided for file resolution");
        return Vec::new();
    }
    match ensure_file_db_synced(false, MAX_AUTO_SYNC_AGE_DAYS) {
        Ok(synced) => {
            if synced {
                tracing::info!("File database was synced automatically (was very stale)");
            } else {
                tracing::debug!("File database is fresh, no sync needed");
            }
        }
        Err(e) => {
            // Sync failed (likely requires root), but continue anyway
            tracing::warn!("File database sync failed: {} (continuing without sync)", e);
        }
    }

    // Batch fetch remote file lists for all official packages to reduce pacman command overhead
    let official_packages: Vec<(&str, &crate::state::types::Source)> = items
        .iter()
        .filter_map(|item| {
            if matches!(item.source, crate::state::types::Source::Official { .. }) {
                Some((item.name.as_str(), &item.source))
            } else {
                None
            }
        })
        .collect();
    let batched_remote_files_cache = if !official_packages.is_empty()
        && matches!(action, crate::state::modal::PreflightAction::Install)
    {
        resolution::batch_get_remote_file_lists(&official_packages)
    } else {
        std::collections::HashMap::new()
    };

    let mut results = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        tracing::info!(
            "[{}/{}] Resolving files for package: {} ({:?})",
            idx + 1,
            items.len(),
            item.name,
            item.source
        );

        // Check if we have batched results for this official package
        let use_batched = matches!(action, crate::state::modal::PreflightAction::Install)
            && matches!(item.source, crate::state::types::Source::Official { .. })
            && batched_remote_files_cache.contains_key(item.name.as_str());

        match if use_batched {
            // Use batched file list
            let remote_files = batched_remote_files_cache
                .get(item.name.as_str())
                .cloned()
                .unwrap_or_default();
            resolution::resolve_install_files_with_remote_list(
                &item.name,
                &item.source,
                remote_files,
            )
        } else {
            resolution::resolve_package_files(&item.name, &item.source, action)
        } {
            Ok(file_info) => {
                tracing::info!(
                    "  Found {} files for {} ({} new, {} changed, {} removed)",
                    file_info.total_count,
                    item.name,
                    file_info.new_count,
                    file_info.changed_count,
                    file_info.removed_count
                );
                results.push(file_info);
            }
            Err(e) => {
                tracing::warn!("  Failed to resolve files for {}: {}", item.name, e);
                // Create empty entry to maintain package order
                results.push(PackageFileInfo {
                    name: item.name.clone(),
                    files: Vec::new(),
                    total_count: 0,
                    new_count: 0,
                    changed_count: 0,
                    removed_count: 0,
                    config_count: 0,
                    pacnew_candidates: 0,
                    pacsave_candidates: 0,
                });
            }
        }
    }

    let elapsed = start_time.elapsed();
    let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        stage = "files",
        item_count = items.len(),
        result_count = results.len(),
        duration_ms = duration_ms,
        "File resolution complete"
    );
    results
}

#[cfg(all(test, unix))]
mod tests;
