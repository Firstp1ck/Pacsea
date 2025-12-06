//! Service cache persistence for install list service impacts.

use crate::state::modal::ServiceImpact;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

/// What: Cache blob combining install list signature with resolved service impact metadata.
///
/// Details:
/// - `install_list_signature` mirrors package names used for cache validation.
/// - `services` preserves the last known service impact data for reuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCache {
    /// Sorted list of package names from install list (used as signature).
    pub install_list_signature: Vec<String>,
    /// Cached resolved service impacts.
    pub services: Vec<ServiceImpact>,
}

/// What: Generate a deterministic signature for service cache comparisons.
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

/// What: Load cached service impact data when the stored signature matches the current list.
///
/// Inputs:
/// - `path`: Filesystem location of the serialized `ServiceCache` JSON.
/// - `current_signature`: Signature derived from the current install list for validation.
///
/// Output:
/// - `Some(Vec<ServiceImpact>)` when the cache exists, deserializes, and signatures agree;
///   `None` otherwise.
///
/// Details:
/// - Reads the JSON, deserializes it, sorts both signatures, and compares them before
///   returning the cached service impact data.
pub fn load_cache(path: &PathBuf, current_signature: &[String]) -> Option<Vec<ServiceImpact>> {
    let raw = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            tracing::debug!(path = %path.display(), "[Cache] Service cache not found");
            return None;
        }
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "[Cache] Failed to read service cache"
            );
            return None;
        }
    };

    let cache: ServiceCache = match serde_json::from_str(&raw) {
        Ok(cache) => cache,
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "[Cache] Failed to parse service cache"
            );
            return None;
        }
    };

    // Check if signature matches
    let mut cached_sig = cache.install_list_signature.clone();
    cached_sig.sort();
    let mut current_sig = current_signature.to_vec();
    current_sig.sort();

    if cached_sig == current_sig {
        tracing::info!(path = %path.display(), count = cache.services.len(), "loaded service cache");
        return Some(cache.services);
    }
    tracing::debug!(path = %path.display(), "service cache signature mismatch, ignoring");
    None
}

/// What: Persist service impact cache payload and signature to disk as JSON.
///
/// Inputs:
/// - `path`: Destination file for the serialized cache contents.
/// - `signature`: Current install list signature to store alongside the payload.
/// - `services`: Service impact metadata being cached.
///
/// Output:
/// - No return value; writes to disk best-effort and logs a debug message when successful.
///
/// Details:
/// - Serializes the data to JSON, writes it to `path`, and includes the record count in logs.
pub fn save_cache(path: &PathBuf, signature: &[String], services: &[ServiceImpact]) {
    let cache = ServiceCache {
        install_list_signature: signature.to_vec(),
        services: services.to_vec(),
    };
    if let Ok(s) = serde_json::to_string(&cache) {
        let _ = fs::write(path, s);
        tracing::debug!(path = %path.display(), count = services.len(), "saved service cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::{ServiceImpact, ServiceRestartDecision};
    use crate::state::{PackageItem, Source};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_services_cache_{label}_{}_{}.json",
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
                name: "sshd".into(),
                version: "9.0.0".into(),
                description: String::new(),
                source: Source::Official {
                    repo: "core".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
                out_of_date: None,
                orphaned: false,
            },
            PackageItem {
                name: "nginx".into(),
                version: "1.24.0".into(),
                description: String::new(),
                source: Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
                out_of_date: None,
                orphaned: false,
            },
        ]
    }

    fn sample_services() -> Vec<ServiceImpact> {
        vec![ServiceImpact {
            unit_name: "sshd.service".into(),
            providers: vec!["sshd".into()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        }]
    }

    #[test]
    /// What: Ensure `compute_signature` normalizes package name ordering.
    /// Inputs:
    /// - Install list cloned from the sample data but iterated in reverse.
    ///
    /// Output:
    /// - Signature equals `["nginx", "sshd"]`.
    fn compute_signature_orders_package_names() {
        let mut packages = sample_packages();
        packages.reverse();
        let signature = compute_signature(&packages);
        assert_eq!(signature, vec![String::from("nginx"), String::from("sshd")]);
    }

    #[test]
    /// What: Confirm `load_cache` rejects persisted caches whose signature does not match.
    /// Inputs:
    /// - Cache saved for `["nginx", "sshd"]` but reloaded with signature `["sshd", "httpd"]`.
    ///
    /// Output:
    /// - `None`.
    fn load_cache_rejects_signature_mismatch() {
        let path = temp_path("mismatch");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let services = sample_services();
        save_cache(&path, &signature, &services);

        let mismatched_signature = vec!["sshd".into(), "httpd".into()];
        assert!(load_cache(&path, &mismatched_signature).is_none());
        let _ = fs::remove_file(&path);
    }

    #[test]
    /// What: Verify cached service metadata survives a save/load round trip.
    /// Inputs:
    /// - Sample `sshd.service` impact written to disk and reloaded with matching signature.
    ///
    /// Output:
    /// - Reloaded metadata matches the original unit name and properties.
    fn save_and_load_cache_roundtrip() {
        let path = temp_path("roundtrip");
        let packages = sample_packages();
        let signature = compute_signature(&packages);
        let services = sample_services();
        save_cache(&path, &signature, &services);

        let reloaded = load_cache(&path, &signature).expect("expected cache to load");
        assert_eq!(reloaded.len(), services.len());
        assert_eq!(reloaded[0].unit_name, services[0].unit_name);
        assert_eq!(reloaded[0].providers, services[0].providers);
        assert_eq!(reloaded[0].is_active, services[0].is_active);
        assert_eq!(reloaded[0].needs_restart, services[0].needs_restart);

        let _ = fs::remove_file(&path);
    }
}
