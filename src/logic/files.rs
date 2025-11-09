//! File list resolution and diff computation for preflight checks.

use crate::state::modal::{FileChange, FileChangeType, PackageFileInfo};
use crate::state::types::{PackageItem, Source};
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

    // Ensure file database is synced (best-effort, cache timestamp in future)
    ensure_file_db_synced();

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

/// What: Attempt a best-effort synchronization of the pacman file database.
///
/// Inputs:
/// - (none): Executes `pacman -Fy` with locale overrides.
///
/// Output:
/// - No return value; logs warnings when the sync fails.
///
/// Details:
/// - Intended to reduce false negatives when later querying remote file lists.
fn ensure_file_db_synced() {
    tracing::debug!("Ensuring pacman file database is synced...");
    let output = Command::new("pacman")
        .args(["-Fy"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                tracing::debug!("File database sync successful");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("File database sync failed: {}", stderr);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to execute pacman -Fy: {}", e);
        }
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
            // For AUR packages, we can't easily get file lists without building
            // For now, return empty list (can be enhanced later with PKGBUILD parsing)
            tracing::debug!(
                "AUR package {}: file list not available without build",
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
fn get_installed_file_list(name: &str) -> Result<Vec<String>, String> {
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
            // For official packages, we could fetch PKGBUILD, but that's expensive
            // For now, return empty (can be enhanced later)
            tracing::debug!(
                "Backup files for official package {}: not available without PKGBUILD fetch",
                name
            );
            Ok(Vec::new())
        }
        Source::Aur => {
            // For AUR packages, we could fetch .SRCINFO, but that's expensive
            // For now, return empty (can be enhanced later)
            tracing::debug!(
                "Backup files for AUR package {}: not available without .SRCINFO fetch",
                name
            );
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
