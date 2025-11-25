use std::collections::HashSet;

use super::explicit_lock;
use crate::state::InstalledPackagesMode;

/// What: Refresh the process-wide cache of explicitly installed package names.
///
/// Inputs:
/// - `mode`: Filter mode for installed packages.
///   - `LeafOnly`: Uses `pacman -Qetq` (explicitly installed AND not required)
///   - `AllExplicit`: Uses `pacman -Qeq` (all explicitly installed)
///
/// Output:
/// - Updates the global explicit-name set; ignores errors.
///
/// Details:
/// - Converts command stdout into a `HashSet` and replaces the shared cache atomically.
pub async fn refresh_explicit_cache(mode: InstalledPackagesMode) {
    let args: &[&str] = match mode {
        InstalledPackagesMode::LeafOnly => &["-Qetq"], // explicitly installed AND not required (leaf)
        InstalledPackagesMode::AllExplicit => &["-Qeq"], // all explicitly installed
    };
    if let Ok(Ok(body)) =
        tokio::task::spawn_blocking(move || crate::util::pacman::run_pacman(args)).await
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
        let _guard = crate::global_test_mutex_lock();

        if let Ok(mut g) = super::explicit_lock().write() {
            g.clear();
        }

        let old_path = std::env::var("PATH").unwrap_or_default();
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
        std::fs::create_dir_all(&root).expect("failed to create test root directory");
        let mut bin = root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).expect("failed to create test bin directory");
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
        std::fs::write(&script, body).expect("failed to write test pacman script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&script)
                .expect("failed to read test pacman script metadata")
                .permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&script, perm)
                .expect("failed to set test pacman script permissions");
        }
        let new_path = format!("{}:{old_path}", bin.to_string_lossy());
        unsafe {
            std::env::set_var("PATH", &new_path);
        }

        super::refresh_explicit_cache(crate::state::InstalledPackagesMode::LeafOnly).await;

        let _ = std::fs::remove_dir_all(&root);

        let set = super::explicit_names();
        assert_eq!(set.len(), 2);
        assert!(set.contains("alpha"));
        assert!(set.contains("beta"));
    }

    #[cfg(not(target_os = "windows"))]
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    /// What: Populate the explicit cache from pacman output using `AllExplicit` mode.
    ///
    /// Inputs:
    /// - Override PATH with a fake pacman returning explicit package names before invoking the refresh.
    ///
    /// Output:
    /// - Cache contains all names after `refresh_explicit_cache` completes with `AllExplicit` mode.
    ///
    /// Details:
    /// - Verifies the async refresh uses `-Qeq` argument (all explicitly installed packages)
    ///   instead of `-Qetq` (leaf packages only), and updates the cache correctly.
    async fn refresh_explicit_cache_populates_cache_with_all_explicit_mode() {
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
        let _guard = crate::global_test_mutex_lock();

        if let Ok(mut g) = super::explicit_lock().write() {
            g.clear();
        }

        let old_path = std::env::var("PATH").unwrap_or_default();
        let _path_guard = PathGuard {
            original: old_path.clone(),
        };

        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_pacman_qeq_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("failed to create test root directory");
        let mut bin = root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).expect("failed to create test bin directory");
        let mut script = bin.clone();
        script.push("pacman");
        let body = r#"#!/usr/bin/env bash
set -e
if [[ "$1" == "-Qeq" ]]; then
  echo "git"
  echo "python"
  echo "wget"
  exit 0
fi
exit 1
"#;
        std::fs::write(&script, body).expect("failed to write test pacman script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&script)
                .expect("failed to read test pacman script metadata")
                .permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&script, perm)
                .expect("failed to set test pacman script permissions");
        }
        let new_path = format!("{}:{old_path}", bin.to_string_lossy());
        unsafe {
            std::env::set_var("PATH", &new_path);
        }

        super::refresh_explicit_cache(crate::state::InstalledPackagesMode::AllExplicit).await;

        let _ = std::fs::remove_dir_all(&root);

        let set = super::explicit_names();
        assert_eq!(set.len(), 3);
        assert!(set.contains("git"));
        assert!(set.contains("python"));
        assert!(set.contains("wget"));
    }
}
