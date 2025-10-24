use crate::state::NewsItem;

type Result<T> = super::Result<T>;

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

fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(s[i..j].to_string())
}

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
