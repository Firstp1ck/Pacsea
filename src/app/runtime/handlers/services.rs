use tokio::sync::mpsc;

use crate::app::runtime::handlers::common::{HandlerConfig, handle_result};
use crate::state::*;

/// What: Handler configuration for service results.
struct ServiceHandlerConfig;

impl HandlerConfig for ServiceHandlerConfig {
    type Result = crate::state::modal::ServiceImpact;

    fn get_resolving(&self, app: &AppState) -> bool {
        app.services_resolving
    }

    fn set_resolving(&self, app: &mut AppState, value: bool) {
        app.services_resolving = value;
    }

    fn get_preflight_resolving(&self, app: &AppState) -> bool {
        app.preflight_services_resolving
    }

    fn set_preflight_resolving(&self, app: &mut AppState, value: bool) {
        app.preflight_services_resolving = value;
    }

    fn stage_name(&self) -> &'static str {
        "services"
    }

    fn update_cache(&self, app: &mut AppState, results: &[Self::Result]) {
        app.install_list_services = results.to_vec();
    }

    fn set_cache_dirty(&self, app: &mut AppState) {
        app.services_cache_dirty = true;
        tracing::debug!(
            "[Runtime] handle_service_result: Marked services_cache_dirty=true, install_list_services has {} entries",
            app.install_list_services.len()
        );
    }

    fn clear_preflight_items(&self, app: &mut AppState) {
        app.preflight_services_items = None;
    }

    fn sync_to_modal(&self, app: &mut AppState, _results: &[Self::Result], was_preflight: bool) {
        // Sync services to preflight modal if it's open
        if was_preflight
            && let crate::state::Modal::Preflight {
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
    }

    fn log_flag_clear(&self, app: &AppState, was_preflight: bool, cancelled: bool) {
        tracing::debug!(
            "[Runtime] handle_service_result: Clearing flags - was_preflight={}, services_resolving={}, preflight_services_resolving={}, cancelled={}",
            was_preflight,
            self.get_resolving(app),
            app.preflight_services_resolving,
            cancelled
        );
    }
}

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
    handle_result(app, &services, tick_tx, &ServiceHandlerConfig);
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
    /// What: Verify that `handle_service_result` updates cache correctly.
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

        handle_service_result(&mut app, services, &tick_tx);

        // Services should be cached
        assert_eq!(app.install_list_services.len(), 1);
        // Flags should be reset
        assert!(!app.services_resolving);
        assert!(!app.preflight_services_resolving);
        // Cache dirty flag should be set
        assert!(app.services_cache_dirty);
    }
}
