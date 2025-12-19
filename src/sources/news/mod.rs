//! Arch Linux news fetching and parsing.

mod aur;
mod cache;
mod fetch;
mod parse;
mod utils;

/// Result type alias for Arch Linux news fetching operations.
type Result<T> = super::Result<T>;

pub use fetch::{fetch_arch_news, fetch_news_content};
pub use parse::parse_arch_news_html;

/// What: Parse raw news/advisory HTML into displayable text (public helper).
///
/// Inputs:
/// - `html`: Raw HTML source to parse.
///
/// Output:
/// - Plaintext content suitable for the details view.
#[must_use]
pub fn parse_news_html(html: &str) -> String {
    parse_arch_news_html(html, None)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_aur;
