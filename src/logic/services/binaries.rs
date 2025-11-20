//! Binary collection and parsing for service impact detection.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::state::types::Source;

use super::command::run_command;

/// What: Collect binary paths shipped by a specific package.
///
/// Inputs:
/// - `package`: Package name for which to inspect the remote file list.
/// - `source`: Source descriptor to determine how to fetch binaries (Official vs AUR).
///
/// Output:
/// - Vector of binary paths (e.g., `/usr/bin/foo`, `/usr/sbin/bar`).
///
/// Details:
/// - For official packages: Executes `pacman -Fl <package>` and filters paths under standard binary directories.
/// - For AUR packages: Uses installed files, paru/yay -Fl, or PKGBUILD parsing as fallback.
/// - Includes executables from `/usr/bin`, `/usr/sbin`, `/bin`, `/sbin`, and `/usr/local/bin`.
pub(crate) fn collect_binaries_for_package(
    package: &str,
    source: &Source,
) -> Result<Vec<String>, String> {
    match source {
        Source::Official { .. } => {
            // Use pacman -Fl for official packages
            let output = run_command(
                "pacman",
                &["-Fl", package],
                &format!("pacman -Fl {}", package),
            )?;
            let binaries = extract_binaries_from_file_list(&output, package);
            Ok(binaries)
        }
        Source::Aur => {
            // For AUR packages, use the same fallback chain as file lists
            // First, check if package is already installed
            if let Ok(installed_files) = crate::logic::files::get_installed_file_list(package)
                && !installed_files.is_empty()
            {
                let binaries = extract_binaries_from_file_list(
                    &installed_files
                        .iter()
                        .map(|f| format!("{} {}", package, f))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    package,
                );
                if !binaries.is_empty() {
                    tracing::debug!(
                        "Found {} binaries from installed AUR package {}",
                        binaries.len(),
                        package
                    );
                    return Ok(binaries);
                }
            }

            // Try to use paru/yay -Fl if available (works for cached AUR packages)
            let has_paru = Command::new("paru").args(["--version"]).output().is_ok();
            let has_yay = Command::new("yay").args(["--version"]).output().is_ok();

            if has_paru {
                tracing::debug!("Trying paru -Fl {} for AUR package binaries", package);
                if let Ok(output) = Command::new("paru")
                    .args(["-Fl", package])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let binaries = extract_binaries_from_file_list(&text, package);
                    if !binaries.is_empty() {
                        tracing::debug!(
                            "Found {} binaries from paru -Fl for {}",
                            binaries.len(),
                            package
                        );
                        return Ok(binaries);
                    }
                }
            }

            if has_yay {
                tracing::debug!("Trying yay -Fl {} for AUR package binaries", package);
                if let Ok(output) = Command::new("yay")
                    .args(["-Fl", package])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let binaries = extract_binaries_from_file_list(&text, package);
                    if !binaries.is_empty() {
                        tracing::debug!(
                            "Found {} binaries from yay -Fl for {}",
                            binaries.len(),
                            package
                        );
                        return Ok(binaries);
                    }
                }
            }

            // Fallback: try to parse PKGBUILD to extract install paths
            match crate::logic::files::fetch_pkgbuild_sync(package) {
                Ok(pkgbuild) => {
                    let files =
                        crate::logic::files::parse_install_paths_from_pkgbuild(&pkgbuild, package);
                    let binaries: Vec<String> = files
                        .into_iter()
                        .filter(|f| {
                            f.starts_with("/usr/bin/")
                                || f.starts_with("/usr/sbin/")
                                || f.starts_with("/bin/")
                                || f.starts_with("/sbin/")
                                || f.starts_with("/usr/local/bin/")
                        })
                        .collect();
                    if !binaries.is_empty() {
                        tracing::debug!(
                            "Found {} binaries from PKGBUILD parsing for {}",
                            binaries.len(),
                            package
                        );
                        return Ok(binaries);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch PKGBUILD for {}: {}", package, e);
                }
            }

            // No binaries available
            Ok(Vec::new())
        }
    }
}

/// What: Extract binary paths from `pacman -Fl` output.
///
/// Inputs:
/// - `file_list`: Raw `pacman -Fl` stdout.
/// - `package`: Package name used to filter unrelated entries.
///
/// Output:
/// - Vector of binary paths sorted in discovery order.
///
/// Details:
/// - Recognises executables under standard binary directories.
/// - Filters out directories and non-executable files.
pub(crate) fn extract_binaries_from_file_list(file_list: &str, package: &str) -> Vec<String> {
    const BINARY_PREFIXES: [&str; 5] = [
        "/usr/bin/",
        "/usr/sbin/",
        "/bin/",
        "/sbin/",
        "/usr/local/bin/",
    ];

    let mut seen = HashSet::new();
    let mut binaries = Vec::new();

    for line in file_list.lines() {
        let (pkg, raw_path) = match line.split_once(' ') {
            Some(parts) => parts,
            None => continue,
        };
        if pkg != package {
            continue;
        }

        let path = raw_path.strip_suffix('/').unwrap_or(raw_path);

        // Check if path is under a binary directory
        let is_binary = BINARY_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix));

        if is_binary {
            // Extract the binary name for matching
            if let Some(binary_name) = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string())
            {
                // Store full path for exact matching
                if seen.insert(path.to_string()) {
                    binaries.push(path.to_string());
                }
                // Also store binary name for flexible matching (if not already added)
                if seen.insert(binary_name.clone()) {
                    binaries.push(binary_name);
                }
            }
        }
    }

    binaries
}
