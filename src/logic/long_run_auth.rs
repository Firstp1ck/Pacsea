//! Long-running privilege/auth readiness helpers.

use crate::logic::privilege::{AuthMode, PrivilegeTool};
use crate::theme::Settings;

/// What: User-facing readiness level for long-running privileged operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongRunReadiness {
    /// Configuration is ready with minimal interruption risk.
    Ready,
    /// Configuration is workable, but prompts are expected during longer runs.
    ReadyViaPam,
    /// Configuration is workable with in-app prompt flow.
    ReadyWithPrompt,
    /// Configuration is usable but has elevated interruption risk.
    Warn,
    /// Configuration is degraded or unresolved.
    Degraded,
}

/// What: Structured readiness evaluation result for long-running operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongRunAuthReadiness {
    /// Active privilege tool resolved from settings/environment.
    pub tool: Option<PrivilegeTool>,
    /// Effective authentication mode after capability coercion.
    pub auth_mode: AuthMode,
    /// Chosen readiness classification.
    pub readiness: LongRunReadiness,
    /// Whether the result should show one-time preflight guidance.
    pub should_warn: bool,
}

/// What: Evaluate long-running authentication readiness for update/install paths.
///
/// Inputs:
/// - `settings`: Active settings used to resolve effective auth mode.
///
/// Output:
/// - Structured readiness result for caller routing and UX hints.
#[must_use]
pub fn evaluate_long_run_auth_readiness(settings: &Settings) -> LongRunAuthReadiness {
    let auth_mode = crate::logic::password::resolve_auth_mode(settings);
    let tool = crate::logic::privilege::resolve_privilege_tool(settings.privilege_mode).ok();
    let readiness = match (tool, auth_mode) {
        (Some(PrivilegeTool::Sudo | PrivilegeTool::Doas), AuthMode::Prompt) => {
            LongRunReadiness::ReadyWithPrompt
        }
        (Some(PrivilegeTool::Sudo | PrivilegeTool::Doas), AuthMode::PasswordlessOnly) => {
            LongRunReadiness::Ready
        }
        (Some(PrivilegeTool::Sudo), AuthMode::Interactive) => LongRunReadiness::Warn,
        (Some(PrivilegeTool::Doas), AuthMode::Interactive) => LongRunReadiness::ReadyViaPam,
        (None, _) => LongRunReadiness::Degraded,
    };
    let should_warn = matches!(
        readiness,
        LongRunReadiness::Warn | LongRunReadiness::Degraded
    );
    LongRunAuthReadiness {
        tool,
        auth_mode,
        readiness,
        should_warn,
    }
}

/// What: Build localized preflight guidance lines for at-risk long-running auth paths.
///
/// Inputs:
/// - `app`: Application state used for i18n.
///
/// Output:
/// - Multi-line warning text suitable for alert/toast usage.
#[must_use]
pub fn build_long_run_warning_message(app: &crate::state::AppState) -> String {
    [
        crate::i18n::t(app, "app.modals.doas_persist_setup.preflight_warning_1"),
        crate::i18n::t(app, "app.modals.doas_persist_setup.preflight_warning_2"),
        crate::i18n::t(app, "app.modals.doas_persist_setup.preflight_warning_3"),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_is_degraded_when_tool_unresolved() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "none");
        }
        let settings = crate::theme::Settings::default();
        let result = evaluate_long_run_auth_readiness(&settings);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result.readiness, LongRunReadiness::Degraded);
        assert!(result.should_warn);
    }

    #[test]
    fn readiness_is_ready_via_pam_for_doas_interactive() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "doas");
            std::env::set_var("PACSEA_TEST_AUTH_MODE", "interactive");
        }
        let settings = crate::theme::Settings::default();
        let result = evaluate_long_run_auth_readiness(&settings);
        unsafe {
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result.readiness, LongRunReadiness::ReadyViaPam);
        assert!(!result.should_warn);
    }

    #[test]
    fn warning_message_is_three_non_empty_lines() {
        let app = crate::state::AppState::default();
        let message = build_long_run_warning_message(&app);
        let parts: Vec<&str> = message.lines().collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|line| !line.trim().is_empty()));
    }
}
