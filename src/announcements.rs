//! Announcement system supporting both version-embedded and remote announcements.

use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use std::cmp::Ordering;

/// What: Version-embedded announcement for a specific app version.
///
/// Inputs: None (static data).
///
/// Output: Represents an announcement tied to a specific version.
///
/// Details:
/// - Shown when the base version (X.X.X) matches, regardless of suffix.
/// - Content is embedded in the binary at compile time.
/// - Version matching compares only the base version (X.X.X), ignoring suffixes.
/// - Announcements show again when the suffix changes (e.g., "0.6.0-pr#85" -> "0.6.0-pr#86").
/// - For example, announcement version "0.6.0-pr#85" will match Cargo.toml version "0.6.0".
pub struct VersionAnnouncement {
    /// Version string this announcement is for (e.g., "0.6.0" or "0.6.0-pr#85").
    /// Only the base version (X.X.X) is used for matching, but full version is used for tracking.
    pub version: &'static str,
    /// Title of the announcement.
    pub title: &'static str,
    /// Markdown content of the announcement.
    pub content: &'static str,
}

/// What: Embedded announcements for specific app versions.
///
/// Inputs: None (static data).
///
/// Output: Array of version announcements.
///
/// Details:
/// - Add new announcements here for each release.
/// - Version matching compares only the base version (X.X.X), so "0.6.0-pr#85" matches "0.6.0".
/// - Announcements show again when the suffix changes (e.g., "0.6.0-pr#85" -> "0.6.0-pr#86").
/// - Cargo.toml can stay at "0.6.0" while announcements use "0.6.0-pr#85" for clarity.
pub const VERSION_ANNOUNCEMENTS: &[VersionAnnouncement] = &[
    // Add version-specific announcements here
    VersionAnnouncement {
        version: "0.6.1",
        title: "Announcement Modal",
        content: "## What's New\n\n- Announcement modal system - view important updates and version notes\n- Fixed global keybinds interfering with modals - keyboard shortcuts now work correctly\n\n## Chore\n\n- Updated PKGBUILD SHASUM\n",
    },
];

/// What: Remote announcement fetched from GitHub Gist.
///
/// Inputs: None (deserialized from JSON).
///
/// Output: Represents a remote announcement with version filtering and expiration.
///
/// Details:
/// - Fetched from configured URL (GitHub Gist raw URL).
/// - Can target specific version ranges.
/// - Can expire after a certain date.
#[derive(Debug, Deserialize)]
pub struct RemoteAnnouncement {
    /// Unique identifier for this announcement (used for tracking read state).
    pub id: String,
    /// Title of the announcement.
    pub title: String,
    /// Markdown content of the announcement.
    pub content: String,
    /// Minimum version (inclusive) that should see this announcement.
    pub min_version: Option<String>,
    /// Maximum version (inclusive) that should see this announcement.
    /// If None, no upper limit.
    pub max_version: Option<String>,
    /// Expiration date in ISO format (YYYY-MM-DD). If None, never expires.
    pub expires: Option<String>,
}

/// What: Compare version strings numerically.
///
/// Inputs:
/// - `a`: Left-hand version string.
/// - `b`: Right-hand version string.
///
/// Output:
/// - `Ordering` indicating which version is greater.
///
/// Details:
/// - Uses the same logic as preflight version comparison.
/// - Splits on `.` and `-`, comparing numeric segments.
fn compare_versions(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<&str> = a.split(['.', '-']).collect();
    let b_parts: Vec<&str> = b.split(['.', '-']).collect();
    let len = a_parts.len().max(b_parts.len());

    for idx in 0..len {
        let a_seg = a_parts.get(idx).copied().unwrap_or("0");
        let b_seg = b_parts.get(idx).copied().unwrap_or("0");

        match (a_seg.parse::<i64>(), b_seg.parse::<i64>()) {
            (Ok(a_num), Ok(b_num)) => match a_num.cmp(&b_num) {
                Ordering::Equal => {}
                ord => return ord,
            },
            _ => match a_seg.cmp(b_seg) {
                Ordering::Equal => {}
                ord => return ord,
            },
        }
    }

    Ordering::Equal
}

