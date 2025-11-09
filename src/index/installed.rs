use super::installed_lock;

/// What: Refresh the process-wide cache of installed package names using `pacman -Qq`.
///
/// Inputs:
/// - None (spawns a blocking task to run pacman)
///
/// Output:
/// - Updates the global installed-name set; ignores errors.
///
/// Details:
/// - Parses command stdout into a `HashSet` and swaps it into the shared cache under a write lock.
pub async fn refresh_installed_cache() {
    /// What: Execute `pacman -Qq` and return the list of installed package names.
    ///
    /// Inputs:
    /// - None (command line is fixed to `-Qq`).
    ///
    /// Output:
    /// - `Ok(String)` with UTF-8 stdout on success; boxed error otherwise.
    ///
    /// Details:
    /// - Treats non-zero exit codes and UTF-8 decoding failures as errors to propagate.
    fn run_pacman_q() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman")
            .args(["-Qq"])
            .output()?;
        if !out.status.success() {
            return Err(format!("pacman -Qq exited with {:?}", out.status).into());
        }
        Ok(String::from_utf8(out.stdout)?)
    }
    if let Ok(Ok(body)) = tokio::task::spawn_blocking(run_pacman_q).await {
        let set: std::collections::HashSet<String> =
            body.lines().map(|s| s.trim().to_string()).collect();
        if let Ok(mut g) = installed_lock().write() {
            *g = set;
        }
    }
}

/// What: Query whether `name` appears in the cached set of installed packages.
///
/// Inputs:
/// - `name`: Package name
///
/// Output:
/// - `true` if `name` is present; `false` when absent or if the cache is unavailable.
///
/// Details:
/// - Acquires a read lock and defers to `HashSet::contains`, returning false on lock poisoning.
pub fn is_installed(name: &str) -> bool {
    installed_lock()
        .read()
        .ok()
        .map(|s| s.contains(name))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    /// What: Return false when the cache is empty or the package is missing.
    ///
    /// Inputs:
    /// - Clear `INSTALLED_SET` and query an unknown package name.
    ///
    /// Output:
    /// - Boolean `false` result.
    ///
    /// Details:
    /// - Confirms empty cache behaves as expected without panicking.
    #[test]
    fn is_installed_returns_false_when_uninitialized_or_missing() {
        let _guard = crate::index::test_mutex()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Ok(mut g) = super::installed_lock().write() {
            g.clear();
        }
        assert!(!super::is_installed("foo"));
    }

    /// What: Verify membership lookups return true only for cached names.
    ///
    /// Inputs:
    /// - Insert `bar` into `INSTALLED_SET` before querying.
    ///
    /// Output:
    /// - `true` for `bar` and `false` for `baz`.
    ///
    /// Details:
    /// - Exercises both positive and negative membership checks.
    #[test]
    fn is_installed_checks_membership_in_cached_set() {
        let _guard = crate::index::test_mutex()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Ok(mut g) = super::installed_lock().write() {
            g.clear();
            g.insert("bar".to_string());
        }
        assert!(super::is_installed("bar"));
        assert!(!super::is_installed("baz"));
    }
}
