//! Sandbox cache persistence for install list sandbox analysis.

use crate::logic::sandbox::SandboxInfo;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// What: Cache blob combining install list signature with resolved sandbox metadata.
///
/// Details:
/// - `install_list_signature` mirrors package names used for cache validation.
/// - `sandbox_info` preserves the last known sandbox analysis data for reuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCache {
    /// Sorted list of package names from install list (used as signature).
    pub install_list_signature: Vec<String>,
    /// Cached resolved sandbox information.
    pub sandbox_info: Vec<SandboxInfo>,
}

/// What: Generate a deterministic signature for sandbox cache comparisons.
///
/// Inputs:
/// - `packages`: Slice of install list entries contributing their package names.
///
/// Output:
/// - Sorted vector of package names that can be compared for cache validity checks.
///
/// Details:
/// - Clones each package name and sorts the collection alphabetically.
#[must_use]
pub fn compute_signature(packages: &[crate::state::PackageItem]) -> Vec<String> {
    let mut names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

/// What: Load cached sandbox data when the stored signature matches the current list.
///
/// Inputs:
/// - `path`: Filesystem location of the serialized `SandboxCache` JSON.
/// - `current_signature`: Signature derived from the current install list for validation.
///
/// Output:
/// - `Some(Vec<SandboxInfo>)` when the cache exists, deserializes, and signatures agree;
///   `None` otherwise.
///
/// Details:
/// - Reads the JSON, deserializes it, sorts both signatures, and compares them before
///   returning the cached sandbox data.
/// - Uses partial matching to load entries for packages that exist in both cache and current list.
#[must_use]
pub fn load_cache(path: &PathBuf, current_signature: &[String]) -> Option<Vec<SandboxInfo>> {
    load_cache_partial(path, current_signature, false)
}

/// What: Load cached sandbox data with partial matching support.
///
/// Inputs:
/// - `path`: Filesystem location of the serialized `SandboxCache` JSON.
/// - `current_signature`: Signature derived from the current install list for validation.
/// - `exact_match_only`: If true, only match when signatures are identical. If false, allow partial matching.
///
/// Output:
/// - `Some(Vec<SandboxInfo>)` when the cache exists and matches (exact or partial);
///   `None` otherwise.
///
/// Details:
/// - If `exact_match_only` is false, loads entries for packages that exist in both
///   the cached signature and the current signature (intersection matching).
/// - This allows preserving sandbox data when packages are added to the install list.
pub fn load_cache_partial(
    path: &PathBuf,
    current_signature: &[String],
    exact_match_only: bool,
) -> Option<Vec<SandboxInfo>> {
    if let Ok(s) = fs::read_to_string(path)
        && let Ok(cache) = serde_json::from_str::<SandboxCache>(&s)
    {
        // Check if signature matches exactly
        let mut cached_sig = cache.install_list_signature.clone();
        cached_sig.sort();
        let mut current_sig = current_signature.to_vec();
        current_sig.sort();

        if cached_sig == current_sig {
            tracing::info!(
                path = %path.display(),
                count = cache.sandbox_info.len(),
                "loaded sandbox cache (exact match)"
            );
            return Some(cache.sandbox_info);
        } else if !exact_match_only {
            // Partial matching: load entries for packages that exist in both signatures
            let cached_set: std::collections::HashSet<&String> = cached_sig.iter().collect();
            let current_set: std::collections::HashSet<&String> = current_sig.iter().collect();

            // Find intersection: packages that exist in both cache and current list
            let intersection: std::collections::HashSet<&String> =
                cached_set.intersection(&current_set).copied().collect();

            if !intersection.is_empty() {
                // Filter cached results to match packages in intersection
                let intersection_names: std::collections::HashSet<&str> =
                    intersection.iter().map(|s| s.as_str()).collect();
                let filtered: Vec<SandboxInfo> = cache
                    .sandbox_info
                    .iter()
                    .filter(|sandbox_info| {
                        intersection_names.contains(sandbox_info.package_name.as_str())
                    })
                    .cloned()
                    .collect();

                if !filtered.is_empty() {
                    tracing::info!(
                        path = %path.display(),
                        cached_count = cache.sandbox_info.len(),
                        filtered_count = filtered.len(),
                        intersection_packages = ?intersection.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        "loaded sandbox cache (partial match)"
                    );
                    return Some(filtered);
                }
            }
        }

        tracing::debug!(
            path = %path.display(),
            "sandbox cache signature mismatch, ignoring"
        );
    }
    None
}

/// What: Persist sandbox cache payload and signature to disk as JSON.
///
/// Inputs:
/// - `path`: Destination file for the serialized cache contents.
/// - `signature`: Current install list signature to store alongside the payload.
/// - `sandbox_info`: Sandbox analysis metadata being cached.
///
/// Output:
/// - No return value; writes to disk best-effort and logs a debug message when successful.
///
/// Details:
/// - Serializes the data to JSON, writes it to `path`, and includes the record count in logs.
pub fn save_cache(path: &PathBuf, signature: &[String], sandbox_info: &[SandboxInfo]) {
    let cache = SandboxCache {
        install_list_signature: signature.to_vec(),
        sandbox_info: sandbox_info.to_vec(),
    };
    if let Ok(s) = serde_json::to_string(&cache) {
        let _ = fs::write(path, s);
        tracing::debug!(
            path = %path.display(),
            count = sandbox_info.len(),
            "saved sandbox cache"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::sandbox::{DependencyDelta, SandboxInfo};
    use crate::state::{PackageItem, Source};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_sandbox_cache_{label}_{}_{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        path
    }

    fn sample_packages() -> Vec<PackageItem> {
        vec![PackageItem {
            name: "yay".into(),
            version: "12.0.0".into(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }]
    }

    fn sample_sandbox_info() -> Vec<SandboxInfo> {
        vec![SandboxInfo {
            package_name: "yay".into(),
            depends: vec![DependencyDelta {
                name: "go".into(),
                is_installed: true,
                installed_version: Some("1.21.0".into()),
                version_satisfied: true,
            }],
            makedepends: vec![],
            checkdepends: vec![],
            optdepends: vec![],
        }]
    }

    #[test]
    /// What: Ensure `compute_signature` normalizes package name ordering.
    /// Inputs:
    /// - Install list cloned from the sample data but iterated in reverse.
    ///
    /// Output:
    /// - Signature equals `["yay"]`.
    fn compute_signature_orders_package_names() {
        let mut packages = sample_packages();
        packages.reverse();
        let signature = compute_signature(&packages);
        assert_eq!(signature, vec![String::from("yay")]);
    }

    #[test]
    /// What: Confirm `load_cache` rejects persisted caches whose signature does not match.
    /// Inputs:
    /// - Cache saved for `["yay"]` but reloaded with signature `["paru"]`.
    ///
    /// Output:
    /// - `None`.
    fn load_cache_rejects_signature_mismatch() {
        let path = temp_path("mismatch");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let sandbox_info = sample_sandbox_info();
        save_cache(&path, &signature, &sandbox_info);

        let mismatched_signature = vec!["paru".into()];
        assert!(load_cache(&path, &mismatched_signature).is_none());
        let _ = fs::remove_file(&path);
    }

    #[test]
    /// What: Verify cached sandbox metadata survives a save/load round trip.
    /// Inputs:
    /// - Sample `yay` sandbox info written to disk and reloaded with matching signature.
    ///
    /// Output:
    /// - Reloaded metadata matches the original package name and properties.
    fn save_and_load_cache_roundtrip() {
        let path = temp_path("roundtrip");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let sandbox_info = sample_sandbox_info();
        save_cache(&path, &signature, &sandbox_info);

        let reloaded = load_cache(&path, &signature).expect("expected cache to load");
        assert_eq!(reloaded.len(), sandbox_info.len());
        assert_eq!(reloaded[0].package_name, sandbox_info[0].package_name);
        assert_eq!(reloaded[0].depends.len(), sandbox_info[0].depends.len());

        let _ = fs::remove_file(&path);
    }

    #[test]
    /// What: Verify partial cache loading preserves entries when new packages are added.
    /// Inputs:
    /// - Cache saved for `["jujutsu-git"]` but reloaded with signature `["jujutsu-git", "pacsea-bin"]`.
    ///
    /// Output:
    /// - Returns `Some(Vec<SandboxInfo>)` containing only `jujutsu-git` entry (partial match).
    fn load_cache_partial_match() {
        let path = temp_path("partial");
        let jujutsu_sandbox = SandboxInfo {
            package_name: "jujutsu-git".into(),
            depends: vec![DependencyDelta {
                name: "python".into(),
                is_installed: true,
                installed_version: Some("3.11.0".into()),
                version_satisfied: true,
            }],
            makedepends: vec![],
            checkdepends: vec![],
            optdepends: vec![],
        };
        let signature = vec!["jujutsu-git".into()];
        save_cache(&path, &signature, std::slice::from_ref(&jujutsu_sandbox));

        // Try to load with expanded signature (new package added)
        let expanded_signature = vec!["jujutsu-git".into(), "pacsea-bin".into()];
        let reloaded =
            load_cache(&path, &expanded_signature).expect("expected partial cache to load");

        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].package_name, "jujutsu-git");
        assert_eq!(reloaded[0].depends.len(), 1);
        assert_eq!(reloaded[0].depends[0].name, "python");

        let _ = fs::remove_file(&path);
    }

    #[test]
    /// What: Verify partial cache loading returns None when no packages overlap.
    /// Inputs:
    /// - Cache saved for `["jujutsu-git"]` but reloaded with signature `["pacsea-bin"]`.
    ///
    /// Output:
    /// - Returns `None` (no overlap).
    fn load_cache_partial_no_overlap() {
        let path = temp_path("no_overlap");
        let jujutsu_sandbox = sample_sandbox_info();
        let signature = vec!["jujutsu-git".into()];
        save_cache(&path, &signature, &jujutsu_sandbox);

        let different_signature = vec!["pacsea-bin".into()];
        assert!(load_cache(&path, &different_signature).is_none());

        let _ = fs::remove_file(&path);
    }
}
