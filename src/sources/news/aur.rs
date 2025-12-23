//! AUR-related functionality for news module.

use crate::sources::news::parse::collapse_blank_lines;

/// What: Extract package name from an AUR package URL.
///
/// Inputs:
/// - `url`: URL to inspect.
///
/// Output:
/// - `Some(pkgname)` if the URL matches `https://aur.archlinux.org/packages/<name>`
///   or official package links we build for AUR items; `None` otherwise.
pub fn extract_aur_pkg_from_url(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    let needle = "aur.archlinux.org/packages/";
    let pos = lower.find(needle)?;
    let after = &url[pos + needle.len()..];
    // Stop at path separator, query string, or URL fragment (e.g., #comment-123)
    let end = after
        .find('/')
        .or_else(|| after.find('?'))
        .or_else(|| after.find('#'))
        .unwrap_or(after.len());
    let pkg = &after[..end];
    if pkg.is_empty() {
        None
    } else {
        Some(pkg.to_string())
    }
}

/// What: Render AUR comments into a readable text block for the details pane.
///
/// Inputs:
/// - `pkg`: Package name.
/// - `comments`: Full comment list (pinned + latest) sorted newest-first.
///
/// Output:
/// - Plaintext content including pinned comments (marked) and newest comments from the last 7 days
///   (or the latest available if timestamps are missing).
pub fn render_aur_comments(pkg: &str, comments: &[crate::state::types::AurComment]) -> String {
    use chrono::{Duration, Utc};

    let now = Utc::now().timestamp();
    let cutoff = now - Duration::days(7).num_seconds();

    let pinned: Vec<&crate::state::types::AurComment> =
        comments.iter().filter(|c| c.pinned).collect();
    let mut recent: Vec<&crate::state::types::AurComment> = comments
        .iter()
        .filter(|c| !c.pinned && c.date_timestamp.is_some_and(|ts| ts >= cutoff && ts <= now))
        .collect();

    // Track if we're using a fallback (showing non-recent comment)
    let is_fallback = recent.is_empty();
    if is_fallback {
        // Show most recent non-pinned comment as fallback if no recent comments exist
        if let Some(first_non_pinned) = comments.iter().find(|c| !c.pinned) {
            recent.push(first_non_pinned);
        }
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("AUR comments for {pkg}"));
    lines.push(String::new());

    if !pinned.is_empty() {
        lines.push("[Pinned]".to_string());
        for c in pinned {
            push_comment_lines(&mut lines, c, true);
        }
        lines.push(String::new());
    }

    if recent.is_empty() {
        lines.push("No recent comments.".to_string());
    } else if is_fallback {
        // Show fallback comment with appropriate label
        lines.push("Latest comment".to_string());
        for c in recent {
            push_comment_lines(&mut lines, c, false);
        }
    } else {
        lines.push("Recent (last 7 days)".to_string());
        for c in recent {
            push_comment_lines(&mut lines, c, false);
        }
    }

    collapse_blank_lines(&lines.iter().map(String::as_str).collect::<Vec<_>>())
}

/// What: Append a single comment (with metadata) into the output lines.
fn push_comment_lines(lines: &mut Vec<String>, c: &crate::state::types::AurComment, pinned: bool) {
    let mut header = String::new();
    if pinned {
        header.push_str("[Pinned] ");
    }
    header.push_str(&c.author);
    if !c.date.is_empty() {
        header.push_str(" — ");
        header.push_str(&c.date);
    }
    if let Some(url) = &c.date_url
        && !url.is_empty()
    {
        header.push(' ');
        header.push('(');
        header.push_str(url);
        header.push(')');
    }
    lines.push(header);
    let content = c.content.trim();
    if !content.is_empty() {
        lines.push(content.to_string());
    }
    lines.push(String::new());
}
