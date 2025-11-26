//! AUR package comments fetching via web scraping.

use chrono::Datelike;
use scraper::{Html, Selector};
use std::time::Duration;

use crate::state::types::AurComment;

type Result<T> = super::Result<T>;

/// What: Fetch AUR package comments by scraping the AUR package page.
///
/// Inputs:
/// - `pkgname`: Package name to fetch comments for.
///
/// Output:
/// - `Ok(Vec<AurComment>)` with parsed comments sorted by date (latest first); `Err` on failure.
///
/// # Errors
/// - Returns `Err` when network request fails
/// - Returns `Err` when HTML parsing fails
/// - Returns `Err` when comment extraction fails
///
/// # Panics
/// - Panics if selector parsing fails in fallback path (should not occur with valid selectors)
///
/// Details:
/// - Fetches HTML from `https://aur.archlinux.org/packages/<pkgname>`
/// - Uses `scraper` to parse HTML and extract comment elements
/// - Parses dates to Unix timestamps for sorting
/// - Sorts comments by date descending (latest first)
/// - Only works for AUR packages
pub async fn fetch_aur_comments(pkgname: String) -> Result<Vec<AurComment>> {
    let url = format!("https://aur.archlinux.org/packages/{pkgname}");

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    // Fetch HTML
    let html_text = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    // Parse HTML
    let document = Html::parse_document(&html_text);

    // AUR comments structure:
    // - Each comment has an <h4 class="comment-header"> with author and date
    // - The content is in a following <div class="article-content"> with id "comment-{id}-content"
    // - Pinned comments appear before "Latest Comments" heading
    let comment_header_selector = Selector::parse("h4.comment-header")
        .map_err(|e| format!("Failed to parse comment header selector: {e}"))?;

    let date_selector =
        Selector::parse("a.date").map_err(|e| format!("Failed to parse date selector: {e}"))?;

    let p_selector =
        Selector::parse("p").map_err(|e| format!("Failed to parse paragraph selector: {e}"))?;

    // Find the "Latest Comments" heading to separate pinned from regular comments
    // Pinned comments appear before this heading
    let heading_selector = Selector::parse("h3, h2, h4")
        .map_err(|e| format!("Failed to parse heading selector: {e}"))?;

    // Check if there's a "Pinned Comments" section
    let has_pinned_section = document.select(&heading_selector).any(|h| {
        let text: String = h.text().collect();
        text.contains("Pinned Comments")
    });

    // Find the "Latest Comments" heading position in the HTML text
    // Comments that appear before this in the HTML are pinned
    let html_text_lower = html_text.to_lowercase();
    let latest_comments_pos = html_text_lower.find("latest comments");

    // Collect all headers
    let all_headers: Vec<_> = document.select(&comment_header_selector).collect();

    // Use a HashSet to track seen comment IDs to avoid duplicates
    let mut seen_comment_ids = std::collections::HashSet::new();
    let mut comments = Vec::new();

    // Process each header and find its corresponding content by ID
    for (index, header) in all_headers.iter().enumerate() {
        // Extract comment ID from header
        let comment_id = header.value().attr("id");

        // Skip if we've already seen this comment ID (deduplication)
        if let Some(id) = comment_id
            && !seen_comment_ids.insert(id)
        {
            continue; // Skip duplicate
        }

        // Extract the full header text to parse author
        let header_text = header.text().collect::<String>();

        // Extract author: text before " commented on"
        let author = header_text.find(" commented on ").map_or_else(
            || {
                // Fallback: try to find author in links or text nodes
                header_text
                    .split_whitespace()
                    .next()
                    .unwrap_or("Unknown")
                    .to_string()
            },
            |pos| header_text[..pos].trim().to_string(),
        );

        // Extract date and URL from <a class="date"> inside the header
        let base_url = format!("https://aur.archlinux.org/packages/{pkgname}");
        let (date_text, date_url) = header.select(&date_selector).next().map_or_else(
            || (String::new(), None),
            |e| {
                let text = e.text().collect::<String>().trim().to_string();
                let url = e.value().attr("href").map(|href| {
                    // Convert relative URLs to absolute
                    if href.starts_with("http://") || href.starts_with("https://") {
                        href.to_string()
                    } else if href.starts_with('#') {
                        // Fragment-only URL: combine with package page URL
                        format!("{base_url}{href}")
                    } else {
                        // Relative path: prepend AUR domain
                        format!("https://aur.archlinux.org{href}")
                    }
                });
                (text, url)
            },
        );

        // Get content by finding the corresponding content div by ID
        let content = comment_id
            .and_then(|id| id.strip_prefix("comment-"))
            .and_then(|comment_id_str| {
                Selector::parse(&format!("div#comment-{comment_id_str}-content")).ok()
            })
            .and_then(|content_id_selector| document.select(&content_id_selector).next())
            .map_or_else(String::new, |div| {
                // Extract text from all <p> tags
                let paragraphs: Vec<String> = div
                    .select(&p_selector)
                    .map(|p| p.text().collect::<String>().trim().to_string())
                    .collect();

                if paragraphs.is_empty() {
                    // Fallback: get all text from the div
                    div.text().collect::<String>().trim().to_string()
                } else {
                    paragraphs.join("\n")
                }
            });

        // Skip empty comments
        if content.is_empty() && author == "Unknown" {
            continue;
        }

        // Parse date to timestamp
        let date_timestamp = parse_date_to_timestamp(&date_text);

        // Convert UTC date to local timezone for display
        let local_date = convert_utc_to_local_date(&date_text);

        // Determine if this comment is pinned
        // Pinned comments appear before the "Latest Comments" heading in the HTML
        // We check the position of the comment header in the HTML text relative to "Latest Comments"
        let is_pinned = if has_pinned_section && let Some(latest_pos) = latest_comments_pos {
            comment_id.map_or(index < 10, |id| {
                html_text
                    .find(id)
                    .map_or(index < 10, |comment_pos| comment_pos < latest_pos)
            })
        } else {
            false
        };

        comments.push(AurComment {
            author,
            date: local_date,
            date_timestamp,
            date_url,
            content,
            pinned: is_pinned,
        });
    }

    // Separate pinned and regular comments
    let mut pinned_comments: Vec<AurComment> =
        comments.iter().filter(|c| c.pinned).cloned().collect();
    let mut regular_comments: Vec<AurComment> =
        comments.into_iter().filter(|c| !c.pinned).collect();

    // Sort pinned comments by date descending (latest first)
    pinned_comments.sort_by(|a, b| {
        match (a.date_timestamp, b.date_timestamp) {
            (Some(ts_a), Some(ts_b)) => ts_b.cmp(&ts_a), // Descending order
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.date.cmp(&a.date), // Fallback to string comparison
        }
    });

    // Sort regular comments by date descending (latest first)
    regular_comments.sort_by(|a, b| {
        match (a.date_timestamp, b.date_timestamp) {
            (Some(ts_a), Some(ts_b)) => ts_b.cmp(&ts_a), // Descending order
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.date.cmp(&a.date), // Fallback to string comparison
        }
    });

    // Combine: pinned first, then regular
    pinned_comments.extend(regular_comments);

    Ok(pinned_comments)
}

