//! PKGBUILD fetching functions.

use crate::sources::get_arch_client;
use crate::util::{curl_args, percent_encode};
use std::process::Command;

/// What: Try to find PKGBUILD in a directory structure.
///
/// Inputs:
/// - `base_dir`: Base directory to search in.
/// - `name`: Package name for logging.
/// - `helper_name`: Helper name for logging (e.g., "paru" or "yay").
///
/// Output:
/// - Returns PKGBUILD content if found, or None.
///
/// Details:
/// - First checks `base_dir`/`PKGBUILD`, then searches subdirectories.
fn find_pkgbuild_in_dir(
    base_dir: &std::path::Path,
    name: &str,
    helper_name: &str,
) -> Option<String> {
    // Try direct path first
    let pkgbuild_path = base_dir.join("PKGBUILD");
    if let Ok(text) = std::fs::read_to_string(&pkgbuild_path)
        && text.contains("pkgname")
    {
        tracing::debug!("Found PKGBUILD for {} via {} -G", name, helper_name);
        return Some(text);
    }

    // Search in subdirectories
    let Ok(entries) = std::fs::read_dir(base_dir) else {
        return None;
    };

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }

        let pkgbuild_path = entry.path().join("PKGBUILD");
        if let Ok(text) = std::fs::read_to_string(&pkgbuild_path)
            && text.contains("pkgname")
        {
            tracing::debug!(
                "Found PKGBUILD for {} via {} -G (in subdir)",
                name,
                helper_name
            );
            return Some(text);
        }
    }

    None
}

/// What: Try to get PKGBUILD using a helper command (paru -G or yay -G).
///
/// Inputs:
/// - `helper`: Helper command name ("paru" or "yay").
/// - `name`: Package name.
///
/// Output:
/// - Returns PKGBUILD content if found, or None.
///
/// Details:
/// - Executes helper -G command in a temp directory and searches for PKGBUILD.
fn try_helper_command(helper: &str, name: &str) -> Option<String> {
    let temp_dir = std::env::temp_dir().join(format!("pacsea_pkgbuild_{name}"));
    let _ = std::fs::create_dir_all(&temp_dir);

    let output = Command::new(helper)
        .args(["-G", name])
        .current_dir(&temp_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return None;
    }

    let result = find_pkgbuild_in_dir(&temp_dir.join(name), name, helper);
    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

/// What: Try to read PKGBUILD directly from known cache paths.
///
/// Inputs:
/// - `name`: Package name.
/// - `home`: Home directory path.
///
/// Output:
/// - Returns PKGBUILD content if found, or None.
///
/// Details:
/// - Checks standard cache locations for paru and yay.
fn try_direct_cache_paths(name: &str, home: &str) -> Option<String> {
    let cache_paths = [
        format!("{home}/.cache/paru/clone/{name}/PKGBUILD"),
        format!("{home}/.cache/yay/{name}/PKGBUILD"),
    ];

    for path_str in cache_paths {
        if let Ok(text) = std::fs::read_to_string(&path_str)
            && text.contains("pkgname")
        {
            tracing::debug!("Found PKGBUILD for {} in cache: {}", name, path_str);
            return Some(text);
        }
    }

    None
}

/// What: Try to find PKGBUILD in cache subdirectories.
///
/// Inputs:
/// - `name`: Package name.
/// - `home`: Home directory path.
///
/// Output:
/// - Returns PKGBUILD content if found, or None.
///
/// Details:
/// - Searches cache directories for packages that might be in subdirectories.
fn try_cache_subdirectories(name: &str, home: &str) -> Option<String> {
    let cache_bases = [
        format!("{home}/.cache/paru/clone"),
        format!("{home}/.cache/yay"),
    ];

    for cache_base in cache_bases {
        let Ok(entries) = std::fs::read_dir(&cache_base) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let matches_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(name));

            if !matches_name {
                continue;
            }

            // Check direct PKGBUILD
            let pkgbuild_path = path.join("PKGBUILD");
            if let Ok(text) = std::fs::read_to_string(&pkgbuild_path)
                && text.contains("pkgname")
            {
                tracing::debug!(
                    "Found PKGBUILD for {} in cache subdirectory: {:?}",
                    name,
                    pkgbuild_path
                );
                return Some(text);
            }

            // Check subdirectories
            let Ok(sub_entries) = std::fs::read_dir(&path) else {
                continue;
            };

            for sub_entry in sub_entries.flatten() {
                if !sub_entry.path().is_dir() {
                    continue;
                }

                let pkgbuild_path = sub_entry.path().join("PKGBUILD");
                if let Ok(text) = std::fs::read_to_string(&pkgbuild_path)
                    && text.contains("pkgname")
                {
                    tracing::debug!(
                        "Found PKGBUILD for {} in cache subdirectory: {:?}",
                        name,
                        pkgbuild_path
                    );
                    return Some(text);
                }
            }
        }
    }

    None
}

/// What: Get PKGBUILD from yay/paru cache (offline method).
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Returns PKGBUILD content if found in cache, or None.
///
/// Details:
/// - Checks yay cache (~/.cache/yay) and paru cache (~/.cache/paru).
/// - Also tries using `yay -G` or `paru -G` commands.
#[must_use]
pub fn get_pkgbuild_from_cache(name: &str) -> Option<String> {
    // Try helper commands first (fastest, uses helper's cache)
    if let Some(text) = try_helper_command("paru", name) {
        return Some(text);
    }
    if let Some(text) = try_helper_command("yay", name) {
        return Some(text);
    }

    // Try reading directly from cache directories
    let home = std::env::var("HOME").ok()?;
    if let Some(text) = try_direct_cache_paths(name, &home) {
        return Some(text);
    }

    // Try finding PKGBUILD in cache subdirectories
    try_cache_subdirectories(name, &home)
}

