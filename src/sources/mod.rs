//! Network and system data retrieval module split into submodules.

mod comments;
mod details;
mod news;
mod pkgbuild;
mod search;
pub mod status;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub use comments::fetch_aur_comments;
pub use details::fetch_details;
pub use news::fetch_arch_news;
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
