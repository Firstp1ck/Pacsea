//! Dependency cache persistence for install list dependencies.

use crate::state::modal::DependencyInfo;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Cached dependency data with install list signature for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCache {
    /// Sorted list of package names from install list (used as signature).
    pub install_list_signature: Vec<String>,
    /// Cached resolved dependencies.
    pub dependencies: Vec<DependencyInfo>,
}

/// Compute a signature from the install list (sorted package names).
pub fn compute_signature(packages: &[crate::state::PackageItem]) -> Vec<String> {
    let mut names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

/// Load dependency cache from disk if it exists and matches the install list signature.
pub fn load_cache(path: &PathBuf, current_signature: &[String]) -> Option<Vec<DependencyInfo>> {
    if let Ok(s) = fs::read_to_string(path)
        && let Ok(cache) = serde_json::from_str::<DependencyCache>(&s)
    {
        // Check if signature matches
        let mut cached_sig = cache.install_list_signature.clone();
        cached_sig.sort();
        let mut current_sig = current_signature.to_vec();
        current_sig.sort();

        if cached_sig == current_sig {
            tracing::info!(path = %path.display(), count = cache.dependencies.len(), "loaded dependency cache");
            return Some(cache.dependencies);
        } else {
            tracing::debug!(path = %path.display(), "dependency cache signature mismatch, ignoring");
        }
    }
    None
}

/// Save dependency cache to disk.
pub fn save_cache(path: &PathBuf, signature: &[String], dependencies: &[DependencyInfo]) {
    let cache = DependencyCache {
        install_list_signature: signature.to_vec(),
        dependencies: dependencies.to_vec(),
    };
    if let Ok(s) = serde_json::to_string(&cache) {
        let _ = fs::write(path, s);
        tracing::debug!(path = %path.display(), count = dependencies.len(), "saved dependency cache");
    }
}
