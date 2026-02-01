//! Theme store with live-reload capability.
//!
//! This module provides the global theme cache and functions to access and reload
//! the theme. It uses the unified resolution logic from `resolve.rs` to determine
//! which theme source to use.

use std::fs;
use std::sync::{OnceLock, RwLock};

use super::config::THEME_SKELETON_CONTENT;
use super::paths::{config_dir, resolve_theme_config_path};
use super::resolve::{ThemeSource, resolve_theme};
use super::types::Theme;

/// Global theme store with live-reload capability.
static THEME_STORE: OnceLock<RwLock<Theme>> = OnceLock::new();

/// What: Load theme using the unified resolution logic.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Returns a fully-populated `Theme`.
///
/// Details:
/// - Uses `resolve_theme()` to determine the best theme source.
/// - When using codebase default and no theme.conf exists, writes the skeleton.
/// - Never exits the process; always returns a valid theme.
fn load_initial_theme() -> Theme {
    let resolved = resolve_theme();

    // If we're using the default theme and no theme.conf exists, create one
    // so the user has a file to edit
    if resolved.source == ThemeSource::Default {
        ensure_theme_file_exists();
    }

    resolved.theme
}

/// Ensure theme.conf exists, creating it from skeleton if needed.
fn ensure_theme_file_exists() {
    let path = resolve_theme_config_path().unwrap_or_else(|| config_dir().join("theme.conf"));

    // Only create if file doesn't exist or is empty
    let should_create = fs::metadata(&path).map_or(true, |meta| meta.len() == 0);

    if should_create {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&path, THEME_SKELETON_CONTENT);
        tracing::info!(path = %path.display(), "Created theme.conf skeleton");
    }
}

/// What: Access the application's theme palette, loading or caching as needed.
///
/// Inputs:
/// - None.
///
/// Output:
/// - A copy of the currently loaded `Theme`.
///
/// # Panics
/// - Panics if the theme store `RwLock` is poisoned
///
/// Details:
/// - Lazily initializes a global `RwLock<Theme>` using `load_initial_theme`.
/// - Subsequent calls reuse the cached theme until `reload_theme` updates it.
pub fn theme() -> Theme {
    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme()));
    *lock.read().expect("theme store poisoned")
}

/// What: Reload the theme configuration on demand.
///
/// Inputs:
/// - None (uses the unified resolution logic).
///
/// Output:
/// - `Ok(())` when the theme is reloaded successfully.
/// - `Err(String)` with a human-readable reason when reloading fails.
///
/// # Errors
/// - Returns `Err` if the theme store lock cannot be acquired
///
/// Details:
/// - Re-runs the full resolution logic (reads settings, theme.conf, queries terminal).
/// - Keeps the in-memory cache up to date so the UI can refresh without restarting Pacsea.
/// - With the new resolution logic, this never fails due to missing/invalid theme.conf
///   as it falls back to terminal theme or codebase default.
pub fn reload_theme() -> std::result::Result<(), String> {
    // Re-run resolution to pick up any settings changes
    let resolved = resolve_theme();

    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme()));
    lock.write().map_or_else(
        |_| Err("Failed to acquire theme store for writing".to_string()),
        |mut guard| {
            *guard = resolved.theme;
            tracing::info!(source = ?resolved.source, "Theme reloaded");
            Ok(())
        },
    )
}
