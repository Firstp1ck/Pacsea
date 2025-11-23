//! Locale resolution with fallback chain support.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::i18n::detection::detect_system_locale;

/// What: Resolve the effective locale to use, following fallback chain.
///
/// Inputs:
/// - `settings_locale`: Locale from settings.conf (empty string means auto-detect)
/// - `i18n_config_path`: Path to config/i18n.yml
///
/// Output:
/// - Resolved locale code (e.g., "de-DE")
///
/// Details:
/// - Priority: `settings_locale` -> system locale -> default from `i18n.yml`
/// - Applies fallback chain from `i18n.yml` (e.g., de-CH -> de-DE -> en-US)
/// - Validates locale format (basic check for valid locale code structure)
#[must_use]
pub fn resolve_locale(settings_locale: &str, i18n_config_path: &PathBuf) -> String {
    let fallbacks = load_fallbacks(i18n_config_path);
    let default_locale = load_default_locale(i18n_config_path);
    let available_locales = load_available_locales(i18n_config_path);

    // Determine initial locale
    let initial_locale = if settings_locale.trim().is_empty() {
        // Auto-detect from system
        detect_system_locale().unwrap_or_else(|| {
            tracing::debug!(
                "System locale detection failed, using default: {}",
                default_locale
            );
            default_locale.clone()
        })
    } else {
        let trimmed = settings_locale.trim().to_string();
        // Validate locale format (basic check)
        if is_valid_locale_format(&trimmed) {
            trimmed
        } else {
            tracing::warn!(
                "Invalid locale format in settings.conf: '{}'. Using system locale or default.",
                trimmed
            );
            detect_system_locale().unwrap_or_else(|| default_locale.clone())
        }
    };

    // Apply fallback chain
    let resolved = resolve_with_fallbacks(
        &initial_locale,
        &fallbacks,
        &default_locale,
        &available_locales,
    );

    if resolved != initial_locale {
        tracing::debug!(
            "Locale '{}' resolved to '{}' via fallback chain",
            initial_locale,
            resolved
        );
    }

    resolved
}

/// What: Validate locale code format.
///
/// Inputs:
/// - `locale`: Locale code to validate
///
/// Output:
/// - `true` if format looks valid, `false` otherwise
///
/// Details:
/// - Checks for basic structure: language[-region] or language[-script][-region]
/// - Allows simple language codes (e.g., "en") or full codes (e.g., "en-US")
/// - Rejects obviously invalid formats (empty, spaces, special chars)
fn is_valid_locale_format(locale: &str) -> bool {
    if locale.is_empty() || locale.len() > 20 {
        return false;
    }

    // Basic pattern: language[-region] or language[-script][-region]
    // Allow: en, en-US, de-DE, zh-Hans-CN, etc.
    // Reject: spaces, most special chars (except hyphens)
    locale.chars().all(|c| c.is_alphanumeric() || c == '-')
        && !locale.starts_with('-')
        && !locale.ends_with('-')
        && !locale.contains("--")
}

/// What: Resolve locale using fallback chain.
///
/// Inputs:
/// - `locale`: Initial locale code
/// - `fallbacks`: Map of locale -> fallback locale
/// - `default_locale`: Ultimate fallback (usually "en-US")
///
/// Output:
/// - Resolved locale that exists in available locales
///
/// Details:
/// - Follows fallback chain until reaching a locale without fallback or default
/// - Prevents infinite loops with cycle detection
/// - Logs warnings for suspicious fallback chains
fn resolve_with_fallbacks(
    locale: &str,
    fallbacks: &HashMap<String, String>,
    default_locale: &str,
    available_locales: &std::collections::HashSet<String>,
) -> String {
    let mut current = locale.to_string();
    let mut visited = std::collections::HashSet::new();

    // Follow fallback chain until we find a valid locale or hit default
    while visited.insert(current.clone()) {
        // Check if we have a fallback for this locale
        if let Some(fallback) = fallbacks.get(&current) {
            tracing::debug!("Locale '{}' has fallback: {}", current, fallback);
            current.clone_from(fallback);
        } else {
            // No fallback defined - check if this locale is available
            if available_locales.contains(&current) {
                // Locale is available, use it directly
                tracing::debug!(
                    "Locale '{}' has no fallback but is available, using it directly",
                    current
                );
                return current;
            } else if current == default_locale {
                // Default locale is always valid
                tracing::debug!(
                    "Locale '{}' has no fallback and is the default locale, using it directly",
                    current
                );
                return current;
            }
            // Locale not available and not default, fall back to default
            tracing::debug!(
                "Locale '{}' has no fallback and is not available, falling back to default: {}",
                current,
                default_locale
            );
            return default_locale.to_string();
        }

        // Safety check: prevent infinite loops
        if visited.len() > 10 {
            tracing::warn!(
                "Fallback chain too long ({} steps) for locale '{}', using default: {}",
                visited.len(),
                locale,
                default_locale
            );
            return default_locale.to_string();
        }
    }

    // Detected a cycle in fallback chain
    tracing::warn!(
        "Detected cycle in fallback chain for locale '{}', using default: {}",
        locale,
        default_locale
    );
    default_locale.to_string()
}

