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

/// What: Parse and normalize an RFC-like date string to `YYYY-MM-DD` format for sorting.
///
/// Input: `s` full date string, e.g., "Mon, 23 Oct 2023 12:34:56 +0000" or "Thu, 21 Aug 2025"
/// Output: Normalized date in `YYYY-MM-DD` format, e.g., "2023-10-23"
///
/// Details:
/// - First tries to parse as RFC 2822 (RSS date format like "Thu, 21 Aug 2025 12:34:56 +0000").
/// - Falls back to RFC 3339 parsing.
/// - If all parsing fails, returns the original stripped date (for backwards compatibility).
pub fn strip_time_and_tz(s: &str) -> String {
    let trimmed = s.trim();

    // Try RFC 2822 format first (RSS/Atom feeds: "Thu, 21 Aug 2025 12:34:56 +0000")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        return dt.format("%Y-%m-%d").to_string();
    }

    // Try RFC 3339 format
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return dt.format("%Y-%m-%d").to_string();
    }

    // Try ISO 8601 without timezone
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return dt.format("%Y-%m-%d").to_string();
    }

    // Try parsing just date part if it's already in YYYY-MM-DD format
    if chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() {
        return trimmed.to_string();
    }

    // Fallback: try to extract date from partial RFC 2822 without timezone
    // Format: "Thu, 21 Aug 2025" or "Thu, 21 Aug 2025 12:34:56"
    if let Some(date) = parse_partial_rfc2822(trimmed) {
        return date;
    }

    // Last resort: strip time and timezone manually and return as-is
    // (This preserves backwards compatibility but won't sort correctly)
    let mut t = trimmed.to_string();
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

/// What: Parse a partial RFC 2822 date string (without timezone) to `YYYY-MM-DD`.
///
/// Input: Partial RFC 2822 string like "Thu, 21 Aug 2025" or "21 Aug 2025"
/// Output: `Some("2025-08-21")` on success, `None` on failure
///
/// Details:
/// - Handles both with and without leading day-of-week.
/// - Parses common month abbreviations (Jan, Feb, etc.).
fn parse_partial_rfc2822(s: &str) -> Option<String> {
    // Try to find day, month, year pattern
    // Common formats: "Thu, 21 Aug 2025", "21 Aug 2025"
    let parts: Vec<&str> = s.split_whitespace().collect();

    // Find the numeric day, month abbreviation, and year
    let (day_str, month_str, year_str) = if parts.len() >= 4 && parts[0].ends_with(',') {
        // "Thu, 21 Aug 2025" format
        (parts.get(1)?, parts.get(2)?, parts.get(3)?)
    } else if parts.len() >= 3 {
        // "21 Aug 2025" format
        (parts.first()?, parts.get(1)?, parts.get(2)?)
    } else {
        return None;
    };

    // Parse day
    let day: u32 = day_str.parse().ok()?;

    // Parse month abbreviation
    let month = match month_str.to_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    };

    // Parse year (take first 4 digits if there's more)
    let year: i32 = year_str.chars().take(4).collect::<String>().parse().ok()?;

    // Validate and format
    if (1..=31).contains(&day) && (1970..=2100).contains(&year) {
        Some(format!("{year:04}-{month:02}-{day:02}"))
    } else {
        None
    }
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
