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
    VersionAnnouncement {
        version: "0.6.2",
        title: "Version 0.6.2",
        content: "## What's New\n\n### ⚡ Force Sync Option\n- Toggle between Normal (-Syu) and Force Sync (-Syyu) in System Update\n- Use ←/→ or Tab keys to switch sync mode\n\n### 🐛 Bug Fixes\n- Install list preserved: System update no longer clears queued packages\n- Faster exit: App closes immediately when exiting during preflight\n- Auto-refresh: Updates count refreshes after install/remove/downgrade\n\n### 🌍 Translations\n- Updated Hungarian translations\n",
    },
    VersionAnnouncement {
        version: "0.7.0",
        title: "Version 0.7.0",
        content: "## What's New\n\n- **Arch Linux News**: Latest announcements and updates from archlinux.org\n- **Security Advisories**: Security alerts with severity indicators and affecte...\n- **Package Updates**: Track version changes for your installed packages with c...\n- **AUR Comments**: Recent community discussions and feedback\n- **Change Detection**: Automatically detects package changes (version, maintai...\n\n",
    },
    VersionAnnouncement {
        version: "0.7.1",
        title: "Version 0.7.1",
        content: "## What's New\n\n### News Mode Enhancements\n- **Separated search inputs**: News mode and Package mode now have independent search fields\n  - No more shared state issues when switching between modes\n  - Search text is preserved when switching modes\n- **Improved mark-as-read behavior**: Mark read actions (`r` key) now only work in normal mode\n  - Prevents accidental marking when typing 'r' in insert mode\n  - More consistent with vim-like behavior\n\n### Toast Notifications\n- Improved toast clearing logic for better user experience\n- Enhanced toast title detection for news, clipboard, and notification types\n- Added notification title translations\n\n### UI Polish\n- Sort menu no longer auto-closes (stays open until you select an option or close it)\n- Added `change_sort` keybind to help footer in News mode\n- Fixed help text punctuation for better readability\n\n",
    },
    VersionAnnouncement {
        version: "0.7.2",
        title: "Version 0.7.2",
        content: "## What's New\n\n- Updated multiple dependencies to address low-severity security vulnerabilities\n- Updated core dependencies including `clap`, `ratatui`, `tokio`, `reqwest`, and more\n- Improved overall security posture of the application\n- Fixed CodeQL security analysis issues (#2, #3, #4, #5)\n- Enhanced input validation in import modals\n\n",
    },
    VersionAnnouncement {
        version: "0.7.3",
        title: "Version 0.7.3",
        content: "## What's New\n\n- TUI install/update/downgrade operations can use passwordless sudo when configured\n- Same behavior as CLI: no password prompt when sudo allows it\n- Remove operations always ask for password for safety\n- Opening config files now uses your `VISUAL` or `EDITOR` environment variable\n- Edit settings, theme, and keybinds in your preferred editor\n\n",
    },
    VersionAnnouncement {
        version: "0.7.4",
        title: "Version 0.7.4",
        content: "## What's New\n\n- New `privilege_tool` setting: `auto` | `sudo` | `doas`\n- Commands now run through the selected tool (or auto-detected one) instead of always using sudo\n- New `auth_mode` setting: `prompt` | `passwordless_only` | `interactive`\n- Interactive mode hands off to the terminal so sudo/doas can handle PAM prompts directly (including fingerprint via fprintd, when configured)\n- Detects the `blackarch` repo and adds a toggle/filter in results when available\n\n",
    },
    VersionAnnouncement {
        version: "0.8.0",
        title: "Version 0.8.0",
        content: "## What's New\n\n### Custom and third-party repositories\n\n- Configure extra repos in **`repos.conf`**, edit them from the app, and apply changes when you are ready (with privilege prompts when needed).\n- Search and filters can include packages from those repos, with sensible handling when the same package name appears more than once.\n- Toggle managed entries on or off; disabled repos are ignored until you enable them again. The repositories screen can refresh with up-to-date status after related dialogs.\n- First run seeds **`repos.conf`** with common third-party recipes (disabled by default); enable only what you need.\n\n### After you add a repo\n\n- If packages you installed also exist in the new repo, a short guided flow explains the situation and helps you choose what to do next (including optional cleanup). Preview-only mode stays accurate; canceling or errors should not leave the UI stuck.\n\n### Overlapping names (AUR vs other sources)\n\n- AUR installs go through your helper in a way that avoids wrong-source surprises when a community mirror lists the same name.\n- Selecting an AUR hit that also appears as a normal Arch listing can show a one-time warning before you continue.\n\n### Optional: AUR voting\n\n- Vote or unvote AUR packages from search when enabled, using SSH to the AUR.\n- Built-in **SSH AUR setup** helps you configure the host entry in your SSH config.\n- Honors your SSH command, timeout, and preview-only mode (no fake vote state).\n\n### Optional: PKGBUILD checks\n\n- Run **ShellCheck** and **Namcap** on the selected package build file from the details view when those tools are installed (timeouts and missing tools handled gracefully).\n- Switch between the PKGBUILD text and check results in the details pane; settings cover raw output and ShellCheck excludes.\n\n### Bug fixes\n\n- **Repositories:** Stricter validation for paths, server URLs, signing keys, and filter keys; safer behavior when apply is interrupted or does not complete successfully.\n- **PKGBUILD checks:** More reliable when starting a check; time limits handled inside the app; clearer messaging when a checker is not installed.\n- **Lists and filters:** Better column alignment when names use wide characters (e.g. some non-Latin scripts).\n",
    },
    VersionAnnouncement {
        version: "0.8.1",
        title: "Version 0.8.1",
        content: "## What's New\n\nCompared to **v0.8.0**, this release improves first-run setup, the updates experience, and the optional **AUR voting** SSH wizard. Everything below is new or different in **v0.8.1**; unchanged areas (custom repos, PKGBUILD checks, core voting behavior, etc.) are not repeated here.\n\n### ✨ Features\n\n- **Startup setup**: A selector runs optional setup tasks in order (optional dependencies, AUR SSH, VirusTotal, news, and related steps). Optional dependencies includes a **[Wizard]** entry. New **sudo timestamp** and **doas persist** setup wizards; install/update/remove can warn when long sessions may hit auth limits.\n- **AUR SSH setup**: Guided flow through local key/config, pasting the key on AUR, then a live SSH check. Copy the public key with **C** or the copy row; open the AUR login with **O** when you need it. More reliable first connection to AUR (including host-key handling). Success feedback appears after the remote check succeeds.\n- **Updates**: Layout shows **repo/name** and **old → new** versions with clearer diff highlighting. Slash filter, multi-select, and navigation behave more predictably, including with wrapped lines and the mouse. The app can indicate when an update list may be incomplete and why.\n\n### 🛡 Security & reliability\n\n- Tighter handling around privileged commands, temporary scripts, and saved command logs.\n\n### 🐛 Fixes\n\n- Calmer first-run order between setup dialogs and version announcements.\n- Clearer labels when a setup task is unavailable (for example wrong privilege tool).\n- Setup dialogs no longer leave stray keypresses for the next screen.\n- Startup news no longer pops up on its own; leaving news setup does not resurrect an old Arch news window.\n- Optional dependency batch installs go through the same auth/preflight path as other installs; terminal integration fixes for multiline follow-up commands and fallback ordering.\n\n",
    },
    VersionAnnouncement {
        version: "0.8.2",
        title: "Version 0.8.2",
        content: "## What's New\n\nCompared to **v0.8.1**, this release focuses on layout customization, smoother PKGBUILD viewing, better modal scrolling, and desktop launcher files. Packaging for **pacsea-git** on the AUR was aligned with the current repo layout (including merged [PR #158](https://github.com/Firstp1ck/Pacsea/pull/158)).\n\n### ✨ Features\n\n- **Configurable UI layout**: Set `main_pane_order` and per-role vertical min/max in `settings.conf` so search, results, and details appear in the order and proportions you prefer.\n- **Mouse wheel in modals**: Scroll the focused row in System Update, Repositories, and Optional Dependencies modals when the pointer is over the list.\n- **Desktop integration**: `.desktop` entry and SVG icon ship with the tree for menu launchers and file managers.\n\n### 🛡 Security & reliability\n\n- **PKGBUILD fetching**: Each fetch runs in its own async task so one slow host does not block the queue; stale results are dropped when you change rows.\n\n### 🐛 Fixes\n\n- Shorter connect timeouts on PKGBUILD `curl` calls so bad hosts fail faster.\n- **pacsea-git** / `makepkg`: clear toolchain env (including `CHOST`) before builds when `makepkg.conf` has cross-compile defaults that would break a normal package build.\n- **Packaging**: Correct source URLs and sparse-checkout paths in `PKGBUILD-git`; icon file permissions set for normal files (not executable).\n\n",
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
