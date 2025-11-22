//! File cache persistence for install list file changes.

use crate::state::modal::PackageFileInfo;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// What: Cache blob combining install list signature with resolved file change metadata.
///
/// Details:
/// - `install_list_signature` mirrors package names used for cache validation.
/// - `files` preserves the last known file change data for reuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCache {
    /// Sorted list of package names from install list (used as signature).
    pub install_list_signature: Vec<String>,
    /// Cached resolved file changes.
    pub files: Vec<PackageFileInfo>,
}

/// What: Generate a deterministic signature for file cache comparisons.
///
/// Inputs:
/// - `packages`: Slice of install list entries contributing their package names.
///
/// Output:
/// - Sorted vector of package names that can be compared for cache validity checks.
///
/// Details:
/// - Clones each package name and sorts the collection alphabetically.
pub fn compute_signature(packages: &[crate::state::PackageItem]) -> Vec<String> {
    let mut names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

/// What: Load cached file change data when the stored signature matches the current list.
///
/// Inputs:
/// - `path`: Filesystem location of the serialized `FileCache` JSON.
/// - `current_signature`: Signature derived from the current install list for validation.
///
/// Output:
/// - `Some(Vec<PackageFileInfo>)` when the cache exists, deserializes, and signatures agree;
///   `None` otherwise.
///
/// Details:
/// - Reads the JSON, deserializes it, sorts both signatures, and compares them before
///   returning the cached file change data.
pub fn load_cache(path: &PathBuf, current_signature: &[String]) -> Option<Vec<PackageFileInfo>> {
    load_cache_partial(path, current_signature, true)
}

/// What: Load cached file change data with partial matching support.
///
/// Inputs:
/// - `path`: Filesystem location of the serialized `FileCache` JSON.
/// - `current_signature`: Signature derived from the current selection for validation.
/// - `exact_match_only`: If true, only match when signatures are identical. If false, allow subset matching.
///
/// Output:
/// - `Some(Vec<PackageFileInfo>)` when the cache exists and matches (exact or partial);
///   `None` otherwise.
///
/// Details:
/// - If `exact_match_only` is false and the current signature is a subset of the cached signature,
///   filters the cached results to match the current selection.
pub fn load_cache_partial(
    path: &PathBuf,
    current_signature: &[String],
    exact_match_only: bool,
) -> Option<Vec<PackageFileInfo>> {
    if let Ok(s) = fs::read_to_string(path)
        && let Ok(cache) = serde_json::from_str::<FileCache>(&s)
    {
        // Check if signature matches
        let mut cached_sig = cache.install_list_signature.clone();
        cached_sig.sort();
        let mut current_sig = current_signature.to_vec();
        current_sig.sort();

        if cached_sig == current_sig {
            tracing::info!(path = %path.display(), count = cache.files.len(), "loaded file cache (exact match)");
            return Some(cache.files);
        } else if !exact_match_only {
            // Check if current signature is a subset of cached signature
            let cached_set: std::collections::HashSet<&String> = cached_sig.iter().collect();
            let current_set: std::collections::HashSet<&String> = current_sig.iter().collect();

            if current_set.is_subset(&cached_set) {
                // Filter cached results to match current selection
                let current_names: std::collections::HashSet<&str> =
                    current_sig.iter().map(std::string::String::as_str).collect();
                let filtered: Vec<PackageFileInfo> = cache
                    .files
                    .iter()
                    .filter(|file_info| current_names.contains(file_info.name.as_str()))
                    .cloned()
                    .collect();

                if !filtered.is_empty() {
                    tracing::info!(
                        path = %path.display(),
                        cached_count = cache.files.len(),
                        filtered_count = filtered.len(),
                        "loaded file cache (partial match)"
                    );
                    return Some(filtered);
                }
            }
        }

        tracing::debug!(path = %path.display(), "file cache signature mismatch, ignoring");
    }
    None
}

/// What: Persist file change cache payload and signature to disk as JSON.
///
/// Inputs:
/// - `path`: Destination file for the serialized cache contents.
/// - `signature`: Current install list signature to store alongside the payload.
/// - `files`: File change metadata being cached.
///
/// Output:
/// - No return value; writes to disk best-effort and logs a debug message when successful.
///
/// Details:
/// - Serializes the data to JSON, writes it to `path`, and includes the record count in logs.
pub fn save_cache(path: &PathBuf, signature: &[String], files: &[PackageFileInfo]) {
    let cache = FileCache {
        install_list_signature: signature.to_vec(),
        files: files.to_vec(),
    };
    if let Ok(s) = serde_json::to_string(&cache) {
        let _ = fs::write(path, s);
        tracing::debug!(path = %path.display(), count = files.len(), "saved file cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::{FileChange, FileChangeType, PackageFileInfo};
    use crate::state::{PackageItem, Source};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_files_cache_{label}_{}_{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        path
    }

    fn sample_packages() -> Vec<PackageItem> {
        vec![
            PackageItem {
                name: "ripgrep".into(),
                version: "14.0.0".into(),
                description: String::new(),
                source: Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            },
            PackageItem {
                name: "fd".into(),
                version: "9.0.0".into(),
                description: String::new(),
                source: Source::Aur,
                popularity: Some(42.0),
            },
        ]
    }

    fn sample_file_infos() -> Vec<PackageFileInfo> {
        vec![PackageFileInfo {
            name: "ripgrep".into(),
            files: vec![FileChange {
                path: "/usr/bin/rg".into(),
                change_type: FileChangeType::New,
                package: "ripgrep".into(),
                is_config: false,
                predicted_pacnew: false,
                predicted_pacsave: false,
            }],
            total_count: 1,
            new_count: 1,
            changed_count: 0,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        }]
    }

    #[test]
    /// What: Ensure `compute_signature` normalizes package name ordering.
    /// Inputs:
    /// - Install list cloned from the sample data but iterated in reverse.
    ///
    /// Output:
    /// - Signature equals `["fd", "ripgrep"]`.
    fn compute_signature_orders_package_names() {
        let mut packages = sample_packages();
        packages.reverse();
        let signature = compute_signature(&packages);
        assert_eq!(signature, vec![String::from("fd"), String::from("ripgrep")]);
    }

    #[test]
    /// What: Confirm `load_cache` rejects persisted caches whose signature does not match.
    /// Inputs:
    /// - Cache saved for `["fd", "ripgrep"]` but reloaded with signature `["ripgrep", "zellij"]`.
    ///
    /// Output:
    /// - `None`.
    fn load_cache_rejects_signature_mismatch() {
        let path = temp_path("mismatch");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let file_infos = sample_file_infos();
        save_cache(&path, &signature, &file_infos);

        let mismatched_signature = vec!["ripgrep".into(), "zellij".into()];
        assert!(load_cache(&path, &mismatched_signature).is_none());
        let _ = fs::remove_file(&path);
    }

    #[test]
    /// What: Verify cached file metadata survives a save/load round trip.
    /// Inputs:
    /// - Sample `ripgrep` file info written to disk and reloaded with matching signature.
    ///
    /// Output:
    /// - Reloaded metadata matches the original counts and file entries.
    fn save_and_load_cache_roundtrip() {
        let path = temp_path("roundtrip");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let file_infos = sample_file_infos();
        save_cache(&path, &signature, &file_infos);

        let reloaded = load_cache(&path, &signature).expect("expected cache to load");
        assert_eq!(reloaded.len(), file_infos.len());
        assert_eq!(reloaded[0].name, file_infos[0].name);
        assert_eq!(reloaded[0].files.len(), file_infos[0].files.len());
        assert_eq!(reloaded[0].total_count, file_infos[0].total_count);
        assert_eq!(reloaded[0].new_count, file_infos[0].new_count);

        let _ = fs::remove_file(&path);
    }
}
