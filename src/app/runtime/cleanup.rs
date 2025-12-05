use crate::state::AppState;

use super::super::persist::{
    maybe_flush_announcement_read, maybe_flush_cache, maybe_flush_deps_cache,
    maybe_flush_files_cache, maybe_flush_install, maybe_flush_news_read, maybe_flush_recent,
    maybe_flush_sandbox_cache, maybe_flush_services_cache,
};
use super::background::Channels;

/// What: Clean up application state and flush caches on exit.
///
/// Inputs:
/// - `app`: Application state
/// - `channels`: Communication channels
///
/// Output: None
///
/// Details:
/// - Resets resolution flags to prevent background tasks from blocking
/// - Signals event reading thread to exit
/// - Flushes all pending cache writes (details, recent, news, install, preflight data)
pub fn cleanup_on_exit(app: &mut AppState, channels: &Channels) {
    // Reset resolution flags on exit to ensure clean shutdown
    // This prevents background tasks from blocking if they're still running
    tracing::debug!("[Runtime] Main loop exited, resetting resolution flags");
    app.deps_resolving = false;
    app.files_resolving = false;
    app.services_resolving = false;
    app.sandbox_resolving = false;

    // Signal event reading thread to exit immediately
    channels
        .event_thread_cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);

    maybe_flush_cache(app);
    maybe_flush_recent(app);
    maybe_flush_news_read(app);
    maybe_flush_announcement_read(app);
    maybe_flush_install(app);
    maybe_flush_deps_cache(app);
    maybe_flush_files_cache(app);
    maybe_flush_services_cache(app);
    maybe_flush_sandbox_cache(app);
}
