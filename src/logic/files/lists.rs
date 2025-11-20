//! File list retrieval functions for remote and installed packages.

use super::pkgbuild_fetch::fetch_pkgbuild_sync;
use super::pkgbuild_parse::parse_install_paths_from_pkgbuild;
use crate::state::types::Source;
use std::process::Command;

/// What: Parse file list from pacman/paru/yay command output.
///
/// Inputs:
/// - `output`: Command output containing file list in format "<pkg> <path>".
///
/// Output:
/// - Returns vector of file paths extracted from the output.
///
/// Details:
/// - Parses lines in format "<pkg> <path>" and extracts the path component.
fn parse_file_list_from_output(output: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(output);
    text.lines()
        .filter_map(|line| line.split_once(' ').map(|(_pkg, path)| path.to_string()))
        .collect()
}

/// What: Try to get file list using an AUR helper command (paru or yay).
///
/// Inputs:
/// - `helper`: Name of the helper command ("paru" or "yay").
/// - `name`: Package name to query.
///
/// Output:
/// - Returns Some(Vec<String>) if successful, None otherwise.
///
/// Details:
/// - Executes helper -Fl command and parses the output.
/// - Returns None if command fails or produces no files.
fn try_aur_helper_file_list(helper: &str, name: &str) -> Option<Vec<String>> {
    tracing::debug!("Trying {} -Fl {} for AUR package file list", helper, name);
    let output = Command::new(helper)
        .args(["-Fl", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let files = parse_file_list_from_output(&output.stdout);
    if files.is_empty() {
        return None;
    }

    tracing::debug!(
        "Found {} files from {} -Fl for {}",
        files.len(),
        helper,
        name
    );
    Some(files)
}

/// What: Get file list for AUR package using multiple fallback strategies.
///
/// Inputs:
/// - `name`: Package name to query.
///
/// Output:
/// - Returns file list if found, empty vector if no sources available.
///
/// Details:
/// - Tries installed files, then paru/yay, then PKGBUILD parsing.
fn get_aur_file_list(name: &str) -> Vec<String> {
    // First, check if package is already installed
    if let Ok(installed_files) = get_installed_file_list(name)
        && !installed_files.is_empty()
    {
        tracing::debug!(
            "Found {} files from installed AUR package {}",
            installed_files.len(),
            name
        );
        return installed_files;
    }

    // Try to use paru/yay -Fl if available (works for cached AUR packages)
    let has_paru = Command::new("paru").args(["--version"]).output().is_ok();
    let has_yay = Command::new("yay").args(["--version"]).output().is_ok();

    if has_paru && let Some(files) = try_aur_helper_file_list("paru", name) {
        return files;
    }

    if has_yay && let Some(files) = try_aur_helper_file_list("yay", name) {
        return files;
    }

    // Fallback: try to parse PKGBUILD to extract install paths
    if let Ok(pkgbuild) = fetch_pkgbuild_sync(name) {
        let files = parse_install_paths_from_pkgbuild(&pkgbuild, name);
        if !files.is_empty() {
            tracing::debug!(
                "Found {} files from PKGBUILD parsing for {}",
                files.len(),
                name
            );
            return files;
        }
    } else {
        tracing::debug!("Failed to fetch PKGBUILD for {}", name);
    }

    // No file list available
    tracing::debug!(
        "AUR package {}: file list not available (not installed, not cached, PKGBUILD parsing failed)",
        name
    );
    Vec::new()
}

/// What: Get file list for official repository package.
///
/// Inputs:
/// - `name`: Package name to query.
/// - `repo`: Repository name (empty string if not specified).
///
/// Output:
/// - Returns file list or error if command fails.
///
/// Details:
/// - Uses pacman -Fl command. Returns empty list if file database is not synced.
fn get_official_file_list(name: &str, repo: &str) -> Result<Vec<String>, String> {
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

    let files = parse_file_list_from_output(&output.stdout);
    tracing::debug!("Found {} files in remote package {}", files.len(), name);
    Ok(files)
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
pub fn get_remote_file_list(name: &str, source: &Source) -> Result<Vec<String>, String> {
    match source {
        Source::Official { repo, .. } => get_official_file_list(name, repo),
        Source::Aur => Ok(get_aur_file_list(name)),
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

    let files = parse_file_list_from_output(&output.stdout);
    tracing::debug!("Found {} files in installed package {}", files.len(), name);
    Ok(files)
}
