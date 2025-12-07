use crate::state::AppState;

use super::super::persist::{
    maybe_flush_announcement_read, maybe_flush_cache, maybe_flush_deps_cache,
    maybe_flush_files_cache, maybe_flush_install, maybe_flush_news_bookmarks,
    maybe_flush_news_read, maybe_flush_news_recent, maybe_flush_pkgbuild_parse_cache,
    maybe_flush_recent, maybe_flush_sandbox_cache, maybe_flush_services_cache,
};
use super::background::Channels;

/// What: Clean up application state and flush caches on exit.
///
/// Inputs:
/// - `app`: Application state
/// - `channels`: Communication channels (will be dropped after this function returns)
///
/// Output: None
///
/// Details:
/// - Resets resolution flags to prevent background tasks from blocking
/// - Cancels preflight operations and clears preflight queues
/// - Signals event reading thread to exit
/// - Flushes all pending cache writes (details, recent, news, install, preflight data)
/// - Note: Request channel senders will be dropped when channels is dropped,
///   causing workers to stop accepting new work. Already-running blocking tasks
///   will complete in the background but won't block app exit.
pub fn cleanup_on_exit(app: &mut AppState, channels: &Channels) {
    // Reset resolution flags on exit to ensure clean shutdown
    // This prevents background tasks from blocking if they're still running
    tracing::debug!("[Runtime] Main loop exited, resetting resolution flags");
    app.deps_resolving = false;
    app.files_resolving = false;
    app.services_resolving = false;
    app.sandbox_resolving = false;

    // Cancel preflight operations and reset preflight flags
    // This ensures clean shutdown when app closes during preflight data loading
    tracing::debug!("[Runtime] Cancelling preflight operations and resetting flags");
    app.preflight_cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    app.preflight_summary_resolving = false;
    app.preflight_deps_resolving = false;
    app.preflight_files_resolving = false;
    app.preflight_services_resolving = false;
    app.preflight_sandbox_resolving = false;

    // Clear preflight queues to prevent background workers from processing
    app.preflight_summary_items = None;
    app.preflight_deps_items = None;
    app.preflight_files_items = None;
    app.preflight_services_items = None;
    app.preflight_sandbox_items = None;

    // Signal event reading thread to exit immediately
    channels
        .event_thread_cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);

    maybe_flush_cache(app);
    maybe_flush_recent(app);
    maybe_flush_news_recent(app);
    maybe_flush_news_bookmarks(app);
    maybe_flush_news_read(app);
    maybe_flush_announcement_read(app);
    maybe_flush_install(app);
    maybe_flush_deps_cache(app);
    maybe_flush_files_cache(app);
    maybe_flush_services_cache(app);
    maybe_flush_sandbox_cache(app);
    maybe_flush_pkgbuild_parse_cache();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{PackageItem, Source, modal::PreflightAction};

    /// What: Test that `cleanup_on_exit` properly cancels preflight operations.
    ///
    /// Inputs:
    /// - App state with packages in install list and preflight operations active
    ///
    /// Output:
    /// - All preflight flags are reset
    /// - `preflight_cancelled` is set to true
    /// - Preflight queues are cleared
    ///
    /// Details:
    /// - Simulates the scenario where app closes during preflight data loading
    /// - Verifies that cleanup properly handles preflight cancellation
    #[tokio::test]
    async fn cleanup_on_exit_cancels_preflight_operations() {
        let mut app = AppState::default();
        let channels = Channels::new(std::path::PathBuf::from("/tmp/test"));

        // Set up install list with packages
        let test_package = PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Test package".to_string(),
            source: Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };
        app.install_list.push(test_package.clone());

        // Set preflight operations as active (simulating loading state)
        app.preflight_summary_resolving = true;
        app.preflight_deps_resolving = true;
        app.preflight_files_resolving = true;
        app.preflight_services_resolving = true;
        app.preflight_sandbox_resolving = true;

        // Set up preflight queues
        app.preflight_summary_items = Some((vec![test_package.clone()], PreflightAction::Install));
        app.preflight_deps_items = Some((vec![test_package.clone()], PreflightAction::Install));
        app.preflight_files_items = Some(vec![test_package.clone()]);
        app.preflight_services_items = Some(vec![test_package.clone()]);
        app.preflight_sandbox_items = Some(vec![test_package]);

        // Verify initial state
        assert!(app.preflight_summary_resolving);
        assert!(app.preflight_deps_resolving);
        assert!(app.preflight_files_resolving);
        assert!(app.preflight_services_resolving);
        assert!(app.preflight_sandbox_resolving);
        assert!(
            !app.preflight_cancelled
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        assert!(app.preflight_summary_items.is_some());
        assert!(app.preflight_deps_items.is_some());
        assert!(app.preflight_files_items.is_some());
        assert!(app.preflight_services_items.is_some());
        assert!(app.preflight_sandbox_items.is_some());

        // Call cleanup
        cleanup_on_exit(&mut app, &channels);

        // Verify all preflight flags are reset
        assert!(!app.preflight_summary_resolving);
        assert!(!app.preflight_deps_resolving);
        assert!(!app.preflight_files_resolving);
        assert!(!app.preflight_services_resolving);
        assert!(!app.preflight_sandbox_resolving);

        // Verify preflight_cancelled is set
        assert!(
            app.preflight_cancelled
                .load(std::sync::atomic::Ordering::Relaxed)
        );

        // Verify preflight queues are cleared
        assert!(app.preflight_summary_items.is_none());
        assert!(app.preflight_deps_items.is_none());
        assert!(app.preflight_files_items.is_none());
        assert!(app.preflight_services_items.is_none());
        assert!(app.preflight_sandbox_items.is_none());
    }
}
