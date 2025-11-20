//! Configuration file parsing utilities.
//!
//! This module provides helpers for parsing configuration files with common
//! patterns like comment skipping and key-value parsing.

/// What: Check if a line should be skipped (empty or comment).
///
/// Inputs:
/// - `line`: Line to check
///
/// Output:
/// - `true` if the line should be skipped, `false` otherwise
///
/// Details:
/// - Skips empty lines and lines starting with `#`, `//`, or `;`
pub fn skip_comment_or_empty(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("//")
        || trimmed.starts_with(';')
}

/// What: Parse a key-value pair from a line.
///
/// Inputs:
/// - `line`: Line containing key=value format
///
/// Output:
/// - `Some((key, value))` if parsing succeeds, `None` otherwise
///
/// Details:
/// - Splits on the first `=` character
/// - Trims whitespace from both key and value
pub fn parse_key_value(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.contains('=') {
        return None;
    }
    let mut parts = trimmed.splitn(2, '=');
    let key = parts.next()?.trim().to_string();
    let value = parts.next()?.trim().to_string();
    Some((key, value))
}
