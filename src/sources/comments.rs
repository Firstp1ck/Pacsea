//! AUR package comments fetching via web scraping.

use scraper::{ElementRef, Html, Selector};
use std::time::Duration;
use tracing::debug;

use crate::state::types::AurComment;

/// Result type alias for AUR comments fetching operations.
type Result<T> = super::Result<T>;

/// Context for extracting comment data from HTML elements.
struct CommentExtractionContext<'a> {
    /// Parsed HTML document
    document: &'a Html,
    /// Selector for date elements
    date_selector: &'a Selector,
    /// Package name for URL construction
    pkgname: &'a str,
    /// Full HTML text for pinned detection
    html_text: &'a str,
    /// Whether pinned section exists
    has_pinned_section: bool,
    /// Position of "Latest Comments" heading
    latest_comments_pos: Option<usize>,
}

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

    // Create HTTP client with short timeout (500ms) to fail fast on slow connections
    // This prioritizes responsiveness over completeness - missed comments will be caught next fetch
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
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

        // Extract comment data from header
        let context = CommentExtractionContext {
            document: &document,
            date_selector: &date_selector,
            pkgname: &pkgname,
            html_text: &html_text,
            has_pinned_section,
            latest_comments_pos,
        };
        if let Some(comment) = extract_comment_from_header(header, comment_id, index, &context) {
            comments.push(comment);
        }
    }

    // Separate, sort, and combine comments
    Ok(separate_and_sort_comments(comments))
}