/// What: Load fallback mappings from i18n.yml.
///
/// Inputs:
/// - `config_path`: Path to config/i18n.yml
///
/// Output:
/// - `HashMap` mapping locale codes to their fallback locales
fn load_fallbacks(config_path: &PathBuf) -> HashMap<String, String> {
    let mut fallbacks = HashMap::new();

    if let Ok(contents) = fs::read_to_string(config_path)
        && let Ok(doc) = serde_norway::from_str::<serde_norway::Value>(&contents)
        && let Some(fallbacks_map) = doc.get("fallbacks").and_then(|v| v.as_mapping())
    {
        for (key, value) in fallbacks_map {
            if let (Some(k), Some(v)) = (key.as_str(), value.as_str()) {
                fallbacks.insert(k.to_string(), v.to_string());
            }
        }
        tracing::debug!(
            "Loaded {} fallback mappings from i18n.yml: {:?}",
            fallbacks.len(),
            fallbacks.keys().collect::<Vec<_>>()
        );
    } else {
        tracing::warn!("Failed to load fallbacks from i18n.yml");
    }

    fallbacks
}

/// What: Load available locales from i18n.yml.
///
/// Inputs:
/// - `config_path`: Path to config/i18n.yml
///
/// Output:
/// - `HashSet` of available locale codes
fn load_available_locales(config_path: &PathBuf) -> std::collections::HashSet<String> {
    let mut locales = std::collections::HashSet::new();

    if let Ok(contents) = fs::read_to_string(config_path)
        && let Ok(doc) = serde_norway::from_str::<serde_norway::Value>(&contents)
        && let Some(locales_map) = doc.get("locales").and_then(|v| v.as_mapping())
    {
        for key in locales_map.keys() {
            if let Some(locale) = key.as_str() {
                locales.insert(locale.to_string());
            }
        }
        tracing::debug!(
            "Loaded {} available locales from i18n.yml: {:?}",
            locales.len(),
            locales.iter().collect::<Vec<_>>()
        );
    }

    locales
}

/// What: Load default locale from i18n.yml.
///
/// Inputs:
/// - `config_path`: Path to config/i18n.yml
///
/// Output:
/// - Default locale code (defaults to "en-US" if not found)
fn load_default_locale(config_path: &PathBuf) -> String {
    if let Ok(contents) = fs::read_to_string(config_path)
        && let Ok(doc) = serde_norway::from_str::<serde_norway::Value>(&contents)
        && let Some(default) = doc.get("default_locale").and_then(|v| v.as_str())
    {
        return default.to_string();
    }

    "en-US".to_string()
}

/// Locale resolver that caches configuration.
pub struct LocaleResolver {
    fallbacks: HashMap<String, String>,
    default_locale: String,
    available_locales: std::collections::HashSet<String>,
}

impl LocaleResolver {
    /// What: Create a new `LocaleResolver` by loading `i18n.yml`.
    ///
    /// Inputs:
    /// - `i18n_config_path`: Path to config/i18n.yml
    ///
    /// Output:
    /// - `LocaleResolver` instance
    #[must_use]
    pub fn new(i18n_config_path: &PathBuf) -> Self {
        Self {
            fallbacks: load_fallbacks(i18n_config_path),
            default_locale: load_default_locale(i18n_config_path),
            available_locales: load_available_locales(i18n_config_path),
        }
    }

