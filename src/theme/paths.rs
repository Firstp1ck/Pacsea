use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};

/// Process-wide override for the Pacsea configuration root, set by the CLI `--config-dir` flag.
///
/// Details: When set, all config, cache (lists), and log paths resolve under this directory
/// instead of `$HOME/.config/pacsea` / `$XDG_CONFIG_HOME/pacsea`.
static CONFIG_DIR_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Whether paths may resolve into the user's real `$HOME`/`$XDG_CONFIG_HOME` config tree.
///
/// Details: Off by default so library code driven by a test harness, which never runs `main`,
/// cannot read or modify the developer's real configuration. `main` enables it at startup.
static HOME_CONFIG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Private per-process config root used while home resolution is disabled.
static ISOLATED_CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// What: Set or clear the process-wide configuration directory override.
///
/// Inputs:
/// - `path`: `Some(dir)` to use `dir` as the config root; `None` to restore default resolution.
///
/// Output:
/// - None (side effect: subsequent path helpers resolve under the override).
///
/// Details:
/// - Called once at startup when `--config-dir` is passed, before logging initializes.
/// - Also used by tests to isolate path resolution; ignores a poisoned lock by recovering it.
/// - Clearing restores the isolated root unless [`set_home_config_enabled`] enabled home paths.
pub fn set_config_dir_override(path: Option<PathBuf>) {
    let mut guard = CONFIG_DIR_OVERRIDE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = path;
}

/// What: Allow or forbid resolving config paths from `$HOME`/`$XDG_CONFIG_HOME`.
///
/// Inputs:
/// - `enabled`: `true` to use the user's real config tree when no override is set.
///
/// Output:
/// - None (side effect: changes where subsequent path helpers resolve).
///
/// Details:
/// - `main` enables this once at startup unless `--config-dir` is passed. Tests that exercise
///   home-based resolution enable it only while `HOME` points at a temporary directory and
///   restore the previous value afterwards.
pub fn set_home_config_enabled(enabled: bool) {
    HOME_CONFIG_ENABLED.store(enabled, Ordering::SeqCst);
}

/// What: Report whether config paths may resolve from `$HOME`/`$XDG_CONFIG_HOME`.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `true` after [`set_home_config_enabled`] enabled home resolution.
///
/// Details:
/// - Lets scoped test guards restore the exact previous state.
#[must_use]
pub fn home_config_enabled() -> bool {
    HOME_CONFIG_ENABLED.load(Ordering::SeqCst)
}

/// What: Read the effective configuration root override, if any.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some(PathBuf)` for `--config-dir`, or for the private isolated root while home
///   resolution is disabled; `None` when paths resolve from `$HOME`/`$XDG_CONFIG_HOME`.
///
/// Details:
/// - Recovers from a poisoned lock instead of panicking.
pub fn config_dir_override() -> Option<PathBuf> {
    let explicit = CONFIG_DIR_OVERRIDE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if explicit.is_some() || home_config_enabled() {
        return explicit;
    }
    Some(
        ISOLATED_CONFIG_DIR
            .get_or_init(create_isolated_config_dir)
            .clone(),
    )
}

