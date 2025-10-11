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

// Removed repo-local config directory support for dev; always prefer HOME config.

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
        let _ = std::fs::create_dir_all(&dir);
        return Some(dir);
    }
    None
}

/// XDG cache directory for Pacsea (ensured to exist)
pub fn cache_dir() -> PathBuf {
    // Unify under HOME config dir first.
    if let Some(dir) = home_config_dir() {
        return dir;
    }
    // Fallback to XDG cache when HOME not available
    let base = xdg_base_dir("XDG_CACHE_HOME", &[".cache"]);
    let dir = base.join("pacsea");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// XDG state directory for Pacsea (ensured to exist)
pub fn state_dir() -> PathBuf {
    // Unify under HOME config dir first.
    if let Some(dir) = home_config_dir() {
        return dir;
    }
    // Fallback to XDG state when HOME not available
    let base = xdg_base_dir("XDG_STATE_HOME", &[".local", "state"]);
    let dir = base.join("pacsea");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// XDG config directory for Pacsea (ensured to exist)
pub fn config_dir() -> PathBuf {
    // Prefer HOME config first.
    if let Some(dir) = home_config_dir() {
        return dir;
    }
    // Fallback to XDG config when HOME not available
    let base = xdg_base_dir("XDG_CONFIG_HOME", &[".config"]);
    let dir = base.join("pacsea");
    let _ = std::fs::create_dir_all(&dir);
    dir
}
