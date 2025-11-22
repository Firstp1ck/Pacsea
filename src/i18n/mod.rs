//! Internationalization (i18n) module for Pacsea.
//!
//! This module provides locale detection, resolution, loading, and translation lookup.
//!
//! # Overview
//!
//! The i18n system supports:
//! - **Locale Detection**: Auto-detects system locale from environment variables (`LANG`, `LC_ALL`, `LC_MESSAGES`)
//! - **Locale Resolution**: Resolves locale with fallback chain (settings -> system -> default)
//! - **Fallback Chain**: Supports locale fallbacks (e.g., `de-CH` -> `de-DE` -> `en-US`)
//! - **Translation Loading**: Loads YAML locale files from `locales/` directory
//! - **Translation Lookup**: Provides `t()`, `t_fmt()`, and `t_fmt1()` helpers for translation access
//!
//! # Locale Files
//!
//! Locale files are stored in `locales/{locale}.yml` (e.g., `locales/en-US.yml`, `locales/de-DE.yml`).
//! Each file contains a nested YAML structure that is flattened into dot-notation keys:
//!
//! ```yaml
//! app:
//!   titles:
//!     search: "Search"
//! ```
//!
//! This becomes accessible as `app.titles.search`.
//!
//! # Configuration
//!
//! The i18n system is configured via `config/i18n.yml`:
//! - `default_locale`: Default locale if auto-detection fails (usually `en-US`)
//! - `fallbacks`: Map of locale codes to their fallback locales
//!
//! # Usage
//!
//! ```rust,no_run
//! use pacsea::i18n;
//! use pacsea::state::AppState;
//!
//! # let mut app = AppState::default();
//! // Simple translation lookup
//! let text = i18n::t(&app, "app.titles.search");
//!
//! // Translation with format arguments
//! let file_path = "/path/to/file";
//! let text = i18n::t_fmt1(&app, "app.toasts.exported_to", file_path);
//! ```
//!
//! # Adding a New Locale
//!
//! 1. Create `locales/{locale}.yml` (e.g., `locales/fr-FR.yml`)
//! 2. Copy structure from `locales/en-US.yml` and translate all strings
//! 3. Optionally add fallback in `config/i18n.yml` if needed (e.g., `fr: fr-FR`)
//! 4. Users can set `locale = fr-FR` in `settings.conf` or leave empty for auto-detection
//!
//! # Error Handling
//!
//! - Missing locale files fall back to English automatically
//! - Invalid locale codes in `settings.conf` trigger warnings and fallback to system/default
//! - Missing translation keys return the key itself (for debugging) and log debug messages
//! - All errors are logged but do not crash the application

mod detection;
mod loader;
mod resolver;
pub mod translations;

pub use detection::detect_system_locale;
pub use loader::{LocaleLoader, load_locale_file};
pub use resolver::{LocaleResolver, resolve_locale};
pub use translations::{TranslationMap, translate, translate_with_fallback};

use std::path::PathBuf;

/// What: Find a config file in development and installed locations.
///
/// Inputs:
/// - `relative_path`: Relative path from config directory (e.g., "i18n.yml")
///
/// Output:
/// - `Some(PathBuf)` pointing to the first existing file found, or `None` if not found
///
/// Details:
/// - Tries locations in order:
///   1. Development location: `CARGO_MANIFEST_DIR/config/{relative_path}` (prioritized when running from source)
///   2. Installed location: `/usr/share/pacsea/config/{relative_path}`
/// - Development location is checked first to allow working with repo files during development
pub fn find_config_file(relative_path: &str) -> Option<PathBuf> {
    // Try development location first (when running from source)
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join(relative_path);
    if dev_path.exists() {
        return Some(dev_path);
    }

    // Try installed location
    let installed_path = PathBuf::from("/usr/share/pacsea/config").join(relative_path);
    if installed_path.exists() {
        return Some(installed_path);
    }

    None
}

/// What: Find the locales directory in development and installed locations.
///
/// Output:
/// - `Some(PathBuf)` pointing to the first existing locales directory found, or `None` if not found
///
/// Details:
/// - Tries locations in order:
///   1. Development location: `CARGO_MANIFEST_DIR/config/locales` (prioritized when running from source)
///   2. Installed location: `/usr/share/pacsea/locales`
/// - Development location is checked first to allow working with repo files during development
pub fn find_locales_dir() -> Option<PathBuf> {
    // Try development location first (when running from source)
    // Note: locales are in config/locales/ in the dev environment
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("locales");
    if dev_path.exists() && dev_path.is_dir() {
        return Some(dev_path);
    }

    // Also try the old location for backwards compatibility
    let dev_path_old = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales");
    if dev_path_old.exists() && dev_path_old.is_dir() {
        return Some(dev_path_old);
    }

    // Try installed location
    let installed_path = PathBuf::from("/usr/share/pacsea/locales");
    if installed_path.exists() && installed_path.is_dir() {
        return Some(installed_path);
    }

    None
}

/// What: Get a translation for a given key from `AppState`.
///
/// Inputs:
/// - `app`: `AppState` containing translation maps
/// - `key`: Dot-notation key (e.g., "app.titles.search")
///
/// Output:
/// - Translated string, or the key itself if translation not found
///
/// Details:
/// - Uses translations from `AppState`
/// - Falls back to English if translation missing
pub fn t(app: &crate::state::AppState, key: &str) -> String {
    crate::i18n::translations::translate_with_fallback(
        key,
        &app.translations,
        &app.translations_fallback,
    )
}

/// What: Get a translation with format arguments.
///
/// Inputs:
/// - `app`: `AppState` containing translation maps
/// - `key`: Dot-notation key
/// - `args`: Format arguments (as Display trait objects)
///
/// Output:
/// - Formatted translated string
///
/// Details:
/// - Replaces placeholders in order: first {} gets first arg, etc.
/// - Supports multiple placeholders: "{} and {}" -> "arg1 and arg2"
pub fn t_fmt(app: &crate::state::AppState, key: &str, args: &[&dyn std::fmt::Display]) -> String {
    let translation = t(app, key);
    let mut result = translation;
    for arg in args {
        result = result.replacen("{}", &arg.to_string(), 1);
    }
    result
}

/// What: Get a translation with a single format argument (convenience function).
///
/// Inputs:
/// - `app`: `AppState` containing translation maps
/// - `key`: Dot-notation key
/// - `arg`: Single format argument
///
/// Output:
/// - Formatted translated string
pub fn t_fmt1<T: std::fmt::Display>(app: &crate::state::AppState, key: &str, arg: T) -> String {
    t_fmt(app, key, &[&arg])
}
