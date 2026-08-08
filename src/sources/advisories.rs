//! security.archlinux.org advisories fetched through arch-toolkit.

use crate::integrations::arch_toolkit::ToolkitContext;
use crate::state::types::NewsFeedItem;

/// Result type alias for advisory fetching operations.
type Result<T> = super::Result<T>;

/// What: Fetch security advisories through arch-toolkit.
///
/// Inputs:
/// - `limit`: Maximum advisories to return.
/// - `cutoff_date`: Optional oldest accepted `YYYY-MM-DD` date.
///
/// Output:
/// - Pacsea feed rows or an actionable network/parse error.
///
/// # Errors
///
/// Returns an error when client construction, transport, response bounds, or feed parsing fails.
///
/// Details:
/// - Standalone callers construct a configured context; runtime aggregation can share one through
///   [`fetch_security_advisories_with_context`]. The feed response is bounded to 512 KiB.
pub async fn fetch_security_advisories(
    limit: usize,
    cutoff_date: Option<&str>,
) -> Result<Vec<NewsFeedItem>> {
    let context = ToolkitContext::new()?;
    fetch_security_advisories_with_context(&context, limit, cutoff_date).await
}

/// What: Fetch advisories with a caller-owned shared toolkit context.
///
/// Inputs:
/// - `context`: Runtime integration context.
/// - `limit`: Maximum advisories to return.
/// - `cutoff_date`: Optional oldest accepted date.
///
/// Output:
/// - Pacsea feed rows or an actionable error.
///
/// # Errors
///
/// Returns an error when bounded toolkit feed retrieval or parsing fails.
///
/// Details:
/// - Toolkit values are converted before reaching cache, filter, CLI, or UI surfaces.
pub async fn fetch_security_advisories_with_context(
    context: &ToolkitContext,
    limit: usize,
    cutoff_date: Option<&str>,
) -> Result<Vec<NewsFeedItem>> {
    crate::integrations::arch_toolkit::news::fetch_advisories(context, limit, cutoff_date)
        .await
        .map_err(Into::into)
}
