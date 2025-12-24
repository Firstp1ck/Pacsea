//! Network and system data retrieval module split into submodules.

use std::sync::{Arc, OnceLock};
use tracing::warn;

/// Security advisories fetching.
mod advisories;
/// AUR comments fetching.
mod comments;
/// Package details fetching.
mod details;
/// News feed fetching.
mod feeds;
/// Arch Linux news fetching.
pub mod news;
/// PKGBUILD content fetching.
mod pkgbuild;
/// Package search functionality.
mod search;
/// Arch Linux status page monitoring.
pub mod status;

/// What: Result type alias for sources module errors.
///
/// Inputs: None (type alias).
///
/// Output: Result type with boxed error trait object.
///
/// Details: Standard error type for network and parsing operations in the sources module.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Shared `ArchClient` instance for AUR operations.
///
/// Inputs: None (static initialization).
///
/// Output: `OnceLock` that holds an `Arc<ArchClient>` or `None` if initialization failed.
///
/// Details:
/// - Thread-safe initialization using `OnceLock`
/// - Must be initialized via `init_arch_client()` or `init_arch_client_with_cache()` before use
/// - Wrapped in `Arc` for sharing across async tasks
static ARCH_CLIENT: OnceLock<Option<Arc<arch_toolkit::ArchClient>>> = OnceLock::new();

/// What: Initialize the `ArchClient` with appropriate configuration.
///
/// Inputs: None
///
/// Output:
/// - `Result<()>` - `Ok` if initialization succeeded, `Err` if it failed
///
/// Details:
/// - Initializes the shared `ArchClient` with caching disabled (default)
/// - Creates `ArchClient` using builder pattern
/// - Sets user agent to "pacsea/{version}"
/// - Uses default timeout (30 seconds)
/// - Caching disabled by default (use `init_arch_client_with_cache()` with `create_cache_config()` to enable caching)
/// - Retry policy uses defaults (3 retries, exponential backoff)
///
/// # Errors
/// - Returns `Err` if `ArchClient` creation fails (e.g., network client setup fails)
/// - Returns `Err` if client was already initialized
pub fn init_arch_client() -> Result<()> {
    init_arch_client_with_cache(None)
}

/// What: Initialize the `ArchClient` with optional caching configuration.
///
/// Inputs:
/// - `cache_config`: Optional cache configuration (None = caching disabled)
///
/// Output:
/// - `Result<()>` - `Ok` if initialization succeeded, `Err` if it failed
///
/// Details:
/// - Initializes the shared `ArchClient` with optional caching support
/// - If `cache_config` is `Some`, caching will be enabled with the provided configuration
/// - If `cache_config` is `None`, caching is disabled (default behavior)
/// - Must be called before any code accesses the client via `get_arch_client()`
/// - Can only be called once (subsequent calls will return an error)
///
/// # Errors
/// - Returns `Err` if `ArchClient` creation fails (e.g., network client setup fails)
/// - Returns `Err` if client was already initialized
///
/// # Example
/// ```rust,no_run
/// use arch_toolkit::CacheConfigBuilder;
/// use std::time::Duration;
///
/// // Initialize with caching enabled for search operations
/// let cache_config = CacheConfigBuilder::new()
///     .enable_search(true)
///     .search_ttl(Duration::from_secs(300))
///     .build();
/// // Note: In actual usage, call from your application:
/// // pacsea::sources::init_arch_client_with_cache(Some(cache_config))?;
/// ```
pub fn init_arch_client_with_cache(cache_config: Option<arch_toolkit::CacheConfig>) -> Result<()> {
    let client_result = create_arch_client(cache_config);
    let client_option = match client_result {
        Ok(client) => Some(client),
        Err(e) => {
            warn!("Failed to initialize ArchClient: {e}. AUR operations may be unavailable.");
            None
        }
    };

    ARCH_CLIENT
        .set(client_option)
        .map_err(|_| "ArchClient already initialized".into())
}

