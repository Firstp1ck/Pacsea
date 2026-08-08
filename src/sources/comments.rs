//! AUR package comments fetched through arch-toolkit.

use crate::integrations::arch_toolkit::ToolkitContext;
use crate::state::types::AurComment;

/// Result type alias for AUR comments fetching operations.
type Result<T> = super::Result<T>;

/// What: Fetch AUR package comments through arch-toolkit.
///
/// Inputs:
/// - `pkgname`: Package name to fetch comments for.
///
/// Output:
/// - Parsed comments with stable IDs, timestamps, links, formatting, and pinned status.
///
/// # Errors
///
/// Returns an error when client construction, bounded retrieval, validation, or parsing fails.
///
/// Details:
/// - Standalone callers construct a configured context; runtime workers share one context through
///   [`fetch_aur_comments_with_context`]. arch-toolkit bounds the streamed response to 10 MiB.
pub async fn fetch_aur_comments(pkgname: String) -> Result<Vec<AurComment>> {
    let context = ToolkitContext::new()?;
    fetch_aur_comments_with_context(&context, pkgname).await
}

/// What: Fetch AUR comments with a caller-owned shared toolkit context.
///
/// Inputs:
/// - `context`: Runtime integration context.
/// - `pkgname`: Package name to fetch.
///
/// Output:
/// - Pacsea comments or an actionable error.
///
/// # Errors
///
/// Returns an error when bounded toolkit retrieval or comment parsing fails.
///
/// Details:
/// - Toolkit values are converted inside the anti-corruption layer.
pub async fn fetch_aur_comments_with_context(
    context: &ToolkitContext,
    pkgname: String,
) -> Result<Vec<AurComment>> {
    crate::integrations::arch_toolkit::aur::comments(context, &pkgname)
        .await
        .map_err(Into::into)
}
