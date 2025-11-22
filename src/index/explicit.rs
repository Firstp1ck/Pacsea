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
    if let Ok(Ok(body)) = tokio::task::spawn_blocking(|| {
        crate::util::pacman::run_pacman(&["-Qetq"]) // explicitly installed AND not required (leaf), names only
    })
    .await
    {
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
#[must_use]
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
        let _guard = crate::global_test_mutex_lock();
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
        let _guard = crate::global_test_mutex_lock();
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

    #[cfg(not(target_os = "windows"))]
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    /// What: Populate the explicit cache from pacman output.
    ///
    /// Inputs:
    /// - Override PATH with a fake pacman returning two explicit package names before invoking the refresh.
    ///
    /// Output:
    /// - Cache contains both names after `refresh_explicit_cache` completes.
    ///
    /// Details:
    /// - Verifies the async refresh reads command output, updates the cache, and the cache contents persist after restoring PATH.
    async fn refresh_explicit_cache_populates_cache_from_pacman_output() {
        let _guard = crate::global_test_mutex_lock();

        if let Ok(mut g) = super::explicit_lock().write() {
            g.clear();
        }

        let old_path = std::env::var("PATH").unwrap_or_default();
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
            original: old_path.clone(),
        };

        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_pacman_qetq_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
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
if [[ "$1" == "-Qetq" ]]; then
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
        let new_path = format!("{}:{old_path}", bin.to_string_lossy());
        unsafe {
            std::env::set_var("PATH", &new_path);
        }

        super::refresh_explicit_cache().await;

        let _ = std::fs::remove_dir_all(&root);

        let set = super::explicit_names();
        assert_eq!(set.len(), 2);
        assert!(set.contains("alpha"));
        assert!(set.contains("beta"));
    }
}
