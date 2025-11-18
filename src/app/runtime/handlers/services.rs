use tokio::sync::mpsc;

use crate::state::*;

/// What: Handle service resolution result event.
///
/// Inputs:
/// - `app`: Application state
/// - `services`: Service resolution results
/// - `tick_tx`: Channel sender for tick events
///
/// Details:
/// - Updates cached services
/// - Syncs services to preflight modal if open
/// - Respects cancellation flag
pub fn handle_service_result(
    app: &mut AppState,
    services: Vec<crate::state::modal::ServiceImpact>,
    tick_tx: &mpsc::UnboundedSender<()>,
) {
    // Check if cancelled before updating
    let cancelled = app
        .preflight_cancelled
        .load(std::sync::atomic::Ordering::Relaxed);
    let was_preflight = app.preflight_services_resolving;
    app.services_resolving = false;
    app.preflight_services_resolving = false; // Also reset preflight flag

    if !cancelled {
        // Update cached services
        tracing::info!(
            stage = "services",
            result_count = services.len(),
            "[Runtime] Service resolution worker completed"
        );
        app.install_list_services = services;
        // Sync services to preflight modal if it's open
        if was_preflight {
            if let crate::state::Modal::Preflight {
                service_info,
                services_loaded,
                ..
            } = &mut app.modal
            {
                *service_info = app.install_list_services.clone();
                *services_loaded = true;
                tracing::debug!(
                    "[Runtime] Synced {} services to preflight modal",
                    service_info.len()
                );
            }
            app.preflight_services_items = None;
        }
        app.services_cache_dirty = true; // Mark cache as dirty for persistence
    } else if was_preflight {
        tracing::debug!("[Runtime] Ignoring service result (preflight cancelled)");
        app.preflight_services_items = None;
    }
    let _ = tick_tx.send(());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Provide a baseline `AppState` for handler tests.
    ///
    /// Inputs: None
    /// Output: Fresh `AppState` with default values
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Verify that handle_service_result updates cache correctly.
    ///
    /// Inputs:
    /// - App state
    /// - Service resolution results
    ///
    /// Output:
    /// - Services are cached
    /// - Flags are reset
    ///
    /// Details:
    /// - Tests that service results are properly processed
    fn handle_service_result_updates_cache() {
        let mut app = new_app();
        app.services_resolving = true;
        app.preflight_services_resolving = true;

        let (tick_tx, _tick_rx) = mpsc::unbounded_channel();

        let services = vec![crate::state::modal::ServiceImpact {
            unit_name: "test.service".to_string(),
            providers: vec!["test-package".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: crate::state::modal::ServiceRestartDecision::Defer,
            restart_decision: crate::state::modal::ServiceRestartDecision::Defer,
        }];

        handle_service_result(&mut app, services.clone(), &tick_tx);

        // Services should be cached
        assert_eq!(app.install_list_services.len(), 1);
        // Flags should be reset
        assert!(!app.services_resolving);
        assert!(!app.preflight_services_resolving);
        // Cache dirty flag should be set
        assert!(app.services_cache_dirty);
    }
}
