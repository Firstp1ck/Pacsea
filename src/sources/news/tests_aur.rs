//! AUR-related tests for news module.

use crate::sources::news::aur::{extract_aur_pkg_from_url, render_aur_comments};

#[test]
/// What: Test `extract_aur_pkg_from_url` identifies AUR package URLs.
///
/// Inputs:
/// - Various AUR package URL formats.
///
/// Output:
/// - Package name extracted correctly from URL.
///
/// Details:
/// - Verifies AUR URL detection for comment rendering branch.
fn test_extract_aur_pkg_from_url() {
    assert_eq!(
        extract_aur_pkg_from_url("https://aur.archlinux.org/packages/foo"),
        Some("foo".to_string())
    );
    assert_eq!(
        extract_aur_pkg_from_url("https://aur.archlinux.org/packages/foo/"),
        Some("foo".to_string())
    );
    assert_eq!(
        extract_aur_pkg_from_url("https://aur.archlinux.org/packages/foo-bar"),
        Some("foo-bar".to_string())
    );
    assert_eq!(
        extract_aur_pkg_from_url("https://aur.archlinux.org/packages/foo?query=bar"),
        Some("foo".to_string())
    );
    // URL fragments (e.g., #comment-123) should be stripped from package name
    assert_eq!(
        extract_aur_pkg_from_url(
            "https://aur.archlinux.org/packages/discord-canary#comment-1050019"
        ),
        Some("discord-canary".to_string())
    );
    assert_eq!(
        extract_aur_pkg_from_url("https://aur.archlinux.org/packages/foo#section"),
        Some("foo".to_string())
    );
    assert_eq!(
        extract_aur_pkg_from_url("https://archlinux.org/news/item"),
        None
    );
}

#[test]
/// What: Test `render_aur_comments` formats comments correctly.
///
/// Inputs:
/// - AUR comments with pinned and recent items.
///
/// Output:
/// - Rendered text includes pinned section and recent comments.
///
/// Details:
/// - Verifies comment rendering for AUR package pages.
fn test_render_aur_comments() {
    use crate::state::types::AurComment;
    use chrono::{Duration, Utc};

    let now = Utc::now().timestamp();
    let cutoff = now - Duration::days(7).num_seconds();

    let comments = vec![
        AurComment {
            id: Some("c1".into()),
            author: "user1".into(),
            date: "2025-01-01 00:00 (UTC)".into(),
            date_timestamp: Some(cutoff + 86400), // Within 7 days
            date_url: Some("https://aur.archlinux.org/packages/foo#comment-1".into()),
            content: "Recent comment".into(),
            pinned: false,
        },
        AurComment {
            id: Some("c2".into()),
            author: "maintainer".into(),
            date: "2024-12-01 00:00 (UTC)".into(),
            date_timestamp: Some(cutoff - 86400), // Older than 7 days
            date_url: Some("https://aur.archlinux.org/packages/foo#comment-2".into()),
            content: "Pinned comment".into(),
            pinned: true,
        },
    ];

    let rendered = render_aur_comments("foo", &comments);
    assert!(rendered.contains("AUR comments for foo"));
    assert!(rendered.contains("[Pinned]"));
    assert!(rendered.contains("Recent (last 7 days)"));
    assert!(rendered.contains("Recent comment"));
    assert!(rendered.contains("Pinned comment"));
}

/// What: Test that comments with None timestamps and future dates are excluded from "Recent".
///
/// Inputs:
/// - Comments with None timestamps
/// - Comments with future dates
///
/// Output:
/// - These comments should not appear in "Recent (last 7 days)" section.
///
/// Details:
/// - Verifies the fix for bug where unparseable or future dates were incorrectly marked as recent.
#[test]
fn test_render_aur_comments_excludes_invalid_dates() {
    use crate::state::types::AurComment;
    use chrono::{Duration, Utc};

    let now = Utc::now().timestamp();
    let cutoff = now - Duration::days(7).num_seconds();

    let comments = vec![
        AurComment {
            id: Some("c1".into()),
            author: "user1".into(),
            date: "2025-04-14 11:52 (UTC+2)".into(),
            date_timestamp: None, // Unparseable date
            date_url: Some("https://aur.archlinux.org/packages/foo#comment-1".into()),
            content: "Comment with unparseable date".into(),
            pinned: false,
        },
        AurComment {
            id: Some("c2".into()),
            author: "user2".into(),
            date: "2025-12-25 00:00 (UTC)".into(),
            date_timestamp: Some(now + Duration::days(365).num_seconds()), // Future date
            date_url: Some("https://aur.archlinux.org/packages/foo#comment-2".into()),
            content: "Comment with future date".into(),
            pinned: false,
        },
        AurComment {
            id: Some("c3".into()),
            author: "user3".into(),
            date: "2024-01-01 00:00 (UTC)".into(),
            date_timestamp: Some(cutoff - 86400), // Older than 7 days
            date_url: Some("https://aur.archlinux.org/packages/foo#comment-3".into()),
            content: "Old comment".into(),
            pinned: false,
        },
        AurComment {
            id: Some("c4".into()),
            author: "user4".into(),
            date: "2025-01-01 00:00 (UTC)".into(),
            date_timestamp: Some(cutoff + 86400), // Within 7 days
            date_url: Some("https://aur.archlinux.org/packages/foo#comment-4".into()),
            content: "Recent comment".into(),
            pinned: false,
        },
    ];

    let rendered = render_aur_comments("foo", &comments);
    assert!(rendered.contains("AUR comments for foo"));
    assert!(rendered.contains("Recent (last 7 days)"));
    assert!(rendered.contains("Recent comment")); // Should include valid recent comment
    assert!(!rendered.contains("Comment with unparseable date")); // Should exclude None timestamp
    assert!(!rendered.contains("Comment with future date")); // Should exclude future date
    assert!(!rendered.contains("Old comment")); // Should exclude old comment
}

/// What: Test that fallback comment shows "Latest comment" instead of "Recent (last 7 days)".
///
/// Inputs:
/// - Comments that are all older than 7 days or have invalid timestamps
///
/// Output:
/// - Should show "Latest comment" label when showing fallback comment
///
/// Details:
/// - Verifies that non-recent comments shown as fallback are labeled correctly
#[test]
fn test_render_aur_comments_fallback_label() {
    use crate::state::types::AurComment;
    use chrono::{Duration, Utc};

    let now = Utc::now().timestamp();
    let cutoff = now - Duration::days(7).num_seconds();

    let comments = vec![AurComment {
        id: Some("c1".into()),
        author: "user1".into(),
        date: "2024-01-01 00:00 (UTC)".into(),
        date_timestamp: Some(cutoff - 86400), // Older than 7 days
        date_url: Some("https://aur.archlinux.org/packages/foo#comment-1".into()),
        content: "Old comment".into(),
        pinned: false,
    }];

    let rendered = render_aur_comments("foo", &comments);
    assert!(rendered.contains("AUR comments for foo"));
    assert!(rendered.contains("Latest comment")); // Should show "Latest comment" for fallback
    assert!(!rendered.contains("Recent (last 7 days)")); // Should not show "Recent" label
    assert!(rendered.contains("Old comment")); // Should show the fallback comment
}
