//! Parser for AUR .SRCINFO files.

use crate::util::{curl_args, percent_encode};
use std::process::Command;

/// What: Fetch .SRCINFO content for an AUR package.
///
/// Inputs:
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
///
/// Details:
/// - Downloads .SRCINFO from AUR cgit repository.
pub(crate) fn fetch_srcinfo(name: &str) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    // Add timeout to prevent hanging (10 seconds)
    let args = curl_args(&url, &["--max-time", "10"]);
    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "curl failed with status: {:?}",
            output.status.code()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    Ok(text)
}

/// What: Parse dependencies from .SRCINFO content.
///
/// Inputs:
/// - `srcinfo`: Raw .SRCINFO file content.
///
/// Output:
/// - Returns a tuple of (depends, makedepends, checkdepends, optdepends) vectors.
///
/// Details:
/// - Parses key-value pairs from .SRCINFO format.
/// - Handles array fields that can appear multiple times.
/// - Filters out virtual packages (.so files).
pub(crate) fn parse_srcinfo_deps(
    srcinfo: &str,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut depends = Vec::new();
    let mut makedepends = Vec::new();
    let mut checkdepends = Vec::new();
    let mut optdepends = Vec::new();

    for line in srcinfo.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // .SRCINFO format: key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Filter out virtual packages (.so files)
            if value.ends_with(".so") || value.contains(".so.") || value.contains(".so=") {
                continue;
            }

            match key {
                "depends" => depends.push(value.to_string()),
                "makedepends" => makedepends.push(value.to_string()),
                "checkdepends" => checkdepends.push(value.to_string()),
                "optdepends" => optdepends.push(value.to_string()),
                _ => {}
            }
        }
    }

    (depends, makedepends, checkdepends, optdepends)
}

/// What: Parse conflicts from .SRCINFO content.
///
/// Inputs:
/// - `srcinfo`: Raw .SRCINFO file content.
///
/// Output:
/// - Returns a vector of conflicting package names.
///
/// Details:
/// - Parses "conflicts" key-value pairs from .SRCINFO format.
/// - Handles array fields that can appear multiple times.
/// - Filters out virtual packages (.so files) and extracts package names from version constraints.
pub(crate) fn parse_srcinfo_conflicts(srcinfo: &str) -> Vec<String> {
    use super::parse::parse_dep_spec;

    let mut conflicts = Vec::new();

    for line in srcinfo.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // .SRCINFO format: key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == "conflicts" {
                // Filter out virtual packages (.so files)
                if value.ends_with(".so") || value.contains(".so.") || value.contains(".so=") {
                    continue;
                }
                // Extract package name (remove version constraints if present)
                let (pkg_name, _) = parse_dep_spec(value);
                if !pkg_name.is_empty() {
                    conflicts.push(pkg_name);
                }
            }
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_srcinfo_deps() {
        let srcinfo = r#"
pkgbase = test-package
pkgname = test-package
pkgver = 1.0.0
pkgrel = 1
depends = foo
depends = bar>=1.2.3
makedepends = make
makedepends = gcc
checkdepends = check
optdepends = optional: optional-package
depends = libfoo.so=1-64
"#;

        let (depends, makedepends, checkdepends, optdepends) = parse_srcinfo_deps(srcinfo);

        // Should have 2 depends (foo and bar>=1.2.3), libfoo.so should be filtered
        assert_eq!(depends.len(), 2);
        assert!(depends.contains(&"foo".to_string()));
        assert!(depends.contains(&"bar>=1.2.3".to_string()));

        // Should have 2 makedepends
        assert_eq!(makedepends.len(), 2);
        assert!(makedepends.contains(&"make".to_string()));
        assert!(makedepends.contains(&"gcc".to_string()));

        // Should have 1 checkdepends
        assert_eq!(checkdepends.len(), 1);
        assert!(checkdepends.contains(&"check".to_string()));

        // Should have 1 optdepends (with "optional:" prefix)
        assert_eq!(optdepends.len(), 1);
        assert!(optdepends.contains(&"optional: optional-package".to_string()));
    }

    #[test]
    /// What: Confirm conflicts parsing extracts package names from .SRCINFO.
    ///
    /// Inputs:
    /// - Sample .SRCINFO content with conflicts field.
    ///
    /// Output:
    /// - Returns vector of conflicting package names.
    ///
    /// Details:
    /// - Validates parsing logic handles multiple conflict entries.
    fn test_parse_srcinfo_conflicts() {
        let srcinfo = r#"
pkgbase = test-package
pkgname = test-package
pkgver = 1.0.0
pkgrel = 1
conflicts = conflicting-pkg1
conflicts = conflicting-pkg2>=2.0
conflicts = libfoo.so=1-64
"#;

        let conflicts = parse_srcinfo_conflicts(srcinfo);

        // Should have 2 conflicts (conflicting-pkg1 and conflicting-pkg2), libfoo.so should be filtered
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts.contains(&"conflicting-pkg1".to_string()));
        assert!(conflicts.contains(&"conflicting-pkg2".to_string()));
    }

    #[test]
    /// What: Ensure conflicts parsing handles empty .SRCINFO correctly.
    ///
    /// Inputs:
    /// - .SRCINFO content without conflicts field.
    ///
    /// Output:
    /// - Returns empty vector.
    ///
    /// Details:
    /// - Confirms graceful handling of missing conflicts.
    fn test_parse_srcinfo_conflicts_empty() {
        let srcinfo = r#"
pkgbase = test-package
pkgname = test-package
pkgver = 1.0.0
"#;

        let conflicts = parse_srcinfo_conflicts(srcinfo);
        assert!(conflicts.is_empty());
    }
}
