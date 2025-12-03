//! File resolution functions for install and remove operations.

use super::backup::get_backup_files;
use super::lists::{get_installed_file_list, get_remote_file_list};
use crate::state::modal::{FileChange, FileChangeType, PackageFileInfo};
use crate::state::types::Source;
use std::collections::{HashMap, HashSet};
use std::process::Command;

/// What: Batch fetch remote file lists for multiple official packages using `pacman -Fl`.
///
/// Inputs:
/// - `packages`: Slice of (`package_name`, source) tuples for official packages.
///
/// Output:
/// - `HashMap` mapping package name to its remote file list.
///
/// Details:
/// - Batches queries into chunks of 50 to avoid command-line length limits.
/// - Parses multi-package `pacman -Fl` output (format: "<pkg> <path>" per line).
#[must_use]
pub fn batch_get_remote_file_lists(packages: &[(&str, &Source)]) -> HashMap<String, Vec<String>> {
    const BATCH_SIZE: usize = 50;
    let mut result_map = HashMap::new();

    // Group packages by repo to batch them together
    let mut repo_groups: HashMap<String, Vec<&str>> = HashMap::new();
    for (name, source) in packages {
        if let Source::Official { repo, .. } = source {
            let repo_key = if repo.is_empty() {
                String::new()
            } else {
                repo.clone()
            };
            repo_groups.entry(repo_key).or_default().push(name);
        }
    }

    for (repo, names) in repo_groups {
        for chunk in names.chunks(BATCH_SIZE) {
            let specs: Vec<String> = chunk
                .iter()
                .map(|name| {
                    if repo.is_empty() {
                        (*name).to_string()
                    } else {
                        format!("{repo}/{name}")
                    }
                })
                .collect();

            let mut args = vec!["-Fl"];
            args.extend(specs.iter().map(String::as_str));

            match Command::new("pacman")
                .args(&args)
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .output()
            {
                Ok(output) if output.status.success() => {
                    let text = String::from_utf8_lossy(&output.stdout);
                    // Parse pacman -Fl output: format is "<pkg> <path>"
                    // Group by package name
                    let mut pkg_files: HashMap<String, Vec<String>> = HashMap::new();
                    for line in text.lines() {
                        if let Some((pkg, path)) = line.split_once(' ') {
                            // Extract package name (remove repo prefix if present)
                            let pkg_name = if let Some((_, name)) = pkg.split_once('/') {
                                name
                            } else {
                                pkg
                            };
                            pkg_files
                                .entry(pkg_name.to_string())
                                .or_default()
                                .push(path.to_string());
                        }
                    }
                    result_map.extend(pkg_files);
                }
                _ => {
                    // If batch fails, fall back to individual queries (but don't do it here to avoid recursion)
                    // The caller will handle individual queries
                    break;
                }
            }
        }
    }
    result_map
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
/// # Errors
/// - Returns `Err` when file resolution fails for install or remove operations (see `resolve_install_files` and `resolve_remove_files`)
///
/// Details:
/// - Delegates to either `resolve_install_files` or `resolve_remove_files`.
pub fn resolve_package_files(
    name: &str,
    source: &Source,
    action: crate::state::modal::PreflightAction,
) -> Result<PackageFileInfo, String> {
    match action {
        crate::state::modal::PreflightAction::Install => resolve_install_files(name, source),
        crate::state::modal::PreflightAction::Remove => resolve_remove_files(name),
        crate::state::modal::PreflightAction::Downgrade => resolve_downgrade_files(name, source),
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
/// # Errors
/// - Returns `Err` when remote file list retrieval fails (see `get_remote_file_list`)
/// - Returns `Err` when installed file list retrieval fails (see `get_installed_file_list`)
///
/// Details:
/// - Compares remote file listings with locally installed files and predicts potential `.pacnew` creations.
pub fn resolve_install_files(name: &str, source: &Source) -> Result<PackageFileInfo, String> {
    // Get remote file list
    let remote_files = get_remote_file_list(name, source)?;
    resolve_install_files_with_remote_list(name, source, remote_files)
}

/// What: Determine new and changed files using a pre-fetched remote file list.
///
/// Inputs:
/// - `name`: Package name examined.
/// - `source`: Source repository information (for backup file lookup).
/// - `remote_files`: Pre-fetched remote file list.
///
/// Output:
/// - Returns a populated `PackageFileInfo`.
///
/// # Errors
/// - Returns `Err` when installed file list retrieval fails (see `get_installed_file_list`)
/// - Returns `Err` when backup files retrieval fails (see `get_backup_files`)
///
/// Details:
/// - Compares remote file listings with locally installed files and predicts potential `.pacnew` creations.
pub fn resolve_install_files_with_remote_list(
    name: &str,
    source: &Source,
    remote_files: Vec<String>,
) -> Result<PackageFileInfo, String> {
    // Get installed file list (if package is already installed)
    let installed_files = get_installed_file_list(name).unwrap_or_default();

    let installed_set: HashSet<&str> = installed_files.iter().map(String::as_str).collect();

    let mut file_changes = Vec::new();
    let mut new_count = 0;
    let mut changed_count = 0;
    let mut config_count = 0;
    let mut pacnew_candidates = 0;

    // Get backup files for this package (for pacnew/pacsave prediction)
    let backup_files = get_backup_files(name, source).unwrap_or_default();
    let backup_set: HashSet<&str> = backup_files.iter().map(String::as_str).collect();

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
/// # Errors
/// - Returns `Err` when installed file list retrieval fails (see `get_installed_file_list`)
/// - Returns `Err` when backup files retrieval fails (see `get_backup_files`)
///
/// Details:
/// - Reads installed file lists and backup arrays to flag configuration files requiring user attention.
pub fn resolve_remove_files(name: &str) -> Result<PackageFileInfo, String> {
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
    let backup_set: HashSet<&str> = backup_files.iter().map(String::as_str).collect();

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

/// What: Enumerate files that would be changed when downgrading a package.
///
/// Inputs:
/// - `name`: Package scheduled for downgrade.
/// - `source`: Source repository information for remote lookups.
///
/// Output:
/// - Returns a `PackageFileInfo` capturing changed files (downgrade replaces files with older versions).
///
/// # Errors
/// - Returns `Err` when remote file list retrieval fails (see `get_remote_file_list`)
/// - Returns `Err` when installed file list retrieval fails (see `get_installed_file_list`)
///
/// Details:
/// - For downgrade, files that exist in both current and target versions are marked as "Changed".
/// - Files are compared between installed and remote (older) versions.
pub fn resolve_downgrade_files(name: &str, source: &Source) -> Result<PackageFileInfo, String> {
    // Get remote file list (older version - what we're downgrading TO)
    let remote_files = get_remote_file_list(name, source)?;
    // Get installed file list (current version - what we're downgrading FROM)
    let installed_files = get_installed_file_list(name)?;

    // Normalize paths (remove trailing slashes for comparison)
    let normalize_path = |p: &str| p.trim_end_matches('/').to_string();

    let installed_set: HashSet<String> =
        installed_files.iter().map(|p| normalize_path(p)).collect();
    let remote_set: HashSet<String> = remote_files.iter().map(|p| normalize_path(p)).collect();

    // Get backup files for this package (for pacnew prediction)
    let backup_files = get_backup_files(name, source).unwrap_or_default();
    let backup_set: HashSet<String> = backup_files.iter().map(|p| normalize_path(p)).collect();

    let mut file_changes = Vec::new();
    let mut changed_count = 0;
    let mut new_count = 0;
    let mut config_count = 0;
    let mut pacnew_candidates = 0;

    // Iterate over installed files to find files that will be changed
    // Files that exist in both versions are "Changed" (being replaced with older version)
    for path in installed_files {
        let normalized_path = normalize_path(&path);
        let is_config = path.starts_with("/etc/");
        let is_dir = path.ends_with('/');

        // Skip directories for now
        if is_dir {
            continue;
        }

        if is_config {
            config_count += 1;
        }

        // If file exists in remote (older) version, it's being changed (downgraded)
        if remote_set.contains(&normalized_path) {
            changed_count += 1;

            // Predict pacnew: file is in backup array and exists (will be changed to older version)
            let predicted_pacnew = backup_set.contains(&normalized_path) && is_config;

            if predicted_pacnew {
                pacnew_candidates += 1;
            }

            file_changes.push(FileChange {
                path,
                change_type: FileChangeType::Changed,
                package: name.to_string(),
                is_config,
                predicted_pacnew,
                predicted_pacsave: false,
            });
        }
        // Files that exist only in installed (newer) version but not in remote (older) version are "Removed"
        else {
            file_changes.push(FileChange {
                path,
                change_type: FileChangeType::Removed,
                package: name.to_string(),
                is_config,
                predicted_pacnew: false,
                predicted_pacsave: backup_set.contains(&normalized_path) && is_config,
            });
        }
    }

    // Also check for files that exist only in remote (older) version but not installed (newer) version - these are "New"
    for path in remote_files {
        let normalized_path = normalize_path(&path);
        let is_config = path.starts_with("/etc/");
        let is_dir = path.ends_with('/');

        // Skip directories for now
        if is_dir {
            continue;
        }

        // If file doesn't exist in installed version, it's "New" (will be added back)
        if !installed_set.contains(&normalized_path) {
            new_count += 1;
            file_changes.push(FileChange {
                path,
                change_type: FileChangeType::New,
                package: name.to_string(),
                is_config,
                predicted_pacnew: false,
                predicted_pacsave: false,
            });
        }
    }

    // Sort files by path for consistent display
    file_changes.sort_by(|a, b| a.path.cmp(&b.path));

    let removed_count = file_changes
        .iter()
        .filter(|f| matches!(f.change_type, FileChangeType::Removed))
        .count();

    Ok(PackageFileInfo {
        name: name.to_string(),
        files: file_changes,
        total_count: changed_count + new_count + removed_count,
        new_count,
        changed_count,
        removed_count,
        config_count,
        pacnew_candidates,
        pacsave_candidates: 0,
    })
}
