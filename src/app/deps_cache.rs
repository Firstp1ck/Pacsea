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
        }
        tracing::debug!(path = %path.display(), "dependency cache signature mismatch, ignoring");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::{DependencyInfo, DependencySource, DependencyStatus};
    use crate::state::{PackageItem, Source};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_deps_cache_{label}_{}_{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
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

    fn sample_dependencies() -> Vec<DependencyInfo> {
        vec![DependencyInfo {
            name: "gcc-libs".into(),
            version: ">=13".into(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".into(),
            },
            required_by: vec!["ripgrep".into()],
            depends_on: Vec::new(),
            is_core: true,
            is_system: false,
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
    /// - Cache saved for ["fd", "ripgrep"] but reloaded with signature ["ripgrep", "zellij"].
    ///
    /// Output:
    /// - `None`.
    fn load_cache_rejects_signature_mismatch() {
        let path = temp_path("mismatch");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let deps = sample_dependencies();
        save_cache(&path, &signature, &deps);

        let mismatched_signature = vec!["ripgrep".into(), "zellij".into()];
        assert!(load_cache(&path, &mismatched_signature).is_none());
        let _ = fs::remove_file(&path);
    }

    #[test]
    /// What: Verify dependency payloads persist and reload unchanged.
    /// Inputs:
    /// - Disk round-trip for the sample dependency list using a matching signature.
    ///
    /// Output:
    /// - Reloaded dependency list matches the original, including status, source, and metadata.
    fn save_and_load_cache_roundtrip() {
        let path = temp_path("roundtrip");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let deps = sample_dependencies();
        let expected = deps.clone();
        save_cache(&path, &signature, &deps);

        let reloaded = load_cache(&path, &signature).expect("expected cache to load");
        assert_eq!(reloaded.len(), expected.len());
        let dep = &reloaded[0];
        let expected_dep = &expected[0];
        assert_eq!(dep.name, expected_dep.name);
        assert_eq!(dep.version, expected_dep.version);
        assert!(matches!(dep.status, DependencyStatus::ToInstall));
        assert!(matches!(
            dep.source,
            DependencySource::Official { ref repo } if repo == "core"
        ));
        assert_eq!(dep.required_by, expected_dep.required_by);
        assert_eq!(dep.depends_on, expected_dep.depends_on);
        assert_eq!(dep.is_core, expected_dep.is_core);
        assert_eq!(dep.is_system, expected_dep.is_system);

        let _ = fs::remove_file(&path);
    }
}
