//! Shared machinery for signature-validated JSON cache modules.
//!
//! The install-list caches (`deps_cache`, `files_cache`, `services_cache`,
//! `sandbox_cache`) all persist the same envelope shape to disk: a sorted
//! install-list signature plus a payload vector. This module hosts the common
//! read/parse, signature-matching, filtering, and save logic so the per-cache
//! modules only keep their on-disk struct definition and thin wrappers.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// What: Strategy used to decide whether a cached signature satisfies the
/// current install-list signature.
///
/// Details:
/// - `Exact` requires both signatures to contain the same names (order-insensitive).
/// - `Subset` accepts caches whose signature is a superset of the current one,
///   allowing cached payloads to be filtered down to the current selection.
/// - `Intersection` accepts caches that share at least one name with the
///   current signature, filtering the payload to the shared names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMatchMode {
    /// Signatures must contain exactly the same package names.
    Exact,
    /// The current signature must be a subset of the cached signature.
    Subset,
    /// The signatures must share at least one package name.
    Intersection,
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
#[must_use]
pub fn compute_signature(packages: &[crate::state::PackageItem]) -> Vec<String> {
    let mut names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

/// What: Read and deserialize a signed cache envelope from disk.
///
/// Inputs:
/// - `path`: Filesystem location of the serialized cache JSON.
/// - `title_label`: Capitalized cache name used at the start of log sentences
///   (e.g. `"Dependency"`).
/// - `lower_label`: Lowercase cache name used mid-sentence in log messages
///   (e.g. `"dependency"`).
///
/// Output:
/// - `Some(C)` when the file exists and parses as `C`; `None` otherwise.
///
/// Details:
/// - A missing file logs at debug level; read or parse failures log at warn
///   level. All failure paths return `None` so callers can fall back to
///   background resolution.
pub fn load_signed_cache<C: DeserializeOwned>(
    path: &Path,
    title_label: &str,
    lower_label: &str,
) -> Option<C> {
    let raw = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            tracing::debug!(path = %path.display(), "[Cache] {title_label} cache not found");
            return None;
        }
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "[Cache] Failed to read {lower_label} cache"
            );
            return None;
        }
    };

    match serde_json::from_str(&raw) {
        Ok(cache) => Some(cache),
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "[Cache] Failed to parse {lower_label} cache"
            );
            None
        }
    }
}

/// What: Check whether a cached signature satisfies the current one under a match mode.
///
/// Inputs:
/// - `mode`: Matching strategy (`Exact`, `Subset`, or `Intersection`).
/// - `current`: Signature derived from the current install list.
/// - `cached`: Signature stored alongside the cached payload.
///
/// Output:
/// - `true` when the signatures match under `mode`; `false` otherwise.
///
/// Details:
/// - `Exact` sorts both signatures and compares for equality (duplicates matter).
/// - `Subset` checks that every current name exists in the cached signature.
/// - `Intersection` checks that at least one current name exists in the cached signature.
#[must_use]
pub fn signature_matches(mode: CacheMatchMode, current: &[String], cached: &[String]) -> bool {
    match mode {
        CacheMatchMode::Exact => {
            let mut current_sig = current.to_vec();
            current_sig.sort();
            let mut cached_sig = cached.to_vec();
            cached_sig.sort();
            current_sig == cached_sig
        }
        CacheMatchMode::Subset => {
            let cached_set: HashSet<&String> = cached.iter().collect();
            current.iter().all(|name| cached_set.contains(name))
        }
        CacheMatchMode::Intersection => {
            let cached_set: HashSet<&String> = cached.iter().collect();
            current.iter().any(|name| cached_set.contains(name))
        }
    }
}

