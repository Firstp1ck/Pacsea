//! Shared HOME/XDG environment isolation for integration tests.
//!
//! Process environment variables are global, so tests that override
//! `HOME`/`XDG_CONFIG_HOME` must not run concurrently with each other.
//! [`EnvGuard`] serializes all such overrides in this test binary through a
//! single global mutex, making the tests safe even without
//! `--test-threads=1`.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

/// What: Global lock serializing all process-env mutation in this test binary.
///
/// Inputs: None (lazily initialized static).
///
/// Output:
/// - Shared `Mutex` acquired by every [`EnvGuard`] for its whole lifetime.
///
/// Details:
/// - Any new test module that mutates process env vars must acquire this same
///   lock (via [`EnvGuard`] or directly) to stay race-free.
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// What: Acquire the global env lock for tests that mutate process env vars.
///
/// Inputs: None.
///
/// Output:
/// - Guard serializing env mutation until dropped.
///
/// Details:
/// - Recovers from poisoning: a panicking holder leaves env vars restored by
///   its own guards, so the lock stays usable.
pub fn acquire() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// What: Guard process-wide HOME/XDG overrides for integration tests.
///
/// Inputs:
/// - `home_root`: Temporary directory used as test HOME root.
///
/// Output:
/// - Guard that restores original `HOME` and `XDG_CONFIG_HOME` on drop.
///
/// Details:
/// - Config path resolution uses process env vars, so this guard isolates
///   tests from developer machine config.
/// - Holds [`ENV_LOCK`] for its lifetime so concurrent tests cannot observe
///   or clobber each other's overrides.
pub struct EnvGuard {
    /// Original `HOME` value, restored on drop.
    original_home: Option<std::ffi::OsString>,
    /// Original `XDG_CONFIG_HOME` value, restored on drop.
    original_xdg: Option<std::ffi::OsString>,
    /// Temp HOME root cleaned up on drop.
    home_root: PathBuf,
    /// Global env lock held until the guard drops; last field so the
    /// `Drop` impl restores env vars before the lock releases.
    _env_lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    /// What: Apply isolated HOME/XDG environment for a test run.
    ///
    /// Inputs:
    /// - `home_root`: Temporary HOME path.
    ///
    /// Output:
    /// - Initialized guard holding the global env lock.
    ///
    /// Details:
    /// - Blocks until no other `EnvGuard` is alive; a poisoned lock (from a
    ///   panicking test) is recovered since env state is restored on drop.
    pub fn new(home_root: PathBuf) -> Self {
        let env_lock = acquire();
        let original_home = std::env::var_os("HOME");
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let config_root = home_root.join(".config").join("pacsea");
        fs::create_dir_all(&config_root).expect("must create isolated config root");
        unsafe {
            std::env::set_var("HOME", &home_root);
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        Self {
            original_home,
            original_xdg,
            home_root,
            _env_lock: env_lock,
        }
    }
}

impl Drop for EnvGuard {
    /// What: Restore the original environment and remove the temp HOME.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Side effects only: env vars restored, temp dir removed.
    ///
    /// Details:
    /// - Runs before the held `_env_lock` guard drops, so the environment is
    ///   fully restored before another test can acquire the lock.
    fn drop(&mut self) {
        unsafe {
            if let Some(home) = self.original_home.as_ref() {
                std::env::set_var("HOME", home);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(xdg) = self.original_xdg.as_ref() {
                std::env::set_var("XDG_CONFIG_HOME", xdg);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
        let _ = fs::remove_dir_all(&self.home_root);
    }
}
