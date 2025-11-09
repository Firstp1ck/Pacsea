//! Dependency cache persistence for install list dependencies.

use crate::state::modal::DependencyInfo;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// What: Cache blob combining install list signature with resolved dependency graph.
///
/// Details:
/// - `install_list_signature` stores sorted package names so cache survives reordering.
/// - `dependencies` mirrors the resolved dependency payload persisted on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCache {
    /// Sorted list of package names from install list (used as signature).
    pub install_list_signature: Vec<String>,
    /// Cached resolved dependencies.
    pub dependencies: Vec<DependencyInfo>,
}

/// What: Generate a deterministic signature for an install list that ignores ordering.
///
/// Inputs:
/// - `packages`: Slice of install list entries used to derive package names.
///
/// Output:
/// - Sorted vector of package names that can be compared between cache reads and writes.
///
/// Details:
/// - Clones the package names and sorts them alphabetically to create an order-agnostic key.
pub fn compute_signature(packages: &[crate::state::PackageItem]) -> Vec<String> {
    let mut names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

/// What: Load dependency cache from disk when the stored signature matches the current list.
///
/// Inputs:
/// - `path`: Filesystem location of the serialized `DependencyCache` JSON.
/// - `current_signature`: Signature derived from the current install list for validation.
///
/// Output:
/// - `Some(Vec<DependencyInfo>)` when the cache exists, deserializes, and signatures match;
///   `None` otherwise.
///
/// Details:
/// - Reads the JSON, deserializes, sorts both signatures, and compares for equality before
///   returning the cached dependencies.
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

/// What: Persist dependency cache payload and signature to disk as JSON.
///
/// Inputs:
/// - `path`: Destination file for the serialized cache contents.
/// - `signature`: Current install list signature to write alongside the payload.
/// - `dependencies`: Resolved dependency details being cached.
///
/// Output:
/// - No return value; writes to disk best-effort and logs on success.
///
/// Details:
/// - Serializes the data to JSON, writes it to `path`, and emits a debug log including count.
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