/// What: Extract base version (X.X.X) from a version string, ignoring suffixes.
///
/// Inputs:
/// - `version`: Version string (e.g., "0.6.0", "0.6.0-pr#85", "0.6.0-beta").
///
/// Output:
/// - Base version string (e.g., "0.6.0").
///
/// Details:
/// - Extracts the semantic version part (major.minor.patch) before any suffix.
/// - Handles versions like "0.6.0", "0.6.0-pr#85", "0.6.0-beta", "1.2.3-rc1".
/// - Splits on '-' to remove pre-release identifiers and other suffixes.
/// - Normalizes to X.X.X format (adds .0 for missing segments).
#[must_use]
pub fn extract_base_version(version: &str) -> String {
    // Split on '-' to remove pre-release identifiers and suffixes
    // This handles formats like "0.6.0-pr#85", "0.6.0-beta", "1.2.3-rc1"
    let base = version.split('-').next().unwrap_or(version);

    // Extract only the X.X.X part (up to 3 numeric segments separated by dots)
    let parts: Vec<&str> = base.split('.').collect();
    match parts.len() {
        n if n >= 3 => {
            // Take first 3 parts and join them
            format!("{}.{}.{}", parts[0], parts[1], parts[2])
        }
        2 => {
            // Handle X.X format, add .0
            format!("{}.{}.0", parts[0], parts[1])
        }
        1 => {
            // Handle X format, add .0.0
            format!("{}.0.0", parts[0])
        }
        _ => base.to_string(),
    }
}

/// What: Check if current version matches the version range.
///
/// Inputs:
/// - `current_version`: Current app version (e.g., "0.6.0").
/// - `min_version`: Optional minimum version (inclusive).
/// - `max_version`: Optional maximum version (inclusive).
///
/// Output:
/// - `true` if current version is within the range, `false` otherwise.
///
/// Details:
/// - If `min_version` is None, no lower bound check.
/// - If `max_version` is None, no upper bound check.
/// - Both bounds are inclusive.
#[must_use]
pub fn version_matches(
    current_version: &str,
    min_version: Option<&str>,
    max_version: Option<&str>,
) -> bool {
    if let Some(min) = min_version
        && compare_versions(current_version, min) == Ordering::Less
    {
        return false;
    }
    if let Some(max) = max_version
        && compare_versions(current_version, max) == Ordering::Greater
    {
        return false;
    }
    true
}