/// What: Fetch PKGBUILD content synchronously (blocking).
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Returns PKGBUILD content as a string, or an error if fetch fails.
///
/// # Errors
/// - Returns `Err` when network request fails
/// - Returns `Err` when PKGBUILD cannot be fetched from AUR or official repositories
///
/// Details:
/// - First tries offline methods (yay/paru cache, yay -G, paru -G).
/// - For AUR packages: tries arch-toolkit first (with automatic rate limiting and retry logic).
/// - Falls back to curl-based AUR fetching if arch-toolkit unavailable or fails.
/// - Falls back to official GitLab repos for official packages.
/// - Leverages automatic rate limiting, retry logic, and optional caching from arch-toolkit for AUR packages.
pub fn fetch_pkgbuild_sync(name: &str) -> Result<String, String> {
    // 1. Try offline methods first (yay/paru cache)
    if let Some(cached) = get_pkgbuild_from_cache(name) {
        tracing::debug!("Using cached PKGBUILD for {} (offline)", name);
        return Ok(cached);
    }

    // 2. Try AUR first using arch-toolkit (for AUR packages)
    if let Some(client) = get_arch_client() {
        let pkgbuild_result = match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(client.aur().pkgbuild(name)),
            Err(_) => {
                // No runtime available, create a new one
                match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt.block_on(client.aur().pkgbuild(name)),
                    Err(e) => {
                        tracing::debug!("Failed to create tokio runtime for PKGBUILD fetch: {}", e);
                        // Fall through to curl-based AUR fetching
                        return Err(format!("Failed to create tokio runtime: {e}"));
                    }
                }
            }
        };

        match pkgbuild_result {
            Ok(text) if !text.trim().is_empty() && text.contains("pkgname") => {
                return Ok(text);
            }
            Ok(_) => {
                tracing::debug!("AUR returned empty or invalid PKGBUILD for {}", name);
                // Fall through to curl-based AUR fetching
            }
            Err(e) => {
                tracing::debug!("AUR PKGBUILD fetch failed for {}: {}", name, e);
                // Check if it's an HTTP error (like 502) - don't fallback to GitLab for AUR packages
                let error_str = e.to_string();
                if error_str.contains("HTTP") || error_str.contains("502") {
                    return Err(format!("AUR returned HTTP error: {e}"));
                }
                // Fall through to curl-based AUR fetching for other errors
            }
        }
    } else {
        tracing::debug!(
            "ArchClient not initialized, falling back to curl for {}",
            name
        );
    }

    // 3. Fallback to curl-based AUR fetching (for backward compatibility or when arch-toolkit unavailable)
    let url_aur = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching PKGBUILD from AUR (curl fallback): {}", url_aur);

    let args = curl_args(&url_aur, &[]);
    let output = Command::new("curl").args(&args).output();

    let aur_failed_http_error = match &output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if !text.trim().is_empty() && text.contains("pkgname") {
                return Ok(text);
            }
            false
        }
        Ok(output) => {
            // curl with -f flag returns exit code 22 for HTTP errors like 502
            // If AUR returns 502 (Bad Gateway), don't try GitLab fallback
            // GitLab should only be used for official packages, not AUR packages
            // AUR 502 indicates a temporary AUR server issue, not that the package doesn't exist in AUR
            output.status.code().is_some_and(|code| code == 22)
        }
        _ => false,
    };

    if aur_failed_http_error {
        tracing::debug!(
            "AUR returned HTTP error (likely 502) for {} - skipping GitLab fallback (likely AUR package or temporary AUR issue)",
            name
        );
        return Err("AUR returned HTTP error (likely 502 Bad Gateway)".to_string());
    }

    // Fallback to official GitLab repos (only for official packages, not AUR)
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
            // Validate that we got a PKGBUILD, not HTML (e.g., login page)
            if !text.trim().is_empty()
                && (text.contains("pkgname") || text.contains("pkgver") || text.contains("pkgdesc"))
                && !text.trim_start().starts_with("<!DOCTYPE")
                && !text.trim_start().starts_with("<html")
            {
                return Ok(text);
            }
            tracing::warn!(
                "GitLab main returned invalid PKGBUILD (likely HTML): first 200 chars: {:?}",
                text.chars().take(200).collect::<String>()
            );
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
        .map_err(|e| format!("curl failed: {e}"))?;

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

    // Validate that we got a PKGBUILD, not HTML (e.g., login page)
    if text.trim_start().starts_with("<!DOCTYPE") || text.trim_start().starts_with("<html") {
        tracing::warn!(
            "GitLab master returned HTML instead of PKGBUILD: first 200 chars: {:?}",
            text.chars().take(200).collect::<String>()
        );
        return Err("GitLab returned HTML page instead of PKGBUILD".to_string());
    }

    if !text.contains("pkgname") && !text.contains("pkgver") && !text.contains("pkgdesc") {
        tracing::warn!(
            "GitLab master returned content that doesn't look like PKGBUILD: first 200 chars: {:?}",
            text.chars().take(200).collect::<String>()
        );
        return Err("Response doesn't appear to be a valid PKGBUILD".to_string());
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
/// # Errors
/// - Returns `Err` when network request fails (curl execution error)
/// - Returns `Err` when .SRCINFO cannot be fetched from AUR
///
/// Details:
/// - Downloads .SRCINFO from AUR cgit repository.
pub fn fetch_srcinfo_sync(name: &str) -> Result<String, String> {
    crate::util::srcinfo::fetch_srcinfo(name, None)
}
