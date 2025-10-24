use std::env;
use std::path::{Path, PathBuf};

/// Determine the configuration file path for Pacsea's THEME, searching in priority order.
/// New layout prefers `theme.conf`; falls back to legacy `pacsea.conf`.
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

/// Determine the configuration file path for Pacsea's SETTINGS.
/// Prefers `settings.conf`; falls back to legacy `pacsea.conf`.
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

/// Determine the configuration file path for Pacsea's KEYBINDS.
/// Prefers `keybinds.conf`; falls back to legacy `pacsea.conf`.
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

/// Resolve an XDG base directory from environment or default to `$HOME` + segments.
///
/// Inputs:
/// - `var`: Environment variable to check (e.g., `XDG_CONFIG_HOME`).
/// - `home_default`: Fallback path segments relative to `$HOME` if `var` is unset/empty.
///
/// Output: Resolved base directory path.
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

/// User's HOME config directory: "$HOME/.config/pacsea" if HOME is set.
/// Ensures it exists. Returns None if HOME is unavailable.
/// Return `$HOME/.config/pacsea`, ensuring it exists.
///
/// Inputs: none
///
/// Output: `Some(PathBuf)` when HOME is set and directory can be created; `None` otherwise.
fn home_config_dir() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        let dir = Path::new(&home).join(".config").join("pacsea");
        if std::fs::create_dir_all(&dir).is_ok() {
            return Some(dir);
        }
    }
    None
}

/// XDG config directory for Pacsea (ensured to exist)
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

/// Logs directory under config: "$HOME/.config/pacsea/logs" (ensured to exist)
pub fn logs_dir() -> PathBuf {
    let base = config_dir();
    let dir = base.join("logs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Lists directory under config: "$HOME/.config/pacsea/lists" (ensured to exist)
pub fn lists_dir() -> PathBuf {
    let base = config_dir();
    let dir = base.join("lists");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    #[test]
    fn paths_config_lists_logs_under_home() {
        let _guard = crate::theme::test_mutex().lock().unwrap();
        let orig_home = std::env::var_os("HOME");
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_paths_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
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