/// What: Check if an announcement has expired.
///
/// Inputs:
/// - `expires`: Optional expiration date in ISO format (YYYY-MM-DD).
///
/// Output:
/// - `true` if expired (date has passed), `false` if not expired or no expiration.
///
/// Details:
/// - Parses ISO date format (YYYY-MM-DD).
/// - Compares with today's date (UTC).
#[must_use]
pub fn is_expired(expires: Option<&str>) -> bool {
    let Some(expires_str) = expires else {
        return false; // No expiration date means never expires
    };

    let Ok(expires_date) = NaiveDate::parse_from_str(expires_str, "%Y-%m-%d") else {
        tracing::warn!(expires = expires_str, "failed to parse expiration date");
        return false; // Invalid date format - don't expire
    };

    let today = Utc::now().date_naive();
    today > expires_date
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Verify base version extraction works correctly.
    ///
    /// Inputs:
    /// - Various version strings with and without suffixes.
    ///
    /// Output:
    /// - Confirms correct base version extraction.
    fn test_extract_base_version() {
        assert_eq!(extract_base_version("0.6.0"), "0.6.0");
        assert_eq!(extract_base_version("0.6.0-pr#85"), "0.6.0");
        assert_eq!(extract_base_version("0.6.0-beta"), "0.6.0");
        assert_eq!(extract_base_version("0.6.0-rc1"), "0.6.0");
        assert_eq!(extract_base_version("1.2.3-alpha.1"), "1.2.3");
        assert_eq!(extract_base_version("1.0.0"), "1.0.0");
        assert_eq!(extract_base_version("2.5.10-dev"), "2.5.10");
        // Handle versions with fewer segments
        assert_eq!(extract_base_version("1.0"), "1.0.0");
        assert_eq!(extract_base_version("1"), "1.0.0");
    }

    #[test]
    /// What: Verify version matching logic works correctly.
    ///
    /// Inputs:
    /// - Various version strings and ranges.
    ///
    /// Output:
    /// - Confirms correct matching behavior.
    fn test_version_matches() {
        assert!(version_matches("0.6.0", Some("0.6.0"), None));
        assert!(version_matches("0.6.0", Some("0.5.0"), None));
        assert!(!version_matches("0.6.0", Some("0.7.0"), None));
        assert!(version_matches("0.6.0", None, Some("0.7.0")));
        assert!(!version_matches("0.6.0", None, Some("0.5.0")));
        assert!(version_matches("0.6.0", Some("0.5.0"), Some("0.7.0")));
        assert!(!version_matches("0.6.0", Some("0.7.0"), Some("0.8.0")));
    }

    #[test]
    /// What: Verify version matching with pre-release versions.
    ///
    /// Inputs:
    /// - Pre-release version strings (e.g., "0.6.0-beta", "1.0.0-rc1").
    ///
    /// Output:
    /// - Confirms correct matching behavior for pre-release versions.
    ///
    /// Details:
    /// - Pre-release versions are compared using string comparison for non-numeric segments.
    /// - When comparing "0.6.0-beta" vs "0.6.0", the "beta" segment is compared as string
    ///   against the default "0", and "beta" > "0" lexicographically.
    fn test_version_matches_prerelease() {
        // Pre-release versions
        assert!(version_matches("0.6.0-beta", Some("0.6.0-beta"), None));
        assert!(version_matches("0.6.0-beta", Some("0.5.0"), None));
        assert!(!version_matches("0.6.0-beta", Some("0.7.0"), None));
        assert!(version_matches("1.0.0-rc1", Some("1.0.0-rc1"), None));
        assert!(version_matches("1.0.0-rc1", Some("0.9.0"), None));
        // Pre-release with non-numeric segment compared as string: "beta" > "0"
        assert!(version_matches("0.6.0-beta", Some("0.6.0"), None));
        assert!(!version_matches("0.6.0", Some("0.6.0-beta"), None));
    }

    #[test]
    /// What: Verify version matching with different segment counts.
    ///
    /// Inputs:
    /// - Versions with different numbers of segments (e.g., "1.0" vs "1.0.0").
    ///
    /// Output:
    /// - Confirms correct matching behavior when segment counts differ.
    ///
    /// Details:
    /// - Missing segments should be treated as "0".
    fn test_version_matches_different_segments() {
        assert!(version_matches("1.0", Some("1.0.0"), None));
        assert!(version_matches("1.0.0", Some("1.0"), None));
        assert!(version_matches("1.0", Some("1.0"), None));
        assert!(version_matches("1.0.0", Some("1.0.0"), None));
        assert!(version_matches("1.0", Some("0.9"), None));
        assert!(!version_matches("1.0", Some("1.1"), None));
    }

    #[test]
    /// What: Verify version matching boundary conditions.
    ///
    /// Inputs:
    /// - Exact min/max version matches.
    ///
    /// Output:
    /// - Confirms boundaries are inclusive.
    ///
    /// Details:
    /// - Both min and max bounds are inclusive, so exact matches should pass.
    fn test_version_matches_boundaries() {
        // Exact min boundary
        assert!(version_matches("0.6.0", Some("0.6.0"), Some("0.7.0")));
        // Exact max boundary
        assert!(version_matches("0.7.0", Some("0.6.0"), Some("0.7.0")));
        // Both boundaries exact
        assert!(version_matches("0.6.0", Some("0.6.0"), Some("0.6.0")));
        // Just below min
        assert!(!version_matches("0.5.9", Some("0.6.0"), Some("0.7.0")));
        // Just above max
        assert!(!version_matches("0.7.1", Some("0.6.0"), Some("0.7.0")));
    }

    #[test]
    /// What: Verify version matching with both bounds None.
    ///
    /// Inputs:
    /// - Version with both min and max as None.
    ///
    /// Output:
    /// - Should always match regardless of version.
    ///
    /// Details:
    /// - When both bounds are None, any version should match.
    fn test_version_matches_no_bounds() {
        assert!(version_matches("0.1.0", None, None));
        assert!(version_matches("1.0.0", None, None));
        assert!(version_matches("999.999.999", None, None));
        assert!(version_matches("0.0.0", None, None));
    }

    #[test]
    /// What: Verify version matching with non-numeric segments.
    ///
    /// Inputs:
    /// - Versions with non-numeric segments (e.g., "0.6.0-alpha" vs "0.6.0-beta").
    ///
    /// Output:
    /// - Confirms string comparison for non-numeric segments.
    ///
    /// Details:
    /// - Non-numeric segments are compared lexicographically.
    /// - When comparing versions with different segment counts, missing segments default to "0".
    /// - Non-numeric segments compared against "0" use string comparison: "alpha" > "0".
    fn test_version_matches_non_numeric_segments() {
        // Non-numeric segments compared as strings
        assert!(version_matches("0.6.0-alpha", Some("0.6.0-alpha"), None));
        assert!(version_matches("0.6.0-beta", Some("0.6.0-alpha"), None));
        assert!(!version_matches("0.6.0-alpha", Some("0.6.0-beta"), None));
        // Numeric vs non-numeric: "alpha" > "0" lexicographically
        assert!(!version_matches("0.6.0", Some("0.6.0-alpha"), None));
        assert!(version_matches("0.6.0-alpha", Some("0.6.0"), None));
    }

    #[test]
    /// What: Verify expiration checking logic.
    ///
    /// Inputs:
    /// - Various expiration dates.
    ///
    /// Output:
    /// - Confirms correct expiration behavior.
    fn test_is_expired() {
        // Future date should not be expired
        assert!(!is_expired(Some("2099-12-31")));
        // Past date should be expired
        assert!(is_expired(Some("2020-01-01")));
        // None should not be expired
        assert!(!is_expired(None));
    }

    #[test]
    /// What: Verify expiration checking with malformed date formats.
    ///
    /// Inputs:
    /// - Invalid date formats that cannot be parsed.
    ///
    /// Output:
    /// - Should not expire (returns false) for invalid formats.
    ///
    /// Details:
    /// - Invalid dates should be treated as non-expiring to avoid hiding announcements
    ///   due to parsing errors.
    /// - Note: Some formats like "2020-1-1" may be parsed successfully by chrono's
    ///   lenient parser, so we test with truly invalid formats.
    fn test_is_expired_malformed_dates() {
        // Invalid formats should not expire
        assert!(!is_expired(Some("invalid-date")));
        assert!(!is_expired(Some("2020/01/01")));
        assert!(!is_expired(Some("01-01-2020")));
        assert!(!is_expired(Some("")));
        assert!(!is_expired(Some("not-a-date")));
        assert!(!is_expired(Some("2020-13-45"))); // Invalid month/day
        assert!(!is_expired(Some("abc-def-ghi"))); // Non-numeric
    }

    #[test]
    /// What: Verify expiration checking edge case with today's date.
    ///
    /// Inputs:
    /// - Today's date as expiration.
    ///
    /// Output:
    /// - Should not expire (uses ">" not ">=").
    ///
    /// Details:
    /// - The comparison uses `today > expires_date`, so today's date should not expire.
    fn test_is_expired_today() {
        let today = Utc::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        // Today's date should not be expired (uses > not >=)
        assert!(!is_expired(Some(&today_str)));
    }

    #[test]
    /// What: Verify expiration checking with empty string.
    ///
    /// Inputs:
    /// - Empty string as expiration date.
    ///
    /// Output:
    /// - Should not expire (treated as invalid format).
    ///
    /// Details:
    /// - Empty string cannot be parsed as a date, so should not expire.
    fn test_is_expired_empty_string() {
        assert!(!is_expired(Some("")));
    }

    #[test]
    /// What: Verify `RemoteAnnouncement` deserialization from valid JSON.
    ///
    /// Inputs:
    /// - Valid JSON strings with all fields present.
    ///
    /// Output:
    /// - Successfully deserializes into `RemoteAnnouncement` struct.
    ///
    /// Details:
    /// - Tests that the struct can be deserialized from JSON format used by GitHub Gist.
    fn test_remote_announcement_deserialize_valid() {
        let json = r#"{
            "id": "test-announcement-1",
            "title": "Test Announcement",
            "content": "This is test content",
            "min_version": "0.6.0",
            "max_version": "0.7.0",
            "expires": "2025-12-31"
        }"#;

        let announcement: RemoteAnnouncement =
            serde_json::from_str(json).expect("should deserialize valid JSON");
        assert_eq!(announcement.id, "test-announcement-1");
        assert_eq!(announcement.title, "Test Announcement");
        assert_eq!(announcement.content, "This is test content");
        assert_eq!(announcement.min_version, Some("0.6.0".to_string()));
        assert_eq!(announcement.max_version, Some("0.7.0".to_string()));
        assert_eq!(announcement.expires, Some("2025-12-31".to_string()));
    }

    #[test]
    /// What: Verify `RemoteAnnouncement` deserialization with optional fields as null.
    ///
    /// Inputs:
    /// - JSON with optional fields set to null.
    ///
    /// Output:
    /// - Successfully deserializes with None for optional fields.
    ///
    /// Details:
    /// - Optional fields (`min_version`, `max_version`, `expires`) can be null or omitted.
    fn test_remote_announcement_deserialize_optional_null() {
        let json = r#"{
            "id": "test-announcement-2",
            "title": "Test Announcement",
            "content": "This is test content",
            "min_version": null,
            "max_version": null,
            "expires": null
        }"#;

        let announcement: RemoteAnnouncement =
            serde_json::from_str(json).expect("should deserialize with null fields");
        assert_eq!(announcement.id, "test-announcement-2");
        assert_eq!(announcement.min_version, None);
        assert_eq!(announcement.max_version, None);
        assert_eq!(announcement.expires, None);
    }

    #[test]
    /// What: Verify `RemoteAnnouncement` deserialization with omitted optional fields.
    ///
    /// Inputs:
    /// - JSON with optional fields completely omitted.
    ///
    /// Output:
    /// - Successfully deserializes with None for omitted fields.
    ///
    /// Details:
    /// - Optional fields can be omitted entirely from JSON.
    fn test_remote_announcement_deserialize_optional_omitted() {
        let json = r#"{
            "id": "test-announcement-3",
            "title": "Test Announcement",
            "content": "This is test content"
        }"#;

        let announcement: RemoteAnnouncement =
            serde_json::from_str(json).expect("should deserialize with omitted fields");
        assert_eq!(announcement.id, "test-announcement-3");
        assert_eq!(announcement.min_version, None);
        assert_eq!(announcement.max_version, None);
        assert_eq!(announcement.expires, None);
    }

    #[test]
    /// What: Verify `RemoteAnnouncement` deserialization fails with invalid JSON.
    ///
    /// Inputs:
    /// - Invalid JSON strings that cannot be parsed.
    ///
    /// Output:
    /// - Returns error when JSON is invalid or missing required fields.
    ///
    /// Details:
    /// - Required fields (`id`, `title`, `content`) must be present and valid.
    fn test_remote_announcement_deserialize_invalid() {
        // Missing required field
        let json_missing_id = r#"{
            "title": "Test",
            "content": "Content"
        }"#;
        assert!(serde_json::from_str::<RemoteAnnouncement>(json_missing_id).is_err());

        // Invalid JSON syntax
        let json_invalid = r#"{
            "id": "test",
            "title": "Test",
            "content": "Content"
        "#;
        assert!(serde_json::from_str::<RemoteAnnouncement>(json_invalid).is_err());

        // Wrong types
        let json_wrong_type = r#"{
            "id": 123,
            "title": "Test",
            "content": "Content"
        }"#;
        assert!(serde_json::from_str::<RemoteAnnouncement>(json_wrong_type).is_err());
    }
}
