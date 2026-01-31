//! Helper functions for passwordless sudo integration tests.
//!
//! This module provides utilities for:
//! - Controlling sudo environment via environment variables
//! - Creating test settings with specific passwordless sudo configurations
//! - Verifying modal states and execution behavior

#![cfg(test)]
// Allow dead code for helper functions that may be used by future tests
#![allow(dead_code)]

use pacsea::state::{AppState, Modal, PackageItem, Source, modal::PasswordPurpose};

/// What: Set the environment variables to control passwordless sudo behavior in tests.
///
/// Inputs:
/// - `enabled`: If true, simulates passwordless sudo available; if false, simulates unavailable.
///
/// Output:
/// - Sets `PACSEA_INTEGRATION_TEST=1` and `PACSEA_TEST_SUDO_PASSWORDLESS` (1 or 0).
///
/// Details:
/// - `PACSEA_INTEGRATION_TEST=1` marks the process as in integration test context so production
///   code honors `PACSEA_TEST_SUDO_PASSWORDLESS`. Without it, the test var is ignored.
pub fn set_passwordless_sudo_env(enabled: bool) {
    // SAFETY: This is only used in tests where we control the environment
    // and test threads run sequentially with --test-threads=1
    unsafe {
        std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
        if enabled {
            std::env::set_var("PACSEA_TEST_SUDO_PASSWORDLESS", "1");
        } else {
            std::env::set_var("PACSEA_TEST_SUDO_PASSWORDLESS", "0");
        }
    }
}

/// What: Clear the passwordless sudo test environment variables.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Removes `PACSEA_INTEGRATION_TEST` and `PACSEA_TEST_SUDO_PASSWORDLESS`.
///
/// Details:
/// - Should be called after each test to ensure clean state.
pub fn clear_passwordless_sudo_env() {
    // SAFETY: This is only used in tests where we control the environment
    // and test threads run sequentially with --test-threads=1
    unsafe {
        std::env::remove_var("PACSEA_INTEGRATION_TEST");
        std::env::remove_var("PACSEA_TEST_SUDO_PASSWORDLESS");
    }
}

/// What: Ensure process is not in integration test context (production-like behavior).
///
/// Inputs:
/// - None.
///
/// Output:
/// - Removes `PACSEA_INTEGRATION_TEST` and `PACSEA_TEST_SUDO_PASSWORDLESS`.
///
/// Details:
/// - Use in tests that verify behavior when the test env var is not honored (e.g. install
///   shows password prompt when `use_passwordless_sudo` is false and test override is disabled).
pub fn ensure_not_integration_test_context() {
    clear_passwordless_sudo_env();
}

