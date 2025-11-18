//! Package metadata fetching and parsing utilities.
//!
//! This module provides functions to fetch package metadata from pacman and
//! parse the output into structured data.

use super::command::{CommandError, CommandRunner};
use std::collections::HashMap;

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
pub(crate) fn fetch_official_metadata<R: CommandRunner>(
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
pub(crate) fn fetch_installed_version<R: CommandRunner>(
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
pub(crate) fn fetch_installed_size<R: CommandRunner>(
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
pub(crate) fn parse_pacman_key_values(output: &str) -> HashMap<String, String> {
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
pub(crate) fn parse_size_to_bytes(raw: &str) -> Option<u64> {
    let mut parts = raw.split_whitespace();
    let number = parts.next()?.replace(',', ".");
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