/// What: Return the cached payload when signatures match exactly, logging the outcome.
///
/// Inputs:
/// - `path`: Cache file path used for log context.
/// - `lower_label`: Lowercase cache name used in log messages (e.g. `"dependency"`).
/// - `current_signature`: Signature derived from the current install list.
/// - `cached_signature`: Signature stored alongside the cached payload.
/// - `items`: Deserialized cache payload to hand back on a match.
///
/// Output:
/// - `Some(items)` when the signatures match exactly; `None` otherwise.
///
/// Details:
/// - Logs `loaded {lower_label} cache` at info level on a match and
///   `{lower_label} cache signature mismatch, ignoring` at debug level otherwise.
pub fn take_exact_match<T>(
    path: &Path,
    lower_label: &str,
    current_signature: &[String],
    cached_signature: &[String],
    items: Vec<T>,
) -> Option<Vec<T>> {
    if signature_matches(CacheMatchMode::Exact, current_signature, cached_signature) {
        tracing::info!(path = %path.display(), count = items.len(), "loaded {lower_label} cache");
        return Some(items);
    }
    tracing::debug!(path = %path.display(), "{lower_label} cache signature mismatch, ignoring");
    None
}

/// What: Return the cached payload (possibly filtered) when signatures match under a mode.
///
/// Inputs:
/// - `mode`: Matching strategy applied after an exact comparison fails.
/// - `path`: Cache file path used for log context.
/// - `lower_label`: Lowercase cache name used in log messages (e.g. `"file"`).
/// - `current_signature`: Signature derived from the current install list.
/// - `cached_signature`: Signature stored alongside the cached payload.
/// - `items`: Deserialized cache payload to hand back (whole or filtered).
/// - `key`: Accessor mapping a payload item to the package name it belongs to.
///
/// Output:
/// - `Some(items)` on an exact signature match; `Some(filtered)` on a
///   non-empty partial match under `Subset`/`Intersection`; `None` otherwise.
///
/// Details:
/// - An exact match always wins and returns the full payload.
/// - Partial matches filter `items` to the names shared by both signatures and
///   only succeed when the filtered payload is non-empty.
/// - Logs `(exact match)`/`(partial match)` variants at info level and a
///   signature mismatch at debug level.
pub fn take_signature_match<T>(
    mode: CacheMatchMode,
    path: &Path,
    lower_label: &str,
    current_signature: &[String],
    cached_signature: &[String],
    items: Vec<T>,
    key: impl Fn(&T) -> &str,
) -> Option<Vec<T>> {
    if signature_matches(CacheMatchMode::Exact, current_signature, cached_signature) {
        tracing::info!(
            path = %path.display(),
            count = items.len(),
            "loaded {lower_label} cache (exact match)"
        );
        return Some(items);
    }
    if mode != CacheMatchMode::Exact && signature_matches(mode, current_signature, cached_signature)
    {
        let cached_set: HashSet<&str> = cached_signature.iter().map(String::as_str).collect();
        let shared: Vec<&str> = current_signature
            .iter()
            .map(String::as_str)
            .filter(|name| cached_set.contains(name))
            .collect();
        let shared_set: HashSet<&str> = shared.iter().copied().collect();
        let cached_count = items.len();
        let filtered: Vec<T> = items
            .into_iter()
            .filter(|item| shared_set.contains(key(item)))
            .collect();
        if !filtered.is_empty() {
            log_partial_match(
                mode,
                path,
                lower_label,
                cached_count,
                filtered.len(),
                &shared,
            );
            return Some(filtered);
        }
    }
    tracing::debug!(path = %path.display(), "{lower_label} cache signature mismatch, ignoring");
    None
}

/// What: Emit the info-level log line for a successful partial cache match.
///
/// Inputs:
/// - `mode`: Matching strategy that produced the partial match.
/// - `path`: Cache file path used for log context.
/// - `lower_label`: Lowercase cache name used in log messages.
/// - `cached_count`: Number of payload items stored in the cache before filtering.
/// - `filtered_count`: Number of payload items remaining after filtering.
/// - `shared`: Package names present in both signatures.
///
/// Output:
/// - No return value; writes a structured tracing event.
///
/// Details:
/// - `Intersection` mode additionally records the shared package names, matching
///   the historical sandbox cache log format.
fn log_partial_match(
    mode: CacheMatchMode,
    path: &Path,
    lower_label: &str,
    cached_count: usize,
    filtered_count: usize,
    shared: &[&str],
) {
    if mode == CacheMatchMode::Intersection {
        tracing::info!(
            path = %path.display(),
            cached_count,
            filtered_count,
            intersection_packages = ?shared,
            "loaded {lower_label} cache (partial match)"
        );
    } else {
        tracing::info!(
            path = %path.display(),
            cached_count,
            filtered_count,
            "loaded {lower_label} cache (partial match)"
        );
    }
}