/// What: Extract comment data from a header element.
///
/// Inputs:
/// - `header`: Header element containing comment metadata
/// - `comment_id`: Optional comment ID from header attribute
/// - `index`: Index of header in collection
/// - `context`: Extraction context with document, selectors, and metadata
///
/// Output:
/// - `Some(AurComment)` if comment is valid; `None` if empty/invalid
///
/// Details:
/// - Extracts author, date, URL, content, and pinned status
/// - Skips empty comments with unknown authors
fn extract_comment_from_header(
    header: &ElementRef,
    comment_id: Option<&str>,
    index: usize,
    context: &CommentExtractionContext,
) -> Option<AurComment> {
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
    let base_url = format!("https://aur.archlinux.org/packages/{}", context.pkgname);
    let (date_text, date_url) = header.select(context.date_selector).next().map_or_else(
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
    // We extract formatted text to preserve markdown-like structures
    let comment_content = comment_id
        .and_then(|id| id.strip_prefix("comment-"))
        .and_then(|comment_id_str| {
            Selector::parse(&format!("div#comment-{comment_id_str}-content")).ok()
        })
        .and_then(|content_id_selector| context.document.select(&content_id_selector).next())
        .map_or_else(String::new, |div| {
            // Parse HTML and extract formatted text
            // This preserves markdown-like structures (bold, italic, code, links, etc.)
            html_to_formatted_text(div)
        });

    // Skip empty comments
    if comment_content.is_empty() && author == "Unknown" {
        return None;
    }

    // Parse date to timestamp
    let date_timestamp = parse_date_to_timestamp(&date_text);
    if date_timestamp.is_none() && !date_text.is_empty() {
        debug!(
            pkgname = %context.pkgname,
            author = %author,
            date_text = %date_text,
            "Failed to parse comment date to timestamp"
        );
    }

    // Convert UTC date to local timezone for display
    let local_date = convert_utc_to_local_date(&date_text);

    // Determine if this comment is pinned
    let is_pinned = determine_pinned_status(comment_id, index, context);

    let stable_id = comment_id.map(str::to_string).or_else(|| date_url.clone());
    Some(AurComment {
        id: stable_id,
        author,
        date: local_date,
        date_timestamp,
        date_url,
        content: comment_content,
        pinned: is_pinned,
    })
}

/// What: Determine if a comment is pinned based on its position in the HTML.
///
/// Inputs:
/// - `comment_id`: Optional comment ID
/// - `index`: Index of comment in collection
/// - `context`: Extraction context with HTML text and pinned section info
///
/// Output:
/// - `true` if comment is pinned; `false` otherwise
///
/// Details:
/// - Pinned comments appear before the "Latest Comments" heading
/// - Uses comment position in HTML relative to "Latest Comments" heading
fn determine_pinned_status(
    comment_id: Option<&str>,
    index: usize,
    context: &CommentExtractionContext,
) -> bool {
    if !context.has_pinned_section {
        return false;
    }

    let Some(latest_pos) = context.latest_comments_pos else {
        return false;
    };

    comment_id.map_or(index < 10, |id| {
        context
            .html_text
            .find(id)
            .map_or(index < 10, |comment_pos| comment_pos < latest_pos)
    })
}

/// What: Separate pinned and regular comments, sort them, and combine.
///
/// Inputs:
/// - `comments`: Vector of all comments
///
/// Output:
/// - Vector with pinned comments first, then regular, both sorted by date descending
///
/// Details:
/// - Separates comments into pinned and regular
/// - Sorts each group by date descending (latest first)
/// - Combines with pinned first
fn separate_and_sort_comments(comments: Vec<AurComment>) -> Vec<AurComment> {
    // Separate pinned and regular comments
    let mut pinned_comments: Vec<AurComment> =
        comments.iter().filter(|c| c.pinned).cloned().collect();
    let mut regular_comments: Vec<AurComment> =
        comments.into_iter().filter(|c| !c.pinned).collect();

    // Sort both groups by date descending
    sort_comments_by_date(&mut pinned_comments);
    sort_comments_by_date(&mut regular_comments);

    // Combine: pinned first, then regular
    pinned_comments.extend(regular_comments);
    pinned_comments
}

/// What: Sort comments by date descending (latest first).
///
/// Inputs:
/// - `comments`: Mutable reference to comments vector to sort
///
/// Output:
/// - Comments are sorted in-place by date descending
///
/// Details:
/// - Uses timestamp for sorting if available
/// - Falls back to string comparison if timestamp is missing
fn sort_comments_by_date(comments: &mut [AurComment]) {
    comments.sort_by(|a, b| {
        match (a.date_timestamp, b.date_timestamp) {
            (Some(ts_a), Some(ts_b)) => ts_b.cmp(&ts_a), // Descending order
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.date.cmp(&a.date), // Fallback to string comparison
        }
    });
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

/// What: Get timezone abbreviation from UTC offset and date.
///
/// Inputs:
/// - `offset_hours`: UTC offset in hours (e.g., 1, 2, -5).
/// - `date`: Date (unused, kept for API compatibility).
///
/// Output:
/// - `Some(String)` with timezone abbreviation if unambiguous; `None` otherwise.
///
/// Details:
/// - Returns `None` for DST-affected timezones to avoid incorrect abbreviations
/// - DST transition dates vary by year and region (e.g., US: second Sunday in March, first Sunday in November)
/// - Month-based DST detection is inaccurate and can show wrong abbreviations near transitions
/// - When `None` is returned, the caller falls back to UTC offset format (e.g., "UTC-5")
/// - Only returns `Some` for unambiguous timezones like UTC
fn get_tz_abbr_from_offset(offset_hours: i32, _date: chrono::NaiveDate) -> Option<String> {
    // Only return abbreviations for unambiguous timezones
    // For DST-affected timezones, return None to use UTC offset format instead
    // This avoids incorrect abbreviations near DST transition dates
    match offset_hours {
        0 => Some("UTC".to_string()),
        _ => None, // Return None for all other offsets to use UTC offset format
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
/// - Attempts to parse common AUR date formats and many other common formats
/// - AUR uses format: "YYYY-MM-DD HH:MM (TZ)" where TZ is timezone abbreviation
/// - Supports ISO 8601, RFC 2822, RFC 3339, and various date separator formats
/// - Returns None if parsing fails (will use string comparison for sorting)
/// - Logs debug information when parsing fails to help diagnose issues
fn parse_date_to_timestamp(date_str: &str) -> Option<i64> {
    let date_str = date_str.trim();

    // Skip empty strings early
    if date_str.is_empty() {
        debug!("Failed to parse empty date string");
        return None;
    }

    // AUR format: "YYYY-MM-DD HH:MM (UTC)" or "YYYY-MM-DD HH:MM (CEST)" etc.
    // Try to parse the date/time part before the timezone
    if let Some(tz_start) = date_str.rfind('(') {
        let date_time_part = date_str[..tz_start].trim();

        // Try parsing "YYYY-MM-DD HH:MM" format
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_time_part, "%Y-%m-%d %H:%M") {
            // AUR dates are in UTC, so we can treat them as UTC
            return dt.and_utc().timestamp().into();
        }

        // Try with seconds: "YYYY-MM-DD HH:MM:SS"
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_time_part, "%Y-%m-%d %H:%M:%S") {
            return dt.and_utc().timestamp().into();
        }
    }

    // Try ISO 8601-like format: "YYYY-MM-DD HH:MM:SS"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S") {
        return dt.and_utc().timestamp().into();
    }

    // Try ISO 8601 format: "YYYY-MM-DDTHH:MM:SS" (with T separator)
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S") {
        return dt.and_utc().timestamp().into();
    }

    // Try ISO 8601 with timezone: "YYYY-MM-DDTHH:MM:SSZ" or "YYYY-MM-DDTHH:MM:SS+HH:MM"
    if let Ok(dt) = chrono::DateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%z") {
        return Some(dt.timestamp());
    }

    // Try date-only format: "YYYY-MM-DD"
    if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        && let Some(dt) = d.and_hms_opt(0, 0, 0)
    {
        return dt.and_utc().timestamp().into();
    }

    // Try RFC 2822 format (e.g., "Mon, 15 May 2025 03:55:00 +0000")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date_str) {
        return Some(dt.timestamp());
    }

    // Try RFC 3339 format (e.g., "2025-05-15T03:55:00Z")
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.timestamp());
    }

    // Try formats with different separators: "YYYY/MM/DD HH:MM"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y/%m/%d %H:%M") {
        return dt.and_utc().timestamp().into();
    }

    // Try formats with different separators: "DD.MM.YYYY HH:MM"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%d.%m.%Y %H:%M") {
        return dt.and_utc().timestamp().into();
    }

    // Try formats with different separators: "MM/DD/YYYY HH:MM"
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%m/%d/%Y %H:%M") {
        return dt.and_utc().timestamp().into();
    }

    // Try Unix timestamp as string
    if let Ok(ts) = date_str.parse::<i64>() {
        // Validate it's a reasonable timestamp (between 2000 and 2100)
        if ts > 946_684_800 && ts < 4_102_444_800 {
            return Some(ts);
        }
    }

    // All parsing attempts failed - log for debugging
    debug!(
        date_str = %date_str,
        date_str_len = date_str.len(),
        "Failed to parse date string to timestamp"
    );
    None
}

