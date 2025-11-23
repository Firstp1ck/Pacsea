//! Locale file loading and parsing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::i18n::translations::TranslationMap;

/// What: Load a locale YAML file and parse it into a `TranslationMap`.
///
/// Inputs:
/// - `locale`: Locale code (e.g., "de-DE")
/// - `locales_dir`: Path to locales directory
///
/// Output:
/// - `Result<TranslationMap, String>` containing translations or error
///
/// # Errors
/// - Returns `Err` when the locale code is empty or has an invalid format
/// - Returns `Err` when the locale file does not exist in the locales directory
/// - Returns `Err` when the locale file cannot be read (I/O error)
/// - Returns `Err` when the locale file is empty
/// - Returns `Err` when the YAML content cannot be parsed
///
/// Details:
/// - Loads file from `locales_dir/{locale}.yml`
/// - Parses YAML structure into nested `HashMap`
/// - Returns error if file not found or invalid YAML
/// - Validates locale format before attempting to load
#[must_use]
pub fn load_locale_file(locale: &str, locales_dir: &Path) -> Result<TranslationMap, String> {
    // Validate locale format
    if locale.is_empty() {
        return Err("Locale code cannot be empty".to_string());
    }

    if !is_valid_locale_format(locale) {
        return Err(format!(
            "Invalid locale code format: '{locale}'. Expected format: language[-region] (e.g., 'en-US', 'de-DE')"
        ));
    }

    let file_path = locales_dir.join(format!("{locale}.yml"));

    if !file_path.exists() {
        return Err(format!(
            "Locale file not found: {}. Available locales can be checked in the locales/ directory.",
            file_path.display()
        ));
    }

    let contents = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read locale file {}: {e}", file_path.display()))?;

    if contents.trim().is_empty() {
        return Err(format!("Locale file is empty: {}", file_path.display()));
    }

    parse_locale_yaml(&contents).map_err(|e| {
        format!(
            "Failed to parse locale file {}: {}. Please check YAML syntax.",
            file_path.display(),
            e
        )
    })
}

/// What: Validate locale code format (same as resolver).
///
/// Inputs:
/// - `locale`: Locale code to validate
///
/// Output:
/// - `true` if format looks valid, `false` otherwise
fn is_valid_locale_format(locale: &str) -> bool {
    if locale.is_empty() || locale.len() > 20 {
        return false;
    }

    locale.chars().all(|c| c.is_alphanumeric() || c == '-')
        && !locale.starts_with('-')
        && !locale.ends_with('-')
        && !locale.contains("--")
}

/// What: Parse YAML content into a `TranslationMap`.
///
/// Inputs:
/// - `yaml_content`: YAML file content as string
///
/// Output:
/// - `Result<TranslationMap, String>` containing parsed translations
///
/// Details:
/// - Expects top-level key matching locale code (e.g., "de-DE:")
/// - Flattens nested structure into dot-notation keys
fn parse_locale_yaml(yaml_content: &str) -> Result<TranslationMap, String> {
    let doc: serde_norway::Value =
        serde_norway::from_str(yaml_content).map_err(|e| format!("Failed to parse YAML: {e}"))?;

    let mut translations = HashMap::new();

    // Get the top-level locale key (e.g., "de-DE")
    if let Some(locale_obj) = doc.as_mapping() {
        for (_locale_key, locale_value) in locale_obj {
            // Flatten the nested structure (skip the top-level locale key)
            flatten_yaml_value(locale_value, "", &mut translations);
        }
    }

    Ok(translations)
}

/// What: Recursively flatten YAML structure into dot-notation keys.
///
/// Inputs:
/// - `value`: Current YAML value
/// - `prefix`: Current key prefix (e.g., "app.titles")
/// - `translations`: Map to populate
///
/// Details:
/// - Converts nested maps to dot-notation (e.g., app.titles.search)
/// - Handles arrays by preserving them as YAML values
fn flatten_yaml_value(
    value: &serde_norway::Value,
    prefix: &str,
    translations: &mut TranslationMap,
) {
    match value {
        serde_norway::Value::Mapping(map) => {
            for (key, val) in map {
                if let Some(key_str) = key.as_str() {
                    let new_prefix = if prefix.is_empty() {
                        key_str.to_string()
                    } else {
                        format!("{prefix}.{key_str}")
                    };
                    flatten_yaml_value(val, &new_prefix, translations);
                }
            }
        }
        serde_norway::Value::String(s) => {
            translations.insert(prefix.to_string(), s.clone());
        }
        serde_norway::Value::Sequence(_seq) => {
            // Store arrays as YAML strings for now
            // Can be enhanced later to handle arrays properly
            if let Ok(yaml_str) = serde_norway::to_string(value) {
                translations.insert(prefix.to_string(), yaml_str.trim().to_string());
            }
        }
        _ => {
            // Convert other types to string representation
            let val_str = value.as_str().map_or_else(
                || {
                    value.as_i64().map_or_else(
                        || {
                            value.as_f64().map_or_else(
                                || {
                                    value
                                        .as_bool()
                                        .map_or_else(|| String::new(), |b| b.to_string())
                                },
                                |n| n.to_string(),
                            )
                        },
                        |n| n.to_string(),
                    )
                },
                std::string::ToString::to_string,
            );
            translations.insert(prefix.to_string(), val_str);
        }
    }
}

/// Locale loader that caches loaded translations.
pub struct LocaleLoader {
    locales_dir: PathBuf,
    cache: HashMap<String, TranslationMap>,
}

impl LocaleLoader {
    /// What: Create a new `LocaleLoader`.
    ///
    /// Inputs:
    /// - `locales_dir`: Path to locales directory
    ///
    /// Output:
    /// - `LocaleLoader` instance
    #[must_use]
    pub fn new(locales_dir: PathBuf) -> Self {
        Self {
            locales_dir,
            cache: HashMap::new(),
        }
    }