/// What: Create the private per-process config root used while home resolution is disabled.
///
/// Inputs:
/// - None (uses the system temporary directory).
///
/// Output:
/// - Path of a newly created directory, or the last attempted path if every attempt failed.
///
/// Details:
/// - Uses exclusive creation with mode `0o700` on Unix so a pre-existing or attacker-planted
///   path is never adopted. The directory outlives the process; the next isolated process
///   sweeps it (see [`remove_stale_isolated_config_dirs`]).
fn create_isolated_config_dir() -> PathBuf {
    let base = env::temp_dir();
    remove_stale_isolated_config_dirs(&base);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let mut candidate = base.clone();
    for attempt in 0..16_u32 {
        candidate = base.join(format!(
            "{ISOLATED_CONFIG_DIR_PREFIX}{}-{nanos}-{attempt}",
            std::process::id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        if builder.create(&candidate).is_ok() {
            break;
        }
    }
    candidate
}

/// File-name prefix of isolated config roots, followed by `<pid>-<nanos>-<attempt>`.
const ISOLATED_CONFIG_DIR_PREFIX: &str = "pacsea-isolated-config-";

/// What: Remove isolated config roots left behind by processes that have exited.
///
/// Inputs:
/// - `base`: Temporary directory holding isolated roots.
///
/// Output:
/// - None (best-effort removal; failures are ignored).
///
/// Details:
/// - Linux only, because liveness is read from `/proc/<pid>`. Roots of live processes,
///   including this one, are kept, and symlinked entries are skipped rather than followed.
#[cfg(target_os = "linux")]
fn remove_stale_isolated_config_dirs(base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|name| name.strip_prefix(ISOLATED_CONFIG_DIR_PREFIX))
            .and_then(|rest| rest.split('-').next())
            .and_then(|pid| pid.parse::<u32>().ok())
        else {
            continue;
        };
        let is_real_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        let exited = !Path::new("/proc").join(pid.to_string()).exists();
        if is_real_dir && pid != std::process::id() && exited {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// What: Skip stale isolated-root cleanup where process liveness is unavailable.
///
/// Inputs:
/// - `_base`: Unused temporary directory.
///
/// Output:
/// - None.
///
/// Details:
/// - Leftover roots stay in the system temp directory on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
const fn remove_stale_isolated_config_dirs(_base: &Path) {}

/// What: Choose where a config file is created when no existing file was found.
///
/// Inputs:
/// - `file_name`: File name inside the Pacsea config root, e.g. `settings.conf`.
///
/// Output:
/// - Target path, or `None` when neither an override nor `$XDG_CONFIG_HOME`/`$HOME` is set.
///
/// Details:
/// - Honors `--config-dir` and the isolated root first, so writers never fall back to the
///   real home directory when the override directory simply lacks the file yet.
#[must_use]
pub fn default_config_file_path(file_name: &str) -> Option<PathBuf> {
    if let Some(root) = config_dir_override() {
        return Some(root.join(file_name));
    }
    env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")))
        .map(|base| base.join("pacsea").join(file_name))
}

/// What: Locate the active theme configuration file, considering modern and legacy layouts.
///
/// Inputs:
/// - None (reads environment variables to build candidate paths).
///
/// Output:
/// - `Some(PathBuf)` pointing to the first readable theme file; `None` when nothing exists.
///
/// Details:
/// - Prefers `$HOME/.config/pacsea/theme.conf`, then legacy `pacsea.conf`, and repeats for XDG paths.
/// - When `--config-dir` is active, only the override directory is searched.
pub fn resolve_theme_config_path() -> Option<PathBuf> {
    if let Some(root) = config_dir_override() {
        return [root.join("theme.conf"), root.join("pacsea.conf")]
            .into_iter()
            .find(|p| p.is_file());
    }
    let home = env::var("HOME").ok();
    let xdg_config = env::var("XDG_CONFIG_HOME").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = home.as_deref() {
        let base = Path::new(h).join(".config").join("pacsea");
        candidates.push(base.join("theme.conf"));
        candidates.push(base.join("pacsea.conf")); // legacy
    }
    if let Some(xdg) = xdg_config.as_deref() {
        let x = Path::new(xdg).join("pacsea");
        candidates.push(x.join("theme.conf"));
        candidates.push(x.join("pacsea.conf")); // legacy
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// What: Locate the active settings configuration file, prioritizing the split layout.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some(PathBuf)` for the resolved settings file; `None` when no candidate exists.
///
/// Details:
/// - Searches `$HOME` and `XDG_CONFIG_HOME` for `settings.conf`, then falls back to `pacsea.conf`.
/// - When `--config-dir` is active, only the override directory is searched.
pub(super) fn resolve_settings_config_path() -> Option<PathBuf> {
    if let Some(root) = config_dir_override() {
        return [root.join("settings.conf"), root.join("pacsea.conf")]
            .into_iter()
            .find(|p| p.is_file());
    }
    let home = env::var("HOME").ok();
    let xdg_config = env::var("XDG_CONFIG_HOME").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = home.as_deref() {
        let base = Path::new(h).join(".config").join("pacsea");
        candidates.push(base.join("settings.conf"));
        candidates.push(base.join("pacsea.conf")); // legacy
    }
    if let Some(xdg) = xdg_config.as_deref() {
        let x = Path::new(xdg).join("pacsea");
        candidates.push(x.join("settings.conf"));
        candidates.push(x.join("pacsea.conf")); // legacy
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// What: Locate the keybindings configuration file for Pacsea.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some(PathBuf)` when a keybinds file is present; `None` otherwise.
///
/// Details:
/// - Checks both `$HOME/.config/pacsea/keybinds.conf` and the legacy `pacsea.conf`, mirrored for XDG.
/// - When `--config-dir` is active, only the override directory is searched.
pub(super) fn resolve_keybinds_config_path() -> Option<PathBuf> {
    if let Some(root) = config_dir_override() {
        return [root.join("keybinds.conf"), root.join("pacsea.conf")]
            .into_iter()
            .find(|p| p.is_file());
    }
    let home = env::var("HOME").ok();
    let xdg_config = env::var("XDG_CONFIG_HOME").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = home.as_deref() {
        let base = Path::new(h).join(".config").join("pacsea");
        candidates.push(base.join("keybinds.conf"));
        candidates.push(base.join("pacsea.conf")); // legacy
    }
    if let Some(xdg) = xdg_config.as_deref() {
        let x = Path::new(xdg).join("pacsea");
        candidates.push(x.join("keybinds.conf"));
        candidates.push(x.join("pacsea.conf")); // legacy
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// What: Locate the `repos.conf` file for third-party repository definitions (TOML).
///
/// Inputs:
/// - None (reads `HOME` / `XDG_CONFIG_HOME`).
///
/// Output:
/// - `Some(PathBuf)` when a candidate file exists; `None` otherwise.
///
/// Details:
/// - Checks `$HOME/.config/pacsea/repos.conf` then `XDG_CONFIG_HOME/pacsea/repos.conf`.
/// - Does not fall back to legacy `pacsea.conf` (repos live only in the split layout).
/// - When `--config-dir` is active, only the override directory is searched.
#[must_use]
pub fn resolve_repos_config_path() -> Option<PathBuf> {
    if let Some(root) = config_dir_override() {
        let candidate = root.join("repos.conf");
        return candidate.is_file().then_some(candidate);
    }
    let home = env::var("HOME").ok();
    let xdg_config = env::var("XDG_CONFIG_HOME").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = home.as_deref() {
        let base = Path::new(h).join(".config").join("pacsea");
        candidates.push(base.join("repos.conf"));
    }
    if let Some(xdg) = xdg_config.as_deref() {
        let x = Path::new(xdg).join("pacsea");
        candidates.push(x.join("repos.conf"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// What: Resolve an XDG base directory, falling back to `$HOME` with provided segments.
///
/// Inputs:
/// - `var`: Environment variable name, e.g., `XDG_CONFIG_HOME`.
/// - `home_default`: Path segments appended to `$HOME` when the variable is unset.
///
/// Output:
/// - `PathBuf` pointing to the derived base directory.
///
/// Details:
/// - Treats empty environment values as unset and gracefully handles missing `$HOME`.
fn xdg_base_dir(var: &str, home_default: &[&str]) -> PathBuf {
    if let Ok(p) = env::var(var)
        && !p.trim().is_empty()
    {
        return PathBuf::from(p);
    }
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut base = PathBuf::from(home);
    for seg in home_default {
        base = base.join(seg);
    }
    base
}

/// What: Build `$HOME/.config/pacsea`, ensuring the directory exists when `$HOME` is set.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some(PathBuf)` when the directory is accessible; `None` if `$HOME` is missing or creation fails.
///
/// Details:
/// - Serves as the preferred base for other configuration directories.
/// - On Windows, also checks `APPDATA` and `USERPROFILE` if `HOME` is not set.
fn home_config_dir() -> Option<PathBuf> {
    // Try HOME first (works on Unix and Windows if set)
    if let Ok(home) = env::var("HOME") {
        let dir = Path::new(&home).join(".config").join("pacsea");
        if std::fs::create_dir_all(&dir).is_ok() {
            return Some(dir);
        }
    }
    // Windows fallback: use APPDATA or USERPROFILE
    #[cfg(windows)]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            let dir = Path::new(&appdata).join("pacsea");
            if std::fs::create_dir_all(&dir).is_ok() {
                return Some(dir);
            }
        }
        if let Ok(userprofile) = env::var("USERPROFILE") {
            let dir = Path::new(&userprofile).join(".config").join("pacsea");
            if std::fs::create_dir_all(&dir).is_ok() {
                return Some(dir);
            }
        }
    }
    None
}

/// What: Resolve the Pacsea configuration directory, ensuring it exists on disk.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `PathBuf` pointing to the Pacsea config directory.
///
/// Details:
/// - Prefers `$HOME/.config/pacsea`, falling back to `XDG_CONFIG_HOME/pacsea` when necessary.
/// - When `--config-dir` is active, returns the override directory (created if missing).
#[must_use]
pub fn config_dir() -> PathBuf {
    if let Some(dir) = config_dir_override() {
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }
    // Prefer HOME ~/.config/pacsea first
    if let Some(dir) = home_config_dir() {
        return dir;
    }
    // Fallback: use XDG_CONFIG_HOME (or default to ~/.config) and ensure
    let base = xdg_base_dir("XDG_CONFIG_HOME", &[".config"]);
    let dir = base.join("pacsea");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// What: Obtain the logs subdirectory inside the Pacsea config folder.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `PathBuf` leading to the `logs` directory (created if missing).
///
/// Details:
/// - Builds upon `config_dir()` and ensures a stable location for log files.
#[must_use]
pub fn logs_dir() -> PathBuf {
    let base = config_dir();
    let dir = base.join("logs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// What: Obtain the lists subdirectory inside the Pacsea config folder.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `PathBuf` leading to the `lists` directory (created if missing).
///
/// Details:
/// - Builds upon `config_dir()` and ensures storage for exported package lists.
#[must_use]
pub fn lists_dir() -> PathBuf {
    let base = config_dir();
    let dir = base.join("lists");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    /// What: Manage temporary HOME override for path resolution tests.
    ///
    /// Inputs:
    /// - `base`: Temporary HOME root directory.
    ///
    /// Output:
    /// - Guard that restores `HOME` and `XDG_CONFIG_HOME` and removes temp directory on drop.
    ///
    /// Details:
    /// - Clears `XDG_CONFIG_HOME` while active so `config_dir` and resolvers cannot use the
    ///   developer's real XDG config tree.
    /// - Provides panic-safe cleanup for tests mutating process-wide environment.
    struct HomeTestGuard {
        orig_home: Option<std::ffi::OsString>,
        orig_xdg: Option<std::ffi::OsString>,
        base: std::path::PathBuf,
        /// Home-resolution flag restored on drop.
        orig_home_config: bool,
    }

    impl HomeTestGuard {
        /// What: Create a HOME override guard for test isolation.
        ///
        /// Inputs:
        /// - `base`: Temporary path to use as `HOME`.
        ///
        /// Output:
        /// - Initialized `HomeTestGuard`.
        ///
        /// Details:
        /// - Captures original `HOME` and `XDG_CONFIG_HOME`, applies test `HOME`, and unsets XDG.
        fn new(base: std::path::PathBuf) -> Self {
            let orig_home = std::env::var_os("HOME");
            let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
            let _ = std::fs::create_dir_all(&base);
            let orig_home_config = crate::theme::home_config_enabled();
            unsafe {
                std::env::set_var("HOME", base.display().to_string());
                std::env::remove_var("XDG_CONFIG_HOME");
            }
            crate::theme::set_home_config_enabled(true);
            Self {
                orig_home,
                orig_xdg,
                base,
                orig_home_config,
            }
        }
    }

    impl Drop for HomeTestGuard {
        fn drop(&mut self) {
            crate::theme::set_home_config_enabled(self.orig_home_config);
            unsafe {
                if let Some(v) = self.orig_home.as_ref() {
                    std::env::set_var("HOME", v);
                } else {
                    std::env::remove_var("HOME");
                }
                if let Some(v) = self.orig_xdg.as_ref() {
                    std::env::set_var("XDG_CONFIG_HOME", v);
                } else {
                    std::env::remove_var("XDG_CONFIG_HOME");
                }
            }
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    /// What: Verify path helpers resolve under the Pacsea config directory rooted at `HOME`.
    ///
    /// Inputs:
    /// - Temporary `HOME` directory substituted to capture generated paths.
    ///
    /// Output:
    /// - `config_dir`, `logs_dir`, and `lists_dir` end with `pacsea`, `logs`, and `lists` respectively.
    ///
    /// Details:
    /// - Restores the original `HOME` and `XDG_CONFIG_HOME` afterwards to avoid polluting the real
    ///   configuration tree.
    fn paths_config_lists_logs_under_home() {
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_paths_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _home_guard = HomeTestGuard::new(base);
        let cfg = super::config_dir();
        let logs = super::logs_dir();
        let lists = super::lists_dir();
        assert!(cfg.ends_with("pacsea"));
        assert!(logs.ends_with("logs"));
        assert!(lists.ends_with("lists"));
    }

    /// What: Clear the config-dir override when a test finishes or panics.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Restores default path resolution on drop.
    ///
    /// Details:
    /// - Keeps the process-wide override from leaking into other tests.
    struct OverrideGuard;

    impl Drop for OverrideGuard {
        fn drop(&mut self) {
            super::set_config_dir_override(None);
        }
    }

    #[test]
    /// What: Verify all path helpers and resolvers honor the `--config-dir` override.
    ///
    /// Inputs:
    /// - Temporary directory installed via `set_config_dir_override`.
    ///
    /// Output:
    /// - `config_dir`/`logs_dir`/`lists_dir` resolve under the override; resolvers only find
    ///   files placed inside it.
    ///
    /// Details:
    /// - Clears the override afterwards via `OverrideGuard` so other tests see defaults.
    fn paths_honor_config_dir_override() {
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_paths_override_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        super::set_config_dir_override(Some(base.clone()));
        let _override_guard = OverrideGuard;

        assert_eq!(super::config_dir(), base);
        assert!(super::logs_dir().starts_with(&base));
        assert!(super::lists_dir().starts_with(&base));

        // Resolvers search only the override directory.
        assert!(super::resolve_theme_config_path().is_none());
        std::fs::write(base.join("settings.conf"), "").expect("write settings.conf");
        assert_eq!(
            super::resolve_settings_config_path(),
            Some(base.join("settings.conf"))
        );
        std::fs::write(base.join("repos.conf"), "").expect("write repos.conf");
        assert_eq!(
            super::resolve_repos_config_path(),
            Some(base.join("repos.conf"))
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    /// What: Ensure `config_dir` stays under the test `HOME` when `XDG_CONFIG_HOME` was set in the environment.
    ///
    /// Inputs:
    /// - A bogus `XDG_CONFIG_HOME` set before `HomeTestGuard` (simulates a developer shell).
    ///
    /// Output:
    /// - `config_dir` is a path under the temporary home root.
    ///
    /// Details:
    /// - Guards against regressions where only `HOME` is overridden and the XDG fallback or other
    ///   helpers could still target the real config tree.
    fn paths_config_stays_under_temp_home_when_xdg_config_home_was_set() {
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_paths_xdg_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let home_root = base.clone();
        unsafe {
            std::env::set_var(
                "XDG_CONFIG_HOME",
                "/nonexistent/pacsea_test_xdg_decoy_must_not_be_used",
            );
        }
        let _home_guard = HomeTestGuard::new(base);
        let cfg = super::config_dir();
        assert!(
            cfg.starts_with(&home_root),
            "config_dir should resolve under test HOME, not decoy XDG_CONFIG_HOME; got {cfg:?}"
        );
    }

    #[test]
    /// What: Library callers that never enable home resolution stay out of `$HOME`.
    ///
    /// Inputs:
    /// - Default process state: no `--config-dir` override, home resolution disabled.
    ///
    /// Output:
    /// - Every path helper and the new-file fallback resolve under the isolated temp root.
    ///
    /// Details:
    /// - Guards the test-suite isolation contract: harness processes never run `main`, so they
    ///   must not read or write the developer's real `~/.config/pacsea`.
    fn paths_default_to_isolated_root_without_home_opt_in() {
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        assert!(!super::home_config_enabled());
        let isolated = super::config_dir_override().expect("isolated root");
        assert!(isolated.starts_with(std::env::temp_dir()));
        assert!(isolated.is_dir());
        let home = std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        for path in [
            super::config_dir(),
            super::logs_dir(),
            super::lists_dir(),
            super::default_config_file_path("settings.conf").expect("fallback"),
        ] {
            assert!(path.starts_with(&isolated), "{path:?} escaped {isolated:?}");
            assert!(!path.starts_with(&home), "{path:?} resolved under HOME");
        }
    }

    #[test]
    /// What: Saving into an override that lacks `settings.conf` creates it there, not in `$HOME`.
    ///
    /// Inputs:
    /// - Empty `--config-dir` override plus a temporary `HOME` with home resolution enabled.
    ///
    /// Output:
    /// - `save_sort_mode` seeds `settings.conf` inside the override; `HOME` stays untouched.
    ///
    /// Details:
    /// - Regression: the missing-file fallback used to ignore the override and target
    ///   `$XDG_CONFIG_HOME`/`$HOME`, so `--config-dir` and test overrides leaked writes.
    fn save_into_empty_override_never_falls_back_to_home() {
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time is before UNIX epoch")
            .as_nanos();
        let tmp = std::env::temp_dir();
        let home = tmp.join(format!(
            "pacsea_test_fallback_home_{}_{stamp}",
            std::process::id()
        ));
        let override_dir = tmp.join(format!(
            "pacsea_test_fallback_cfg_{}_{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&override_dir).expect("create override dir");
        let home_guard = HomeTestGuard::new(home.clone());
        super::set_config_dir_override(Some(override_dir.clone()));
        let _override_guard = OverrideGuard;

        crate::theme::save_sort_mode(crate::state::SortMode::BestMatches);

        assert!(override_dir.join("settings.conf").is_file());
        assert!(!home.join(".config").exists(), "save leaked into HOME");
        drop(home_guard);
        let _ = std::fs::remove_dir_all(&override_dir);
    }
}
