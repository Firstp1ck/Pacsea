//! Parsing utilities for dependency specifications.

use std::collections::HashSet;
use std::sync::OnceLock;

/// What: Get all possible localized labels for "Depends On" field from pacman/yay/paru output.
///
/// Output:
/// - `HashSet` of all possible labels across all locales
///
/// Details:
/// - Loads labels from locale files at runtime
/// - Falls back to hardcoded list if locale files can't be loaded
/// - Cached on first access for performance
fn get_depends_labels() -> &'static HashSet<String> {
    static LABELS: OnceLock<HashSet<String>> = OnceLock::new();
    LABELS.get_or_init(|| {
        let mut labels = HashSet::new();

        // Try to load from all locale files
        let locales_dir = crate::i18n::find_locales_dir().unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("config")
                .join("locales")
        });
        if let Ok(entries) = std::fs::read_dir(&locales_dir) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str()
                    && file_name.to_lowercase().ends_with(".yml")
                {
                    let locale = file_name.strip_suffix(".yml").unwrap_or(file_name);
                    if let Ok(translations) = crate::i18n::load_locale_file(locale, &locales_dir) {
                        // Extract labels from app.parsing.pacman_depends_labels (can be array or single string)
                        if let Some(labels_str) =
                            translations.get("app.parsing.pacman_depends_labels")
                            && let Ok(yaml_value) =
                                serde_norway::from_str::<serde_norway::Value>(labels_str)
                            && let Some(seq) = yaml_value.as_sequence()
                        {
                            // Parse YAML array
                            for item in seq {
                                if let Some(label) = item.as_str() {
                                    labels.insert(label.to_string());
                                }
                            }
                        } else if let Some(label) =
                            translations.get("app.parsing.pacman_depends_label")
                        {
                            // Fallback to single label format
                            labels.insert(label.clone());
                        }
                    }
                }
            }
        }

        // Fallback: add common labels if loading failed or didn't find all
        labels.insert("Depends On".to_string());
        labels.insert("Ist abhängig von".to_string());
        labels.insert("Dépend de".to_string());
        labels.insert("Depende de".to_string());
        labels.insert("Dipende da".to_string());
        labels.insert("Zależy od".to_string());
        labels.insert("Зависит от".to_string());
        labels.insert("依存".to_string());

        labels
    })
}

/// What: Get all possible localized "None" equivalents.
///
/// Output:
/// - `HashSet` of all possible "None" labels across all locales
fn get_none_labels() -> &'static HashSet<String> {
    static LABELS: OnceLock<HashSet<String>> = OnceLock::new();
    LABELS.get_or_init(|| {
        let mut labels = HashSet::new();

        // Try to load from all locale files
        let locales_dir = crate::i18n::find_locales_dir().unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("config")
                .join("locales")
        });
        if let Ok(entries) = std::fs::read_dir(&locales_dir) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str()
                    && file_name.to_lowercase().ends_with(".yml")
                {
                    let locale = file_name.strip_suffix(".yml").unwrap_or(file_name);
                    if let Ok(translations) = crate::i18n::load_locale_file(locale, &locales_dir) {
                        // Extract labels from app.parsing.pacman_none_labels (can be array or single string)
                        if let Some(labels_str) = translations.get("app.parsing.pacman_none_labels")
                            && let Ok(yaml_value) =
                                serde_norway::from_str::<serde_norway::Value>(labels_str)
                            && let Some(seq) = yaml_value.as_sequence()
                        {
                            // Parse YAML array
                            for item in seq {
                                if let Some(label) = item.as_str() {
                                    labels.insert(label.to_string());
                                }
                            }
                        } else if let Some(label) =
                            translations.get("app.parsing.pacman_none_label")
                        {
                            // Fallback to single label format
                            labels.insert(label.clone());
                        }
                    }
                }
            }
        }

        // Fallback: add common labels
        labels.insert("None".to_string());
        labels.insert("Keine".to_string());
        labels.insert("Aucune".to_string());
        labels.insert("Ninguna".to_string());
        labels.insert("Nessuna".to_string());

        labels
    })
}

