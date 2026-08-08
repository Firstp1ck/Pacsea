//! Arch news and security advisory adapters.

use crate::state::NewsItem;
use crate::state::types::{AdvisorySeverity, NewsFeedItem, NewsFeedSource};

use super::ToolkitContext;

/// What: Fetch bounded Arch news through arch-toolkit.
///
/// Inputs:
/// - `context`: Shared caller-owned HTTP client policy.
/// - `limit`: Maximum returned rows.
/// - `cutoff_date`: Optional oldest accepted `YYYY-MM-DD` date.
///
/// Output:
/// - Pacsea news rows or an actionable error.
///
/// Details:
/// - arch-toolkit enforces a 512 KiB response ceiling and normalized dates.
pub async fn fetch_arch_news(
    context: &ToolkitContext,
    limit: usize,
    cutoff_date: Option<&str>,
) -> Result<Vec<NewsItem>, String> {
    arch_toolkit::news::fetch_arch_news(context.http_client(), limit, cutoff_date)
        .await
        .map(|items| {
            items
                .into_iter()
                .map(|item| NewsItem {
                    date: item.date,
                    title: item.title,
                    url: item.url,
                })
                .collect()
        })
        .map_err(|error| format!("Arch news unavailable: {error}; check the network and retry"))
}

/// What: Fetch bounded security advisories through arch-toolkit.
///
/// Inputs:
/// - `context`: Shared caller-owned HTTP client policy.
/// - `limit`: Maximum returned rows.
/// - `cutoff_date`: Optional oldest accepted `YYYY-MM-DD` date.
///
/// Output:
/// - Pacsea feed rows or an actionable error.
///
/// Details:
/// - Stable advisory IDs, severity, packages, and 512 KiB response bounds come from toolkit.
pub async fn fetch_advisories(
    context: &ToolkitContext,
    limit: usize,
    cutoff_date: Option<&str>,
) -> Result<Vec<NewsFeedItem>, String> {
    arch_toolkit::news::fetch_security_advisories(context.http_client(), limit, cutoff_date)
        .await
        .map(|items| items.into_iter().map(advisory).collect())
        .map_err(|error| {
            format!("security advisories unavailable: {error}; check the network and retry")
        })
}

/// What: Extract bounded article text through arch-toolkit's pure parser.
///
/// Inputs:
/// - `html`: Already fetched article HTML.
/// - `base_url`: Validated article URL used to resolve relative links.
///
/// Output:
/// - Extracted text or an actionable parse error.
///
/// Details:
/// - Fetching, caching, conditional requests, package metadata decoration, and presentation stay in Pacsea.
pub fn extract_article_text(html: &str, base_url: &str) -> Result<String, String> {
    arch_toolkit::news::extract_article_text(html, base_url)
        .map_err(|error| format!("article parsing failed: {error}"))
}

/// What: Convert one toolkit advisory into Pacsea feed state.
///
/// Inputs:
/// - `value`: Toolkit advisory.
///
/// Output:
/// - Equivalent Pacsea news feed item.
///
/// Details:
/// - Source identity remains `SecurityAdvisory` for read-state and filtering compatibility.
fn advisory(value: arch_toolkit::SecurityAdvisory) -> NewsFeedItem {
    NewsFeedItem {
        id: value.id,
        date: value.date,
        title: value.title,
        summary: value.summary,
        url: value.url,
        source: NewsFeedSource::SecurityAdvisory,
        severity: Some(severity(value.severity)),
        packages: value.packages,
    }
}

/// What: Convert toolkit advisory severity into Pacsea state.
///
/// Inputs:
/// - `value`: Toolkit severity.
///
/// Output:
/// - Equivalent Pacsea severity.
///
/// Details:
/// - Rank ordering maps one-to-one.
const fn severity(value: arch_toolkit::AdvisorySeverity) -> AdvisorySeverity {
    match value {
        arch_toolkit::AdvisorySeverity::Unknown => AdvisorySeverity::Unknown,
        arch_toolkit::AdvisorySeverity::Low => AdvisorySeverity::Low,
        arch_toolkit::AdvisorySeverity::Medium => AdvisorySeverity::Medium,
        arch_toolkit::AdvisorySeverity::High => AdvisorySeverity::High,
        arch_toolkit::AdvisorySeverity::Critical => AdvisorySeverity::Critical,
    }
}

#[cfg(test)]
mod tests {
    /// What: Verify toolkit RSS parsing preserves entity decoding, dates, and cutoffs.
    ///
    /// Inputs:
    /// - Deterministic two-item RSS fixture.
    ///
    /// Output:
    /// - One decoded row after cutoff filtering.
    ///
    /// Details:
    /// - No network request is performed.
    #[test]
    fn news_fixture_preserves_cutoff_and_entities() {
        let fixture = r"<rss><channel>
<item><title>Package &amp; repo</title><link>https://archlinux.org/news/new/</link><pubDate>Thu, 21 Aug 2025 12:34:56 +0000</pubDate></item>
<item><title>Old</title><link>https://archlinux.org/news/old/</link><pubDate>Thu, 20 Aug 2020 12:00:00 +0000</pubDate></item>
</channel></rss>";
        let items = arch_toolkit::news::parse_arch_news_rss(fixture, 10, Some("2025-01-01"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Package & repo");
        assert_eq!(items[0].date, "2025-08-21");
    }

    /// What: Verify toolkit advisory conversion preserves identity and package metadata.
    ///
    /// Inputs:
    /// - Deterministic toolkit advisory.
    ///
    /// Output:
    /// - Pacsea feed row with matching ID, severity, package, and source.
    ///
    /// Details:
    /// - Protects persisted read-state identifiers.
    #[test]
    fn advisory_conversion_preserves_identity() {
        let item = super::advisory(arch_toolkit::SecurityAdvisory {
            id: "ASA-1".to_string(),
            date: "2026-01-01".to_string(),
            title: "openssl issue".to_string(),
            summary: None,
            url: Some("https://security.archlinux.org/ASA-1".to_string()),
            severity: arch_toolkit::AdvisorySeverity::High,
            packages: vec!["openssl".to_string()],
        });
        assert_eq!(item.id, "ASA-1");
        assert_eq!(item.packages, vec!["openssl"]);
        assert_eq!(
            item.severity,
            Some(crate::state::types::AdvisorySeverity::High)
        );
        assert_eq!(
            item.source,
            crate::state::types::NewsFeedSource::SecurityAdvisory
        );
    }
}
