//! Network and system data retrieval module split into submodules.

/// Security advisories fetching.
mod advisories;
/// AUR comments fetching.
mod comments;
/// Package details fetching.
mod details;
/// News feed fetching.
mod feeds;
/// Arch Linux news fetching.
mod news;
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

pub use advisories::fetch_security_advisories;
pub use comments::fetch_aur_comments;
pub use details::fetch_details;
pub use feeds::fetch_news_feed;
pub use news::{fetch_arch_news, fetch_news_content};
pub use pkgbuild::fetch_pkgbuild_fast;
pub use search::fetch_all_with_errors;
pub use status::fetch_arch_status_text;

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
static TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
/// What: Provide a shared mutex to serialize tests that mutate PATH or curl shims.
///
/// Input: None.
/// Output: `&'static Mutex<()>` guard to synchronize tests touching global state.
///
/// Details: Lazily initializes a global `Mutex` via `OnceLock` for cross-test coordination.
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