/// What: Extract dependency specifications from the `pacman -Si` "Depends On" field.
///
/// Inputs:
/// - `text`: Raw stdout emitted by `pacman -Si` for a package.
///
/// Output:
/// - Returns package specification strings without virtual shared-library entries.
///
/// Details:
/// - Scans the "Depends On" line, split on whitespace, and removes `.so` patterns that represent virtual deps.
/// - Validates that tokens look like valid package names (alphanumeric, dashes, underscores, version operators).
/// - Filters out common words and description text that might be parsed incorrectly.
pub(super) fn parse_pacman_si_deps(text: &str) -> Vec<String> {
    let depends_labels = get_depends_labels();
    let none_labels = get_none_labels();

    for line in text.lines() {
        // Check if line starts with any known "Depends On" label
        let is_depends_line = depends_labels.iter().any(|label| line.starts_with(label))
            || (line.contains("Depends") && line.contains("On"));

        if is_depends_line && let Some(colon_pos) = line.find(':') {
            let deps_str = line[colon_pos + 1..].trim();
            // Check if deps_str matches any "None" equivalent
            if deps_str.is_empty()
                || none_labels
                    .iter()
                    .any(|label| deps_str.eq_ignore_ascii_case(label))
            {
                return Vec::new();
            }
            // Split by whitespace, filter out empty strings and .so files (virtual packages)
            // Also filter out tokens that don't look like package names
            #[allow(clippy::case_sensitive_file_extension_comparisons)]
            return deps_str
                .split_whitespace()
                .map(|s| s.trim().to_string())
                .filter(|s| {
                    if s.is_empty() {
                        return false;
                    }
                    // Filter out .so files (virtual packages)
                    // Patterns: "libedit.so=0-64", "libgit2.so", "libfoo.so.1"
                    let s_lower = s.to_lowercase();
                    if s_lower.ends_with(".so")
                        || s_lower.contains(".so.")
                        || s_lower.contains(".so=")
                    {
                        return false;
                    }
                    // Filter out common words that might appear in descriptions
                    // These are not valid package names
                    let common_words = [
                        "for", "to", "with", "is", "that", "using", "usually", "bundled",
                        "bindings", "tooling", "the", "and", "or", "in", "on", "at", "by", "from",
                        "as", "if", "when", "where", "which", "what", "how", "why",
                    ];
                    let lower = s.to_lowercase();
                    if common_words.contains(&lower.as_str()) {
                        return false;
                    }
                    // Filter out tokens that are too short (likely not package names)
                    // Package names are typically at least 2 characters
                    if s.len() < 2 {
                        return false;
                    }
                    // Filter out tokens that look like description text
                    // Valid package names contain alphanumeric, dashes, underscores, and version operators
                    // But shouldn't be just punctuation or start/end with certain characters
                    let first_char = s.chars().next().unwrap_or(' ');
                    if !first_char.is_alphanumeric() && first_char != '-' && first_char != '_' {
                        return false;
                    }
                    // Filter out tokens ending with colons (likely from error messages or malformed output)
                    if s.ends_with(':') {
                        return false;
                    }
                    // Check if it contains at least one alphanumeric character
                    if !s.chars().any(char::is_alphanumeric) {
                        return false;
                    }
                    true
                })
                .collect();
        }
    }
    Vec::new()
}


/// What: Extract conflict specifications from the `pacman -Si` "Conflicts With" field.
///
/// Inputs:
/// - `text`: Raw stdout emitted by `pacman -Si` for a package.
///
/// Output:
/// - Returns package names that conflict with this package.
///
/// Details:
/// - Scans the "Conflicts With" line, splits on whitespace, and filters out invalid entries.
/// - Similar to `parse_pacman_si_deps` but for conflicts field.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub(super) fn parse_pacman_si_conflicts(text: &str) -> Vec<String> {
    let none_labels = get_none_labels();

    for line in text.lines() {
        // Check if line starts with "Conflicts With" (or localized variants)
        let is_conflicts_line = line.starts_with("Conflicts With")
            || line.starts_with("Konflikt mit")
            || (line.contains("Conflicts") && line.contains("With"));

        if is_conflicts_line && let Some(colon_pos) = line.find(':') {
            let conflicts_str = line[colon_pos + 1..].trim();
            // Check if conflicts_str matches any "None" equivalent
            if conflicts_str.is_empty()
                || none_labels
                    .iter()
                    .any(|label| conflicts_str.eq_ignore_ascii_case(label))
            {
                return Vec::new();
            }
            // Split by whitespace and parse package names (may include version constraints)
            return conflicts_str
                .split_whitespace()
                .map(|s| s.trim().to_string())
                .filter(|s| {
                    if s.is_empty() {
                        return false;
                    }
                    // Filter out .so files (virtual packages)
                    let s_lower = s.to_lowercase();
                    if s_lower.ends_with(".so")
                        || s_lower.contains(".so.")
                        || s_lower.contains(".so=")
                    {
                        return false;
                    }
                    // Filter out common words
                    let common_words = [
                        "for", "to", "with", "is", "that", "using", "usually", "bundled",
                        "bindings", "tooling", "the", "and", "or", "in", "on", "at", "by", "from",
                        "as", "if", "when", "where", "which", "what", "how", "why",
                    ];
                    let lower = s.to_lowercase();
                    if common_words.contains(&lower.as_str()) {
                        return false;
                    }
                    // Filter out tokens that are too short
                    if s.len() < 2 {
                        return false;
                    }
                    // Filter out tokens that don't look like package names
                    let first_char = s.chars().next().unwrap_or(' ');
                    if !first_char.is_alphanumeric() && first_char != '-' && first_char != '_' {
                        return false;
                    }
                    if s.ends_with(':') {
                        return false;
                    }
                    if !s.chars().any(char::is_alphanumeric) {
                        return false;
                    }
                    true
                })
                .map(|s| {
                    // Extract package name (remove version constraints if present)
                    parse_dep_spec(&s).0
                })
                .collect();
        }
    }
    Vec::new()
}

