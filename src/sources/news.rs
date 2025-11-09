use crate::state::NewsItem;

type Result<T> = super::Result<T>;

/// What: Fetch recent Arch Linux news items.
///
/// Input: `limit` maximum number of items to return (best-effort)
/// Output: `Ok(Vec<NewsItem>)` with date/title/url; `Err` on network or parse failures
///
/// Details: Downloads the Arch Linux news RSS feed and iteratively parses `<item>` blocks,
/// extracting `<title>`, `<link>`, and `<pubDate>`. The `pubDate` value is normalized to a
/// date-only form via `strip_time_and_tz`.
pub async fn fetch_arch_news(limit: usize) -> Result<Vec<NewsItem>> {
    let url = "https://archlinux.org/feeds/news/";
    let body = tokio::task::spawn_blocking(move || super::curl_text(url)).await??;
    let mut items: Vec<NewsItem> = Vec::new();
    let mut pos = 0;
    while items.len() < limit {
        if let Some(start) = body[pos..].find("<item>") {
            let s = pos + start;
            let end = body[s..]
                .find("</item>")
                .map(|e| s + e + 7)
                .unwrap_or(body.len());
            let chunk = &body[s..end];
            let title = extract_between(chunk, "<title>", "</title>").unwrap_or_default();
            let link = extract_between(chunk, "<link>", "</link>").unwrap_or_default();
            let raw_date = extract_between(chunk, "<pubDate>", "</pubDate>")
                .map(|d| d.trim().to_string())
                .unwrap_or_default();
            let date = strip_time_and_tz(&raw_date);
            items.push(NewsItem {
                date,
                title,
                url: link,
            });
            pos = end;
        } else {
            break;
        }
    }
    Ok(items)
}

/// What: Return the substring strictly between `start` and `end` markers (if present).
///
/// Input: `s` source text; `start` opening marker; `end` closing marker
/// Output: `Some(String)` of enclosed content; `None` if markers are missing
///
/// Details: Searches for the first occurrence of `start`, then the next occurrence of `end`
/// after it; returns the interior substring when both are found in order.
fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
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
fn strip_time_and_tz(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    #[test]
    /// What: Validate HTML substring extraction and time-stripping helpers used by news parsing.
    ///
    /// Inputs:
    /// - Sample tags `"<a>hi</a>"`, non-matching input, and date strings with optional time and timezone components.
    ///
    /// Output:
    /// - `extract_between` returns the inner text when delimiters exist and `None` otherwise; `strip_time_and_tz` removes trailing time/zone portions.
    ///
    /// Details:
    /// - Combines assertions into one test to keep helper coverage concise while guarding string-manipulation edge cases.
    fn news_extract_between_and_strip_time_tz() {
        // extract_between
        assert_eq!(
            super::extract_between("<a>hi</a>", "<a>", "</a>").unwrap(),
            "hi"
        );
        assert!(super::extract_between("nope", "<a>", "</a>").is_none());
        // strip_time_and_tz
        assert_eq!(
            super::strip_time_and_tz("Mon, 23 Oct 2023 12:34:56 +0000"),
            "Mon, 23 Oct 2023"
        );
        assert_eq!(
            super::strip_time_and_tz("Mon, 23 Oct 2023 12:34:56"),
            "Mon, 23 Oct 2023"
        );
        assert_eq!(
            super::strip_time_and_tz("Mon, 23 Oct 2023"),
            "Mon, 23 Oct 2023"
        );
    }
}
