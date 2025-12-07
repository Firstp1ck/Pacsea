//! Arch Linux news fetching and parsing.

use crate::state::NewsItem;
use ego_tree::NodeRef;
use scraper::{Html, Node, Selector};
use tracing::{info, warn};

type Result<T> = super::Result<T>;

/// What: Fetch recent Arch Linux news items.
///
/// Input: `limit` maximum number of items to return (best-effort)
/// Output: `Ok(Vec<NewsItem>)` with date/title/url; `Err` on network or parse failures
///
/// # Errors
/// - Returns `Err` when network request fails (curl execution error)
/// - Returns `Err` when RSS feed cannot be fetched from Arch Linux website
/// - Returns `Err` when response body cannot be decoded as UTF-8
///
/// Details: Downloads the Arch Linux news RSS feed and iteratively parses `<item>` blocks,
/// extracting `<title>`, `<link>`, and `<pubDate>`. The `pubDate` value is normalized to a
/// date-only form via `strip_time_and_tz`.
pub async fn fetch_arch_news(limit: usize) -> Result<Vec<NewsItem>> {
    let url = "https://archlinux.org/feeds/news/";
    let body = tokio::task::spawn_blocking(move || crate::util::curl::curl_text(url))
        .await?
        .map_err(|e| {
            warn!(error = %e, "failed to fetch arch news feed");
            e
        })?;
    info!(bytes = body.len(), "fetched arch news feed");
    let mut items: Vec<NewsItem> = Vec::new();
    let mut pos = 0;
    while items.len() < limit {
        if let Some(start) = body[pos..].find("<item>") {
            let s = pos + start;
            let end = body[s..].find("</item>").map_or(body.len(), |e| s + e + 7);
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
    info!(count = items.len(), "parsed arch news feed");
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

/// What: Fetch the full article content from an Arch news URL.
///
/// Inputs:
/// - `url`: The news article URL (e.g., `https://archlinux.org/news/...`)
///
/// Output:
/// - `Ok(String)` with the article text content; `Err` on network/parse failure.
///
/// # Errors
/// - Network fetch failures
/// - HTML parsing failures
///
/// Details:
/// - Fetches the HTML page and extracts content from the article body.
/// - Strips HTML tags and normalizes whitespace.
pub async fn fetch_news_content(url: &str) -> Result<String> {
    let url_owned = url.to_string();
    let url_for_log = url_owned.clone();
    let body = tokio::task::spawn_blocking(move || crate::util::curl::curl_text(&url_owned))
        .await?
        .map_err(|e| {
            warn!(error = %e, url = %url_for_log, "failed to fetch news content");
            e
        })?;
    info!(url, bytes = body.len(), "fetched news page");

    // Extract article content from HTML
    let content = parse_arch_news_html(&body);
    let parsed_len = content.len();
    if parsed_len == 0 {
        warn!(url, "parsed news content is empty");
    } else {
        info!(url, parsed_len, "parsed news content");
    }
    Ok(content)
}

/// What: Parse Arch Linux news HTML and extract article text using `scraper`.
///
/// Inputs:
/// - `html`: Raw HTML content of the news page.
///
/// Output:
/// - Extracted article text with formatting preserved (paragraphs, bullets, code markers).
fn parse_arch_news_html(html: &str) -> String {
    let document = Html::parse_document(html);
    let selectors = [
        Selector::parse("div.article-content").ok(),
        Selector::parse("article").ok(),
    ];

    let mut buf = String::new();
    let mut found = false;
    for sel in selectors.iter().flatten() {
        if let Some(element) = document.select(sel).next()
            && let Some(node) = document.tree.get(element.id())
        {
            render_node(&mut buf, node, false);
            found = true;
            break;
        }
    }
    if !found && let Some(root) = document.tree.get(document.root_element().id()) {
        render_node(&mut buf, root, false);
    }

    prune_news_boilerplate(&buf)
}

/// What: Render a node (and children) into text while preserving basic formatting.
///
/// Inputs:
/// - `buf`: Output buffer to append text into
/// - `node`: Node to render
/// - `in_pre`: Whether we are inside a <pre> block (preserve whitespace)
fn render_node(buf: &mut String, node: NodeRef<Node>, in_pre: bool) {
    match node.value() {
        Node::Text(t) => push_text(buf, t.as_ref(), in_pre),
        Node::Element(el) => {
            let name = el.name();
            let is_block = matches!(
                name,
                "p" | "div" | "section" | "article" | "header" | "footer" | "main"
            );
            let is_list = matches!(name, "ul" | "ol");
            let is_li = name == "li";
            let is_br = name == "br";
            let is_pre_tag = name == "pre";
            let is_code = name == "code";

            if is_block && !buf.ends_with('\n') {
                buf.push('\n');
            }
            if is_li {
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
                buf.push_str("• ");
            }
            if is_br {
                buf.push('\n');
            }

            if is_code {
                let mut tmp = String::new();
                for child in node.children() {
                    render_node(&mut tmp, child, in_pre);
                }
                if !tmp.is_empty() {
                    if !buf.ends_with('`') {
                        buf.push('`');
                    }
                    buf.push_str(tmp.trim());
                    buf.push('`');
                }
                return;
            }

            if is_pre_tag {
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
                let mut tmp = String::new();
                for child in node.children() {
                    render_node(&mut tmp, child, true);
                }
                buf.push_str(tmp.trim_end());
                buf.push('\n');
                return;
            }

            let next_pre = in_pre;
            for child in node.children() {
                render_node(buf, child, next_pre);
            }

            if is_block || is_list || is_li {
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
                if !buf.ends_with("\n\n") {
                    buf.push('\n');
                }
            }
        }
        _ => {}
    }
}

/// What: Append text content to buffer, preserving whitespace when in <pre>, otherwise collapsing runs.
///
/// Inputs:
/// - `buf`: Output buffer to append into.
/// - `text`: Text content from the node.
/// - `in_pre`: Whether whitespace should be preserved (inside `<pre>`).
///
/// Output:
/// - Mutates `buf` with appended text respecting whitespace rules.
fn push_text(buf: &mut String, text: &str, in_pre: bool) {
    if in_pre {
        buf.push_str(text);
        return;
    }

    // Collapse consecutive whitespace to a single space, but keep newlines produced by block tags.
    let mut last_was_space = buf.ends_with(' ');
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                buf.push(' ');
                last_was_space = true;
            }
        } else {
            buf.push(ch);
            last_was_space = false;
        }
    }
}

