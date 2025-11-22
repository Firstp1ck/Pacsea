//! Service unit file collection and parsing.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::state::types::Source;

use super::command::run_command;

/// What: Collect service unit filenames shipped by a specific package.
///
/// Inputs:
/// - `package`: Package name for which to inspect the remote file list.
/// - `source`: Source descriptor to determine how to fetch units (Official vs AUR).
///
/// Output:
/// - Vector of unit filenames (e.g., `sshd.service`). Empty when the package
///   ships no systemd units.
///
/// Details:
/// - Executes `pacman -Fl <package>` and filters paths under the standard
///   systemd directories.
/// - For AUR packages, uses fallback methods (installed files, paru/yay -Fl).
pub(super) fn collect_service_units_for_package(
    package: &str,
    source: &Source,
) -> Result<Vec<String>, String> {
    match source {
        Source::Official { .. } => {
            // Use pacman -Fl for official packages
            let output = run_command(
                "pacman",
                &["-Fl", package],
                &format!("pacman -Fl {package}"),
            )?;
            let units = extract_service_units_from_file_list(&output, package);
            Ok(units)
        }
        Source::Aur => {
            // For AUR packages, try fallback methods similar to collect_binaries_for_package
            // First, check if package is already installed
            if let Ok(installed_files) = crate::logic::files::get_installed_file_list(package)
                && !installed_files.is_empty()
            {
                let file_list = installed_files
                    .iter()
                    .map(|f| format!("{package} {f}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let units = extract_service_units_from_file_list(&file_list, package);
                if !units.is_empty() {
                    tracing::debug!(
                        "Found {} service units from installed AUR package {}",
                        units.len(),
                        package
                    );
                    return Ok(units);
                }
            }

            // Try to use paru/yay -Fl if available (works for cached AUR packages)
            let has_paru = Command::new("paru").args(["--version"]).output().is_ok();
            let has_yay = Command::new("yay").args(["--version"]).output().is_ok();

            if has_paru {
                tracing::debug!("Trying paru -Fl {} for AUR package service units", package);
                if let Ok(output) = Command::new("paru")
                    .args(["-Fl", package])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let units = extract_service_units_from_file_list(&text, package);
                    if !units.is_empty() {
                        tracing::debug!(
                            "Found {} service units from paru -Fl for {}",
                            units.len(),
                            package
                        );
                        return Ok(units);
                    }
                }
            }

            if has_yay {
                tracing::debug!("Trying yay -Fl {} for AUR package service units", package);
                if let Ok(output) = Command::new("yay")
                    .args(["-Fl", package])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let units = extract_service_units_from_file_list(&text, package);
                    if !units.is_empty() {
                        tracing::debug!(
                            "Found {} service units from yay -Fl for {}",
                            units.len(),
                            package
                        );
                        return Ok(units);
                    }
                }
            }

            // For AUR packages without file lists available, return empty (not an error)
            // The binary-based detection will still work
            tracing::debug!(
                "No file list available for AUR package {} (will use binary-based detection)",
                package
            );
            Ok(Vec::new())
        }
    }
}

/// What: Extract unit filenames from `pacman -Fl` output.
///
/// Inputs:
/// - `file_list`: Raw `pacman -Fl` stdout.
/// - `package`: Package name used to filter unrelated entries in the output.
///
/// Output:
/// - Vector of unit filenames sorted in discovery order.
///
/// Details:
/// - Recognises units residing under `/usr/lib/systemd/system/` or the legacy
///   `/lib/systemd/system/` prefixes.
/// - Discards duplicate unit entries while preserving discovery order.
pub(super) fn extract_service_units_from_file_list(file_list: &str, package: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut units = Vec::new();

    for line in file_list.lines() {
        let (pkg, raw_path) = match line.split_once(' ') {
            Some(parts) => parts,
            None => continue,
        };
        if pkg != package {
            continue;
        }

        let path = raw_path.strip_suffix('/').unwrap_or(raw_path);
        if !is_service_path(path) {
            continue;
        }

        if let Some(file_name) = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .filter(|name| seen.insert(name.clone()))
        {
            units.push(file_name);
        }
    }

    units
}

/// What: Determine whether a path refers to a systemd service unit file.
///
/// Inputs:
/// - `path`: File path extracted from `pacman -Fl`.
///
/// Output:
/// - `true` when the path resides under a known systemd unit directory and
///   ends with `.service`; otherwise `false`.
///
/// Details:
/// - Supports both `/usr/lib/systemd/system` and `/lib/systemd/system` roots.
pub(super) fn is_service_path(path: &str) -> bool {
    const PREFIXES: [&str; 2] = ["/usr/lib/systemd/system/", "/lib/systemd/system/"];
    PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix) && path.ends_with(".service"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Ensure unit extraction recognises service files and ignores others.
    ///
    /// Inputs:
    /// - Synthetic `pacman -Fl` output containing service files, directories, and
    ///   irrelevant paths.
    ///
    /// Output:
    /// - Confirms only valid `.service` entries are returned.
    ///
    /// Details:
    /// - Verifies both `/usr/lib/systemd/system/` and `/lib/systemd/system/` paths.
    fn extract_service_units_from_file_list_filters_correctly() {
        let output = "\
mockpkg /usr/lib/systemd/system/example.service
mockpkg /usr/lib/systemd/system/example.service/
mockpkg /usr/lib/systemd/system/example.timer
mockpkg /lib/systemd/system/legacy.service
mockpkg /usr/bin/mock
otherpkg /usr/lib/systemd/system/other.service
";
        let units = extract_service_units_from_file_list(output, "mockpkg");
        assert_eq!(
            units,
            vec!["example.service".to_string(), "legacy.service".to_string()]
        );
    }

    #[test]
    /// What: Ensure duplicate `.service` listings are deduplicated without disturbing order.
    ///
    /// Inputs:
    /// - Synthetic `pacman -Fl` output containing repeated entries for the same units.
    ///
    /// Output:
    /// - Confirms the resulting list contains each unit once in discovery order.
    ///
    /// Details:
    /// - Validates that later duplicates are ignored and first occurrences are retained.
    fn extract_service_units_from_file_list_deduplicates_preserving_order() {
        let output = "\
mockpkg /usr/lib/systemd/system/alpha.service
mockpkg /usr/lib/systemd/system/beta.service
mockpkg /usr/lib/systemd/system/alpha.service/
mockpkg /usr/lib/systemd/system/gamma.service
mockpkg /usr/lib/systemd/system/beta.service/
";
        let units = extract_service_units_from_file_list(output, "mockpkg");
        assert_eq!(
            units,
            vec![
                "alpha.service".to_string(),
                "beta.service".to_string(),
                "gamma.service".to_string()
            ]
        );
    }
}
