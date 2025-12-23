//! Constants and type aliases for `AppState`.

use std::num::NonZeroUsize;

/// Maximum number of recent searches to retain (most-recent-first).
pub const RECENT_CAPACITY: usize = 20;

/// What: Provide the non-zero capacity used by the LRU recent cache.
///
/// Inputs: None.
///
/// Output:
/// - Non-zero capacity for the recent LRU cache.
///
/// Details:
/// - Uses a const unchecked constructor because the capacity constant is guaranteed
///   to be greater than zero.
#[must_use]
pub const fn recent_capacity() -> NonZeroUsize {
    // SAFETY: `RECENT_CAPACITY` is a non-zero constant.
    unsafe { NonZeroUsize::new_unchecked(RECENT_CAPACITY) }
}

/// File database sync result type.
pub type FileSyncResult = std::sync::Arc<std::sync::Mutex<Option<Result<bool, String>>>>;
