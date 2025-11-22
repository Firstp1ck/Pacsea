use std::env;
use std::path::{Path, PathBuf};

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
pub(crate) fn resolve_theme_config_path() -> Option<PathBuf> {
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
pub(crate) fn resolve_settings_config_path() -> Option<PathBuf> {
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
pub(crate) fn resolve_keybinds_config_path() -> Option<PathBuf> {
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
pub fn config_dir() -> PathBuf {
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
pub fn lists_dir() -> PathBuf {
    let base = config_dir();
    let dir = base.join("lists");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
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
    /// - Restores the original `HOME` afterwards to avoid polluting the real configuration tree.
    fn paths_config_lists_logs_under_home() {
        let _guard = crate::theme::test_mutex().lock()
            .expect("Test mutex poisoned");
        let orig_home = std::env::var_os("HOME");
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_paths_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&base);
        unsafe { std::env::set_var("HOME", base.display().to_string()) };
        let cfg = super::config_dir();
        let logs = super::logs_dir();
        let lists = super::lists_dir();
        assert!(cfg.ends_with("pacsea"));
        assert!(logs.ends_with("logs"));
        assert!(lists.ends_with("lists"));
        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }
}
