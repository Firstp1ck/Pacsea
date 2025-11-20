//! Internationalization helpers for CLI commands.

use std::collections::HashMap;
use pacsea::i18n::{self, find_locales_dir, load_locale_file, resolve_locale};
use pacsea::i18n::translations::{translate_with_fallback, TranslationMap};

/// What: Load translations for CLI usage.
///
/// Inputs:
/// - None (uses system locale detection and fallback chain).
///
/// Output:
/// - Tuple of (primary translations, fallback translations).
///
/// Details:
/// - Resolves locale from system or settings.
/// - Loads primary locale and English fallback.
/// - Returns empty maps if loading fails (graceful degradation).
pub fn load_cli_translations() -> (TranslationMap, TranslationMap) {
    // Get locales directory
    let locales_dir = match find_locales_dir() {
        Some(dir) => dir,
        None => {
            tracing::debug!("Locales directory not found, using English fallback");
            return (HashMap::new(), HashMap::new());
        }
    };

    // Resolve locale (try to read from settings, fallback to system/default)
    let i18n_config_path = match i18n::find_config_file("i18n.yml") {
        Some(path) => path,
        None => {
            tracing::debug!("i18n.yml not found, using default locale");
            let fallback = load_locale_file("en-US", &locales_dir).unwrap_or_default();
            return (fallback.clone(), fallback);
        }
    };

    // Try to read locale from settings
    let settings_locale = pacsea::theme::settings().locale.clone();

    let resolved_locale = resolve_locale(&settings_locale, &i18n_config_path);

    // Load primary locale
    let primary = load_locale_file(&resolved_locale, &locales_dir).unwrap_or_default();

    // Always load English as fallback
    let fallback = if resolved_locale == "en-US" {
        primary.clone()
    } else {
        load_locale_file("en-US", &locales_dir).unwrap_or_default()
    };

    (primary, fallback)
}

/// What: Get a translation for CLI usage.
///
/// Inputs:
/// - `key`: Dot-notation key (e.g., "app.cli.refresh.starting").
///
/// Output:
/// - Translated string, or key itself if translation not found.
///
/// Details:
/// - Uses lazy static to cache translations (loaded once).
/// - Falls back to English if primary locale missing.
/// - Returns key itself if both missing (for debugging).
pub fn t(key: &str) -> String {
    use std::sync::OnceLock;

    static TRANSLATIONS: OnceLock<(TranslationMap, TranslationMap)> = OnceLock::new();

    let (primary, fallback) = TRANSLATIONS.get_or_init(load_cli_translations);

    translate_with_fallback(key, primary, fallback)
}

/// What: Get a translation with format arguments.
///
/// Inputs:
/// - `key`: Dot-notation key.
/// - `args`: Format arguments (as Display trait objects).
///
/// Output:
/// - Formatted translated string.
///
/// Details:
/// - Replaces placeholders in order: first {} gets first arg, etc.
/// - Supports multiple placeholders: "{} and {}" -> "arg1 and arg2".
pub fn t_fmt(key: &str, args: &[&dyn std::fmt::Display]) -> String {
    let translation = t(key);
    let mut result = translation;
    for arg in args {
        result = result.replacen("{}", &arg.to_string(), 1);
    }
    result
}

/// What: Get a translation with a single format argument (convenience function).
///
/// Inputs:
/// - `key`: Dot-notation key.
/// - `arg`: Single format argument.
///
/// Output:
/// - Formatted translated string.
pub fn t_fmt1<T: std::fmt::Display>(key: &str, arg: T) -> String {
    t_fmt(key, &[&arg])
}

/// What: Get a translation with two format arguments (convenience function).
///
/// Inputs:
/// - `key`: Dot-notation key.
/// - `arg1`: First format argument.
/// - `arg2`: Second format argument.
///
/// Output:
/// - Formatted translated string.
pub fn t_fmt2<T1: std::fmt::Display, T2: std::fmt::Display>(
    key: &str,
    arg1: T1,
    arg2: T2,
) -> String {
    t_fmt(key, &[&arg1, &arg2])
}

