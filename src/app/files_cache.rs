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
    if let Ok(s) = fs::read_to_string(path)
        && let Ok(cache) = serde_json::from_str::<FileCache>(&s)
    {
        // Check if signature matches
        let mut cached_sig = cache.install_list_signature.clone();
        cached_sig.sort();
        let mut current_sig = current_signature.to_vec();
        current_sig.sort();

        if cached_sig == current_sig {
            tracing::info!(path = %path.display(), count = cache.files.len(), "loaded file cache");
            return Some(cache.files);
        } else {
            tracing::debug!(path = %path.display(), "file cache signature mismatch, ignoring");
        }
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