/// What: Convert HTML content to formatted text preserving markdown-like structures.
///
/// Inputs:
/// - `element`: HTML element to parse
///
/// Output:
/// - Formatted text string with markdown-like syntax for bold, italic, code, etc.
///
/// Details:
/// - Converts HTML tags to markdown-like syntax:
///   - `<strong>`, `<b>` → `**text**`
///   - `<em>`, `<i>` → `*text*`
///   - `<code>` → `` `text` ``
///   - `<pre>` → preserves code blocks with triple backticks
///   - `<a>` → preserves links as `[text](url)`
///   - `<p>` → newlines between paragraphs
fn html_to_formatted_text(element: ElementRef) -> String {
    let mut result = String::new();

    // Process paragraphs to preserve structure
    let p_selector = Selector::parse("p").ok();
    if let Some(ref p_sel) = p_selector {
        let paragraphs: Vec<_> = element.select(p_sel).collect();
        if !paragraphs.is_empty() {
            for (i, para) in paragraphs.iter().enumerate() {
                if i > 0 {
                    result.push('\n');
                }
                result.push_str(&convert_element_to_markdown(para));
            }
            return result.trim().to_string();
        }
    }

    // If no paragraphs, process the whole element
    result = convert_element_to_markdown(&element);
    result.trim().to_string()
}