/// What: Convert UTC date string from AUR to local timezone string.
///
/// Inputs:
/// - `utc_date_str`: UTC date string from AUR page (e.g., "2025-05-15 03:55 (UTC)").
///
/// Output:
/// - Local timezone date string formatted as "YYYY-MM-DD HH:MM (TZ)" where TZ is local timezone abbreviation.
/// - Returns original string if parsing fails.
///
/// Details:
/// - Parses UTC date from AUR format
/// - Converts to local timezone using system timezone
/// - Formats with local timezone abbreviation
fn convert_utc_to_local_date(utc_date_str: &str) -> String {
    let utc_date_str = utc_date_str.trim();

    // AUR format: "YYYY-MM-DD HH:MM (UTC)" or "YYYY-MM-DD HH:MM (CEST)" etc.
    // Try to parse the date/time part before the timezone
    if let Some(tz_start) = utc_date_str.rfind('(') {
        let date_time_part = utc_date_str[..tz_start].trim();

        // Try parsing "YYYY-MM-DD HH:MM" format as UTC
        if let Ok(naive_dt) =
            chrono::NaiveDateTime::parse_from_str(date_time_part, "%Y-%m-%d %H:%M")
        {
            // Treat as UTC and convert to local timezone
            let utc_dt = naive_dt.and_utc();
            let local_dt = utc_dt.with_timezone(&chrono::Local);

            // Format with local timezone
            // Format: "YYYY-MM-DD HH:MM (TZ)"
            let formatted = local_dt.format("%Y-%m-%d %H:%M");

            // Get timezone abbreviation
            // Try multiple methods to get the actual timezone name (CEST, CET, etc.)
            let tz_abbr = get_timezone_abbreviation(&local_dt);

            return format!("{formatted} ({tz_abbr})");
        }
    }

    // If parsing fails, return original string
    utc_date_str.to_string()
}