/// What: Split a dependency specification into name and version requirement components.
///
/// Inputs:
/// - `spec`: Dependency string from pacman helpers (e.g., `python>=3.12`).
///
/// Output:
/// - Returns a tuple `(name, version_constraint)` with an empty constraint when none is present.
///
/// Details:
/// - Searches for comparison operators in precedence order to avoid mis-parsing combined expressions.
pub(super) fn parse_dep_spec(spec: &str) -> (String, String) {
    for op in ["<=", ">=", "=", "<", ">"] {
        if let Some(pos) = spec.find(op) {
            let name = spec[..pos].trim().to_string();
            let version = spec[pos..].trim().to_string();
            return (name, version);
        }
    }
    (spec.trim().to_string(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Confirm dependency specs without operators return empty version constraints.
    ///
    /// Inputs:
    /// - Spec string `"glibc"` with no comparison operator.
    ///
    /// Output:
    /// - Tuple of name `"glibc"` and empty version string.
    ///
    /// Details:
    /// - Guards the default branch where no recognised operator exists.
    fn parse_dep_spec_basic() {
        let (name, version) = parse_dep_spec("glibc");
        assert_eq!(name, "glibc");
        assert_eq!(version, "");
    }

    #[test]
    /// What: Ensure specs containing `>=` split into name and constraint correctly.
    ///
    /// Inputs:
    /// - Spec string `"python>=3.12"`.
    ///
    /// Output:
    /// - Returns name `"python"` and version `">=3.12"`.
    ///
    /// Details:
    /// - Exercises multi-character operator detection order.
    fn parse_dep_spec_with_version() {
        let (name, version) = parse_dep_spec("python>=3.12");
        assert_eq!(name, "python");
        assert_eq!(version, ">=3.12");
    }

    #[test]
    /// What: Verify equality constraints are detected and returned verbatim.
    ///
    /// Inputs:
    /// - Spec string `"firefox=121.0"`.
    ///
    /// Output:
    /// - Produces name `"firefox"` and version `"=121.0"`.
    ///
    /// Details:
    /// - Confirms the operator precedence loop catches single-character `=` after multi-character checks.
    fn parse_dep_spec_equals() {
        let (name, version) = parse_dep_spec("firefox=121.0");
        assert_eq!(name, "firefox");
        assert_eq!(version, "=121.0");
    }

    #[test]
    /// What: Confirm conflicts parsing extracts package names from pacman output.
    ///
    /// Inputs:
    /// - Sample pacman -Si output with "Conflicts With" field.
    ///
    /// Output:
    /// - Returns vector of conflicting package names.
    ///
    /// Details:
    /// - Validates parsing logic handles whitespace-separated conflict lists.
    fn parse_pacman_si_conflicts_basic() {
        let text =
            "Name            : test-package\nConflicts With : conflicting-pkg1 conflicting-pkg2\n";
        let conflicts = parse_pacman_si_conflicts(text);
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts.contains(&"conflicting-pkg1".to_string()));
        assert!(conflicts.contains(&"conflicting-pkg2".to_string()));
    }

    #[test]
    /// What: Ensure conflicts parsing handles version constraints correctly.
    ///
    /// Inputs:
    /// - Pacman output with conflicts containing version constraints.
    ///
    /// Output:
    /// - Returns package names without version constraints.
    ///
    /// Details:
    /// - Confirms version operators are stripped from conflict names.
    fn parse_pacman_si_conflicts_with_versions() {
        let text = "Name            : test-package\nConflicts With : old-pkg<2.0 new-pkg>=3.0\n";
        let conflicts = parse_pacman_si_conflicts(text);
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts.contains(&"old-pkg".to_string()));
        assert!(conflicts.contains(&"new-pkg".to_string()));
    }

    #[test]
    /// What: Validate conflicts parsing handles "None" correctly.
    ///
    /// Inputs:
    /// - Pacman output with "Conflicts With: None".
    ///
    /// Output:
    /// - Returns empty vector.
    ///
    /// Details:
    /// - Ensures "None" label is recognized and filtered out.
    fn parse_pacman_si_conflicts_none() {
        let text = "Name            : test-package\nConflicts With : None\n";
        let conflicts = parse_pacman_si_conflicts(text);
        assert!(conflicts.is_empty());
    }
}
