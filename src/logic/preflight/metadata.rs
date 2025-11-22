//! Package metadata fetching and parsing utilities.
//!
//! This module provides functions to fetch package metadata from pacman and
//! parse the output into structured data.

use super::command::{CommandError, CommandRunner};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What: Extract remote download/install sizes for an official package via
/// `pacman -Si`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `repo`: Repository name (e.g., `"core"`).
/// - `name`: Package identifier.
/// - `expected_version`: Version string to cross-check.
///
/// Output:
/// - `Ok(OfficialMetadata)` containing optional size metrics.
/// - `Err(CommandError)` when the command fails.
///
/// Details:
/// - Performs best-effort verification of the returned version, logging
///   mismatches for diagnostics.
pub(super) fn fetch_official_metadata<R: CommandRunner>(
    runner: &R,
    repo: &str,
    name: &str,
    expected_version: &str,
) -> Result<OfficialMetadata, CommandError> {
    let spec = format!("{repo}/{name}");
    let output = runner.run("pacman", &["-Si", &spec])?;
    let fields = parse_pacman_key_values(&output);

    if let Some(version) = fields.get("Version")
        && version.trim() != expected_version
    {
        tracing::debug!(
            "Preflight summary: pacman -Si reported version {} for {} (expected {})",
            version.trim(),
            spec,
            expected_version
        );
    }

    let download_size = fields
        .get("Download Size")
        .and_then(|raw| parse_size_to_bytes(raw));
    let install_size = fields
        .get("Installed Size")
        .and_then(|raw| parse_size_to_bytes(raw));

    Ok(OfficialMetadata {
        download_size,
        install_size,
    })
}

/// What: Retrieve installed package version via `pacman -Q`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `name`: Package identifier.
///
/// Output:
/// - `Ok(String)` containing the installed version.
/// - `Err(CommandError)` when fetch fails.
///
/// Details:
/// - Trims stdout and returns the last whitespace-separated token.
pub(super) fn fetch_installed_version<R: CommandRunner>(
    runner: &R,
    name: &str,
) -> Result<String, CommandError> {
    let output = runner.run("pacman", &["-Q", name])?;
    let mut parts = output.split_whitespace();
    let _pkg_name = parts.next();
    parts
        .next_back()
        .map(|value| value.to_string())
        .ok_or_else(|| CommandError::Parse {
            program: "pacman -Q".to_string(),
            field: "version".to_string(),
        })
}

/// What: Retrieve the installed size of a package via `pacman -Qi`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `name`: Package identifier.
///
/// Output:
/// - `Ok(u64)` representing bytes installed.
/// - `Err(CommandError)` when parsing fails.
///
/// Details:
/// - Parses the `Installed Size` field using [`parse_size_to_bytes`].
pub(super) fn fetch_installed_size<R: CommandRunner>(
    runner: &R,
    name: &str,
) -> Result<u64, CommandError> {
    let output = runner.run("pacman", &["-Qi", name])?;
    let fields = parse_pacman_key_values(&output);
    fields
        .get("Installed Size")
        .and_then(|raw| parse_size_to_bytes(raw))
        .ok_or_else(|| CommandError::Parse {
            program: "pacman -Qi".to_string(),
            field: "Installed Size".to_string(),
        })
}

/// What: Metadata extracted from `pacman -Si` to inform download/install
/// calculations.
///
/// Inputs: Populated by [`fetch_official_metadata`].
///
/// Output: Holds optional download and install sizes in bytes.
///
/// Details:
/// - Values are `None` when the upstream output omits a field.
#[derive(Default, Debug)]
pub(crate) struct OfficialMetadata {
    pub(crate) download_size: Option<u64>,
    pub(crate) install_size: Option<u64>,
}

/// What: Transform pacman key-value output into a `HashMap`.
///
/// Inputs:
/// - `output`: Raw stdout from `pacman` invocations.
///
/// Output:
/// - `HashMap<String, String>` mapping field names to raw string values.
///
/// Details:
/// - Continuation lines (prefixed with a space) are appended to the previous
///   key's value.
pub(super) fn parse_pacman_key_values(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut last_key: Option<String> = None;

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_string();
            let val = value.trim().to_string();
            map.insert(key.clone(), val);
            last_key = Some(key);
        } else if line.starts_with(' ')
            && let Some(key) = &last_key
        {
            map.entry(key.clone())
                .and_modify(|existing| {
                    if !existing.ends_with(' ') {
                        existing.push(' ');
                    }
                    existing.push_str(line.trim());
                })
                .or_insert_with(|| line.trim().to_string());
        }
    }

    map
}

