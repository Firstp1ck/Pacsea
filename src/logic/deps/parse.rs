//! Parsing utilities for dependency specifications.

/// Parse "Depends On" field from pacman -Si output.
///
/// The "Depends On" field contains space-separated dependency specifications.
/// Example: "curl  expat  perl  perl-error  perl-mailtools  openssl  pcre2  grep  shadow  zlib-ng"
/// Filters out virtual packages (.so files) like "libedit.so=0-64"
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
            return deps_str
                .split_whitespace()
                .map(|s| s.trim().to_string())
                .filter(|s| {
                    if s.is_empty() {
                        return false;
                    }
                    // Filter out .so files (virtual packages)
                    // Patterns: "libedit.so=0-64", "libgit2.so", "libfoo.so.1"
                    // Check if it ends with .so or contains .so. or .so=
                    !(s.ends_with(".so") || s.contains(".so.") || s.contains(".so="))
                })
                .collect();
        }
    }
    Vec::new()
}

/// Parse dependency output from pacman/paru -Sp.
///
/// The output format can be:
///   - Package names: "core/glibc", "extra/python>=3.12"
///   - Full URLs: "/mirror/archlinux/extra/os/x86_64/package-1.0-1-x86_64.pkg.tar.zst"
///   - Library files: "libgit2.so" (virtual packages/provides)
///
/// Returns cleaned package names, filtering out invalid entries.
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

/// Parse a dependency specification into (name, version_constraint).
///
/// Examples:
///   "glibc" -> ("glibc", "")
///   "python>=3.12" -> ("python", ">=3.12")
///   "firefox=121.0" -> ("firefox", "=121.0")
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
    fn parse_dep_spec_basic() {
        let (name, version) = parse_dep_spec("glibc");
        assert_eq!(name, "glibc");
        assert_eq!(version, "");
    }

    #[test]
    fn parse_dep_spec_with_version() {
        let (name, version) = parse_dep_spec("python>=3.12");
        assert_eq!(name, "python");
        assert_eq!(version, ">=3.12");
    }

    #[test]
    fn parse_dep_spec_equals() {
        let (name, version) = parse_dep_spec("firefox=121.0");
        assert_eq!(name, "firefox");
        assert_eq!(version, "=121.0");
    }
}
