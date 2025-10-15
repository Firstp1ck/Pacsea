use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use super::config::{
    THEME_SKELETON_CONTENT, load_theme_from_file, try_load_theme_with_diagnostics,
};
use super::paths::resolve_theme_config_path;
use super::types::Theme;

/// Global theme store with live-reload capability.
static THEME_STORE: OnceLock<RwLock<Theme>> = OnceLock::new();

fn load_initial_theme_or_exit() -> Theme {
    if let Some(path) = resolve_theme_config_path() {
        match try_load_theme_with_diagnostics(&path) {
            Ok(t) => {
                tracing::info!(path = %path.display(), "loaded theme configuration");
                return t;
            }
            Err(msg) => {
                // If the file exists but is empty (0 bytes), treat as first-run and write skeleton.
                if let Ok(meta) = fs::metadata(&path)
                    && meta.len() == 0
                {
                    if let Some(dir) = path.parent() {
                        let _ = fs::create_dir_all(dir);
                    }
                    let _ = fs::write(&path, THEME_SKELETON_CONTENT);
                    if let Some(t) = load_theme_from_file(&path) {
                        tracing::info!(path = %path.display(), "wrote default theme skeleton and loaded");
                        return t;
                    }
                }
                tracing::error!(
                    path = %path.display(),
                    error = %msg,
                    "theme configuration errors"
                );
            }
        }
        std::process::exit(1);
    } else {
        // No config found: write default skeleton to $XDG_CONFIG_HOME/pacsea/theme.conf
        let xdg_base = env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")));
        if let Some(base) = xdg_base {
            let target = base.join("pacsea").join("theme.conf");
            if !target.exists() {
                if let Some(dir) = target.parent() {
                    let _ = fs::create_dir_all(dir);
                }
                let _ = fs::write(&target, THEME_SKELETON_CONTENT);
            }
            if let Some(t) = load_theme_from_file(&target) {
                tracing::info!(path = %target.display(), "initialized theme from default path");
                return t;
            }
        }
        tracing::error!(
            "theme configuration missing or incomplete. Please edit $XDG_CONFIG_HOME/pacsea/theme.conf (or ~/.config/pacsea/theme.conf)."
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
    let path = resolve_theme_config_path().or_else(|| {
        env::var("HOME").ok().map(|h| {
            Path::new(&h)
                .join(".config")
                .join("pacsea")
                .join("theme.conf")
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
