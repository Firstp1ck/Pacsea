/// What: Parse packages from pacman -Qu output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `old_version`, `new_version`) tuples
///
/// Details:
/// - Parses `"package-name old_version -> new_version"` format
pub fn parse_checkupdates(output: &[u8]) -> Vec<(String, String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package-name old_version -> new_version" format
                trimmed.find(" -> ").and_then(|arrow_pos| {
                    let before_arrow = &trimmed[..arrow_pos];
                    let after_arrow = &trimmed[arrow_pos + 4..];
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let old_version = parts[1..].join(" "); // In case version has spaces
                        let new_version = after_arrow.trim().to_string();
                        Some((name, old_version, new_version))
                    } else {
                        None
                    }
                })
            }
        })
        .collect()
}

/// What: Parse packages from checkupdates output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `new_version`) tuples
///
/// Details:
/// - Parses "package-name version" format (checkupdates only shows new version)
/// - Old version must be retrieved separately from installed packages
pub fn parse_checkupdates_tool(output: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package-name version" format
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0].to_string();
                    let new_version = parts[1..].join(" "); // In case version has spaces
                    Some((name, new_version))
                } else {
                    None
                }
            }
        })
        .collect()
}

/// What: Get installed version of a package.
///
/// Inputs:
/// - `package_name`: Name of the package
///
/// Output:
/// - `Some(version)` if package is installed, `None` otherwise
///
/// Details:
/// - Uses `pacman -Q` to get the installed version
pub fn get_installed_version(package_name: &str) -> Option<String> {
    use std::process::{Command, Stdio};

    let output = Command::new("pacman")
        .args(["-Q", package_name])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        // Format: "package-name version"
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 2 {
            Some(parts[1..].join(" "))
        } else {
            None
        }
    } else {
        None
    }
}

/// What: Parse packages from -Qua output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `old_version`, `new_version`) tuples
///
/// Details:
/// - Parses "package old -> new" format
pub fn parse_qua(output: &[u8]) -> Vec<(String, String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package old -> new" format
                trimmed.find(" -> ").and_then(|arrow_pos| {
                    let before_arrow = &trimmed[..arrow_pos];
                    let after_arrow = &trimmed[arrow_pos + 4..];
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let old_version = parts[1..].join(" "); // In case version has spaces
                        let new_version = after_arrow.trim().to_string();
                        Some((name, old_version, new_version))
                    } else {
                        None
                    }
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_checkupdates;

    /// What: Test that pacman -Qu parsing correctly extracts old and new versions.
    ///
    /// Inputs:
    /// - Sample pacman -Qu output with format `"package-name old_version -> new_version"`
    ///
    /// Output:
    /// - Verifies that `old_version` and `new_version` are correctly parsed and different
    ///
    /// Details:
    /// - Tests parsing of pacman -Qu output format
    #[test]
    fn test_parse_checkupdates_extracts_correct_versions() {
        let test_cases = vec![
            ("bat 0.26.0-1 -> 0.26.0-2", "bat", "0.26.0-1", "0.26.0-2"),
            (
                "comgr 2:6.4.4-2 -> 2:7.1.0-1",
                "comgr",
                "2:6.4.4-2",
                "2:7.1.0-1",
            ),
            (
                "composable-kernel 6.4.4-1 -> 7.1.0-1",
                "composable-kernel",
                "6.4.4-1",
                "7.1.0-1",
            ),
        ];

        for (input, expected_name, expected_old, expected_new) in test_cases {
            let output = input.as_bytes();
            let entries = parse_checkupdates(output);

            assert_eq!(entries.len(), 1, "Failed to parse: {input}");
            let (name, old_version, new_version) = &entries[0];
            assert_eq!(name, expected_name, "Wrong name for: {input}");
            assert_eq!(old_version, expected_old, "Wrong old_version for: {input}");
            assert_eq!(new_version, expected_new, "Wrong new_version for: {input}");
        }
    }

    /// What: Test that pacman -Qu parsing handles multiple packages.
    ///
    /// Inputs:
    /// - Multi-line pacman -Qu output
    ///
    /// Output:
    /// - Verifies that all packages are parsed correctly
    #[test]
    fn test_parse_checkupdates_multiple_packages() {
        let input = "bat 0.26.0-1 -> 0.26.0-2\ncomgr 2:6.4.4-2 -> 2:7.1.0-1\n";
        let output = input.as_bytes();
        let entries = parse_checkupdates(output);

        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            (
                "bat".to_string(),
                "0.26.0-1".to_string(),
                "0.26.0-2".to_string()
            )
        );
        assert_eq!(
            entries[1],
            (
                "comgr".to_string(),
                "2:6.4.4-2".to_string(),
                "2:7.1.0-1".to_string()
            )
        );
    }
}
