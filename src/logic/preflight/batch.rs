//! Batch fetching utilities for package metadata.
//!
//! This module provides functions to efficiently fetch installed versions and
//! sizes for multiple packages in batches.

use super::command::{CommandError, CommandRunner};
use super::metadata::{
    fetch_installed_size, fetch_installed_version, parse_pacman_key_values, parse_size_to_bytes,
};
use crate::state::types::PackageItem;

/// What: Batch fetch installed versions for multiple packages using `pacman -Q`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `items`: Packages to query.
///
/// Output:
/// - Vector of results, one per package (Ok(version) or Err).
///
/// Details:
/// - Batches queries into chunks of 50 to avoid command-line length limits.
/// - `pacman -Q` outputs "name version" per line, one per package.
pub(super) fn batch_fetch_installed_versions<R: CommandRunner>(
    runner: &R,
    items: &[PackageItem],
) -> Vec<Result<String, CommandError>> {
    const BATCH_SIZE: usize = 50;
    let mut results = Vec::with_capacity(items.len());

    for chunk in items.chunks(BATCH_SIZE) {
        let names: Vec<&str> = chunk.iter().map(|i| i.name.as_str()).collect();
        let mut args = vec!["-Q"];
        args.extend(names.iter().copied());
        match runner.run("pacman", &args) {
            Ok(output) => {
                // Parse output: each line is "name version"
                let mut version_map = std::collections::HashMap::new();
                for line in output.lines() {
                    let mut parts = line.split_whitespace();
                    if let (Some(name), Some(version)) = (parts.next(), parts.next_back()) {
                        version_map.insert(name, version.to_string());
                    }
                }
                // Map results back to original order
                for item in chunk {
                    if let Some(version) = version_map.get(item.name.as_str()) {
                        results.push(Ok(version.clone()));
                    } else {
                        results.push(Err(CommandError::Parse {
                            program: "pacman -Q".to_string(),
                            field: format!("version for {}", item.name),
                        }));
                    }
                }
            }
            Err(_) => {
                // If batch fails, fall back to individual queries
                for item in chunk {
                    match fetch_installed_version(runner, &item.name) {
                        Ok(v) => results.push(Ok(v)),
                        Err(err) => results.push(Err(err)),
                    }
                }
            }
        }
    }
    results
}

/// What: Batch fetch installed sizes for multiple packages using `pacman -Qi`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `items`: Packages to query.
///
/// Output:
/// - Vector of results, one per package (`Ok(size_bytes)` or `Err`).
///
/// Details:
/// - Batches queries into chunks of 50 to avoid command-line length limits.
/// - Parses multi-package `pacman -Qi` output (packages separated by blank lines).
pub(super) fn batch_fetch_installed_sizes<R: CommandRunner>(
    runner: &R,
    items: &[PackageItem],
) -> Vec<Result<u64, CommandError>> {
    const BATCH_SIZE: usize = 50;
    let mut results = Vec::with_capacity(items.len());

    for chunk in items.chunks(BATCH_SIZE) {
        let names: Vec<&str> = chunk.iter().map(|i| i.name.as_str()).collect();
        let mut args = vec!["-Qi"];
        args.extend(names.iter().copied());
        match runner.run("pacman", &args) {
            Ok(output) => {
                // Parse multi-package output: packages are separated by blank lines
                let mut package_blocks = Vec::new();
                let mut current_block = String::new();
                for line in output.lines() {
                    if line.trim().is_empty() {
                        if !current_block.is_empty() {
                            package_blocks.push(current_block.clone());
                            current_block.clear();
                        }
                    } else {
                        current_block.push_str(line);
                        current_block.push('\n');
                    }
                }
                if !current_block.is_empty() {
                    package_blocks.push(current_block);
                }

                // Parse each block to extract package name and size
                let mut size_map = std::collections::HashMap::new();
                for block in package_blocks {
                    let block_fields = parse_pacman_key_values(&block);
                    if let (Some(name), Some(size_str)) = (
                        block_fields.get("Name").map(|s| s.trim()),
                        block_fields.get("Installed Size").map(|s| s.trim()),
                    ) && let Some(size_bytes) = parse_size_to_bytes(size_str)
                    {
                        size_map.insert(name.to_string(), size_bytes);
                    }
                }

                // Map results back to original order
                for item in chunk {
                    if let Some(size) = size_map.get(&item.name) {
                        results.push(Ok(*size));
                    } else {
                        results.push(Err(CommandError::Parse {
                            program: "pacman -Qi".to_string(),
                            field: format!("Installed Size for {}", item.name),
                        }));
                    }
                }
            }
            Err(_) => {
                // If batch fails, fall back to individual queries
                for item in chunk {
                    match fetch_installed_size(runner, &item.name) {
                        Ok(s) => results.push(Ok(s)),
                        Err(err) => results.push(Err(err)),
                    }
                }
            }
        }
    }
    results
}