/// What: Remove Arch news boilerplate (nav/header) from extracted text.
///
/// Inputs:
/// - `text`: Plain text extracted from the news HTML.
///
/// Output:
/// - Text with leading navigation/header lines removed, starting after the date line when found.
fn prune_news_boilerplate(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    // Find a date line like YYYY-MM-DD ...
    let date_idx = lines.iter().position(|l| {
        let t = l.trim();
        t.len() >= 10
            && t.as_bytes().get(4) == Some(&b'-')
            && t.as_bytes().get(7) == Some(&b'-')
            && t[..4].chars().all(|c| c.is_ascii_digit())
            && t[5..7].chars().all(|c| c.is_ascii_digit())
            && t[8..10].chars().all(|c| c.is_ascii_digit())
    });

    if let Some(idx) = date_idx {
        // Take everything after the date line
        let mut out: Vec<&str> = lines.iter().skip(idx + 1).map(|s| s.trim_end()).collect();
        // Drop leading empty lines
        while matches!(out.first(), Some(l) if l.trim().is_empty()) {
            out.remove(0);
        }
        // Drop footer/copyright block if present
        if let Some(c_idx) = out.iter().position(|l| l.contains("Copyright \u{00a9}")) {
            out.truncate(c_idx);
        }
        // Also drop known footer lines
        out.retain(|l| {
            let t = l.trim();
            !(t.starts_with("The Arch Linux name and logo")
                || t.starts_with("trademarks.")
                || t.starts_with("The registered trademark")
                || t.starts_with("Linux\u{00ae} is used")
                || t.starts_with("the exclusive licensee"))
        });
        return out.join("\n");
    }

    text.trim().to_string()
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
            super::extract_between("<a>hi</a>", "<a>", "</a>")
                .expect("extract_between should find 'hi' in test string"),
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
