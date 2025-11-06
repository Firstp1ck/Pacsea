use super::installed_lock;

/// What: Refresh the process-wide cache of installed package names using `pacman -Qq`.
///
/// Inputs:
/// - None (spawns a blocking task to run pacman)
///
/// Output:
/// - Updates the global installed-name set; ignores errors.
pub async fn refresh_installed_cache() {
    // pacman -Qq to list installed names
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
pub fn is_installed(name: &str) -> bool {
    installed_lock()
        .read()
        .ok()
        .map(|s| s.contains(name))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    /// What: is_installed returns false when cache missing or name absent
    ///
    /// - Input: Clear INSTALLED_SET; query unknown name
    /// - Output: false
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

    /// What: is_installed checks membership correctly
    ///
    /// - Input: Insert "bar" into INSTALLED_SET
    /// - Output: true for bar, false for baz
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