/// What: Get timezone abbreviation (CEST, CET, PST, etc.) for a local datetime.
///
/// Inputs:
/// - `local_dt`: Local datetime to get timezone for.
///
/// Output:
/// - Timezone abbreviation string (e.g., "CEST", "CET", "UTC+2").
///
/// Details:
/// - First tries chrono's %Z format specifier
/// - Falls back to TZ environment variable parsing
/// - Finally falls back to UTC offset format
fn get_timezone_abbreviation(local_dt: &chrono::DateTime<chrono::Local>) -> String {
    // Try chrono's %Z format specifier first
    let tz_from_format = local_dt.format("%Z").to_string();

    // Check if %Z gave us a valid abbreviation (3-6 chars, alphabetic)
    if !tz_from_format.is_empty()
        && tz_from_format.len() >= 3
        && tz_from_format.len() <= 6
        && tz_from_format.chars().all(char::is_alphabetic)
        && !tz_from_format.starts_with("UTC")
    {
        return tz_from_format;
    }

    // Try to get timezone from TZ environment variable
    if let Ok(tz_env) = std::env::var("TZ") {
        // Extract timezone abbreviation from TZ variable
        // TZ can be like "Europe/Berlin" or "CEST-2" or just "CEST"
        if let Some(tz_name) = tz_env.rsplit('/').next() {
            // Check if it looks like a timezone abbreviation (3-6 uppercase letters)
            if tz_name.len() >= 3
                && tz_name.len() <= 6
                && tz_name.chars().all(|c| c.is_uppercase() || c == '-')
            {
                // Extract just the abbreviation part (before any offset)
                let abbr = tz_name.split('-').next().unwrap_or(tz_name);
                if abbr.len() >= 3 && abbr.chars().all(char::is_alphabetic) {
                    return abbr.to_string();
                }
            }
        }
    }

    // Fallback: Try to determine timezone abbreviation from offset and date
    let offset_secs = local_dt.offset().local_minus_utc();
    let hours = offset_secs / 3600;
    let minutes = (offset_secs.abs() % 3600) / 60;

    // Try to get timezone abbreviation from common mappings based on offset
    if let Some(tz_abbr) = get_tz_abbr_from_offset(hours, local_dt.date_naive()) {
        return tz_abbr;
    }

    // Final fallback: Use UTC offset format
    if offset_secs == 0 {
        "UTC".to_string()
    } else if minutes == 0 {
        format!("UTC{hours:+}")
    } else {
        format!("UTC{hours:+}:{minutes:02}")
    }
}