/// What: Execute a closure with a specific passwordless sudo environment setting.
///
/// Inputs:
/// - `enabled`: Whether passwordless sudo should be simulated as available.
/// - `f`: Closure to execute with the environment set.
///
/// Output:
/// - Returns the result of the closure.
///
/// Details:
/// - Sets `PACSEA_INTEGRATION_TEST=1` and `PACSEA_TEST_SUDO_PASSWORDLESS`, executes the
///   closure, then clears both. Ensures cleanup even if the closure panics.
pub fn with_sudo_env<T, F: FnOnce() -> T>(enabled: bool, f: F) -> T {
    set_passwordless_sudo_env(enabled);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    clear_passwordless_sudo_env();
    match result {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// What: Create a test package item with specified source.
///
/// Inputs:
/// - `name`: Package name.
/// - `source`: Package source (Official or AUR).
///
/// Output:
/// - `PackageItem` ready for testing.
///
/// Details:
/// - Helper to create test packages with consistent structure.
pub fn create_test_package(name: &str, source: Source) -> PackageItem {
    PackageItem {
        name: name.into(),
        version: "1.0.0".into(),
        description: format!("Test package {name}"),
        source,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

/// What: Create an official package for testing.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - `PackageItem` with Official source.
///
/// Details:
/// - Convenience function for creating official repository packages.
pub fn create_official_package(name: &str) -> PackageItem {
    create_test_package(
        name,
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )
}

/// What: Create an AUR package for testing.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - `PackageItem` with AUR source.
///
/// Details:
/// - Convenience function for creating AUR packages.
pub fn create_aur_package(name: &str) -> PackageItem {
    create_test_package(name, Source::Aur)
}

/// What: Verify that the app modal is a `PasswordPrompt` with the specified purpose.
///
/// Inputs:
/// - `app`: Reference to the application state.
/// - `expected_purpose`: Expected password prompt purpose.
///
/// Output:
/// - Panics if modal is not `PasswordPrompt` or purpose doesn't match.
///
/// Details:
/// - Used to verify that password prompt is shown with correct purpose.
pub fn verify_password_prompt_modal(app: &AppState, expected_purpose: PasswordPurpose) {
    match &app.modal {
        Modal::PasswordPrompt { purpose, .. } => {
            assert_eq!(
                *purpose, expected_purpose,
                "Expected PasswordPrompt with purpose {expected_purpose:?}, got {purpose:?}"
            );
        }
        other => {
            panic!(
                "Expected PasswordPrompt modal with purpose {expected_purpose:?}, got {other:?}"
            );
        }
    }
}

/// What: Verify that the app modal is a `PasswordPrompt` with specific items.
///
/// Inputs:
/// - `app`: Reference to the application state.
/// - `expected_purpose`: Expected password prompt purpose.
/// - `expected_item_names`: Expected package names in the prompt.
///
/// Output:
/// - Panics if modal doesn't match expected state.
///
/// Details:
/// - Verifies both purpose and items in the password prompt.
pub fn verify_password_prompt_with_items(
    app: &AppState,
    expected_purpose: PasswordPurpose,
    expected_item_names: &[&str],
) {
    match &app.modal {
        Modal::PasswordPrompt {
            purpose,
            items,
            input,
            cursor,
            error,
        } => {
            assert_eq!(
                *purpose, expected_purpose,
                "Expected PasswordPrompt with purpose {expected_purpose:?}, got {purpose:?}"
            );
            assert_eq!(
                items.len(),
                expected_item_names.len(),
                "Expected {} items, got {}",
                expected_item_names.len(),
                items.len()
            );
            for (i, expected_name) in expected_item_names.iter().enumerate() {
                assert_eq!(
                    items[i].name, *expected_name,
                    "Expected item {i} to be '{expected_name}', got '{}'",
                    items[i].name
                );
            }
            // Verify initial state
            assert!(input.is_empty(), "Password input should be empty initially");
            assert_eq!(*cursor, 0, "Cursor should be at position 0 initially");
            assert!(error.is_none(), "Error should be None initially");
        }
        other => {
            panic!(
                "Expected PasswordPrompt modal with purpose {expected_purpose:?}, got {other:?}"
            );
        }
    }
}

/// What: Verify that the app modal is NOT a `PasswordPrompt`.
///
/// Inputs:
/// - `app`: Reference to the application state.
///
/// Output:
/// - Panics if modal is `PasswordPrompt`.
///
/// Details:
/// - Used to verify that password prompt is skipped when passwordless sudo is active.
pub fn verify_no_password_prompt(app: &AppState) {
    if let Modal::PasswordPrompt { purpose, .. } = &app.modal {
        panic!("Expected no PasswordPrompt modal, but got PasswordPrompt with purpose {purpose:?}");
    }
}

/// What: Verify that the app modal is `PreflightExec` (execution started).
///
/// Inputs:
/// - `app`: Reference to the application state.
///
/// Output:
/// - Panics if modal is not `PreflightExec`.
///
/// Details:
/// - Used to verify that execution started directly without password prompt.
pub fn verify_preflight_exec_modal(app: &AppState) {
    match &app.modal {
        Modal::PreflightExec { .. } => {
            // Success - execution started
        }
        other => {
            panic!("Expected PreflightExec modal, got {other:?}");
        }
    }
}

/// What: Verify that execution started with the correct password state.
///
/// Inputs:
/// - `app`: Reference to the application state.
/// - `expected_password`: Expected password value (None for passwordless, Some for with password).
///
/// Output:
/// - Panics if `pending_executor_request` doesn't match expected state.
///
/// Details:
/// - Checks `pending_executor_request` to verify password was handled correctly.
pub fn verify_executor_request_password(app: &AppState, expected_password: Option<&str>) {
    if let Some(ref request) = app.pending_executor_request {
        match request {
            pacsea::install::ExecutorRequest::Install { password, .. }
            | pacsea::install::ExecutorRequest::Remove { password, .. }
            | pacsea::install::ExecutorRequest::Update { password, .. }
            | pacsea::install::ExecutorRequest::Downgrade { password, .. }
            | pacsea::install::ExecutorRequest::CustomCommand { password, .. } => {
                verify_password_match(password.as_deref(), expected_password);
            }
            pacsea::install::ExecutorRequest::Scan { .. } => {
                // Scan doesn't have password field
                assert!(
                    expected_password.is_none(),
                    "Scan request doesn't have password field"
                );
            }
        }
    } else if expected_password.is_some() {
        panic!("Expected pending_executor_request with password, but it's None");
    }
}

/// What: Helper to verify password matches expected value.
///
/// Inputs:
/// - `actual`: Actual password from request.
/// - `expected`: Expected password value.
///
/// Output:
/// - Panics if passwords don't match.
///
/// Details:
/// - Internal helper to reduce code duplication.
fn verify_password_match(actual: Option<&str>, expected: Option<&str>) {
    match (actual, expected) {
        (None, None) => { /* Success - both None */ }
        (Some(act), Some(exp)) => {
            // Avoid logging password contents; only indicate mismatch generically.
            assert!(act == exp, "Password mismatch");
        }
        (None, Some(_exp)) => {
            // Do not include expected password in panic message.
            panic!("Expected password to be set, but it was None");
        }
        (Some(_act), None) => {
            // Do not include actual password in panic message.
            panic!("Expected no password, but a password was provided");
        }
    }
}

/// What: Create a default `AppState` for testing.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Fresh `AppState` with default values.
///
/// Details:
/// - Convenience function for creating test app state.
pub fn new_test_app() -> AppState {
    AppState::default()
}

/// What: Create an `AppState` with `dry_run` enabled.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `AppState` with `dry_run = true`.
///
/// Details:
/// - Tests should use `dry_run` to avoid actual system modifications.
pub fn new_dry_run_app() -> AppState {
    AppState {
        dry_run: true,
        ..Default::default()
    }
}
