//! File list retrieval functions for remote and installed packages.

use super::pkgbuild_fetch::fetch_pkgbuild_sync;
use super::pkgbuild_parse::parse_install_paths_from_pkgbuild;
use crate::state::types::Source;
use std::process::Command;

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
pub fn get_remote_file_list(name: &str, source: &Source) -> Result<Vec<String>, String> {
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
