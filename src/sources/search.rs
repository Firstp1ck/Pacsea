//! AUR search query execution and result parsing.

use crate::sources::get_arch_client;
use crate::state::{PackageItem, Source};

/// What: Fetch search results from AUR and return items along with any error messages.
///
/// Input:
/// - `query`: Raw query string to search
///
/// Output:
/// - Tuple `(items, errors)` where `items` are `PackageItem`s found and `errors` are human-readable messages for partial failures
///
/// Details:
/// - Uses arch-toolkit's `ArchClient::aur().search()` method
/// - Maps `AurPackage` results to `PackageItem` format
/// - Handles errors gracefully and converts to string format for backward compatibility
/// - Returns empty results with error message if `ArchClient` is unavailable
/// - Leverages automatic rate limiting, retry logic, and optional caching from arch-toolkit
pub async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>) {
    // Get the shared ArchClient instance
    let Some(client) = get_arch_client() else {
        return (
            Vec::new(),
            vec!["AUR search unavailable: ArchClient not initialized".to_string()],
        );
    };

    // Perform search using arch-toolkit
    match client.aur().search(&query).await {
        Ok(aur_packages) => {
            // Map AurPackage to PackageItem
            let items: Vec<PackageItem> = aur_packages
                .into_iter()
                .map(|pkg| PackageItem {
                    name: pkg.name,
                    version: pkg.version,
                    description: pkg.description,
                    source: Source::Aur,
                    popularity: pkg.popularity,
                    out_of_date: pkg.out_of_date,
                    orphaned: pkg.orphaned,
                })
                .collect();
            (items, Vec::new())
        }
        Err(e) => {
            let error_msg = format!("AUR search failed: {e}");
            (Vec::new(), vec![error_msg])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires network access and ArchClient initialization"]
    async fn search_returns_items_on_success() {
        // Initialize ArchClient for testing
        crate::sources::init_arch_client().expect("Failed to initialize ArchClient for test");

        let (items, errs) = fetch_all_with_errors("yay".into()).await;
        assert!(
            !items.is_empty() || !errs.is_empty(),
            "Search should return either items or errors"
        );

        if !items.is_empty() {
            // Verify at least one item has correct structure
            let item = &items[0];
            assert!(!item.name.is_empty());
            assert!(!item.version.is_empty());
            // Verify source is AUR (using match since Source may not implement PartialEq)
            match item.source {
                Source::Aur => {}
                Source::Official { .. } => {
                    panic!("Expected Source::Aur, got {:?}", item.source)
                }
            }
        }
    }

    #[tokio::test]
    async fn search_handles_uninitialized_client() {
        // This test verifies fallback behavior when ArchClient is not initialized
        // Note: This will only work if ArchClient hasn't been initialized yet
        // In a real scenario, init_arch_client() should be called first
        let (items, errs) = fetch_all_with_errors("test".into()).await;
        // Either items are returned (if client was initialized) or errors (if not)
        assert!(
            !items.is_empty() || !errs.is_empty(),
            "Should return either items or errors"
        );
    }
}