/// What: Serialize a signed cache envelope to compact JSON and write it to disk.
///
/// Inputs:
/// - `path`: Destination file for the serialized cache contents.
/// - `cache`: Envelope value (signature plus payload) to serialize.
/// - `item_count`: Number of payload items, used for log context.
/// - `lower_label`: Lowercase cache name used in log messages (e.g. `"service"`).
///
/// Output:
/// - No return value; writes to disk best-effort and logs at debug level.
///
/// Details:
/// - Uses `serde_json::to_string` (non-pretty) to keep the on-disk format
///   byte-compatible with the historical per-module implementations.
/// - Write errors are intentionally ignored, matching prior best-effort behavior.
pub fn save_signed_cache<C: Serialize>(
    path: &Path,
    cache: &C,
    item_count: usize,
    lower_label: &str,
) {
    if let Ok(s) = serde_json::to_string(cache) {
        let _ = fs::write(path, s);
        tracing::debug!(path = %path.display(), count = item_count, "saved {lower_label} cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Build a `Vec<String>` signature from string literals.
    ///
    /// Inputs:
    /// - `names`: Slice of package name literals.
    ///
    /// Output:
    /// - Owned signature vector for use with the matching helpers.
    ///
    /// Details:
    /// - Test-only convenience to keep assertions terse.
    fn sig(names: &[&str]) -> Vec<String> {
        names.iter().map(ToString::to_string).collect()
    }

    #[test]
    /// What: Verify `Exact` mode matches order-insensitively and rejects differing sets.
    /// Inputs:
    /// - Equal sets in different orders; a differing set; duplicate-name sets.
    ///
    /// Output:
    /// - Matches for reordered equal sets, rejections for differing or duplicated sets.
    fn signature_matches_exact_mode() {
        assert!(signature_matches(
            CacheMatchMode::Exact,
            &sig(&["b", "a"]),
            &sig(&["a", "b"])
        ));
        assert!(!signature_matches(
            CacheMatchMode::Exact,
            &sig(&["a", "b"]),
            &sig(&["a", "c"])
        ));
        assert!(!signature_matches(
            CacheMatchMode::Exact,
            &sig(&["a", "a"]),
            &sig(&["a"])
        ));
        assert!(signature_matches(
            CacheMatchMode::Exact,
            &sig(&[]),
            &sig(&[])
        ));
    }

    #[test]
    /// What: Verify `Subset` mode accepts subsets (including equality) and rejects extras.
    /// Inputs:
    /// - Current signatures that are subsets, equal sets, and supersets of the cached one.
    ///
    /// Output:
    /// - Matches for subset/equal, rejection when current has names missing from cache.
    fn signature_matches_subset_mode() {
        assert!(signature_matches(
            CacheMatchMode::Subset,
            &sig(&["a"]),
            &sig(&["a", "b"])
        ));
        assert!(signature_matches(
            CacheMatchMode::Subset,
            &sig(&["a", "b"]),
            &sig(&["a", "b"])
        ));
        assert!(!signature_matches(
            CacheMatchMode::Subset,
            &sig(&["a", "c"]),
            &sig(&["a", "b"])
        ));
        // The empty set is a subset of anything.
        assert!(signature_matches(
            CacheMatchMode::Subset,
            &sig(&[]),
            &sig(&["a"])
        ));
    }

    #[test]
    /// What: Verify `Intersection` mode requires at least one shared name.
    /// Inputs:
    /// - Overlapping, disjoint, and empty signature pairs.
    ///
    /// Output:
    /// - Matches when any name is shared, rejections for disjoint or empty current sets.
    fn signature_matches_intersection_mode() {
        assert!(signature_matches(
            CacheMatchMode::Intersection,
            &sig(&["a", "z"]),
            &sig(&["a", "b"])
        ));
        assert!(!signature_matches(
            CacheMatchMode::Intersection,
            &sig(&["x", "y"]),
            &sig(&["a", "b"])
        ));
        assert!(!signature_matches(
            CacheMatchMode::Intersection,
            &sig(&[]),
            &sig(&["a"])
        ));
    }

    #[test]
    /// What: Verify `take_exact_match` hands back the payload only on exact matches.
    /// Inputs:
    /// - Matching and mismatching signature pairs with a small payload.
    ///
    /// Output:
    /// - `Some(payload)` for the match, `None` for the mismatch.
    fn take_exact_match_returns_payload_only_on_match() {
        let path = Path::new("/nonexistent/test-cache.json");
        let items = vec!["alpha", "beta"];
        let matched = take_exact_match(
            path,
            "test",
            &sig(&["a", "b"]),
            &sig(&["b", "a"]),
            items.clone(),
        );
        assert_eq!(matched, Some(items.clone()));
        let mismatched = take_exact_match(path, "test", &sig(&["a"]), &sig(&["b"]), items);
        assert_eq!(mismatched, None);
    }

    #[test]
    /// What: Verify subset-mode filtering keeps only items for currently selected names.
    /// Inputs:
    /// - Cache signature `["a", "b"]` with payload for both; current signature `["a"]`.
    ///
    /// Output:
    /// - Only the `"a"` payload item survives filtering.
    fn take_signature_match_filters_subset() {
        let path = Path::new("/nonexistent/test-cache.json");
        let items = vec!["a".to_string(), "b".to_string()];
        let filtered = take_signature_match(
            CacheMatchMode::Subset,
            path,
            "test",
            &sig(&["a"]),
            &sig(&["a", "b"]),
            items,
            |item| item.as_str(),
        );
        assert_eq!(filtered, Some(vec!["a".to_string()]));
    }

    #[test]
    /// What: Verify intersection-mode keeps shared entries and rejects disjoint sets.
    /// Inputs:
    /// - Cache signature `["a"]` with one payload item; current signatures `["a", "b"]`
    ///   (overlap) and `["c"]` (disjoint).
    ///
    /// Output:
    /// - The overlap returns the `"a"` item; the disjoint case returns `None`.
    fn take_signature_match_intersection_behavior() {
        let path = Path::new("/nonexistent/test-cache.json");
        let overlap = take_signature_match(
            CacheMatchMode::Intersection,
            path,
            "test",
            &sig(&["a", "b"]),
            &sig(&["a"]),
            vec!["a".to_string()],
            |item| item.as_str(),
        );
        assert_eq!(overlap, Some(vec!["a".to_string()]));
        let disjoint = take_signature_match(
            CacheMatchMode::Intersection,
            path,
            "test",
            &sig(&["c"]),
            &sig(&["a"]),
            vec!["a".to_string()],
            |item| item.as_str(),
        );
        assert_eq!(disjoint, None);
    }

    #[test]
    /// What: Verify exact-only mode does not fall back to partial matching.
    /// Inputs:
    /// - Current signature `["a"]` against cached `["a", "b"]` with `Exact` mode.
    ///
    /// Output:
    /// - `None`, even though a subset match would have succeeded.
    fn take_signature_match_exact_mode_skips_partial() {
        let path = Path::new("/nonexistent/test-cache.json");
        let result = take_signature_match(
            CacheMatchMode::Exact,
            path,
            "test",
            &sig(&["a"]),
            &sig(&["a", "b"]),
            vec!["a".to_string()],
            |item| item.as_str(),
        );
        assert_eq!(result, None);
    }
}
