//! Integration tests for interactive authentication mode (`auth_mode = interactive`).
//!
//! Tests cover:
//! - Install workflow (direct) skips password prompt in interactive mode
//! - Install workflow (direct, multiple) skips password prompt in interactive mode
//! - Remove workflow skips password prompt in interactive mode
//! - `resolve_auth_mode` returns Interactive when `PACSEA_TEST_AUTH_MODE=interactive`
//! - `resolve_auth_mode` falls back to `PasswordlessOnly` via legacy env var
//! - `PACSEA_TEST_AUTH_MODE` takes precedence over `PACSEA_TEST_SUDO_PASSWORDLESS`

#![cfg(test)]

use super::helpers::*;
use pacsea::install::ExecutorRequest;
use pacsea::logic::password::resolve_auth_mode;
use pacsea::logic::privilege::AuthMode;
use pacsea::state::Modal;

/// What: Set `PACSEA_TEST_AUTH_MODE=interactive` for test context.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Sets integration test marker and `auth_mode` override.
///
/// Details:
/// - Enables interactive auth mode in all resolve paths via the test override.
fn set_interactive_auth_env() {
    unsafe {
        std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
        std::env::set_var("PACSEA_TEST_AUTH_MODE", "interactive");
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }
}

/// What: Clear interactive auth test environment variables.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Removes `PACSEA_INTEGRATION_TEST`, `PACSEA_TEST_AUTH_MODE`, and `PACSEA_TEST_HEADLESS`.
///
/// Details:
/// - Should be called after each test to ensure clean state.
fn clear_interactive_auth_env() {
    unsafe {
        std::env::remove_var("PACSEA_INTEGRATION_TEST");
        std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        std::env::remove_var("PACSEA_TEST_HEADLESS");
        std::env::remove_var("PACSEA_TEST_SUDO_PASSWORDLESS");
    }
}

/// What: Execute a closure with interactive auth mode environment.
///
/// Inputs:
/// - `f`: Closure to execute.
///
/// Output:
/// - Returns the result of the closure.
///
/// Details:
/// - Sets up, executes, and tears down the interactive auth test environment.
fn with_interactive_auth_env<T, F: FnOnce() -> T>(f: F) -> T {
    set_interactive_auth_env();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    clear_interactive_auth_env();
    match result {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

// =============================================================================
// resolve_auth_mode tests in integration context
// =============================================================================

#[test]
/// What: Verify `resolve_auth_mode` returns `Interactive` via test override.
fn integration_resolve_auth_mode_interactive_env() {
    with_interactive_auth_env(|| {
        let settings = pacsea::theme::Settings::default();
        let mode = resolve_auth_mode(&settings);
        assert_eq!(mode, AuthMode::Interactive);
    });
}

#[test]
/// What: Verify `PACSEA_TEST_AUTH_MODE` takes precedence over `PACSEA_TEST_SUDO_PASSWORDLESS`.
fn integration_auth_mode_env_takes_precedence() {
    with_interactive_auth_env(|| {
        unsafe {
            std::env::set_var("PACSEA_TEST_SUDO_PASSWORDLESS", "1");
        }
        let settings = pacsea::theme::Settings::default();
        let mode = resolve_auth_mode(&settings);
        assert_eq!(
            mode,
            AuthMode::Interactive,
            "PACSEA_TEST_AUTH_MODE=interactive should win over PACSEA_TEST_SUDO_PASSWORDLESS=1"
        );
    });
}

#[test]
/// What: Verify `resolve_auth_mode` falls back to `PasswordlessOnly` via legacy env.
fn integration_resolve_auth_mode_legacy_passwordless_env() {
    with_sudo_env(true, || {
        let settings = pacsea::theme::Settings::default();
        let mode = resolve_auth_mode(&settings);
        assert_eq!(
            mode,
            AuthMode::PasswordlessOnly,
            "PACSEA_TEST_SUDO_PASSWORDLESS=1 should map to PasswordlessOnly"
        );
    });
}

// =============================================================================
// Install Direct — Interactive mode
// =============================================================================

#[test]
/// What: Direct install with interactive auth mode skips password prompt.
///
/// Details:
/// - In interactive mode, Pacsea does a terminal handoff for auth instead of showing a modal.
/// - In test context with `PACSEA_TEST_HEADLESS=1`, the auth is simulated as successful.
/// - After handoff, `PreflightExec` modal or executor request should be set with `password: None`.
fn integration_install_direct_interactive_auth() {
    with_interactive_auth_env(|| {
        let item = create_official_package("test-pkg");
        let mut app = new_dry_run_app();

        pacsea::install::start_integrated_install(&mut app, &item, true);

        verify_no_password_prompt(&app);

        match &app.modal {
            Modal::PreflightExec { items, .. } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].name, "test-pkg");
            }
            _ => {
                assert!(
                    app.pending_executor_request.is_some(),
                    "Expected PreflightExec modal or pending executor request, got {:?}",
                    app.modal
                );
            }
        }

        if let Some(ExecutorRequest::Install { password, .. }) = &app.pending_executor_request {
            assert!(
                password.is_none(),
                "Interactive auth should not embed password in executor request"
            );
        }
    });
}

