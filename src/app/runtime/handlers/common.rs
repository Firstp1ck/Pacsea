//! Common handler infrastructure for result processing.
//!
//! This module provides a generic trait-based system for handling resolution results
//! with shared cancellation, flag management, and cache update logic.

use tokio::sync::mpsc;

use crate::state::AppState;

/// What: Configuration for a result handler that specifies how to access
/// and update AppState fields for a specific result type.
///
/// Inputs: Used by generic handler infrastructure
///
/// Output: Provides field accessors and update logic
///
/// Details:
/// - Each handler type implements this trait to specify its field accessors
/// - The generic handler uses these to perform common operations
pub trait HandlerConfig {
    /// The result type this handler processes
    type Result: Clone;

    /// What: Get the current resolving flag value.
    ///
    /// Inputs: `app` - Application state
    ///
    /// Output: Current value of the resolving flag
    fn get_resolving(&self, app: &AppState) -> bool;

    /// What: Set the resolving flag to false.
    ///
    /// Inputs: `app` - Mutable application state
    ///
    /// Output: None (side effect: resets flag)
    fn set_resolving(&self, app: &mut AppState, value: bool);

    /// What: Get the current preflight resolving flag value.
    ///
    /// Inputs: `app` - Application state
    ///
    /// Output: Current value of the preflight resolving flag
    fn get_preflight_resolving(&self, app: &AppState) -> bool;

    /// What: Set the preflight resolving flag to false.
    ///
    /// Inputs: `app` - Mutable application state
    ///
    /// Output: None (side effect: resets flag)
    fn set_preflight_resolving(&self, app: &mut AppState, value: bool);

    /// What: Get the stage name for logging.
    ///
    /// Inputs: None
    ///
    /// Output: Stage name string (e.g., "files", "services")
    fn stage_name(&self) -> &'static str;

    /// What: Update the cache with new results.
    ///
    /// Inputs:
    /// - `app` - Mutable application state
    /// - `results` - New resolution results
    ///
    /// Output: None (side effect: updates cache)
    fn update_cache(&self, app: &mut AppState, results: &[Self::Result]);

    /// What: Mark the cache as dirty.
    ///
    /// Inputs: `app` - Mutable application state
    ///
    /// Output: None (side effect: sets dirty flag)
    fn set_cache_dirty(&self, app: &mut AppState);

    /// What: Clear preflight items if cancellation occurred.
    ///
    /// Inputs: `app` - Mutable application state
    ///
    /// Output: None (side effect: clears preflight items)
    fn clear_preflight_items(&self, app: &mut AppState);

    /// What: Sync results to the preflight modal if open.
    ///
    /// Inputs:
    /// - `app` - Mutable application state
    /// - `results` - Resolution results to sync
    /// - `was_preflight` - Whether this was a preflight resolution
    ///
    /// Output: None (side effect: updates modal)
    fn sync_to_modal(&self, app: &mut AppState, results: &[Self::Result], was_preflight: bool);

    /// What: Log debug information about clearing flags.
    ///
    /// Inputs:
    /// - `app` - Application state
    /// - `was_preflight` - Whether this was a preflight resolution
    /// - `cancelled` - Whether the operation was cancelled
    ///
    /// Output: None (side effect: logging)
    fn log_flag_clear(&self, app: &AppState, was_preflight: bool, cancelled: bool);

    /// What: Check if resolution is complete (all items have data).
    ///
    /// Inputs:
    /// - `app` - Application state
    /// - `results` - Latest resolution results
    ///
    /// Output: `true` if resolution is complete, `false` if more data is expected
    ///
    /// Details:
    /// - Default implementation returns `true` (assumes complete)
    /// - Can be overridden to check for incomplete data
    fn is_resolution_complete(&self, app: &AppState, results: &[Self::Result]) -> bool {
        let _ = (app, results);
        true
    }
}

/// What: Generic handler function that processes resolution results with common logic.
///
/// Inputs:
/// - `app` - Mutable application state
/// - `results` - Resolution results to process
/// - `tick_tx` - Channel sender for tick events
/// - `config` - Handler configuration implementing HandlerConfig
///
/// Output: None (side effect: updates app state and sends tick)
///
/// Details:
/// - Handles cancellation checking
/// - Manages resolving flags
/// - Updates cache and syncs to modal
/// - Sends tick event
pub fn handle_result<C: HandlerConfig>(
    app: &mut AppState,
    results: Vec<C::Result>,
    tick_tx: &mpsc::UnboundedSender<()>,
    config: C,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = config.get_preflight_resolving(app);

    config.log_flag_clear(app, was_preflight, cancelled);

    // Check if resolution is complete before clearing flags
    let is_complete = config.is_resolution_complete(app, &results);

    // Only reset resolving flags if resolution is complete
    if is_complete {
        config.set_resolving(app, false);
    }
    config.set_preflight_resolving(app, false);

    if cancelled {
        if was_preflight {
            tracing::debug!(
                "[Runtime] Ignoring {} result (preflight cancelled)",
                config.stage_name()
            );
            config.clear_preflight_items(app);
        }
        let _ = tick_tx.send(());
        return;
    }

    // Update cache and sync to modal
    tracing::info!(
        stage = config.stage_name(),
        result_count = results.len(),
        was_preflight = was_preflight,
        "[Runtime] {} resolution worker completed",
        config.stage_name()
    );

    config.update_cache(app, &results);
    config.sync_to_modal(app, &results, was_preflight);

    if was_preflight {
        config.clear_preflight_items(app);
    }
    config.set_cache_dirty(app);

    let _ = tick_tx.send(());
}
