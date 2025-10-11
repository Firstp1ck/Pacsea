use std::env;
use std::path::{Path, PathBuf};

/// Repository-local config path embedded at compile time when building with Cargo.
/// Returns `config/pacsea.conf` under `CARGO_MANIFEST_DIR` when available.
pub(crate) fn repo_config_path() -> Option<PathBuf> {
    if let Some(dir) = option_env!("CARGO_MANIFEST_DIR") {
        let p = Path::new(dir).join("config").join("pacsea.conf");
        return Some(p);
    }
    None
}

/// Determine the configuration file path for Pacsea's theme, searching in priority order:
/// 1) Repository-local `config/pacsea.conf` (when built with Cargo and present)
/// 2) "$HOME/.config/pacsea/pacsea.conf"
/// 3) "$XDG_CONFIG_HOME/pacsea/pacsea.conf" or "$XDG_CONFIG_HOME/pacsea.conf"
/// 4) "config/pacsea.conf" under the current working directory (fallback for manual runs)
pub(crate) fn resolve_config_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok();
    let xdg_config = env::var("XDG_CONFIG_HOME").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    // Prefer repository config if it exists (useful for `cargo run`)
    if let Some(rcp) = repo_config_path()
        && rcp.is_file()
    {
        candidates.push(rcp);
    }
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
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("config").join("pacsea.conf"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Repository-local config directory (the repo's `config/`) when building with Cargo.
/// Returns `Some(<repo>/config)` if available; ensures the directory exists.
fn repo_config_dir() -> Option<PathBuf> {
    if let Some(dir) = option_env!("CARGO_MANIFEST_DIR") {
        let p = Path::new(dir).join("config");
        let _ = std::fs::create_dir_all(&p);
        return Some(p);
    }
    None
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
        let _ = std::fs::create_dir_all(&dir);
        return Some(dir);
    }
    None
}

/// XDG cache directory for Pacsea (ensured to exist)
pub fn cache_dir() -> PathBuf {
    // Prefer repo config dir for dev; otherwise unify under HOME config dir.
    if let Some(dir) = repo_config_dir() {
        return dir;
    }
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
    // Prefer repo config dir for dev; otherwise unify under HOME config dir.
    if let Some(dir) = repo_config_dir() {
        return dir;
    }
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
    // Prefer repo config dir for dev; otherwise prefer HOME config first.
    if let Some(dir) = repo_config_dir() {
        return dir;
    }
    if let Some(dir) = home_config_dir() {
        return dir;
    }
    // Fallback to XDG config when HOME not available
    let base = xdg_base_dir("XDG_CONFIG_HOME", &[".config"]);
    let dir = base.join("pacsea");
    let _ = std::fs::create_dir_all(&dir);
    dir
}