#[test]
/// What: Direct install-all with interactive auth mode skips password prompt.
fn integration_install_direct_all_interactive_auth() {
    with_interactive_auth_env(|| {
        let items = vec![
            create_official_package("pkg-a"),
            create_aur_package("pkg-b"),
        ];
        let mut app = new_dry_run_app();

        pacsea::install::start_integrated_install_all(&mut app, &items, true);

        verify_no_password_prompt(&app);

        match &app.modal {
            Modal::PreflightExec {
                items: modal_items, ..
            } => {
                assert_eq!(modal_items.len(), 2);
            }
            _ => {
                assert!(
                    app.pending_executor_request.is_some(),
                    "Expected PreflightExec modal or pending executor request, got {:?}",
                    app.modal
                );
            }
        }

        if let Some(ExecutorRequest::Install { password, .. }) = &app.pending_executor_request {
            assert!(
                password.is_none(),
                "Interactive auth should not embed password in executor request"
            );
        }
    });
}

#[test]
/// What: Direct remove with interactive auth mode skips password prompt.
///
/// Details:
/// - Remove keeps its safety confirmation, but interactive auth should still bypass
///   in-app password collection and execute with `password: None`.
fn integration_remove_direct_interactive_auth() {
    with_interactive_auth_env(|| {
        let names = vec!["pkg-a".to_string(), "pkg-b".to_string()];
        let mut app = new_dry_run_app();

        pacsea::install::start_integrated_remove_all(
            &mut app,
            &names,
            true,
            pacsea::state::modal::CascadeMode::Basic,
        );

        verify_no_password_prompt(&app);

        match &app.modal {
            Modal::PreflightExec { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].name, "pkg-a");
                assert_eq!(items[1].name, "pkg-b");
            }
            _ => {
                assert!(
                    app.pending_executor_request.is_some(),
                    "Expected PreflightExec modal or pending executor request, got {:?}",
                    app.modal
                );
            }
        }

        if let Some(ExecutorRequest::Remove { password, .. }) = &app.pending_executor_request {
            assert!(
                password.is_none(),
                "Interactive auth should not embed password in remove request"
            );
        }
    });
}

// =============================================================================
// AuthMode default fallback (no test env)
// =============================================================================

#[test]
/// What: Without test env overrides, default settings resolve to `Prompt`.
fn integration_default_settings_resolve_to_prompt() {
    clear_interactive_auth_env();
    ensure_not_integration_test_context();
    let settings = pacsea::theme::Settings::default();
    let mode = resolve_auth_mode(&settings);
    assert_eq!(
        mode,
        AuthMode::Prompt,
        "Default settings should resolve to Prompt mode"
    );
}

#[test]
/// What: Install direct without interactive env shows password prompt (the default).
fn integration_install_direct_default_shows_password_prompt() {
    clear_interactive_auth_env();
    ensure_not_integration_test_context();

    let item = create_official_package("test-pkg");
    let mut app = new_dry_run_app();

    pacsea::install::start_integrated_install(&mut app, &item, true);

    verify_password_prompt_modal(&app, pacsea::state::modal::PasswordPurpose::Install);
}