    /// What: Load locale file, using cache if available.
    ///
    /// Inputs:
    /// - `locale`: Locale code to load
    ///
    /// Output:
    /// - `Result<TranslationMap, String>` containing translations
    ///
    /// # Errors
    /// - Returns `Err` when the locale file cannot be loaded (see `load_locale_file` for specific error conditions)
    ///
    /// Details:
    /// - Caches loaded translations to avoid re-reading files
    /// - Returns cached version if available
    /// - Logs warnings for missing or invalid locale files
    #[must_use]
    pub fn load(&mut self, locale: &str) -> Result<TranslationMap, String> {
        if self.cache.contains_key(locale) {
            Ok(self
                .cache
                .get(locale)
                .expect("locale should be in cache after contains_key check")
                .clone())
        } else {
            match load_locale_file(locale, &self.locales_dir) {
                Ok(translations) => {
                    let key_count = translations.len();
                    tracing::debug!(
                        "Loaded locale '{}' with {} translation keys",
                        locale,
                        key_count
                    );
                    self.cache.insert(locale.to_string(), translations.clone());
                    Ok(translations)
                }
                Err(e) => {
                    tracing::warn!("Failed to load locale '{}': {}", locale, e);
                    Err(e)
                }
            }
        }
    }

    /// What: Get locales directory path.
    #[must_use]
    pub fn locales_dir(&self) -> &Path {
        &self.locales_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_locale_yaml() {
        let yaml = r#"
de-DE:
  app:
    titles:
      search: "Suche"
      help: "Hilfe"
"#;
        let result = parse_locale_yaml(yaml).expect("Failed to parse test locale YAML");
        assert_eq!(result.get("app.titles.search"), Some(&"Suche".to_string()));
        assert_eq!(result.get("app.titles.help"), Some(&"Hilfe".to_string()));
    }

    #[test]
    fn test_parse_locale_yaml_nested() {
        let yaml = r#"
en-US:
  app:
    modals:
      preflight:
        title_install: " Preflight: Install "
        tabs:
          summary: "Summary"
          deps: "Deps"
"#;
        let result = parse_locale_yaml(yaml).expect("Failed to parse test locale YAML");
        assert_eq!(
            result.get("app.modals.preflight.title_install"),
            Some(&" Preflight: Install ".to_string())
        );
        assert_eq!(
            result.get("app.modals.preflight.tabs.summary"),
            Some(&"Summary".to_string())
        );
        assert_eq!(
            result.get("app.modals.preflight.tabs.deps"),
            Some(&"Deps".to_string())
        );
    }

    #[test]
    fn test_parse_locale_yaml_invalid() {
        let yaml = "invalid: yaml: content: [";
        assert!(parse_locale_yaml(yaml).is_err());
    }

    #[test]
    fn test_load_locale_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let locales_dir = temp_dir.path();

        // Create a test locale file
        let locale_file = locales_dir.join("test-LOCALE.yml");
        let yaml_content = r#"
test-LOCALE:
  app:
    titles:
      search: "Test Search"
"#;
        fs::write(&locale_file, yaml_content).expect("Failed to write test locale file");

        let result =
            load_locale_file("test-LOCALE", locales_dir).expect("Failed to load test locale file");
        assert_eq!(
            result.get("app.titles.search"),
            Some(&"Test Search".to_string())
        );
    }

    #[test]
    fn test_load_locale_file_not_found() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let locales_dir = temp_dir.path();

        let result = load_locale_file("nonexistent", locales_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_load_locale_file_invalid_format() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let locales_dir = temp_dir.path();

        // Test with invalid locale format
        let result = load_locale_file("invalid-format-", locales_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid locale code format"));
    }

    #[test]
    fn test_load_locale_file_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let locales_dir = temp_dir.path();

        // Create an empty locale file
        let locale_file = locales_dir.join("empty.yml");
        fs::write(&locale_file, "").expect("Failed to write empty test locale file");

        let result = load_locale_file("empty", locales_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_locale_loader_caching() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test");
        let locales_dir = temp_dir.path();

        // Create a test locale file
        let locale_file = locales_dir.join("cache-test.yml");
        let yaml_content = r#"
cache-test:
  app:
    titles:
      search: "Cached"
"#;
        fs::write(&locale_file, yaml_content).expect("Failed to write test locale file");

        let mut loader = LocaleLoader::new(locales_dir.to_path_buf());

        // First load
        let result1 = loader
            .load("cache-test")
            .expect("Failed to load locale in test");
        assert_eq!(
            result1.get("app.titles.search"),
            Some(&"Cached".to_string())
        );

        // Second load should use cache
        let result2 = loader
            .load("cache-test")
            .expect("Failed to load cached locale in test");
        assert_eq!(
            result2.get("app.titles.search"),
            Some(&"Cached".to_string())
        );

        // Both should be the same reference (cached)
        assert_eq!(result1.len(), result2.len());
    }

    #[test]
    fn test_is_valid_locale_format() {
        // Valid formats
        assert!(is_valid_locale_format("en-US"));
        assert!(is_valid_locale_format("de-DE"));
        assert!(is_valid_locale_format("zh-Hans-CN"));
        assert!(is_valid_locale_format("en"));

        // Invalid formats
        assert!(!is_valid_locale_format(""));
        assert!(!is_valid_locale_format("-en-US"));
        assert!(!is_valid_locale_format("en-US-"));
        assert!(!is_valid_locale_format("en--US"));
        assert!(!is_valid_locale_format("en US"));
    }
}
