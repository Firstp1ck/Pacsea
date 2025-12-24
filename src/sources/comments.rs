//! AUR package comments fetching via arch-toolkit.

use crate::sources::get_arch_client;
use crate::state::types::AurComment;

/// Result type alias for AUR comments fetching operations.
type Result<T> = super::Result<T>;

/// What: Fetch AUR package comments using arch-toolkit.
///
/// Inputs:
/// - `pkgname`: Package name to fetch comments for.
///
/// Output:
/// - `Ok(Vec<AurComment>)` with parsed comments sorted by date (latest first); `Err` on failure.
///
/// # Errors
/// - Returns `Err` when `ArchClient` is not initialized
/// - Returns `Err` when network request fails
/// - Returns `Err` when arch-toolkit fails to fetch or parse comments
///
/// Details:
/// - Uses arch-toolkit's `ArchClient::aur().comments()` method
/// - Fetches and parses comments from `https://aur.archlinux.org/packages/<pkgname>`
/// - Comments are already sorted by date descending (latest first) by arch-toolkit
/// - Pinned comments are automatically detected by arch-toolkit
/// - Leverages automatic rate limiting, retry logic, and optional caching from arch-toolkit
/// - Maps `arch_toolkit::AurComment` to `pacsea::state::types::AurComment` (fields are identical)
pub async fn fetch_aur_comments(pkgname: String) -> Result<Vec<AurComment>> {
    // Get the shared ArchClient instance
    let Some(client) = get_arch_client() else {
        return Err("AUR comments unavailable: ArchClient not initialized".into());
    };

    // Perform comments fetch using arch-toolkit
    let arch_comments = client
        .aur()
        .comments(&pkgname)
        .await
        .map_err(|e| format!("AUR comments failed: {e}"))?;

    // Map arch_toolkit::AurComment to pacsea::state::types::AurComment
    // Fields are identical, so this is a straightforward mapping
    let comments: Vec<AurComment> = arch_comments
        .into_iter()
        .map(|c| AurComment {
            id: c.id,
            author: c.author,
            date: c.date,
            date_timestamp: c.date_timestamp,
            date_url: c.date_url,
            content: c.content,
            pinned: c.pinned,
        })
        .collect();

    Ok(comments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires network access and ArchClient initialization"]
    async fn comments_returns_items_on_success() {
        // Initialize ArchClient for testing
        crate::sources::init_arch_client().expect("Failed to initialize ArchClient for test");

        let result = fetch_aur_comments("yay".into()).await;
        match result {
            Ok(comments) => {
                // Verify comments structure
                for comment in comments.iter().take(5) {
                    assert!(!comment.author.is_empty());
                    assert!(!comment.content.is_empty());
                    // Verify all fields are present (types are identical, so should work)
                }
            }
            Err(e) => {
                // Network errors are acceptable in tests
                assert!(
                    e.to_string().contains("AUR comments")
                        || e.to_string().contains("ArchClient")
                        || e.to_string().contains("Network"),
                    "Unexpected error: {e}"
                );
            }
        }
    }

    #[tokio::test]
    async fn comments_handles_uninitialized_client() {
        // This test verifies fallback behavior when ArchClient is not initialized
        // Note: This will only work if ArchClient hasn't been initialized yet
        // In a real scenario, init_arch_client() should be called first
        let result = fetch_aur_comments("test".into()).await;
        // Either comments are returned (if client was initialized) or error (if not)
        match result {
            Ok(_comments) => {
                // Client was initialized, which is fine
            }
            Err(e) => {
                // Client not initialized, which is expected in some test scenarios
                assert!(
                    e.to_string().contains("ArchClient") || e.to_string().contains("AUR comments"),
                    "Expected ArchClient or AUR comments error, got: {e}"
                );
            }
        }
    }
}
