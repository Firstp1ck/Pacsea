use std::fs;
use std::sync::{OnceLock, RwLock};

use super::config::{
    THEME_SKELETON_CONTENT, load_theme_from_file, try_load_theme_with_diagnostics,
};
use super::paths::{config_dir, resolve_theme_config_path};
use super::types::Theme;

/// Global theme store with live-reload capability.
static THEME_STORE: OnceLock<RwLock<Theme>> = OnceLock::new();

/// What: Load theme colors from disk or generate a skeleton configuration if nothing exists yet.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Returns a fully-populated `Theme` on success.
/// - Terminates the process with an error when recovery is impossible.
///
/// Details:
/// - Prefers existing config files found by `resolve_theme_config_path`.
/// - Writes `THEME_SKELETON_CONTENT` when encountering empty or missing files to keep the app usable.
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
    } else {
        // No config found: write default skeleton to config_dir()/theme.conf
        let config_directory = config_dir();
        let target = config_directory.join("theme.conf");
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
        tracing::error!(
            path = %target.display(),
            "theme configuration missing or incomplete. Please edit the theme.conf file at the path shown above."
        );
    }
    std::process::exit(1);
}

/// What: Access the application's theme palette, loading or caching as needed.
///
/// Inputs:
/// - None.
///
/// Output:
/// - A copy of the currently loaded `Theme`.
///
/// Details:
/// - Lazily initializes a global `RwLock<Theme>` using `load_initial_theme_or_exit`.
/// - Subsequent calls reuse the cached theme until `reload_theme` updates it.
pub fn theme() -> Theme {
    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme_or_exit()));
    *lock.read().expect("theme store poisoned")
}

/// What: Reload the theme configuration from disk on demand.
///
/// Inputs:
/// - None (locates the config through `resolve_theme_config_path`).
///
/// Output:
/// - `Ok(())` when the theme is reloaded successfully.
/// - `Err(String)` with a human-readable reason when reloading fails.
///
/// # Errors
/// - Returns `Err` when no theme configuration file is found
/// - Returns `Err` when theme file cannot be loaded or parsed
/// - Returns `Err` when theme validation fails
///
/// Details:
/// - Keeps the in-memory cache up to date so the UI can refresh without restarting Pacsea.
/// - Returns an error if the theme file is missing or contains validation problems.
pub fn reload_theme() -> std::result::Result<(), String> {
    let path = resolve_theme_config_path().or_else(|| Some(config_dir().join("theme.conf")));
    let Some(p) = path else {
        return Err("No theme configuration file found".to_string());
    };
    let new_theme = super::config::try_load_theme_with_diagnostics(&p)?;
    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme_or_exit()));
    lock.write().map_or_else(
        |_| Err("Failed to acquire theme store for writing".to_string()),
        |mut guard| {
            *guard = new_theme;
            Ok(())
        },
    )
}