/// What: Convert human-readable pacman size strings to bytes.
///
/// Inputs:
/// - `raw`: String such as `"1.5 MiB"` or `"512 KiB"`.
///
/// Output:
/// - `Some(u64)` with byte representation on success.
/// - `None` when parsing fails.
///
/// Details:
/// - Supports B, KiB, MiB, GiB, and TiB units.
pub(super) fn parse_size_to_bytes(raw: &str) -> Option<u64> {
    let mut parts = raw.split_whitespace();
    let number = parts.next()?.replace(',', "");
    let value = number.parse::<f64>().ok()?;
    let unit = parts.next().unwrap_or("B");
    let multiplier = match unit {
        "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    Some((value * multiplier) as u64)
}

/// What: Find AUR package file in pacman cache or AUR helper caches.
///
/// Inputs:
/// - `name`: Package name to search for.
/// - `version`: Package version (optional, for matching).
///
/// Output:
/// - `Some(PathBuf)` pointing to the package file if found.
/// - `None` if no package file is found in any cache.
///
/// Details:
/// - Checks pacman cache (`/var/cache/pacman/pkg/`).
/// - Checks AUR helper caches (paru/yay build directories).
/// - Matches package files by name prefix and optionally by version.
fn find_aur_package_file(name: &str, version: Option<&str>) -> Option<PathBuf> {
    // Try pacman cache first (fastest, most reliable)
    if let Ok(pacman_cache) = Path::new("/var/cache/pacman/pkg").read_dir() {
        for entry in pacman_cache.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                // Match package name prefix (e.g., "yay-12.3.2-1-x86_64.pkg.tar.zst")
                if file_name.starts_with(name)
                    && (file_name.ends_with(".pkg.tar.zst") || file_name.ends_with(".pkg.tar.xz"))
                {
                    // If version specified, try to match it
                    if let Some(ver) = version {
                        if file_name.contains(ver) {
                            return Some(path);
                        }
                    } else {
                        return Some(path);
                    }
                }
            }
        }
    }

    // Try AUR helper caches
    if let Ok(home) = std::env::var("HOME") {
        let cache_paths = [
            format!("{home}/.cache/paru/clone/{name}"),
            format!("{home}/.cache/yay/{name}"),
        ];

        for cache_base in cache_paths {
            let cache_dir = Path::new(&cache_base);
            if let Ok(entries) = fs::read_dir(cache_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                        && file_name.starts_with(name)
                        && (file_name.ends_with(".pkg.tar.zst")
                            || file_name.ends_with(".pkg.tar.xz"))
                    {
                        if let Some(ver) = version {
                            if file_name.contains(ver) {
                                return Some(path);
                            }
                        } else {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

/// What: Extract download and install sizes from an AUR package file.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `pkg_path`: Path to the package file.
///
/// Output:
/// - `Ok(OfficialMetadata)` with `download_size` (file size) and `install_size` (from package metadata).
/// - `Err(CommandError)` when extraction fails.
///
/// Details:
/// - Download size is the actual file size on disk.
/// - Install size is extracted via `pacman -Qp` command.
fn extract_aur_package_sizes<R: CommandRunner>(
    runner: &R,
    pkg_path: &Path,
) -> Result<OfficialMetadata, CommandError> {
    // Get download size (file size on disk)
    let download_size = fs::metadata(pkg_path).ok().map(|meta| meta.len());

    // Get install size from package metadata
    let install_size = if let Some(pkg_str) = pkg_path.to_str() {
        match runner.run("pacman", &["-Qp", pkg_str]) {
            Ok(output) => {
                let fields = parse_pacman_key_values(&output);
                fields
                    .get("Installed Size")
                    .and_then(|raw| parse_size_to_bytes(raw))
            }
            Err(_) => None,
        }
    } else {
        None
    };

    Ok(OfficialMetadata {
        download_size,
        install_size,
    })
}

/// What: Fetch metadata for AUR packages by checking local caches.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `name`: Package name.
/// - `version`: Package version (optional, for matching).
///
/// Output:
/// - `Ok(OfficialMetadata)` with sizes if package file found in cache.
/// - `Err(CommandError)` when extraction fails or package not found.
///
/// Details:
/// - Checks pacman cache and AUR helper caches for built package files.
/// - Extracts sizes from found package files.
/// - Returns None values if package file is not found (graceful degradation).
pub(super) fn fetch_aur_metadata<R: CommandRunner>(
    runner: &R,
    name: &str,
    version: Option<&str>,
) -> Result<OfficialMetadata, CommandError> {
    if let Some(pkg_path) = find_aur_package_file(name, version) {
        extract_aur_package_sizes(runner, &pkg_path)
    } else {
        // Package file not found in cache - return None values (graceful degradation)
        Ok(OfficialMetadata {
            download_size: None,
            install_size: None,
        })
    }
}

#[cfg(not(windows))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::preflight::command::{CommandError, CommandRunner};
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::Mutex;

    type MockCommandKey = (String, Vec<String>);
    type MockCommandResult = Result<String, CommandError>;
    type MockResponseMap = HashMap<MockCommandKey, MockCommandResult>;

    #[derive(Default)]
    struct MockRunner {
        responses: Mutex<MockResponseMap>,
    }

    impl MockRunner {
        fn with(responses: MockResponseMap) -> Self {
            Self {
                responses: Mutex::new(responses),
            }
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError> {
            let key = (
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            );
            let mut guard = self.responses.lock().expect("poisoned responses mutex");
            guard.remove(&key).unwrap_or_else(|| {
                Err(CommandError::Failed {
                    program: program.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                    status: std::process::ExitStatus::from_raw(1),
                })
            })
        }
    }

    #[test]
    /// What: Ensure `parse_size_to_bytes` correctly converts various size formats.
    ///
    /// Inputs:
    /// - Various size strings with different units (B, KiB, MiB, GiB, TiB).
    ///
    /// Output:
    /// - Returns correct byte counts for valid inputs.
    ///
    /// Details:
    /// - Tests edge cases like decimal values and comma separators.
    fn test_parse_size_to_bytes() {
        assert_eq!(parse_size_to_bytes("10 B"), Some(10));
        assert_eq!(parse_size_to_bytes("1 KiB"), Some(1024));
        assert_eq!(
            parse_size_to_bytes("2.5 MiB"),
            Some((2.5 * 1024.0 * 1024.0) as u64)
        );
        assert_eq!(
            parse_size_to_bytes("1.5 GiB"),
            Some((1.5 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
        assert_eq!(
            parse_size_to_bytes("1,234.5 MiB"),
            Some((1234.5 * 1024.0 * 1024.0) as u64)
        );
        assert_eq!(parse_size_to_bytes("invalid"), None);
        assert_eq!(parse_size_to_bytes(""), None);
    }

    #[test]
    /// What: Ensure AUR metadata fetching returns None when package file is not found.
    ///
    /// Inputs:
    /// - AUR package name that doesn't exist in any cache.
    ///
    /// Output:
    /// - Returns `Ok(OfficialMetadata)` with `None` values for both sizes.
    ///
    /// Details:
    /// - Tests graceful degradation when package file is not available.
    fn test_fetch_aur_metadata_not_found() {
        let runner = MockRunner::default();
        let result = fetch_aur_metadata(&runner, "nonexistent-package", Some("1.0.0"));
        assert!(result.is_ok());
        let meta = result.unwrap();
        assert_eq!(meta.download_size, None);
        assert_eq!(meta.install_size, None);
    }

    #[test]
    /// What: Ensure AUR metadata fetching extracts sizes from package file when found.
    ///
    /// Inputs:
    /// - Mock package file path and `pacman -Qp` output with install size.
    ///
    /// Output:
    /// - Returns `Ok(OfficialMetadata)` with extracted sizes.
    ///
    /// Details:
    /// - Tests size extraction from package metadata via `pacman -Qp`.
    fn test_extract_aur_package_sizes() {
        // Create a temporary file for testing
        let temp_dir = std::env::temp_dir().join(format!("pacsea_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let pkg_path = temp_dir.join("test-1.0.0-1-x86_64.pkg.tar.zst");
        std::fs::write(&pkg_path, b"fake package data").unwrap();

        // Set up mock response using the actual temp file path
        let mut responses = HashMap::new();
        responses.insert(
            (
                "pacman".into(),
                vec!["-Qp".into(), pkg_path.to_string_lossy().to_string()],
            ),
            Ok("Name            : test\nInstalled Size  : 5.00 MiB\n".to_string()),
        );

        let runner = MockRunner::with(responses);

        let result = extract_aur_package_sizes(&runner, &pkg_path);
        assert!(result.is_ok());
        let meta = result.unwrap();

        // Download size should be the file size (17 bytes in this case)
        assert_eq!(meta.download_size, Some(17));
        // Install size should be parsed from pacman -Qp output
        assert_eq!(meta.install_size, Some(5 * 1024 * 1024));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