/// What: Internal function to create a new `ArchClient` instance.
///
/// Inputs:
/// - `cache_config`: Optional cache configuration (None = caching disabled)
///
/// Output:
/// - `Result<Arc<ArchClient>>` with configured client, or error if initialization fails
///
/// Details:
/// - Creates `ArchClient` using builder pattern
/// - Sets user agent to "pacsea/{version}"
/// - Uses default timeout (30 seconds)
/// - Caching can be optionally enabled via `cache_config` parameter
/// - Retry policy uses defaults (3 retries, exponential backoff)
///
/// # Errors
/// - Returns `Err` if `ArchClient` creation fails (e.g., network client setup fails)
fn create_arch_client(
    cache_config: Option<arch_toolkit::CacheConfig>,
) -> Result<Arc<arch_toolkit::ArchClient>> {
    let user_agent = format!("pacsea/{}", env!("CARGO_PKG_VERSION"));
    let mut builder = arch_toolkit::ArchClient::builder().user_agent(user_agent);

    // Optionally enable caching if config is provided
    if let Some(config) = cache_config {
        builder = builder.cache_config(config);
    }

    let client = builder
        .build()
        .map_err(|e| format!("Failed to create ArchClient: {e}"))?;
    Ok(Arc::new(client))
}

/// What: Create cache configuration with recommended TTLs for Pacsea.
///
/// Inputs: None
///
/// Output:
/// - `CacheConfig` with memory caching enabled for search, comments, and PKGBUILD
///
/// Details:
/// - Enables memory cache (fast, no persistence)
/// - Sets TTLs: search (5min), comments (10min), pkgbuild (1hr)
/// - Memory cache size: 200 entries (reasonable for typical usage)
/// - Disk cache disabled by default (requires cache-disk feature)
/// - TTLs are chosen to balance freshness vs performance
/// - Aligns with arch-toolkit defaults and Pacsea's existing cache patterns
#[must_use]
pub fn create_cache_config() -> arch_toolkit::CacheConfig {
    use arch_toolkit::CacheConfigBuilder;
    use std::time::Duration;

    CacheConfigBuilder::new()
        .enable_search(true)
        .search_ttl(Duration::from_secs(300)) // 5 minutes
        .enable_comments(true)
        .comments_ttl(Duration::from_secs(600)) // 10 minutes
        .enable_pkgbuild(true)
        .pkgbuild_ttl(Duration::from_secs(3600)) // 1 hour
        .memory_cache_size(200) // Reasonable size for typical usage
        .build()
}

/// What: Get the shared `ArchClient` instance.
///
/// Inputs: None
///
/// Output:
/// - `Option<Arc<ArchClient>>` - `Some(client)` if initialized successfully, `None` otherwise
///
/// Details:
/// - Returns the initialized `ArchClient` if available
/// - Returns `None` if not initialized or if initialization failed
/// - Thread-safe access via `OnceLock`
/// - Must call `init_arch_client()` or `init_arch_client_with_cache()` first
#[must_use]
pub fn get_arch_client() -> Option<Arc<arch_toolkit::ArchClient>> {
    ARCH_CLIENT.get().and_then(|opt| opt.as_ref()).cloned()
}

pub use advisories::fetch_security_advisories;
pub use comments::fetch_aur_comments;
pub use details::fetch_details;
pub use feeds::{
    NewsFeedContext, check_circuit_breaker, extract_endpoint_pattern,
    extract_retry_after_from_error, fetch_continuation_items, fetch_news_feed,
    get_aur_json_changes, get_official_json_changes, increase_archlinux_backoff,
    load_official_json_cache, official_json_cache_path, optimize_max_age_for_startup,
    rate_limit_archlinux, record_circuit_breaker_outcome, reset_archlinux_backoff,
    take_network_error,
};
pub use news::{fetch_arch_news, fetch_news_content, parse_news_html};
pub use pkgbuild::fetch_pkgbuild_fast;
pub use search::fetch_all_with_errors;
pub use status::fetch_arch_status_text;

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
#[allow(dead_code)] // Used by tests in other modules
static TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
/// What: Provide a shared mutex to serialize tests that mutate PATH or curl shims.
///
/// Input: None.
/// Output: `&'static Mutex<()>` guard to synchronize tests touching global state.
///
/// Details: Lazily initializes a global `Mutex` via `OnceLock` for cross-test coordination.
#[allow(dead_code)] // Used by tests in other modules
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
