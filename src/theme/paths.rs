use std::env;
use std::path::{Path, PathBuf};

/// Determine the configuration file path for Pacsea's theme, searching in priority order:
/// 1) "$HOME/.config/pacsea/pacsea.conf"
/// 2) "$XDG_CONFIG_HOME/pacsea/pacsea.conf" or "$XDG_CONFIG_HOME/pacsea.conf"
pub(crate) fn resolve_config_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok();
    let xdg_config = env::var("XDG_CONFIG_HOME").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = home.as_deref() {
        candidates.push(
            Path::new(h)
                .join(".config")
                .join("pacsea")
                .join("pacsea.conf"),
        );
    }
    if let Some(xdg) = xdg_config.as_deref() {
        let x = Path::new(xdg);
        candidates.push(x.join("pacsea").join("pacsea.conf"));
        candidates.push(x.join("pacsea.conf"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

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
