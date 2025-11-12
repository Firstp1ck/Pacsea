//! Parsing utilities for dependency specifications.

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
pub(crate) fn parse_pacman_si_deps(text: &str) -> Vec<String> {
    for line in text.lines() {
        if line.starts_with("Depends On")
            && let Some(colon_pos) = line.find(':')
        {
            let deps_str = line[colon_pos + 1..].trim();
            if deps_str.is_empty() || deps_str == "None" {
                return Vec::new();
            }
            // Split by whitespace, filter out empty strings and .so files (virtual packages)
            // Also filter out tokens that don't look like package names
            return deps_str
                .split_whitespace()
                .map(|s| s.trim().to_string())
                .filter(|s| {
                    if s.is_empty() {
                        return false;
                    }
                    // Filter out .so files (virtual packages)
                    // Patterns: "libedit.so=0-64", "libgit2.so", "libfoo.so.1"
                    if s.ends_with(".so") || s.contains(".so.") || s.contains(".so=") {
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
                    if !s.chars().any(|c| c.is_alphanumeric()) {
                        return false;
                    }
                    true
                })
                .collect();
        }
    }
    Vec::new()
}

/// What: Extract optional dependency specifications from the `pacman -Si` "Optional Deps" field.
///
/// Inputs:
/// - `text`: Raw stdout emitted by `pacman -Si` for a package.
///
/// Output:
/// - Returns package specification strings without virtual shared-library entries.
///
/// Details:
/// - Scans the "Optional Deps" line, split on whitespace, and removes `.so` patterns that represent virtual deps.
/// - Uses the same validation logic as parse_pacman_si_deps to filter out invalid tokens.
pub(crate) fn parse_pacman_si_optdeps(text: &str) -> Vec<String> {
    for line in text.lines() {
        if line.starts_with("Optional Deps")
            && let Some(colon_pos) = line.find(':')
        {
            let deps_str = line[colon_pos + 1..].trim();
            if deps_str.is_empty() || deps_str == "None" {
                return Vec::new();
            }
            // Split by whitespace, filter out empty strings and .so files (virtual packages)
            // Also filter out tokens that don't look like package names
            return deps_str
                .split_whitespace()
                .map(|s| s.trim().to_string())
                .filter(|s| {
                    if s.is_empty() {
                        return false;
                    }
                    // Filter out .so files (virtual packages)
                    if s.ends_with(".so") || s.contains(".so.") || s.contains(".so=") {
                        return false;
                    }
                    // Filter out common words that might appear in descriptions
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
                    // Filter out tokens ending with colons (likely from error messages or malformed output)
                    if s.ends_with(':') {
                        return false;
                    }
                    // Check if it contains at least one alphanumeric character
                    if !s.chars().any(|c| c.is_alphanumeric()) {
                        return false;
                    }
                    true
                })
                .collect();
        }
    }
    Vec::new()
}

/// What: Normalize dependency names from `pacman -Sp` or helper outputs.
///
/// Inputs:
/// - `text`: Multi-line command output containing potential dependency entries.
///
/// Output:
/// - Returns a vector of cleaned package names with virtual or malformed entries removed.
///
/// Details:
/// - Handles repository prefixes, download URLs, and shared-library provides while extracting canonical names.
#[allow(dead_code)]
pub(crate) fn parse_dependency_output(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            // Handle full URLs/paths (e.g., "/mirror/archlinux/extra/os/x86_64/package-1.0-1-x86_64.pkg.tar.zst")
            if line.contains(".pkg.tar.zst") {
                // Extract package name from path
                // Format: .../package-name-version-revision-arch.pkg.tar.zst
                if let Some(pkg_start) = line.rfind('/') {
                    let filename = &line[pkg_start + 1..];
                    if let Some(pkg_end) = filename.find(".pkg.tar.zst") {
                        let pkg_with_ver = &filename[..pkg_end];
                        // Extract package name (everything before the last version-like segment)
                        // e.g., "jujutsu-0.35.0-1-x86_64" -> "jujutsu"
                        if let Some(name_end) = pkg_with_ver.rfind('-') {
                            // Try to find where version starts (look for pattern like "-1-x86_64" or version numbers)
                            let potential_name = &pkg_with_ver[..name_end];
                            // Check if there's another dash (version-revision-arch pattern)
                            if let Some(ver_start) = potential_name.rfind('-') {
                                // Might be "package-version-revision-arch", extract just package
                                return Some(potential_name[..ver_start].to_string());
                            }
                            return Some(potential_name.to_string());
                        }
                        return Some(pkg_with_ver.to_string());
                    }
                }
                return None;
            }

            // Handle .so files (shared libraries) - these are virtual packages
            // Skip them as they're not actual package dependencies
            if line.ends_with(".so") || line.contains(".so.") {
                return None;
            }

            // Handle repo/package format (e.g., "core/glibc" -> "glibc")
            if let Some(slash_pos) = line.find('/') {
                let after_slash = &line[slash_pos + 1..];
                // Check if it's still a valid package name (not a path)
                if !after_slash.contains('/') && !after_slash.contains("http") {
                    return Some(after_slash.to_string());
                }
            }

            // Plain package name
            Some(line.to_string())
        })
        .collect()
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
pub(crate) fn parse_dep_spec(spec: &str) -> (String, String) {
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
}
