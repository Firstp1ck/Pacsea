use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use super::config::{
    SKELETON_CONFIG_CONTENT, load_theme_from_file, try_load_theme_with_diagnostics,
};
use super::paths::{repo_config_path, resolve_config_path};
use super::types::Theme;

/// Global theme store with live-reload capability.
static THEME_STORE: OnceLock<RwLock<Theme>> = OnceLock::new();

fn load_initial_theme_or_exit() -> Theme {
    // When built and run via Cargo, prioritize repository config and create it
    if let Some(rcp) = repo_config_path() {
        if rcp.is_file() {
            if let Some(t) = load_theme_from_file(&rcp) {
                return t;
            }
        } else {
            // Create repository-local config from skeleton for dev runs
            if let Some(dir) = rcp.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&rcp, SKELETON_CONFIG_CONTENT);
            if let Some(t) = load_theme_from_file(&rcp) {
                return t;
            }
        }
    }
    if let Some(path) = resolve_config_path() {
        match try_load_theme_with_diagnostics(&path) {
            Ok(t) => return t,
            Err(msg) => {
                eprintln!(
                    "Pacsea: theme configuration errors in {}:\n{}",
                    path.display(),
                    msg
                );
            }
        }
        std::process::exit(1);
    } else {
        // No config found: write default skeleton to $XDG_CONFIG_HOME/pacsea/pacsea.conf
        let xdg_base = env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")));
        if let Some(base) = xdg_base {
            let target = base.join("pacsea").join("pacsea.conf");
            if !target.exists() {
                if let Some(dir) = target.parent() {
                    let _ = fs::create_dir_all(dir);
                }
                // Prefer repository config content if available; otherwise, use the skeleton template
                let content = env::current_dir()
                    .ok()
                    .map(|cwd| cwd.join("config").join("pacsea.conf"))
                    .and_then(|p| fs::read_to_string(p).ok())
                    .unwrap_or_else(|| SKELETON_CONFIG_CONTENT.to_string());
                let _ = fs::write(&target, content);
            }
            if let Some(t) = load_theme_from_file(&target) {
                return t;
            }
        }
        eprintln!(
            "Pacsea: theme configuration missing or incomplete. Please edit $XDG_CONFIG_HOME/pacsea/pacsea.conf (or ~/.config/pacsea/pacsea.conf)."
        );
        std::process::exit(1);
    }
}

/// Return the application's theme palette, loading from config if available.
///
/// The config file is searched in the following locations (first match wins):
/// - "$HOME/pacsea.conf"
/// - "$HOME/.config/pacsea.conf"
/// - "$HOME/.config/pacsea/pacsea.conf"
/// - "config/pacsea.conf" (useful for repository-local testing)
///
/// Format: key = value, one per line; values are colors as "#RRGGBB" or "R,G,B".
pub fn theme() -> Theme {
    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme_or_exit()));
    *lock.read().expect("theme store poisoned")
}

/// Reload the theme from disk without restarting the app.
/// Returns Ok(()) on success; Err(msg) if the config is missing or incomplete.
pub fn reload_theme() -> std::result::Result<(), String> {
    let path = resolve_config_path().or_else(|| {
        env::var("HOME").ok().map(|h| {
            Path::new(&h)
                .join(".config")
                .join("pacsea")
                .join("pacsea.conf")
        })
    });
    let Some(p) = path else {
        return Err("No theme configuration file found".to_string());
    };
    let new_theme = super::config::try_load_theme_with_diagnostics(&p)?;
    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme_or_exit()));
    if let Ok(mut guard) = lock.write() {
        *guard = new_theme;
        Ok(())
    } else {
        Err("Failed to acquire theme store for writing".to_string())
    }
}
