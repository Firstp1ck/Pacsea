//! File cache persistence for install list file changes.

use crate::state::modal::PackageFileInfo;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Cached file data with install list signature for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCache {
    /// Sorted list of package names from install list (used as signature).
    pub install_list_signature: Vec<String>,
    /// Cached resolved file changes.
    pub files: Vec<PackageFileInfo>,
}

/// Compute a signature from the install list (sorted package names).
pub fn compute_signature(packages: &[crate::state::PackageItem]) -> Vec<String> {
    let mut names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

/// Load file cache from disk if it exists and matches the install list signature.
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

/// Save file cache to disk.
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
