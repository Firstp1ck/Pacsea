//! Translation map and lookup utilities.

use std::collections::HashMap;

/// Translation map: dot-notation key -> translated string.
pub type TranslationMap = HashMap<String, String>;

/// What: Look up a translation in the translation map.
///
/// Inputs:
/// - `key`: Dot-notation key (e.g., "app.titles.search")
/// - `translations`: Translation map to search
///
/// Output:
/// - `Option<String>` containing translation or None if not found
///
/// Details:
/// - Direct key lookup
/// - Returns None if key not found
#[must_use]
pub fn translate(key: &str, translations: &TranslationMap) -> Option<String> {
    translations.get(key).cloned()
}

/// What: Look up translation with fallback to English.
///
/// Inputs:
/// - `key`: Dot-notation key
/// - `translations`: Primary translation map
/// - `fallback_translations`: Fallback translation map (usually English)
///
/// Output:
/// - Translated string (from primary or fallback, or key itself if both missing)
///
/// Details:
/// - Tries primary translations first
/// - Falls back to English if not found
/// - Returns key itself if neither has translation (for debugging)
/// - Logs warnings for missing keys (only once per key to avoid spam)
pub fn translate_with_fallback(
    key: &str,
    translations: &TranslationMap,
    fallback_translations: &TranslationMap,
) -> String {
    // Try primary translations first
    if let Some(translation) = translations.get(key) {
        return translation.clone();
    }

    // Try fallback translations
    if let Some(translation) = fallback_translations.get(key) {
        // Log that we're using fallback (only at debug level to avoid spam)
        tracing::debug!(
            "Translation key '{}' not found in primary locale, using fallback",
            key
        );
        return translation.clone();
    }

    // Neither has the key - log warning and return key itself
    // Use debug level to avoid flooding logs, but make it discoverable
    tracing::debug!(
        "Missing translation key: '{}'. Returning key as-is. Please add this key to locale files.",
        key
    );
    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate() {
        let mut translations = HashMap::new();
        translations.insert("app.titles.search".to_string(), "Suche".to_string());

        assert_eq!(
            translate("app.titles.search", &translations),
            Some("Suche".to_string())
        );
        assert_eq!(translate("app.titles.help", &translations), None);
    }

    #[test]
    fn test_translate_with_fallback() {
        let mut primary = HashMap::new();
        primary.insert("app.titles.search".to_string(), "Suche".to_string());

        let mut fallback = HashMap::new();
        fallback.insert("app.titles.search".to_string(), "Search".to_string());
        fallback.insert("app.titles.help".to_string(), "Help".to_string());

        assert_eq!(
            translate_with_fallback("app.titles.search", &primary, &fallback),
            "Suche"
        );
        assert_eq!(
            translate_with_fallback("app.titles.help", &primary, &fallback),
            "Help"
        );
        assert_eq!(
            translate_with_fallback("app.titles.missing", &primary, &fallback),
            "app.titles.missing"
        );
    }
}
