//! Backup file detection and retrieval functions.

use super::pkgbuild_fetch::{fetch_pkgbuild_sync, fetch_srcinfo_sync};
use super::pkgbuild_parse::{parse_backup_from_pkgbuild, parse_backup_from_srcinfo};
use crate::state::types::Source;
use std::process::Command;

/// What: Identify files marked for backup handling during install or removal operations.
///
/// Inputs:
/// - `name`: Package whose backup array should be inspected.
/// - `source`: Source descriptor to decide how to gather backup information.
///
/// Output:
/// - Returns a list of backup file paths or an empty list when the data cannot be retrieved.
///
/// # Errors
/// - Returns `Err` when `pacman -Qii` command execution fails for installed packages
/// - Returns `Err` when PKGBUILD or .SRCINFO fetch fails and no fallback is available
///
/// Details:
/// - Prefers querying the installed package via `pacman -Qii`; falls back to best-effort heuristics.
#[must_use]
pub fn get_backup_files(name: &str, source: &Source) -> Result<Vec<String>, String> {
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
/// # Errors
/// - Returns `Err` when `pacman -Qii` command execution fails (I/O error)
/// - Returns `Err` when `pacman -Qii` exits with non-zero status for reasons other than package not found
///
/// Details:
/// - Parses the `Backup Files` section, handling wrapped lines to ensure complete coverage.
pub fn get_backup_files_from_installed(name: &str) -> Result<Vec<String>, String> {
    tracing::debug!("Running: pacman -Qii {}", name);
    let output = Command::new("pacman")
        .args(["-Qii", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| {
            tracing::error!("Failed to execute pacman -Qii {}: {}", name, e);
            format!("pacman -Qii failed: {e}")
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
        return Err(format!("pacman -Qii failed for {name}: {stderr}"));
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
