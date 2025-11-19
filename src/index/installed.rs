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
    if let Ok(Ok(body)) =
        tokio::task::spawn_blocking(|| crate::util::pacman::run_pacman(&["-Qq"])).await
    {
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
        let _guard = crate::global_test_mutex()
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
        let _guard = crate::global_test_mutex()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Ok(mut g) = super::installed_lock().write() {
            g.clear();
            g.insert("bar".to_string());
        }
        assert!(super::is_installed("bar"));
        assert!(!super::is_installed("baz"));
    }

    #[cfg(not(target_os = "windows"))]
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    /// What: Populate the installed cache from pacman output.
    ///
    /// Inputs:
    /// - Override PATH with a fake pacman that emits installed package names before invoking the refresh.
    ///
    /// Output:
    /// - Cache lookup succeeds for the emitted names after `refresh_installed_cache` completes.
    ///
    /// Details:
    /// - Exercises the async refresh path, ensures PATH is restored, and verifies cache contents via helper accessors.
    async fn refresh_installed_cache_populates_cache_from_pacman_output() {
        let _guard = crate::global_test_mutex_lock();

        if let Ok(mut g) = super::installed_lock().write() {
            g.clear();
        }

        let original_path = std::env::var("PATH").unwrap_or_default();
        struct PathGuard {
            original: String,
        }
        impl Drop for PathGuard {
            fn drop(&mut self) {
                unsafe {
                    std::env::set_var("PATH", &self.original);
                }
            }
        }
        let _path_guard = PathGuard {
            original: original_path.clone(),
        };

        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_pacman_qq_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut bin = root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let mut script = bin.clone();
        script.push("pacman");
        let body = r#"#!/usr/bin/env bash
set -e
if [[ "$1" == "-Qq" ]]; then
  echo "alpha"
  echo "beta"
  exit 0
fi
exit 1
"#;
        std::fs::write(&script, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&script).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&script, perm).unwrap();
        }
        let new_path = format!("{}:{}", bin.to_string_lossy(), original_path);
        unsafe {
            std::env::set_var("PATH", &new_path);
        }

        super::refresh_installed_cache().await;

        let _ = std::fs::remove_dir_all(&root);

        assert!(super::is_installed("alpha"));
        assert!(super::is_installed("beta"));
        assert!(!super::is_installed("gamma"));
    }
}
