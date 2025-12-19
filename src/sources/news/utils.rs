//! Utility functions for news parsing and URL handling.

/// What: Return the substring strictly between `start` and `end` markers (if present).
///
/// Input: `s` source text; `start` opening marker; `end` closing marker
/// Output: `Some(String)` of enclosed content; `None` if markers are missing
///
/// Details: Searches for the first occurrence of `start`, then the next occurrence of `end`
/// after it; returns the interior substring when both are found in order.
pub fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(s[i..j].to_string())
}

/// What: Strip the trailing time and optional timezone from an RFC-like date string.
///
/// Input: `s` full date string, e.g., "Mon, 23 Oct 2023 12:34:56 +0000"
/// Output: Date-only portion, e.g., "Mon, 23 Oct 2023"
///
/// Details: First trims any trailing " +ZZZZ" timezone, then detects and removes an
/// 8-character time segment ("HH:MM:SS") if present, returning the remaining prefix.
pub fn strip_time_and_tz(s: &str) -> String {
    let mut t = s.trim().to_string();
    if let Some(pos) = t.rfind(" +") {
        t.truncate(pos);
        t = t.trim_end().to_string();
    }
    if t.len() >= 9 {
        let n = t.len();
        let time_part = &t[n - 8..n];
        let looks_time = time_part.chars().enumerate().all(|(i, c)| match i {
            2 | 5 => c == ':',
            _ => c.is_ascii_digit(),
        });
        if looks_time && t.as_bytes()[n - 9] == b' ' {
            t.truncate(n - 9);
        }
    }
    t.trim_end().to_string()
}

/// What: Check if a URL is from archlinux.org (including www subdomain).
///
/// Inputs:
/// - `url`: URL string to check
///
/// Output:
/// - `true` if URL is from archlinux.org or www.archlinux.org, `false` otherwise
///
/// Details:
/// - Checks for both `https://archlinux.org/` and `https://www.archlinux.org/` prefixes.
pub fn is_archlinux_url(url: &str) -> bool {
    url.starts_with("https://archlinux.org/") || url.starts_with("https://www.archlinux.org/")
}

/// What: Resolve relative hrefs against the provided origin.
pub fn resolve_href(href: &str, base_origin: Option<&str>) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if let Some(origin) = base_origin
        && href.starts_with('/')
    {
        return format!("{origin}{href}");
    }
    href.to_string()
}

/// What: Extract `<scheme://host>` from a URL for resolving relative links.
pub fn extract_origin(url: &str) -> Option<String> {
    let scheme_split = url.split_once("://")?;
    let scheme = scheme_split.0;
    let rest = scheme_split.1;
    let host_end = rest.find('/').unwrap_or(rest.len());
    if host_end == 0 {
        return None;
    }
    let host = &rest[..host_end];
    Some(format!("{scheme}://{host}"))
}

/// What: Check if a URL points to an Arch package details page.
pub fn is_arch_package_url(url: &str) -> bool {
    url.contains("://archlinux.org/packages/")
}
