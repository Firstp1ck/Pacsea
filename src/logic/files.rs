//! File list resolution and diff computation for preflight checks.

use crate::state::modal::{FileChange, FileChangeType, PackageFileInfo};
use crate::state::types::{PackageItem, Source};
use crate::util::{curl_args, percent_encode};
use std::collections::HashSet;
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
pub fn resolve_file_changes(
    items: &[PackageItem],
    action: crate::state::modal::PreflightAction,
) -> Vec<PackageFileInfo> {
    tracing::info!(
        "Starting file resolution for {} package(s) ({:?})",
        items.len(),
        action
    );

    if items.is_empty() {
        tracing::warn!("No packages provided for file resolution");
        return Vec::new();
    }

    // Check if file database is stale, but don't force sync (let user decide)
    // Only sync if database doesn't exist or is very old (>30 days)
    const MAX_AUTO_SYNC_AGE_DAYS: u64 = 30;
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

    let mut results = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        tracing::info!(
            "[{}/{}] Resolving files for package: {} ({:?})",
            idx + 1,
            items.len(),
            item.name,
            item.source
        );

        match resolve_package_files(&item.name, &item.source, action) {
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

    tracing::info!(
        "File resolution complete. Returning {} package file infos",
        results.len()
    );
    results
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
    if !force {
        if let Some(is_stale) = is_file_db_stale(max_age_days) {
            if !is_stale {
                tracing::debug!("File database is fresh, skipping sync");
                return Ok(false);
            }
            tracing::debug!(
                "File database is stale (older than {} days), syncing...",
                max_age_days
            );
        } else {
            // Can't determine timestamp, try to sync anyway
            tracing::debug!("Cannot determine file database timestamp, attempting sync...");
        }
    } else {
        tracing::debug!("Force syncing pacman file database...");
    }

    let output = Command::new("pacman")
        .args(["-Fy"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| format!("Failed to execute pacman -Fy: {}", e))?;

    if output.status.success() {
        tracing::debug!("File database sync successful");
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error_msg = format!("File database sync failed: {}", stderr);
        tracing::warn!("{}", error_msg);
        Err(error_msg)
    }
}

/// What: Dispatch to the correct file resolution routine based on preflight action.
///
/// Inputs:
/// - `name`: Package name being evaluated.
/// - `source`: Package source needed for install lookups.
/// - `action`: Whether the package is being installed or removed.
///
/// Output:
/// - Returns a `PackageFileInfo` on success or an error message.
///
/// Details:
/// - Delegates to either `resolve_install_files` or `resolve_remove_files`.
fn resolve_package_files(
    name: &str,
    source: &Source,
    action: crate::state::modal::PreflightAction,
) -> Result<PackageFileInfo, String> {
    match action {
        crate::state::modal::PreflightAction::Install => resolve_install_files(name, source),
        crate::state::modal::PreflightAction::Remove => resolve_remove_files(name),
    }
}

/// What: Determine new and changed files introduced by installing or upgrading a package.
///
/// Inputs:
/// - `name`: Package name examined.
/// - `source`: Source repository information for remote lookups.
///
/// Output:
/// - Returns a populated `PackageFileInfo` or an error when file lists cannot be retrieved.
///
/// Details:
/// - Compares remote file listings with locally installed files and predicts potential `.pacnew` creations.
fn resolve_install_files(name: &str, source: &Source) -> Result<PackageFileInfo, String> {
    // Get remote file list
    let remote_files = get_remote_file_list(name, source)?;

    // Get installed file list (if package is already installed)
    let installed_files = get_installed_file_list(name).unwrap_or_default();

    let installed_set: HashSet<&str> = installed_files.iter().map(|s| s.as_str()).collect();

    let mut file_changes = Vec::new();
    let mut new_count = 0;
    let mut changed_count = 0;
    let mut config_count = 0;
    let mut pacnew_candidates = 0;

    // Get backup files for this package (for pacnew/pacsave prediction)
    let backup_files = get_backup_files(name, source).unwrap_or_default();
    let backup_set: HashSet<&str> = backup_files.iter().map(|s| s.as_str()).collect();

    for path in remote_files {
        let is_config = path.starts_with("/etc/");
        let is_dir = path.ends_with('/');

        // Skip directories for now (we can add them later if needed)
        if is_dir {
            continue;
        }

        let change_type = if installed_set.contains(path.as_str()) {
            changed_count += 1;
            FileChangeType::Changed
        } else {
            new_count += 1;
            FileChangeType::New
        };

        if is_config {
            config_count += 1;
        }

        // Predict pacnew: file is in backup array and exists (will be changed)
        let predicted_pacnew = backup_set.contains(path.as_str())
            && installed_set.contains(path.as_str())
            && is_config;

        if predicted_pacnew {
            pacnew_candidates += 1;
        }

        file_changes.push(FileChange {
            path,
            change_type,
            package: name.to_string(),
            is_config,
            predicted_pacnew,
            predicted_pacsave: false, // Only for remove operations
        });
    }

    // Sort files by path for consistent display
    file_changes.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(PackageFileInfo {
        name: name.to_string(),
        files: file_changes,
        total_count: new_count + changed_count,
        new_count,
        changed_count,
        removed_count: 0,
        config_count,
        pacnew_candidates,
        pacsave_candidates: 0,
    })
}

/// What: Enumerate files that would be removed when uninstalling a package.
///
/// Inputs:
/// - `name`: Package scheduled for removal.
///
/// Output:
/// - Returns a `PackageFileInfo` capturing removed files and predicted `.pacsave` candidates.
///
/// Details:
/// - Reads installed file lists and backup arrays to flag configuration files requiring user attention.
fn resolve_remove_files(name: &str) -> Result<PackageFileInfo, String> {
    // Get installed file list
    let installed_files = get_installed_file_list(name)?;

    let mut file_changes = Vec::new();
    let mut config_count = 0;
    let mut pacsave_candidates = 0;

    // Get backup files for this package (for pacsave prediction)
    let backup_files = get_backup_files(
        name,
        &Source::Official {
            repo: String::new(),
            arch: String::new(),
        },
    )
    .unwrap_or_default();
    let backup_set: HashSet<&str> = backup_files.iter().map(|s| s.as_str()).collect();

    for path in installed_files {
        let is_config = path.starts_with("/etc/");
        let is_dir = path.ends_with('/');

        // Skip directories for now
        if is_dir {
            continue;
        }

        if is_config {
            config_count += 1;
        }

        // Predict pacsave: file is in backup array and will be removed
        let predicted_pacsave = backup_set.contains(path.as_str()) && is_config;

        if predicted_pacsave {
            pacsave_candidates += 1;
        }

        file_changes.push(FileChange {
            path,
            change_type: FileChangeType::Removed,
            package: name.to_string(),
            is_config,
            predicted_pacnew: false,
            predicted_pacsave,
        });
    }

    // Sort files by path for consistent display
    file_changes.sort_by(|a, b| a.path.cmp(&b.path));

    let removed_count = file_changes.len();

    Ok(PackageFileInfo {
        name: name.to_string(),
        files: file_changes,
        total_count: removed_count,
        new_count: 0,
        changed_count: 0,
        removed_count,
        config_count,
        pacnew_candidates: 0,
        pacsave_candidates,
    })
}

/// What: Fetch the list of files published in repositories for a given package.
///
/// Inputs:
/// - `name`: Package name in question.
/// - `source`: Source descriptor differentiating official repositories from AUR packages.
///
/// Output:
/// - Returns the list of file paths or an error when retrieval fails.
///
/// Details:
/// - Uses `pacman -Fl` for official packages and currently returns an empty list for AUR entries.
fn get_remote_file_list(name: &str, source: &Source) -> Result<Vec<String>, String> {
    match source {
        Source::Official { repo, .. } => {
            // Use pacman -Fl to get remote file list
            // Note: This may fail if file database isn't synced, but we try anyway
            tracing::debug!("Running: pacman -Fl {}", name);
            let spec = if repo.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", repo, name)
            };

            let output = Command::new("pacman")
                .args(["-Fl", &spec])
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .output()
                .map_err(|e| {
                    tracing::error!("Failed to execute pacman -Fl {}: {}", spec, e);
                    format!("pacman -Fl failed: {}", e)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Check if error is due to missing file database
                if stderr.contains("database file") && stderr.contains("does not exist") {
                    tracing::warn!(
                        "File database not synced for {} (pacman -Fy requires root). Skipping file list.",
                        name
                    );
                    return Ok(Vec::new()); // Return empty instead of error
                }
                tracing::error!(
                    "pacman -Fl {} failed with status {:?}: {}",
                    spec,
                    output.status.code(),
                    stderr
                );
                return Err(format!("pacman -Fl failed for {}: {}", spec, stderr));
            }

            let text = String::from_utf8_lossy(&output.stdout);
            let mut files = Vec::new();

            // Parse pacman -Fl output: format is "<pkg> <path>"
            for line in text.lines() {
                if let Some((_pkg, path)) = line.split_once(' ') {
                    files.push(path.to_string());
                }
            }

            tracing::debug!("Found {} files in remote package {}", files.len(), name);
            Ok(files)
        }
        Source::Aur => {
            // First, check if package is already installed
            if let Ok(installed_files) = get_installed_file_list(name)
                && !installed_files.is_empty()
            {
                tracing::debug!(
                    "Found {} files from installed AUR package {}",
                    installed_files.len(),
                    name
                );
                return Ok(installed_files);
            }

            // Try to use paru/yay -Fl if available (works for cached AUR packages)
            let has_paru = Command::new("paru").args(["--version"]).output().is_ok();
            let has_yay = Command::new("yay").args(["--version"]).output().is_ok();

            if has_paru {
                tracing::debug!("Trying paru -Fl {} for AUR package file list", name);
                if let Ok(output) = Command::new("paru")
                    .args(["-Fl", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let mut files = Vec::new();
                    for line in text.lines() {
                        if let Some((_pkg, path)) = line.split_once(' ') {
                            files.push(path.to_string());
                        }
                    }
                    if !files.is_empty() {
                        tracing::debug!("Found {} files from paru -Fl for {}", files.len(), name);
                        return Ok(files);
                    }
                }
            }

            if has_yay {
                tracing::debug!("Trying yay -Fl {} for AUR package file list", name);
                if let Ok(output) = Command::new("yay")
                    .args(["-Fl", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let mut files = Vec::new();
                    for line in text.lines() {
                        if let Some((_pkg, path)) = line.split_once(' ') {
                            files.push(path.to_string());
                        }
                    }
                    if !files.is_empty() {
                        tracing::debug!("Found {} files from yay -Fl for {}", files.len(), name);
                        return Ok(files);
                    }
                }
            }

            // Fallback: try to parse PKGBUILD to extract install paths
            match fetch_pkgbuild_sync(name) {
                Ok(pkgbuild) => {
                    let files = parse_install_paths_from_pkgbuild(&pkgbuild, name);
                    if !files.is_empty() {
                        tracing::debug!(
                            "Found {} files from PKGBUILD parsing for {}",
                            files.len(),
                            name
                        );
                        return Ok(files);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch PKGBUILD for {}: {}", name, e);
                }
            }

            // No file list available
            tracing::debug!(
                "AUR package {}: file list not available (not installed, not cached, PKGBUILD parsing failed)",
                name
            );
            Ok(Vec::new())
        }
    }
}

/// What: Retrieve the list of files currently installed for a package.
///
/// Inputs:
/// - `name`: Package name queried via `pacman -Ql`.
///
/// Output:
/// - Returns file paths owned by the package or an empty list when it is not installed.
///
/// Details:
/// - Logs errors if the command fails for reasons other than the package being absent.
pub fn get_installed_file_list(name: &str) -> Result<Vec<String>, String> {
    tracing::debug!("Running: pacman -Ql {}", name);
    let output = Command::new("pacman")
        .args(["-Ql", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| {
            tracing::error!("Failed to execute pacman -Ql {}: {}", name, e);
            format!("pacman -Ql failed: {}", e)
        })?;

    if !output.status.success() {
        // Package not installed - this is OK for install operations
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("was not found") {
            tracing::debug!("Package {} is not installed", name);
            return Ok(Vec::new());
        }
        tracing::error!(
            "pacman -Ql {} failed with status {:?}: {}",
            name,
            output.status.code(),
            stderr
        );
        return Err(format!("pacman -Ql failed for {}: {}", name, stderr));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();

    // Parse pacman -Ql output: format is "<pkg> <path>"
    for line in text.lines() {
        if let Some((_pkg, path)) = line.split_once(' ') {
            files.push(path.to_string());
        }
    }

    tracing::debug!("Found {} files in installed package {}", files.len(), name);
    Ok(files)
}

/// What: Identify files marked for backup handling during install or removal operations.
///
/// Inputs:
/// - `name`: Package whose backup array should be inspected.
/// - `source`: Source descriptor to decide how to gather backup information.
///
/// Output:
/// - Returns a list of backup file paths or an empty list when the data cannot be retrieved.
///
/// Details:
/// - Prefers querying the installed package via `pacman -Qii`; falls back to best-effort heuristics.
fn get_backup_files(name: &str, source: &Source) -> Result<Vec<String>, String> {
    // First try: if package is installed, use pacman -Qii
    if let Ok(backup_files) = get_backup_files_from_installed(name)
        && !backup_files.is_empty()
    {
        tracing::debug!(
            "Found {} backup files from installed package {}",
            backup_files.len(),
            name
        );
        return Ok(backup_files);
    }

    // Second try: parse from PKGBUILD/.SRCINFO (best-effort, may fail)
    match source {
        Source::Official { .. } => {
            // Try to fetch PKGBUILD and parse backup array
            match fetch_pkgbuild_sync(name) {
                Ok(pkgbuild) => {
                    let backup_files = parse_backup_from_pkgbuild(&pkgbuild);
                    if !backup_files.is_empty() {
                        tracing::debug!(
                            "Found {} backup files from PKGBUILD for {}",
                            backup_files.len(),
                            name
                        );
                        return Ok(backup_files);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch PKGBUILD for {}: {}", name, e);
                }
            }
            Ok(Vec::new())
        }
        Source::Aur => {
            // Try to fetch .SRCINFO first (more reliable for AUR)
            match fetch_srcinfo_sync(name) {
                Ok(srcinfo) => {
                    let backup_files = parse_backup_from_srcinfo(&srcinfo);
                    if !backup_files.is_empty() {
                        tracing::debug!(
                            "Found {} backup files from .SRCINFO for {}",
                            backup_files.len(),
                            name
                        );
                        return Ok(backup_files);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch .SRCINFO for {}: {}", name, e);
                }
            }
            // Fallback to PKGBUILD if .SRCINFO failed
            match fetch_pkgbuild_sync(name) {
                Ok(pkgbuild) => {
                    let backup_files = parse_backup_from_pkgbuild(&pkgbuild);
                    if !backup_files.is_empty() {
                        tracing::debug!(
                            "Found {} backup files from PKGBUILD for {}",
                            backup_files.len(),
                            name
                        );
                        return Ok(backup_files);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch PKGBUILD for {}: {}", name, e);
                }
            }
            Ok(Vec::new())
        }
    }
}

/// What: Collect backup file entries for an installed package through `pacman -Qii`.
///
/// Inputs:
/// - `name`: Installed package identifier.
///
/// Output:
/// - Returns the backup array as a vector of file paths or an empty list when not installed.
///
/// Details:
/// - Parses the `Backup Files` section, handling wrapped lines to ensure complete coverage.
fn get_backup_files_from_installed(name: &str) -> Result<Vec<String>, String> {
    tracing::debug!("Running: pacman -Qii {}", name);
    let output = Command::new("pacman")
        .args(["-Qii", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| {
            tracing::error!("Failed to execute pacman -Qii {}: {}", name, e);
            format!("pacman -Qii failed: {}", e)
        })?;

    if !output.status.success() {
        // Package not installed - this is OK
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("was not found") {
            tracing::debug!("Package {} is not installed", name);
            return Ok(Vec::new());
        }
        tracing::error!(
            "pacman -Qii {} failed with status {:?}: {}",
            name,
            output.status.code(),
            stderr
        );
        return Err(format!("pacman -Qii failed for {}: {}", name, stderr));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut backup_files = Vec::new();
    let mut in_backup_section = false;

    // Parse pacman -Qii output: look for "Backup Files" field
    for line in text.lines() {
        if line.starts_with("Backup Files") {
            in_backup_section = true;
            // Extract files from the same line if present
            if let Some(colon_pos) = line.find(':') {
                let files_str = line[colon_pos + 1..].trim();
                if !files_str.is_empty() && files_str != "None" {
                    for file in files_str.split_whitespace() {
                        backup_files.push(file.to_string());
                    }
                }
            }
        } else if in_backup_section {
            // Continuation lines (indented)
            if line.starts_with("    ") || line.starts_with("\t") {
                for file in line.split_whitespace() {
                    backup_files.push(file.to_string());
                }
            } else {
                // End of backup section
                break;
            }
        }
    }

    tracing::debug!(
        "Found {} backup files for installed package {}",
        backup_files.len(),
        name
    );
    Ok(backup_files)
}

/// What: Fetch PKGBUILD content synchronously (blocking).
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Returns PKGBUILD content as a string, or an error if fetch fails.
///
/// Details:
/// - Uses curl to fetch PKGBUILD from AUR or official GitLab repos.
pub fn fetch_pkgbuild_sync(name: &str) -> Result<String, String> {
    // Try AUR first (works for both AUR and official packages via AUR mirror)
    let url_aur = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching PKGBUILD from AUR: {}", url_aur);

    let args = curl_args(&url_aur, &[]);
    let output = Command::new("curl").args(&args).output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if !text.trim().is_empty() && text.contains("pkgname") {
                return Ok(text);
            }
        }
        _ => {}
    }

    // Fallback to official GitLab repos
    let url_main = format!(
        "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/main/PKGBUILD",
        percent_encode(name)
    );
    tracing::debug!("Fetching PKGBUILD from GitLab main: {}", url_main);

    let args = curl_args(&url_main, &[]);
    let output = Command::new("curl").args(&args).output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
        _ => {}
    }

    // Try master branch as fallback
    let url_master = format!(
        "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/master/PKGBUILD",
        percent_encode(name)
    );
    tracing::debug!("Fetching PKGBUILD from GitLab master: {}", url_master);

    let args = curl_args(&url_master, &[]);
    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "curl failed with status: {:?}",
            output.status.code()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        return Err("Empty PKGBUILD content".to_string());
    }

    Ok(text)
}

/// What: Fetch .SRCINFO content synchronously (blocking).
///
/// Inputs:
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
///
/// Details:
/// - Downloads .SRCINFO from AUR cgit repository.
fn fetch_srcinfo_sync(name: &str) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let args = curl_args(&url, &[]);
    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "curl failed with status: {:?}",
            output.status.code()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    Ok(text)
}

/// What: Parse backup array from PKGBUILD content.
///
/// Inputs:
/// - `pkgbuild`: Raw PKGBUILD file content.
///
/// Output:
/// - Returns a vector of backup file paths.
///
/// Details:
/// - Parses bash array syntax: `backup=('file1' 'file2' '/etc/config')`
/// - Handles single-line and multi-line array definitions.
fn parse_backup_from_pkgbuild(pkgbuild: &str) -> Vec<String> {
    let mut backup_files = Vec::new();
    let mut in_backup_array = false;
    let mut current_line = String::new();

    for line in pkgbuild.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Look for backup= array declaration
        if line.starts_with("backup=") || line.starts_with("backup =") {
            in_backup_array = true;
            current_line = line.to_string();

            // Check if array is on single line: backup=('file1' 'file2')
            if let Some(start) = line.find('(')
                && let Some(end) = line.rfind(')')
            {
                let array_content = &line[start + 1..end];
                parse_backup_array_content(array_content, &mut backup_files);
                in_backup_array = false;
                current_line.clear();
            } else if line.contains('(') {
                // Multi-line array starting
                if let Some(start) = line.find('(') {
                    let array_content = &line[start + 1..];
                    parse_backup_array_content(array_content, &mut backup_files);
                }
            }
        } else if in_backup_array {
            // Continuation of multi-line array
            current_line.push(' ');
            current_line.push_str(line);

            // Check if array ends
            if line.contains(')') {
                if let Some(end) = line.rfind(')') {
                    let remaining = &line[..end];
                    parse_backup_array_content(remaining, &mut backup_files);
                }
                in_backup_array = false;
                current_line.clear();
            } else {
                // Still in array, parse this line
                parse_backup_array_content(line, &mut backup_files);
            }
        }
    }

    backup_files
}

/// What: Parse backup array content (handles quoted strings).
///
/// Inputs:
/// - `content`: String content containing quoted file paths.
/// - `backup_files`: Vector to append parsed file paths to.
///
/// Details:
/// - Extracts quoted strings (single or double quotes) from array content.
fn parse_backup_array_content(content: &str, backup_files: &mut Vec<String>) {
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut current_file = String::new();

    for ch in content.chars() {
        match ch {
            '\'' | '"' => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if ch == quote_char {
                    // End of quoted string
                    if !current_file.is_empty() {
                        backup_files.push(current_file.clone());
                        current_file.clear();
                    }
                    in_quotes = false;
                    quote_char = '\0';
                } else {
                    // Different quote type, treat as part of string
                    current_file.push(ch);
                }
            }
            _ if in_quotes => {
                current_file.push(ch);
            }
            _ => {
                // Skip whitespace and other characters outside quotes
            }
        }
    }

    // Handle unclosed quote (edge case)
    if !current_file.is_empty() && in_quotes {
        backup_files.push(current_file);
    }
}

/// What: Parse backup array from .SRCINFO content.
///
/// Inputs:
/// - `srcinfo`: Raw .SRCINFO file content.
///
/// Output:
/// - Returns a vector of backup file paths.
///
/// Details:
/// - Parses key-value pairs: `backup = file1`
/// - Handles multiple backup entries.
fn parse_backup_from_srcinfo(srcinfo: &str) -> Vec<String> {
    let mut backup_files = Vec::new();

    for line in srcinfo.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // .SRCINFO format: backup = file_path
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == "backup" && !value.is_empty() {
                backup_files.push(value.to_string());
            }
        }
    }

    backup_files
}

/// What: Parse install paths from PKGBUILD content.
///
/// Inputs:
/// - `pkgbuild`: Raw PKGBUILD file content.
/// - `pkgname`: Package name (used for default install paths).
///
/// Output:
/// - Returns a vector of file paths that would be installed.
///
/// Details:
/// - Parses `package()` functions and `install` scripts to extract file paths.
/// - Handles common patterns like `install -Dm755`, `cp`, `mkdir -p`, etc.
/// - Extracts paths from `package()` functions that use `install` commands.
/// - This is a best-effort heuristic and may not capture all files.
pub fn parse_install_paths_from_pkgbuild(pkgbuild: &str, pkgname: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut in_package_function = false;
    let mut package_function_depth = 0;

    for line in pkgbuild.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Detect package() function start
        if trimmed.starts_with("package()") || trimmed.starts_with("package_") {
            in_package_function = true;
            package_function_depth = 0;
            continue;
        }

        // Track function depth (handle nested functions)
        if in_package_function {
            if trimmed.contains('{') {
                package_function_depth += trimmed.matches('{').count();
            }
            if trimmed.contains('}') {
                let closing_count = trimmed.matches('}').count();
                if package_function_depth >= closing_count {
                    package_function_depth -= closing_count;
                } else {
                    package_function_depth = 0;
                }
                if package_function_depth == 0 {
                    in_package_function = false;
                    continue;
                }
            }

            // Parse install commands within package() function
            // Common patterns:
            // install -Dm755 "$srcdir/binary" "$pkgdir/usr/bin/binary"
            // install -Dm644 "$srcdir/config" "$pkgdir/etc/config"
            // cp -r "$srcdir/data" "$pkgdir/usr/share/app"

            if trimmed.contains("install") && trimmed.contains("$pkgdir") {
                // Extract destination path from install command
                // Pattern: install ... "$pkgdir/path/to/file"
                if let Some(pkgdir_pos) = trimmed.find("$pkgdir") {
                    let after_pkgdir = &trimmed[pkgdir_pos + 7..]; // Skip "$pkgdir"
                    // Find the path (may be quoted)
                    let path_start = after_pkgdir
                        .chars()
                        .position(|c| c != ' ' && c != '/' && c != '"' && c != '\'')
                        .unwrap_or(0);
                    let path_part = &after_pkgdir[path_start..];

                    // Extract path until space, quote, or end
                    let path_end = path_part
                        .chars()
                        .position(|c| c == ' ' || c == '"' || c == '\'' || c == ';')
                        .unwrap_or(path_part.len());

                    let mut path = path_part[..path_end].to_string();
                    // Remove leading slash if present (we'll add it)
                    if path.starts_with('/') {
                        path.remove(0);
                    }
                    if !path.is_empty() {
                        files.push(format!("/{}", path));
                    }
                }
            } else if trimmed.contains("cp") && trimmed.contains("$pkgdir") {
                // Extract destination from cp command
                // Pattern: cp ... "$pkgdir/path/to/file"
                if let Some(pkgdir_pos) = trimmed.find("$pkgdir") {
                    let after_pkgdir = &trimmed[pkgdir_pos + 7..];
                    let path_start = after_pkgdir
                        .chars()
                        .position(|c| c != ' ' && c != '/' && c != '"' && c != '\'')
                        .unwrap_or(0);
                    let path_part = &after_pkgdir[path_start..];
                    let path_end = path_part
                        .chars()
                        .position(|c| c == ' ' || c == '"' || c == '\'' || c == ';')
                        .unwrap_or(path_part.len());

                    let mut path = path_part[..path_end].to_string();
                    if path.starts_with('/') {
                        path.remove(0);
                    }
                    if !path.is_empty() {
                        files.push(format!("/{}", path));
                    }
                }
            }
        }
    }

    // Remove duplicates and sort
    files.sort();
    files.dedup();

    // If we didn't find any files, try to infer common paths based on package name
    if files.is_empty() {
        // Common default paths for AUR packages
        files.push(format!("/usr/bin/{}", pkgname));
        files.push(format!("/usr/share/{}", pkgname));
    }

    files
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::state::modal::FileChangeType;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    struct PathGuard {
        original: Option<String>,
    }

    impl PathGuard {
        fn push(dir: &std::path::Path) -> Self {
            let original = std::env::var("PATH").ok();
            let mut new_path = dir.display().to_string();
            if let Some(ref orig) = original {
                new_path.push(':');
                new_path.push_str(orig);
            }
            unsafe {
                std::env::set_var("PATH", &new_path);
            }
            Self { original }
        }
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            if let Some(ref orig) = self.original {
                unsafe {
                    std::env::set_var("PATH", orig);
                }
            } else {
                unsafe {
                    std::env::remove_var("PATH");
                }
            }
        }
    }

    fn write_executable(dir: &std::path::Path, name: &str, body: &str) {
        let path = dir.join(name);
        let mut file = fs::File::create(&path).expect("create stub");
        file.write_all(body.as_bytes()).expect("write stub");
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod stub");
    }

    #[test]
    fn test_parse_backup_from_pkgbuild_single_line() {
        let pkgbuild = r#"
pkgname=test
pkgver=1.0
backup=('/etc/config' '/etc/other.conf')
"#;
        let backup_files = parse_backup_from_pkgbuild(pkgbuild);
        assert_eq!(backup_files.len(), 2);
        assert!(backup_files.contains(&"/etc/config".to_string()));
        assert!(backup_files.contains(&"/etc/other.conf".to_string()));
    }

    #[test]
    fn test_parse_backup_from_pkgbuild_multi_line() {
        let pkgbuild = r#"
pkgname=test
pkgver=1.0
backup=(
    '/etc/config'
    '/etc/other.conf'
    '/etc/more.conf'
)
"#;
        let backup_files = parse_backup_from_pkgbuild(pkgbuild);
        assert_eq!(backup_files.len(), 3);
        assert!(backup_files.contains(&"/etc/config".to_string()));
        assert!(backup_files.contains(&"/etc/other.conf".to_string()));
        assert!(backup_files.contains(&"/etc/more.conf".to_string()));
    }

    #[test]
    fn test_parse_backup_from_srcinfo() {
        let srcinfo = r#"
pkgbase = test-package
pkgname = test-package
pkgver = 1.0.0
backup = /etc/config
backup = /etc/other.conf
backup = /etc/more.conf
"#;
        let backup_files = parse_backup_from_srcinfo(srcinfo);
        assert_eq!(backup_files.len(), 3);
        assert!(backup_files.contains(&"/etc/config".to_string()));
        assert!(backup_files.contains(&"/etc/other.conf".to_string()));
        assert!(backup_files.contains(&"/etc/more.conf".to_string()));
    }

    #[test]
    fn test_parse_backup_array_content() {
        let content = "'/etc/config' '/etc/other.conf'";
        let mut backup_files = Vec::new();
        parse_backup_array_content(content, &mut backup_files);
        assert_eq!(backup_files.len(), 2);
        assert!(backup_files.contains(&"/etc/config".to_string()));
        assert!(backup_files.contains(&"/etc/other.conf".to_string()));
    }

    #[test]
    /// What: Resolve install file information using stubbed pacman output while verifying pacnew detection.
    ///
    /// Inputs:
    /// - Stub `pacman` script returning canned `-Fl`, `-Ql`, and `-Qii` outputs for package `pkg`.
    ///
    /// Output:
    /// - `resolve_install_files` reports one changed config file and one new regular file with pacnew prediction.
    ///
    /// Details:
    /// - Uses a temporary PATH override and the global test mutex to isolate command stubbing from other tests.
    fn resolve_install_files_marks_changed_and_new_entries() {
        let _test_guard = crate::logic::test_mutex().lock().unwrap();
        let dir = tempdir().expect("tempdir");
        let _path_guard = PathGuard::push(dir.path());
        write_executable(
            dir.path(),
            "pacman",
            r#"#!/bin/sh
if [ "$1" = "-Fl" ]; then
cat <<'EOF'
pkg /etc/app.conf
pkg /usr/share/doc/
pkg /usr/bin/newtool
EOF
exit 0
fi
if [ "$1" = "-Ql" ]; then
cat <<'EOF'
pkg /etc/app.conf
EOF
exit 0
fi
if [ "$1" = "-Qii" ]; then
cat <<'EOF'
Backup Files  : /etc/app.conf
EOF
exit 0
fi
if [ "$1" = "-Fy" ]; then
exit 0
fi
exit 1
"#,
        );

        let source = Source::Official {
            repo: "core".into(),
            arch: "x86_64".into(),
        };
        let info = super::resolve_install_files("pkg", &source).expect("install resolution");

        assert_eq!(info.total_count, 2);
        assert_eq!(info.new_count, 1);
        assert_eq!(info.changed_count, 1);
        assert_eq!(info.config_count, 1);
        assert_eq!(info.pacnew_candidates, 1);

        let mut paths: Vec<&str> = info.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["/etc/app.conf", "/usr/bin/newtool"]);

        let config_entry = info
            .files
            .iter()
            .find(|f| f.path == "/etc/app.conf")
            .expect("config entry");
        assert!(matches!(config_entry.change_type, FileChangeType::Changed));
        assert!(config_entry.predicted_pacnew);
        assert!(!config_entry.predicted_pacsave);

        let new_entry = info
            .files
            .iter()
            .find(|f| f.path == "/usr/bin/newtool")
            .expect("new entry");
        assert!(matches!(new_entry.change_type, FileChangeType::New));
        assert!(!new_entry.predicted_pacnew);
    }

    #[test]
    /// What: Resolve removal file information with stubbed pacman output to confirm pacsave predictions.
    ///
    /// Inputs:
    /// - Stub `pacman` script returning canned `-Ql` and `-Qii` outputs listing a config and regular file.
    ///
    /// Output:
    /// - `resolve_remove_files` reports both files as removed while flagging the config as a pacsave candidate.
    ///
    /// Details:
    /// - Shares the PATH guard helper to ensure the stubbed command remains isolated per test.
    fn resolve_remove_files_marks_pacsave_candidates() {
        let _test_guard = crate::logic::test_mutex().lock().unwrap();
        let dir = tempdir().expect("tempdir");
        let _path_guard = PathGuard::push(dir.path());
        write_executable(
            dir.path(),
            "pacman",
            r#"#!/bin/sh
if [ "$1" = "-Ql" ]; then
cat <<'EOF'
pkg /etc/app.conf
pkg /usr/bin/newtool
EOF
exit 0
fi
if [ "$1" = "-Qii" ]; then
cat <<'EOF'
Backup Files  : /etc/app.conf
EOF
exit 0
fi
if [ "$1" = "-Fy" ] || [ "$1" = "-Fl" ]; then
exit 0
fi
exit 1
"#,
        );

        let info = super::resolve_remove_files("pkg").expect("remove resolution");

        assert_eq!(info.removed_count, 2);
        assert_eq!(info.config_count, 1);
        assert_eq!(info.pacsave_candidates, 1);

        let config_entry = info
            .files
            .iter()
            .find(|f| f.path == "/etc/app.conf")
            .expect("config entry");
        assert!(config_entry.is_config);
        assert!(config_entry.predicted_pacsave);
        assert!(!config_entry.predicted_pacnew);

        let regular_entry = info
            .files
            .iter()
            .find(|f| f.path == "/usr/bin/newtool")
            .expect("regular entry");
        assert!(!regular_entry.is_config);
        assert!(!regular_entry.predicted_pacsave);
    }
}
