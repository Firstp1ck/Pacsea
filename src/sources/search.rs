//! AUR search query execution and result parsing.

use crate::integrations::arch_toolkit::ToolkitContext;
use crate::state::PackageItem;

/// What: Fetch search results from AUR and return items along with any error messages.
///
/// Input:
/// - `query` raw query string to search
///
/// Output:
/// - Tuple `(items, errors)` where `items` are `PackageItem`s found and `errors` are human-readable messages for partial failures
///
/// Details:
/// - Constructs a standalone toolkit context for CLI/non-runtime callers; the TUI uses
///   [`fetch_all_with_context`] so workers share one configured client.
pub async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>) {
    let context = match ToolkitContext::new() {
        Ok(context) => context,
        Err(error) => return (Vec::new(), vec![error.to_string()]),
    };
    fetch_all_with_context(&context, query).await
}

/// What: Fetch AUR search results with a caller-owned shared toolkit context.
///
/// Inputs:
/// - `context`: Runtime integration context.
/// - `query`: Raw query string.
///
/// Output:
/// - Pacsea package rows and nonfatal user-facing errors.
///
/// Details:
/// - arch-toolkit owns bounded transport and parsing; Pacsea retains the 200-row cap and UI model.
pub async fn fetch_all_with_context(
    context: &ToolkitContext,
    query: String,
) -> (Vec<PackageItem>, Vec<String>) {
    match crate::integrations::arch_toolkit::aur::search(context, &query).await {
        Ok(items) => (items, Vec::new()),
        Err(error) => (Vec::new(), vec![error]),
    }
}

#[cfg(test)]
mod tests {
    /// What: Verify an invalid search query is rejected without returning rows.
    ///
    /// Inputs:
    /// - An empty query and a standalone toolkit context.
    ///
    /// Output:
    /// - No rows and one actionable nonfatal error.
    ///
    /// Details:
    /// - Validation occurs before any network request.
    #[tokio::test]
    async fn invalid_search_query_returns_nonfatal_error() {
        let context = crate::integrations::arch_toolkit::ToolkitContext::new()
            .expect("toolkit context should construct");
        let (items, errors) = super::fetch_all_with_context(&context, String::new()).await;
        assert!(items.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("AUR search unavailable"));
    }
}
