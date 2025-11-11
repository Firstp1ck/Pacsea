//! Parser for AUR .SRCINFO files.

use crate::util::percent_encode;
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
    let output = Command::new("curl")
        .args(["-sSLf", "--max-time", "10", &url])
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
}
