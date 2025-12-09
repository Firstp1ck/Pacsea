//! security.archlinux.org advisory fetcher.
use crate::state::types::{AdvisorySeverity, NewsFeedItem, NewsFeedSource};
use tracing::{info, warn};

/// Result type alias for advisory fetching operations.
type Result<T> = super::Result<T>;

/// What: Fetch security advisories from security.archlinux.org and convert to feed items.
///
/// Inputs:
/// - `limit`: Maximum number of advisories to return (best-effort).
/// - `cutoff_date`: Optional date string (YYYY-MM-DD) for early filtering.
///
/// Output:
/// - `Ok(Vec<NewsFeedItem>)` on success; `Err` on network/parse failure.
///
/// Details:
/// - Uses the public JSON advisory feed.
/// - Normalizes severity strings and packages; skips entries without an ID.
/// - If `cutoff_date` is provided, stops fetching when items exceed the date limit.
///
/// # Errors
/// - Network fetch failures
/// - JSON parsing failures
pub async fn fetch_security_advisories(
    limit: usize,
    cutoff_date: Option<&str>,
) -> Result<Vec<NewsFeedItem>> {
    // Official advisory Atom feed
    let url = "https://security.archlinux.org/advisory/feed.atom";
    let resp = reqwest::get(url).await?;
    let status = resp.status();
    let body = resp.text().await?;
    info!(
        status = status.as_u16(),
        bytes = body.len(),
        "fetched advisories feed"
    );
    if !status.is_success() {
        let preview: String = body.chars().take(300).collect();
        warn!(
            status = status.as_u16(),
            preview = preview,
            "advisory feed returned non-success status"
        );
        return Err(format!("advisory feed status {status}").into());
    }

    let mut items = Vec::new();
    let mut pos = 0;
    while items.len() < limit {
        let Some(start) = body[pos..].find("<entry>") else {
            break;
        };
        let s = pos + start;
        let end = body[s..].find("</entry>").map_or(body.len(), |e| s + e + 8);
        let chunk = &body[s..end];

        let title = extract_between(chunk, "<title>", "</title>").unwrap_or_default();
        let link = extract_link_href(chunk).unwrap_or_default();
        let raw_date = extract_between(chunk, "<updated>", "</updated>")
            .or_else(|| extract_between(chunk, "<published>", "</published>"))
            .unwrap_or_default();
        let date = strip_time(&raw_date);
        // Early date filtering: stop if item is older than cutoff_date
        if let Some(cutoff) = cutoff_date
            && date.as_str() < cutoff
        {
            break;
        }
        let summary = extract_between(chunk, "<summary>", "</summary>");
        let id = if !link.is_empty() {
            link.clone()
        } else if !title.is_empty() {
            title.clone()
        } else {
            raw_date.clone()
        };

        items.push(NewsFeedItem {
            id,
            date,
            title: if title.is_empty() {
                "Advisory".to_string()
            } else {
                title
            },
            summary,
            url: if link.is_empty() { None } else { Some(link) },
            source: NewsFeedSource::SecurityAdvisory,
            severity: Some(AdvisorySeverity::Unknown),
            packages: Vec::new(),
        });
        pos = end;
    }
    info!(count = items.len(), "parsed advisories feed");
    Ok(items)
}

/// What: Normalize severity string into `AdvisorySeverity`.
///
/// Inputs:
/// - `severity`: Optional severity string from feed.
///
/// Output:
/// - Matching `AdvisorySeverity` variant (default Unknown).
fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(s[i..j].to_string())
}

/// What: Extract href attribute value from a link tag in HTML string.
///
/// Inputs:
/// - `s`: HTML string containing a link tag.
///
/// Output: Option containing the href value if found.
///
/// Details: Parses HTML to find link tag and extract href attribute value.
fn extract_link_href(s: &str) -> Option<String> {
    // Look for link tag with href attribute
    let link_pos = s.find("<link")?;
    let rest = &s[link_pos..];
    let href_pos = rest.find("href=\"")?;
    let after = &rest[href_pos + 6..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// What: Strip time component from ISO 8601 datetime string.
///
/// Inputs:
/// - `s`: ISO 8601 datetime string (e.g., "2024-01-01T12:00:00").
///
/// Output: Date string without time component.
///
/// Details: Splits on 'T' and returns only the date part.
fn strip_time(s: &str) -> String {
    s.split('T').next().unwrap_or(s).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{extract_between, strip_time};

    #[test]
    fn extract_and_strip_helpers() {
        assert_eq!(
            extract_between("<a>hi</a>", "<a>", "</a>").as_deref(),
            Some("hi")
        );
        assert_eq!(extract_between("nope", "<a>", "</a>"), None);
        assert_eq!(strip_time("2025-12-07T14:00:00Z"), "2025-12-07");
        assert_eq!(strip_time("2025-12-07"), "2025-12-07");
    }
}