/// Convert an HTML element to markdown-like syntax by processing nested elements.
fn convert_element_to_markdown(element: &ElementRef) -> String {
    let html = element.html();
    let mut working_html = html;

    // Process <pre> blocks first (code blocks)
    let pre_selector = Selector::parse("pre").ok();
    if let Some(ref pre_sel) = pre_selector {
        for pre in element.select(pre_sel) {
            let text = pre.text().collect::<String>();
            let pre_html = pre.html();
            let replacement = format!("```\n{}\n```", text.trim());
            working_html = working_html.replace(&pre_html, &replacement);
        }
    }

    // Process <a> tags (links)
    let a_selector = Selector::parse("a").ok();
    if let Some(ref a_sel) = a_selector {
        for link in element.select(a_sel) {
            let text = link.text().collect::<String>().trim().to_string();
            if let Some(href) = link.value().attr("href") {
                let link_html = link.html();
                let replacement = format!("[{text}]({href})");
                working_html = working_html.replace(&link_html, &replacement);
            }
        }
    }

    // Process <strong> and <b> tags (bold)
    let strong_selector = Selector::parse("strong, b").ok();
    if let Some(ref strong_sel) = strong_selector {
        for bold in element.select(strong_sel) {
            let text = bold.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                let bold_html = bold.html();
                let replacement = format!("**{text}**");
                working_html = working_html.replace(&bold_html, &replacement);
            }
        }
    }

    // Process <em> and <i> tags (italic)
    let em_selector = Selector::parse("em, i").ok();
    if let Some(ref em_sel) = em_selector {
        for italic in element.select(em_sel) {
            let text = italic.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                let italic_html = italic.html();
                let replacement = format!("*{text}*");
                working_html = working_html.replace(&italic_html, &replacement);
            }
        }
    }

    // Process <code> tags
    let code_selector = Selector::parse("code").ok();
    if let Some(ref code_sel) = code_selector {
        for code in element.select(code_sel) {
            let text = code.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                let code_html = code.html();
                let replacement = format!("`{text}`");
                working_html = working_html.replace(&code_html, &replacement);
            }
        }
    }

    // Parse the modified HTML and extract text (this removes remaining HTML tags)
    let temp_doc = Html::parse_fragment(&working_html);
    let mut result = temp_doc.root_element().text().collect::<String>();

    // Decode HTML entities
    result = result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Test that DST-affected timezones return None to use UTC offset format.
    ///
    /// Inputs:
    /// - Various dates and offsets for DST-affected timezones
    ///
    /// Output:
    /// - Function should return None to fall back to UTC offset format
    ///
    /// Details:
    /// - DST transition dates vary by year and region
    /// - US DST: second Sunday in March to first Sunday in November
    /// - Month-based detection is inaccurate, so we use UTC offset format instead
    #[test]
    fn test_dst_affected_timezones_return_none() {
        // Test various dates that would be incorrectly handled by month-based DST detection
        let test_cases = vec![
            (
                chrono::NaiveDate::from_ymd_opt(2024, 3, 1).expect("valid test date"),
                -5,
            ), // Early March (before DST starts)
            (
                chrono::NaiveDate::from_ymd_opt(2024, 3, 15).expect("valid test date"),
                -5,
            ), // Mid March (after DST starts)
            (
                chrono::NaiveDate::from_ymd_opt(2024, 10, 31).expect("valid test date"),
                -5,
            ), // Late October (DST still active)
            (
                chrono::NaiveDate::from_ymd_opt(2024, 11, 4).expect("valid test date"),
                -5,
            ), // Early November (after DST ends)
            (
                chrono::NaiveDate::from_ymd_opt(2024, 11, 15).expect("valid test date"),
                -5,
            ), // Mid November (after DST ends)
            // Test other US timezones
            (
                chrono::NaiveDate::from_ymd_opt(2024, 3, 1).expect("valid test date"),
                -6,
            ), // Central Time
            (
                chrono::NaiveDate::from_ymd_opt(2024, 3, 1).expect("valid test date"),
                -7,
            ), // Mountain Time
            (
                chrono::NaiveDate::from_ymd_opt(2024, 3, 1).expect("valid test date"),
                -8,
            ), // Pacific Time
            // Test European timezones
            (
                chrono::NaiveDate::from_ymd_opt(2024, 3, 1).expect("valid test date"),
                1,
            ), // CET/CEST
            (
                chrono::NaiveDate::from_ymd_opt(2024, 3, 1).expect("valid test date"),
                2,
            ), // CEST/EET
        ];

        for (date, offset) in test_cases {
            let result = get_tz_abbr_from_offset(offset, date);
            // Should return None to use UTC offset format
            // This is safer than guessing DST based on month ranges
            assert!(
                result.is_none(),
                "Should return None for DST-affected timezones to use UTC offset format. Date: {date:?}, Offset: {offset}, Got: {result:?}"
            );
        }
    }

    /// What: Test that UTC (offset 0) returns the correct abbreviation.
    ///
    /// Inputs:
    /// - Offset: 0 (UTC)
    /// - Various dates
    ///
    /// Output:
    /// - Should return "UTC" since it's unambiguous
    ///
    /// Details:
    /// - UTC is not affected by DST, so it's safe to return the abbreviation
    #[test]
    fn test_utc_returns_abbreviation() {
        let test_dates = vec![
            chrono::NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid test date"),
            chrono::NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid test date"),
            chrono::NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid test date"),
        ];

        for date in test_dates {
            let result = get_tz_abbr_from_offset(0, date);
            assert_eq!(
                result,
                Some("UTC".to_string()),
                "UTC should always return 'UTC' abbreviation. Date: {date:?}, Got: {result:?}"
            );
        }
    }

    /// What: Test date parsing with various AUR date formats.
    ///
    /// Inputs:
    /// - Various date string formats that might come from AUR
    ///
    /// Output:
    /// - Should successfully parse valid AUR date formats
    ///
    /// Details:
    /// - Tests common AUR date formats including UTC+2 format
    #[test]
    fn test_parse_date_to_timestamp() {
        // Test standard AUR formats
        assert!(
            parse_date_to_timestamp("2025-04-14 11:52 (UTC)").is_some(),
            "Should parse UTC format"
        );
        assert!(
            parse_date_to_timestamp("2025-04-14 11:52 (CEST)").is_some(),
            "Should parse CEST format"
        );
        assert!(
            parse_date_to_timestamp("2025-04-14 11:52 (UTC+2)").is_some(),
            "Should parse UTC+2 format"
        );
        assert!(
            parse_date_to_timestamp("2024-12-01 10:00 (UTC)").is_some(),
            "Should parse December date"
        );

        // Test edge cases
        assert!(
            parse_date_to_timestamp("").is_none(),
            "Empty string should return None"
        );
        assert!(
            parse_date_to_timestamp("invalid date").is_none(),
            "Invalid date should return None"
        );

        // Test ISO 8601 formats
        assert!(
            parse_date_to_timestamp("2025-04-14 11:52:30").is_some(),
            "Should parse ISO 8601-like format with seconds"
        );
        assert!(
            parse_date_to_timestamp("2025-04-14T11:52:30").is_some(),
            "Should parse ISO 8601 format with T separator"
        );

        // Test date-only format
        assert!(
            parse_date_to_timestamp("2025-04-14").is_some(),
            "Should parse date-only format"
        );

        // Test alternative separator formats
        assert!(
            parse_date_to_timestamp("2025/04/14 11:52").is_some(),
            "Should parse format with / separators"
        );
        assert!(
            parse_date_to_timestamp("14.04.2025 11:52").is_some(),
            "Should parse DD.MM.YYYY format"
        );
        assert!(
            parse_date_to_timestamp("04/14/2025 11:52").is_some(),
            "Should parse MM/DD/YYYY format"
        );

        // Test Unix timestamp as string
        assert!(
            parse_date_to_timestamp("1735689600").is_some(),
            "Should parse Unix timestamp string"
        );

        // Verify the parsed timestamp is reasonable
        if let Some(ts) = parse_date_to_timestamp("2025-04-14 11:52 (UTC)") {
            // April 14, 2025 should be a valid future timestamp
            assert!(ts > 0, "Timestamp should be positive");
        }

        // Verify timestamps are consistent across formats
        let ts1 = parse_date_to_timestamp("2025-04-14 11:52 (UTC)");
        let ts2 = parse_date_to_timestamp("2025-04-14 11:52:00");
        assert_eq!(
            ts1, ts2,
            "Same date/time should produce same timestamp regardless of format"
        );
    }
}