    /// What: Resolve locale using cached fallback configuration.
    ///
    /// Inputs:
    /// - `settings_locale`: Locale from settings.conf
    ///
    /// Output:
    /// - Resolved locale code
    #[must_use]
    pub fn resolve(&self, settings_locale: &str) -> String {
        let initial_locale = if settings_locale.trim().is_empty() {
            detect_system_locale().unwrap_or_else(|| self.default_locale.clone())
        } else {
            let trimmed = settings_locale.trim().to_string();
            // Validate locale format (basic check)
            if is_valid_locale_format(&trimmed) {
                trimmed
            } else {
                tracing::warn!(
                    "Invalid locale format in settings.conf: '{}'. Using system locale or default.",
                    trimmed
                );
                detect_system_locale().unwrap_or_else(|| self.default_locale.clone())
            }
        };

        tracing::debug!(
            "Resolving locale '{}' with {} fallbacks available",
            initial_locale,
            self.fallbacks.len()
        );
        if initial_locale == "ch" {
            tracing::debug!(
                "Checking for 'ch' in fallbacks: {}",
                self.fallbacks.contains_key("ch")
            );
            if let Some(fallback) = self.fallbacks.get("ch") {
                tracing::debug!("Found fallback for 'ch': {}", fallback);
            }
        }

        let resolved = resolve_with_fallbacks(
            &initial_locale,
            &self.fallbacks,
            &self.default_locale,
            &self.available_locales,
        );

        if resolved != initial_locale {
            tracing::debug!(
                "Locale '{}' resolved to '{}' via fallback chain",
                initial_locale,
                resolved
            );
        }

        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_with_fallbacks() {
        let mut fallbacks = HashMap::new();
        fallbacks.insert("de-CH".to_string(), "de-DE".to_string());
        fallbacks.insert("de".to_string(), "de-DE".to_string());

        let mut available_locales = std::collections::HashSet::new();
        available_locales.insert("de-DE".to_string());
        available_locales.insert("en-US".to_string());

        // Test fallback chain: de-CH -> de-DE (available, stops here)
        assert_eq!(
            resolve_with_fallbacks("de-CH", &fallbacks, "en-US", &available_locales),
            "de-DE" // de-CH -> de-DE (available, stops)
        );

        // Test that de-DE is used directly when available
        assert_eq!(
            resolve_with_fallbacks("de-DE", &fallbacks, "en-US", &available_locales),
            "de-DE"
        );

        // Test that default locale returns itself
        assert_eq!(
            resolve_with_fallbacks("en-US", &fallbacks, "en-US", &available_locales),
            "en-US"
        );

        // Test single-part locale fallback
        // de -> de-DE (available, stops)
        assert_eq!(
            resolve_with_fallbacks("de", &fallbacks, "en-US", &available_locales),
            "de-DE" // de -> de-DE (available, stops)
        );

        // Test that unavailable locale falls back to default
        let mut available_locales_no_de = std::collections::HashSet::new();
        available_locales_no_de.insert("en-US".to_string());
        assert_eq!(
            resolve_with_fallbacks("de-DE", &fallbacks, "en-US", &available_locales_no_de),
            "en-US" // de-DE not available, falls back to default
        );
    }

    #[test]
    fn test_resolve_with_fallbacks_cycle_detection() {
        let mut fallbacks = HashMap::new();
        // Create a cycle: a -> b -> c -> a
        fallbacks.insert("a".to_string(), "b".to_string());
        fallbacks.insert("b".to_string(), "c".to_string());
        fallbacks.insert("c".to_string(), "a".to_string());

        let available_locales = std::collections::HashSet::new();

        // Should detect cycle and return default
        let result = resolve_with_fallbacks("a", &fallbacks, "en-US", &available_locales);
        assert_eq!(result, "en-US");
    }

    #[test]
    fn test_resolve_with_fallbacks_long_chain() {
        let mut fallbacks = HashMap::new();
        // Create a long chain
        for i in 0..15 {
            let next = i + 1;
            fallbacks.insert(format!("loc{i}"), format!("loc{next}"));
        }

        let available_locales = std::collections::HashSet::new();

        // Should hit max length limit and return default
        let result = resolve_with_fallbacks("loc0", &fallbacks, "en-US", &available_locales);
        assert_eq!(result, "en-US");
    }

    #[test]
    fn test_is_valid_locale_format() {
        // Valid formats
        assert!(is_valid_locale_format("en-US"));
        assert!(is_valid_locale_format("de-DE"));
        assert!(is_valid_locale_format("zh-Hans-CN"));
        assert!(is_valid_locale_format("en"));
        assert!(is_valid_locale_format("fr-FR"));

        // Invalid formats
        assert!(!is_valid_locale_format(""));
        assert!(!is_valid_locale_format("-en-US"));
        assert!(!is_valid_locale_format("en-US-"));
        assert!(!is_valid_locale_format("en--US"));
        assert!(!is_valid_locale_format("en US"));
        assert!(!is_valid_locale_format("en@US"));
        assert!(!is_valid_locale_format(&"x".repeat(21))); // Too long
    }

    #[test]
    fn test_load_fallbacks() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let config_path = temp_dir.path().join("i18n.yml");

        let yaml_content = r"
default_locale: en-US
fallbacks:
  de-CH: de-DE
  de: de-DE
  fr: fr-FR
";
        fs::write(&config_path, yaml_content).expect("Failed to write test config file");

        let fallbacks = load_fallbacks(&config_path);
        assert_eq!(fallbacks.get("de-CH"), Some(&"de-DE".to_string()));
        assert_eq!(fallbacks.get("de"), Some(&"de-DE".to_string()));
        assert_eq!(fallbacks.get("fr"), Some(&"fr-FR".to_string()));
    }