/// What: Get timezone abbreviation from UTC offset and date (for DST-aware abbreviations).
///
/// Inputs:
/// - `offset_hours`: UTC offset in hours (e.g., 1, 2, -5).
/// - `date`: Date to determine if DST is active.
///
/// Output:
/// - `Some(String)` with timezone abbreviation if known; `None` otherwise.
///
/// Details:
/// - Maps common UTC offsets to timezone abbreviations (CET/CEST, EST/EDT, etc.)
/// - Considers DST based on the date (roughly March-October for Northern Hemisphere)
fn get_tz_abbr_from_offset(offset_hours: i32, date: chrono::NaiveDate) -> Option<String> {
    let month = date.month();
    // Rough DST period: March (3) to October (10) for Northern Hemisphere
    let is_dst = (3..=10).contains(&month);

    match offset_hours {
        1 => {
            // Central European Time (winter) - UTC+1
            Some("CET".to_string()) // Central European Time
        }
        2 => {
            // Central European Summer Time (summer) - UTC+2
            // Could also be EET (Eastern European Time) in winter, but CEST is more common
            if is_dst {
                Some("CEST".to_string()) // Central European Summer Time
            } else {
                Some("EET".to_string()) // Eastern European Time (winter)
            }
        }
        -5 => {
            // Eastern Time (US)
            if is_dst {
                Some("EDT".to_string()) // Eastern Daylight Time
            } else {
                Some("EST".to_string()) // Eastern Standard Time
            }
        }
        -6 => {
            // Central Time (US)
            if is_dst {
                Some("CDT".to_string()) // Central Daylight Time
            } else {
                Some("CST".to_string()) // Central Standard Time
            }
        }
        -7 => {
            // Mountain Time (US)
            if is_dst {
                Some("MDT".to_string()) // Mountain Daylight Time
            } else {
                Some("MST".to_string()) // Mountain Standard Time
            }
        }
        -8 => {
            // Pacific Time (US)
            if is_dst {
                Some("PDT".to_string()) // Pacific Daylight Time
            } else {
                Some("PST".to_string()) // Pacific Standard Time
            }
        }
        0 => Some("UTC".to_string()),
        _ => None, // Unknown offset, return None to use UTC offset format
    }
}

/// What: Parse a date string to Unix timestamp.
///
/// Inputs:
/// - `date_str`: Date string from AUR page (e.g., "2025-05-15 03:55 (UTC)").
///
/// Output:
/// - `Some(i64)` with Unix timestamp if parsing succeeds; `None` otherwise.
///
/// Details:
/// - Attempts to parse common AUR date formats
/// - AUR uses format: "YYYY-MM-DD HH:MM (TZ)" where TZ is timezone abbreviation
/// - Returns None if parsing fails (will use string comparison for sorting)
fn parse_date_to_timestamp(date_str: &str) -> Option<i64> {
    let date_str = date_str.trim();

    // AUR format: "YYYY-MM-DD HH:MM (UTC)" or "YYYY-MM-DD HH:MM (CEST)" etc.
    // Try to parse the date/time part before the timezone
    if let Some(tz_start) = date_str.rfind('(') {
        let date_time_part = date_str[..tz_start].trim();

        // Try parsing "YYYY-MM-DD HH:MM" format
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_time_part, "%Y-%m-%d %H:%M") {
            // AUR dates are in UTC, so we can treat them as UTC
            return dt.and_utc().timestamp().into();
        }
    }

    // Try ISO 8601-like format: "YYYY-MM-DD HH:MM:SS"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S") {
        return dt.and_utc().timestamp().into();
    }

    // Try date-only format: "YYYY-MM-DD"
    if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        && let Some(dt) = d.and_hms_opt(0, 0, 0)
    {
        return dt.and_utc().timestamp().into();
    }

    // Try RFC 2822 format
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date_str) {
        return Some(dt.timestamp());
    }

    // Try RFC 3339 format
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.timestamp());
    }

    None
}
