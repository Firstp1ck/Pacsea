//! Database synchronization functions for pacman file database.

use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

/// What: Retrieve the most recent modification timestamp of the pacman sync database.
///
/// Inputs:
/// - (none): Reads metadata from `/var/lib/pacman/sync` on the local filesystem.
///
/// Output:
/// - Returns the latest `SystemTime` seen among `.files` databases, or `None` if unavailable.
///
/// Details:
/// - Inspects only files ending with the `.files` extension to match pacman's file list databases.
pub fn get_file_db_sync_timestamp() -> Option<SystemTime> {
    // Check modification time of pacman sync database files
    // The sync database files are in /var/lib/pacman/sync/
    let sync_dir = Path::new("/var/lib/pacman/sync");

    if !sync_dir.exists() {
        tracing::debug!("Pacman sync directory does not exist");
        return None;
    }

    // Get the most recent modification time from any .files database
    let mut latest_time: Option<SystemTime> = None;

    if let Ok(entries) = std::fs::read_dir(sync_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Look for .files database files (e.g., core.files, extra.files)
            if path.extension().and_then(|s| s.to_str()) == Some("files")
                && let Ok(metadata) = std::fs::metadata(&path)
                && let Ok(modified) = metadata.modified()
            {
                latest_time = Some(latest_time.map_or(modified, |prev| {
                    if modified > prev { modified } else { prev }
                }));
            }
        }
    }

    latest_time
}

/// What: Summarize sync database staleness with age, formatted date, and UI color bucket.
///
/// Inputs:
/// - (none): Uses `get_file_db_sync_timestamp` to determine the last sync.
///
/// Output:
/// - Returns `(age_days, formatted_date, color_category)` or `None` when the timestamp cannot be read.
///
/// Details:
/// - Buckets age into three categories: green (<7 days), yellow (<30 days), red (>=30 days).
#[must_use]
pub fn get_file_db_sync_info() -> Option<(u64, String, u8)> {
    let sync_time = get_file_db_sync_timestamp()?;

    let now = SystemTime::now();
    let age = now.duration_since(sync_time).ok()?;
    let age_days = age.as_secs() / 86400; // Convert to days

    // Format date
    let date_str = crate::util::ts_to_date(
        sync_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64),
    );

    // Determine color category
    let color_category = if age_days < 7 {
        0 // Green (< week)
    } else if age_days < 30 {
        1 // Yellow (< month)
    } else {
        2 // Red (>= month)
    };

    Some((age_days, date_str, color_category))
}

/// What: Check if the pacman file database is stale and needs syncing.
///
/// Inputs:
/// - `max_age_days`: Maximum age in days before considering the database stale.
///
/// Output:
/// - Returns `Some(true)` if stale, `Some(false)` if fresh, `None` if timestamp cannot be determined.
///
/// Details:
/// - Uses `get_file_db_sync_timestamp()` to check the last sync time.
#[must_use]
pub fn is_file_db_stale(max_age_days: u64) -> Option<bool> {
    let sync_time = get_file_db_sync_timestamp()?;
    let now = SystemTime::now();
    let age = now.duration_since(sync_time).ok()?;
    let age_days = age.as_secs() / 86400;
    Some(age_days >= max_age_days)
}

/// What: Attempt a best-effort synchronization of the pacman file database.
///
/// Inputs:
/// - `force`: If true, sync regardless of timestamp. If false, only sync if stale.
/// - `max_age_days`: Maximum age in days before considering the database stale (default: 7).
///
/// Output:
/// - Returns `Ok(true)` if sync was performed, `Ok(false)` if sync was skipped (fresh DB), `Err` if sync failed.
///
/// Details:
/// - Checks timestamp first if `force` is false, only syncing when stale.
/// - Intended to reduce false negatives when later querying remote file lists.
pub fn ensure_file_db_synced(force: bool, max_age_days: u64) -> Result<bool, String> {
    // Check if we need to sync
    if force {
        tracing::debug!("Force syncing pacman file database...");
    } else {
        if let Some(is_stale) = is_file_db_stale(max_age_days) {
            if is_stale {
                tracing::debug!(
                    "File database is stale (older than {} days), syncing...",
                    max_age_days
                );
            } else {
                tracing::debug!("File database is fresh, skipping sync");
                return Ok(false);
            }
        } else {
            // Can't determine timestamp, try to sync anyway
            tracing::debug!("Cannot determine file database timestamp, attempting sync...");
        }
    }

    let output = Command::new("pacman")
        .args(["-Fy"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| format!("Failed to execute pacman -Fy: {e}"))?;

    if output.status.success() {
        tracing::debug!("File database sync successful");
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error_msg = format!("File database sync failed: {stderr}");
        tracing::warn!("{}", error_msg);
        Err(error_msg)
    }
}
