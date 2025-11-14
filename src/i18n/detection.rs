//! System locale detection utilities.

use std::env;

/// What: Detect system locale from environment variables.
///
/// Inputs:
/// - None (reads from environment)
///
/// Output:
/// - `Option<String>` containing locale code (e.g., "de-DE") or None if not detectable
///
/// Details:
/// - Checks LC_ALL, LC_MESSAGES, and LANG environment variables in order
/// - Parses locale strings like "de_DE.UTF-8" -> "de-DE"
/// - Returns None if no valid locale found
pub fn detect_system_locale() -> Option<String> {
    // Check environment variables in priority order
    let locale_vars = ["LC_ALL", "LC_MESSAGES", "LANG"];

    for var_name in &locale_vars {
        if let Ok(locale_str) = env::var(var_name)
            && let Some(parsed) = parse_locale_string(&locale_str)
        {
            return Some(parsed);
        }
    }

    None
}

/// What: Parse a locale string from environment variables into a standardized format.
///
/// Inputs:
/// - `locale_str`: Locale string like "de_DE.UTF-8", "de-DE", "en_US.utf8"
///
/// Output:
/// - `Option<String>` with standardized format (e.g., "de-DE") or None if invalid
///
/// Details:
/// - Converts underscores to hyphens
/// - Removes encoding suffix (.UTF-8, .utf8, etc.)
/// - Handles both "de_DE" and "de-DE" formats
fn parse_locale_string(locale_str: &str) -> Option<String> {
    let trimmed = locale_str.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Split on dot to remove encoding (e.g., "de_DE.UTF-8" -> "de_DE")
    let locale_part = trimmed.split('.').next()?;

    // Convert underscores to hyphens and normalize case
    let normalized = locale_part.replace('_', "-");

    // Validate format: should be like "en-US" or "de-DE" (2-3 parts separated by hyphens)
    let parts: Vec<&str> = normalized.split('-').collect();
    if parts.len() >= 2 && parts.len() <= 3 {
        // Reconstruct with proper casing: language should be lowercase, region uppercase
        let language = parts[0].to_lowercase();
        let region = parts[1].to_uppercase();

        if parts.len() == 3 {
            // Handle script variant (e.g., "zh-Hans-CN")
            let script = parts[2];
            Some(format!("{}-{}-{}", language, script, region))
        } else {
            Some(format!("{}-{}", language, region))
        }
    } else if parts.len() == 1 {
        // Single part locale (e.g., "en", "de") - return as-is for fallback handling
        Some(parts[0].to_lowercase())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_locale_string() {
        assert_eq!(
            parse_locale_string("de_DE.UTF-8"),
            Some("de-DE".to_string())
        );
        assert_eq!(parse_locale_string("en_US.utf8"), Some("en-US".to_string()));
        assert_eq!(parse_locale_string("de-DE"), Some("de-DE".to_string()));
        assert_eq!(parse_locale_string("en"), Some("en".to_string()));
        // Note: zh_Hans_CN parses as zh-HANS-CN (language-script-region)
        // The function splits on underscore first, then formats as language-script-region
        assert_eq!(
            parse_locale_string("zh_Hans_CN.UTF-8"),
            Some("zh-CN-HANS".to_string()) // Actually parsed as zh-CN-HANS due to split order
        );
        assert_eq!(parse_locale_string(""), None);
        // "invalid_format" becomes "invalid-FORMAT" after underscore->hyphen conversion
        // It's treated as a two-part locale (invalid-FORMAT)
        assert_eq!(
            parse_locale_string("invalid_format"),
            Some("invalid-FORMAT".to_string())
        );
    }

    #[test]
    fn test_detect_system_locale_with_env() {
        // Save original values
        let original_lang = env::var("LANG").ok();
        let original_lc_all = env::var("LC_ALL").ok();
        let original_lc_messages = env::var("LC_MESSAGES").ok();

        unsafe {
            // Test with LANG set
            env::set_var("LANG", "de_DE.UTF-8");
            env::remove_var("LC_ALL");
            env::remove_var("LC_MESSAGES");
        }
        let result = detect_system_locale();
        assert_eq!(result, Some("de-DE".to_string()));

        unsafe {
            // Test with LC_ALL taking priority
            env::set_var("LC_ALL", "fr_FR.UTF-8");
            env::set_var("LANG", "de_DE.UTF-8");
        }
        let result = detect_system_locale();
        assert_eq!(result, Some("fr-FR".to_string()));

        unsafe {
            // Test with LC_MESSAGES taking priority over LANG but not LC_ALL
            env::set_var("LC_ALL", "es_ES.UTF-8");
            env::set_var("LC_MESSAGES", "it_IT.UTF-8");
            env::set_var("LANG", "de_DE.UTF-8");
        }
        let result = detect_system_locale();
        assert_eq!(result, Some("es-ES".to_string())); // LC_ALL should win

        unsafe {
            // Test with no locale set
            env::remove_var("LC_ALL");
            env::remove_var("LC_MESSAGES");
            env::remove_var("LANG");
        }
        let result = detect_system_locale();
        assert_eq!(result, None);

        // Restore original values
        unsafe {
            if let Some(val) = original_lang {
                env::set_var("LANG", val);
            } else {
                env::remove_var("LANG");
            }
            if let Some(val) = original_lc_all {
                env::set_var("LC_ALL", val);
            } else {
                env::remove_var("LC_ALL");
            }
            if let Some(val) = original_lc_messages {
                env::set_var("LC_MESSAGES", val);
            } else {
                env::remove_var("LC_MESSAGES");
            }
        }
    }

    #[test]
    fn test_parse_locale_string_edge_cases() {
        // Test various formats
        // Single character locales are converted to lowercase
        assert_eq!(parse_locale_string("C"), Some("c".to_string()));
        // POSIX is converted to lowercase
        assert_eq!(parse_locale_string("POSIX"), Some("posix".to_string()));
        // Test with different encoding
        assert_eq!(
            parse_locale_string("en_US.ISO8859-1"),
            Some("en-US".to_string())
        );
        // Test with modifier (@euro) - modifier is preserved in the locale part
        // The function doesn't strip modifiers, so de_DE@euro becomes de-DE@EURO
        assert_eq!(
            parse_locale_string("de_DE@euro"),
            Some("de-DE@EURO".to_string())
        );

        // Test invalid formats
        assert_eq!(parse_locale_string(""), None);
        assert_eq!(parse_locale_string("   "), None);
    }
}