    #[test]
    fn test_load_default_locale() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let config_path = temp_dir.path().join("i18n.yml");

        let yaml_content = r"
default_locale: de-DE
fallbacks:
  de-CH: de-DE
";
        fs::write(&config_path, yaml_content).expect("Failed to write test config file");

        let default = load_default_locale(&config_path);
        assert_eq!(default, "de-DE");
    }

    #[test]
    fn test_load_default_locale_missing() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let config_path = temp_dir.path().join("i18n.yml");

        let yaml_content = r"
fallbacks:
  de-CH: de-DE
";
        fs::write(&config_path, yaml_content).expect("Failed to write test config file");

        let default = load_default_locale(&config_path);
        assert_eq!(default, "en-US"); // Should default to en-US
    }

    #[test]
    fn test_resolve_locale_with_settings() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let config_path = temp_dir.path().join("i18n.yml");

        let yaml_content = r"
default_locale: en-US
fallbacks:
  de-CH: de-DE
";
        fs::write(&config_path, yaml_content).expect("Failed to write test config file");

        // Test with explicit locale from settings
        // de-CH -> de-DE -> (no fallback) -> en-US (default)
        let result = resolve_locale("de-CH", &config_path);
        assert_eq!(result, "en-US"); // Should fallback through chain to default

        // Test with valid locale that has no fallback
        let result = resolve_locale("en-US", &config_path);
        assert_eq!(result, "en-US");

        // Test with invalid locale format
        let result = resolve_locale("invalid-format-", &config_path);
        // Should fallback to system/default (may vary based on environment)
        assert!(!result.is_empty());
    }

    #[test]
    fn test_resolve_locale_empty_settings() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let config_path = temp_dir.path().join("i18n.yml");

        let yaml_content = r"
default_locale: en-US
fallbacks:
  de-CH: de-DE
";
        fs::write(&config_path, yaml_content).expect("Failed to write test config file");

        // Test with empty settings (should auto-detect or use default)
        let result = resolve_locale("", &config_path);
        // Result depends on system locale, but should not be empty
        assert!(!result.is_empty());

        // Test with whitespace-only settings
        let result = resolve_locale("   ", &config_path);
        assert!(!result.is_empty());
    }
}
