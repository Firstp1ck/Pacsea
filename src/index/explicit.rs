use std::collections::HashSet;

use super::explicit_lock;

/// What: Refresh the process-wide cache of explicitly installed (leaf) package names via `pacman -Qetq`.
///
/// Inputs:
/// - None (spawns a blocking task to run pacman)
///
/// Output:
/// - Updates the global explicit-name set; ignores errors.
///
/// Details:
/// - Converts command stdout into a `HashSet` and replaces the shared cache atomically.
pub async fn refresh_explicit_cache() {
    /// What: Execute `pacman -Qetq` and capture the list of explicit leaf packages.
    ///
    /// Inputs:
    /// - None (arguments fixed to `-Qetq`).
    ///
    /// Output:
    /// - `Ok(String)` containing UTF-8 stdout of package names; error otherwise.
    ///
    /// Details:
    /// - Propagates non-zero exit codes and UTF-8 decoding failures as boxed errors.
    fn run_pacman_qe() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman")
            .args(["-Qetq"]) // explicitly installed AND not required (leaf), names only
            .output()?;
        if !out.status.success() {
            return Err(format!("pacman -Qetq exited with {:?}", out.status).into());
        }
        Ok(String::from_utf8(out.stdout)?)
    }
    if let Ok(Ok(body)) = tokio::task::spawn_blocking(run_pacman_qe).await {
        let set: HashSet<String> = body.lines().map(|s| s.trim().to_string()).collect();
        if let Ok(mut g) = explicit_lock().write() {
            *g = set;
        }
    }
}

/// What: Return a cloned set of explicitly installed package names.
///
/// Inputs:
/// - None
///
/// Output:
/// - A cloned `HashSet<String>` of explicit names (empty on lock failure).
///
/// Details:
/// - Returns an owned copy so callers can mutate the result without holding the lock.
pub fn explicit_names() -> HashSet<String> {
    explicit_lock()
        .read()
        .map(|s| s.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    /// What: Return an empty set when the explicit cache has not been populated.
    ///
    /// Inputs:
    /// - Clear `EXPLICIT_SET` before calling `explicit_names`.
    ///
    /// Output:
    /// - Empty `HashSet<String>`.
    ///
    /// Details:
    /// - Confirms the helper gracefully handles uninitialized state.
    #[test]
    fn explicit_names_returns_empty_when_uninitialized() {
        let _guard = crate::index::test_mutex().lock().unwrap();
        // Ensure empty state
        if let Ok(mut g) = super::explicit_lock().write() {
            g.clear();
        }
        let set = super::explicit_names();
        assert!(set.is_empty());
    }

    /// What: Clone the cached explicit set for callers.
    ///
    /// Inputs:
    /// - Populate `EXPLICIT_SET` with `a` and `b` prior to the call.
    ///
    /// Output:
    /// - Returned set contains the inserted names.
    ///
    /// Details:
    /// - Ensures cloning semantics (rather than references) are preserved.
    #[test]
    fn explicit_names_returns_cloned_set() {
        let _guard = crate::index::test_mutex().lock().unwrap();
        if let Ok(mut g) = super::explicit_lock().write() {
            g.clear();
            g.insert("a".to_string());
            g.insert("b".to_string());
        }
        let mut set = super::explicit_names();
        assert_eq!(set.len(), 2);
        let mut v: Vec<String> = set.drain().collect();
        v.sort();
        assert_eq!(v, vec!["a", "b"]);
    }
}
