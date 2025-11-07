//! File list resolution and diff computation for preflight checks.

use crate::state::modal::{FileChange, FileChangeType, PackageFileInfo};
use crate::state::types::{PackageItem, Source};
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

/// Get the file database sync timestamp.
///
/// Returns the modification time of the pacman sync database files directory,
/// or None if it cannot be determined.
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

/// Get file database sync age and formatted date string.
///
/// Returns (age_days, formatted_date_string, color_category)
/// where color_category: 0 = green (< week), 1 = yellow (< month), 2 = red (>= month)
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

/// Resolve file changes for a list of packages.
///
/// This function queries pacman to determine which files will be added, changed, or removed
/// by comparing remote file lists with installed file lists.
///
/// Inputs:
/// - `items`: List of packages to install/remove
/// - `action`: Whether this is an Install or Remove operation
///
/// Output:
/// - Vector of `PackageFileInfo` with file changes for each package
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

/// Ensure the pacman file database is synced (best-effort).
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

/// Resolve file changes for a single package.
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

/// Resolve files for an install/update operation.
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

/// Resolve files for a remove operation.
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

/// Get the remote file list for a package.
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

/// Get the installed file list for a package.
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

/// Get backup files for a package (files that will create .pacnew/.pacsave).
///
/// For installed packages, uses `pacman -Qii` to read the backup array.
/// For remote packages, attempts to parse PKGBUILD/.SRCINFO backup array.
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

/// Get backup files from an installed package using `pacman -Qii`.
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
