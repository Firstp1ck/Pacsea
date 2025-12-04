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
/// - Shown once per version when user first launches that version.
/// - Content is embedded in the binary at compile time.
pub struct VersionAnnouncement {
    /// Version string this announcement is for (e.g., "0.6.0").
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
/// - Each announcement is shown once per version.
pub const VERSION_ANNOUNCEMENTS: &[VersionAnnouncement] = &[
    // Add version-specific announcements here
    // Example:
    // VersionAnnouncement {
    //     version: "0.6.0",
    //     title: "Welcome to Pacsea 0.6.0",
    //     content: "## What's New\n\n- Announcement popup feature\n- ...",
    // },
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
}
