//! Privilege password validation utilities.
//!
//! Delegates to [`crate::logic::privilege`] for tool-aware checks.

use crate::logic::privilege::AuthMode;

/// What: Resolve the effective authentication mode from settings.
///
/// Inputs:
/// - `settings`: Reference to the application settings.
///
/// Output:
/// - The resolved [`AuthMode`] to use for privilege escalation.
///
/// Details:
/// - If `auth_mode` is explicitly set to something other than the default (`Prompt`),
///   it takes precedence.
/// - If `auth_mode` is `Prompt` (the default) and the legacy `use_passwordless_sudo`
///   is `true`, maps to `PasswordlessOnly` for backward compatibility and logs a
///   deprecation warning.
/// - When both `auth_mode != Prompt` and `use_passwordless_sudo = true` are set,
///   `auth_mode` wins and a deprecation warning is logged.
#[must_use]
pub fn resolve_auth_mode(settings: &crate::theme::Settings) -> AuthMode {
    if crate::logic::privilege::is_integration_test() {
        if let Ok(val) = std::env::var("PACSEA_TEST_AUTH_MODE") {
            tracing::debug!(val = %val, "Using test override for resolve_auth_mode");
            if let Some(mode) = AuthMode::from_config_key(&val) {
                return mode;
            }
        }
        if std::env::var("PACSEA_TEST_SUDO_PASSWORDLESS")
            .ok()
            .as_deref()
            == Some("1")
        {
            tracing::debug!("Legacy test env PACSEA_TEST_SUDO_PASSWORDLESS=1 → PasswordlessOnly");
            return AuthMode::PasswordlessOnly;
        }
    }

    let explicit_auth_mode = settings.auth_mode;
    let legacy_passwordless = settings.use_passwordless_sudo;

    match (explicit_auth_mode, legacy_passwordless) {
        (AuthMode::Prompt, true) => {
            tracing::warn!(
                "Deprecated: 'use_passwordless_sudo = true' is active. \
                 Mapping to auth_mode = passwordless_only. \
                 Please migrate to 'auth_mode = passwordless_only' in settings.conf."
            );
            AuthMode::PasswordlessOnly
        }
        (mode, true) if mode != AuthMode::Prompt => {
            tracing::warn!(
                auth_mode = %mode,
                "Deprecated: 'use_passwordless_sudo' is set alongside 'auth_mode'. \
                 'auth_mode = {mode}' takes precedence. \
                 Please remove 'use_passwordless_sudo' from settings.conf."
            );
            mode
        }
        (mode, _) => mode,
    }
}

/// What: Determine whether the Pacsea password modal should be skipped.
///
/// Inputs:
/// - `settings`: Reference to the application settings.
///
/// Output:
/// - `true` if the password modal should be skipped, `false` if it should be shown.
///
/// Details:
/// - Resolves the effective [`AuthMode`] via [`resolve_auth_mode`].
/// - `Interactive` always skips the modal.
/// - `PasswordlessOnly` skips only when `{tool} -n true` succeeds on the system.
/// - `Prompt` never skips the modal.
/// - Tool-agnostic: works identically for sudo and doas.
#[must_use]
pub fn should_skip_password_modal(settings: &crate::theme::Settings) -> bool {
    let mode = resolve_auth_mode(settings);
    match mode {
        AuthMode::Interactive => {
            tracing::info!("Auth mode is 'interactive'; skipping Pacsea password modal");
            true
        }
        AuthMode::PasswordlessOnly => should_use_passwordless_sudo(settings),
        AuthMode::Prompt => false,
    }
}

/// What: Check if passwordless privilege escalation is available for the current user.
///
/// Inputs:
/// - None (uses the active privilege tool from settings).
///
/// Output:
/// - `Ok(true)` if passwordless execution is available, `Ok(false)` if not, or `Err(String)` on error.
///
/// # Errors
///
/// - Returns `Err` if the check cannot be executed (e.g., tool not installed).
///
/// Details:
/// - Delegates to [`crate::logic::privilege::PrivilegeTool::check_passwordless`].
/// - Uses the resolved privilege tool (sudo or doas) based on settings.
/// - Both sudo and doas support `-n true` for non-interactive checking.
pub fn check_passwordless_sudo_available() -> Result<bool, String> {
    let tool = crate::logic::privilege::active_tool();
    tool.check_passwordless()
}

/// What: Check if passwordless privilege escalation should be used based on settings and system availability.
///
/// Inputs:
/// - `settings`: Reference to the application settings.
///
/// Output:
/// - `true` if passwordless execution should be used, `false` otherwise.
///
/// Details:
/// - First checks tool capabilities: tools without stdin-password support (e.g. doas)
///   bypass the in-app modal and return `true`.
/// - For stdin-capable tools (sudo), checks if `use_passwordless_sudo` is enabled
///   in settings (safety barrier).
/// - If not enabled for stdin-capable tools, returns `false` immediately.
/// - If enabled, checks if passwordless execution is actually available on the system.
/// - Returns `true` only if both conditions are met.
/// - Test overrides flow through [`check_passwordless_sudo_available`] → privilege module.
#[must_use]
pub fn should_use_passwordless_sudo(settings: &crate::theme::Settings) -> bool {
    // In integration test context, honor the test override directly.
    // This bypasses the settings check so tests can simulate passwordless without
    // modifying the persisted Settings struct.
    if crate::logic::privilege::is_integration_test()
        && let Ok(val) = std::env::var("PACSEA_TEST_SUDO_PASSWORDLESS")
    {
        tracing::debug!(
            val = %val,
            "Using test override for should_use_passwordless_sudo"
        );
        return val == "1";
    }

    let tool = crate::logic::privilege::active_tool();
    if !tool.capabilities().supports_stdin_password {
        tracing::debug!(
            tool = %tool,
            "Active privilege tool does not support stdin password; skipping in-app password prompt"
        );
        return true;
    }

    let auth_mode = resolve_auth_mode(settings);
    let require_legacy_toggle = auth_mode != AuthMode::PasswordlessOnly;
    if require_legacy_toggle && !settings.use_passwordless_sudo {
        tracing::debug!("Passwordless privilege disabled in settings, requiring password prompt");
        return false;
    }

    match check_passwordless_sudo_available() {
        Ok(true) => {
            tracing::info!("Passwordless privilege enabled in settings and available on system");
            true
        }
        Ok(false) => {
            tracing::debug!(
                "Passwordless privilege enabled in settings but not available on system, requiring password prompt"
            );
            false
        }
        Err(e) => {
            tracing::debug!(
                "Passwordless privilege check failed ({}), requiring password prompt",
                e
            );
            false
        }
    }
}

/// What: Validate a privilege tool password without executing any command.
///
/// Inputs:
/// - `password`: Password to validate.
///
/// Output:
/// - `Ok(true)` if password is valid, `Ok(false)` if invalid, or `Err(String)` on error.
///
/// # Errors
///
/// - Returns `Err` if the validation command cannot be executed (e.g., tool not available).
/// - Returns `Err` if the active tool does not support password validation (e.g., doas).
///
/// Details:
/// - Delegates to [`crate::logic::privilege::validate_password`].
/// - Only works for tools that support stdin password piping (currently sudo).
/// - For doas, returns an error since doas cannot validate passwords via stdin.
pub fn validate_sudo_password(password: &str) -> Result<bool, String> {
    let tool = crate::logic::privilege::active_tool();
    crate::logic::privilege::validate_password(tool, password)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Check if passwordless sudo is configured (test helper).
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - `true` if passwordless sudo is available, `false` otherwise.
    ///
    /// Details:
    /// - Uses the public `check_passwordless_sudo_available()` function.
    /// - Returns `false` if sudo is not available or requires a password.
    fn is_passwordless_sudo() -> bool {
        check_passwordless_sudo_available().unwrap_or(false)
    }

    #[test]
    /// What: Test passwordless sudo check returns a valid result.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Returns `Ok(bool)` without panicking.
    ///
    /// Details:
    /// - Verifies the function returns a valid result (either true or false).
    /// - Does not assert on the actual value since it depends on system configuration.
    fn test_check_passwordless_sudo_available() {
        let result = check_passwordless_sudo_available();
        // Should return Ok with either true or false, depending on system config
        assert!(result.is_ok());
    }

    #[test]
    /// What: Ensure doas skips in-app password prompt decision.
    ///
    /// Inputs:
    /// - Integration test env forcing only doas availability.
    /// - Default settings with `use_passwordless_sudo = false`.
    ///
    /// Output:
    /// - Returns `true` from `should_use_passwordless_sudo`.
    ///
    /// Details:
    /// - doas cannot validate stdin passwords in-app.
    /// - Pacsea must skip the modal and let terminal doas prompt directly.
    fn test_should_use_passwordless_sudo_true_for_doas_without_stdin_support() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "doas");
        }

        let settings = crate::theme::Settings {
            use_passwordless_sudo: false,
            ..crate::theme::Settings::default()
        };
        let should_skip_prompt = should_use_passwordless_sudo(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert!(
            should_skip_prompt,
            "doas should bypass in-app password prompt"
        );
    }

    // -- resolve_auth_mode ---------------------------------------------------

    #[test]
    /// What: Default settings resolve to `Prompt` auth mode.
    ///
    /// Inputs: Default settings (`auth_mode` = Prompt, `use_passwordless_sudo` = false).
    ///
    /// Output: `AuthMode::Prompt`.
    ///
    /// Details: Ensures no accidental legacy mapping fires for default config.
    fn test_resolve_auth_mode_default_is_prompt() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        }

        let settings = crate::theme::Settings::default();
        let mode = resolve_auth_mode(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert_eq!(mode, AuthMode::Prompt);
    }

    #[test]
    /// What: Explicit `auth_mode = interactive` takes effect.
    ///
    /// Inputs: Settings with `auth_mode = Interactive`.
    ///
    /// Output: `AuthMode::Interactive`.
    ///
    /// Details: Verifies direct setting without legacy fallback.
    fn test_resolve_auth_mode_explicit_interactive() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        }

        let settings = crate::theme::Settings {
            auth_mode: AuthMode::Interactive,
            ..crate::theme::Settings::default()
        };
        let mode = resolve_auth_mode(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert_eq!(mode, AuthMode::Interactive);
    }

    #[test]
    /// What: Legacy `use_passwordless_sudo = true` maps to `PasswordlessOnly`.
    ///
    /// Inputs: Settings with `auth_mode = Prompt` (default) and `use_passwordless_sudo = true`.
    ///
    /// Output: `AuthMode::PasswordlessOnly`.
    ///
    /// Details: Backward compatibility mapping fires when `auth_mode` is still default.
    fn test_resolve_auth_mode_legacy_passwordless_maps() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        }

        let settings = crate::theme::Settings {
            use_passwordless_sudo: true,
            ..crate::theme::Settings::default()
        };
        let mode = resolve_auth_mode(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert_eq!(mode, AuthMode::PasswordlessOnly);
    }

    #[test]
    /// What: Explicit `auth_mode = interactive` wins over legacy `use_passwordless_sudo`.
    ///
    /// Inputs: `auth_mode = Interactive` and `use_passwordless_sudo = true`.
    ///
    /// Output: `AuthMode::Interactive` (explicit `auth_mode` wins).
    ///
    /// Details: When both keys are set, `auth_mode` takes precedence over legacy.
    fn test_resolve_auth_mode_explicit_wins_over_legacy() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        }

        let settings = crate::theme::Settings {
            auth_mode: AuthMode::Interactive,
            use_passwordless_sudo: true,
            ..crate::theme::Settings::default()
        };
        let mode = resolve_auth_mode(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert_eq!(mode, AuthMode::Interactive);
    }

    #[test]
    /// What: Test override env var controls resolved auth mode.
    ///
    /// Inputs: `PACSEA_TEST_AUTH_MODE=interactive` with default settings.
    ///
    /// Output: `AuthMode::Interactive`.
    ///
    /// Details: Integration test override should bypass settings entirely.
    fn test_resolve_auth_mode_env_override() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::set_var("PACSEA_TEST_AUTH_MODE", "interactive");
        }

        let settings = crate::theme::Settings::default();
        let mode = resolve_auth_mode(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert_eq!(mode, AuthMode::Interactive);
    }

    // -- should_skip_password_modal ------------------------------------------

    #[test]
    /// What: `should_skip_password_modal` returns true for interactive mode.
    ///
    /// Inputs: Settings with `auth_mode = Interactive`.
    ///
    /// Output: `true`.
    ///
    /// Details: Interactive always skips the modal, regardless of tool availability.
    fn test_should_skip_password_modal_interactive() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        }

        let settings = crate::theme::Settings {
            auth_mode: AuthMode::Interactive,
            ..crate::theme::Settings::default()
        };
        let skip = should_skip_password_modal(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert!(skip, "Interactive mode should always skip password modal");
    }

    #[test]
    /// What: `should_skip_password_modal` returns true for interactive mode with doas.
    ///
    /// Inputs: Settings with `auth_mode = Interactive` and only doas available.
    ///
    /// Output: `true`.
    ///
    /// Details: Interactive mode is tool-agnostic — must skip for doas too.
    fn test_should_skip_password_modal_interactive_doas() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "doas");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        }

        let settings = crate::theme::Settings {
            auth_mode: AuthMode::Interactive,
            ..crate::theme::Settings::default()
        };
        let skip = should_skip_password_modal(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert!(
            skip,
            "Interactive mode should skip password modal for doas too"
        );
    }

    #[test]
    /// What: `should_skip_password_modal` returns false for default prompt mode.
    ///
    /// Inputs: Default settings.
    ///
    /// Output: `false`.
    ///
    /// Details: Prompt mode always shows the modal.
    fn test_should_skip_password_modal_prompt() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
            std::env::remove_var("PACSEA_TEST_AUTH_MODE");
        }

        let settings = crate::theme::Settings::default();
        let skip = should_skip_password_modal(&settings);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }

        assert!(!skip, "Prompt mode should always show password modal");
    }

    #[test]
    #[ignore = "Uses sudo with wrong password - may lock user out. Run with --ignored"]
    /// What: Test password validation handles invalid passwords.
    ///
    /// Inputs:
    /// - Invalid password string.
    ///
    /// Output:
    /// - Returns `Ok(false)` for invalid password.
    ///
    /// Details:
    /// - Verifies the function correctly identifies invalid passwords.
    /// - Skips assertion if passwordless sudo is configured (common in CI).
    /// - Marked as ignored to prevent user lockout from failed sudo attempts.
    fn test_validate_sudo_password_invalid() {
        // Skip test if passwordless sudo is configured (common in CI environments)
        if is_passwordless_sudo() {
            return;
        }

        // This test uses an obviously wrong password
        // It should return Ok(false) without panicking
        let result = validate_sudo_password("definitely_wrong_password_12345");
        // Result may be Ok(false) or Err depending on system configuration
        if let Ok(valid) = result {
            // Should be false for invalid password
            assert!(!valid);
        } else {
            // Error is acceptable (e.g., sudo not available)
        }
    }

    #[test]
    #[ignore = "Uses sudo with wrong password - may lock user out. Run with --ignored"]
    /// What: Test password validation handles empty passwords.
    ///
    /// Inputs:
    /// - Empty password string.
    ///
    /// Output:
    /// - Returns `Ok(false)` for empty password.
    ///
    /// Details:
    /// - Verifies the function correctly handles empty passwords.
    /// - Skips assertion if passwordless sudo is configured (common in CI).
    /// - Marked as ignored to prevent user lockout from failed sudo attempts.
    fn test_validate_sudo_password_empty() {
        // Skip test if passwordless sudo is configured (common in CI environments)
        if is_passwordless_sudo() {
            return;
        }

        let result = validate_sudo_password("");
        // Empty password should be invalid
        if let Ok(valid) = result {
            assert!(!valid);
        } else {
            // Error is acceptable
        }
    }

    #[test]
    #[ignore = "Uses sudo with wrong password - may lock user out. Run with --ignored"]
    /// What: Test password validation handles special characters.
    ///
    /// Inputs:
    /// - Password with special characters that need escaping.
    ///
    /// Output:
    /// - Handles special characters without panicking.
    ///
    /// Details:
    /// - Verifies the function correctly escapes special characters in passwords.
    /// - Marked as ignored to prevent user lockout from failed sudo attempts.
    fn test_validate_sudo_password_special_chars() {
        // Test with password containing special shell characters
        let passwords = vec![
            "pass'word",
            "pass\"word",
            "pass$word",
            "pass`word",
            "pass\\word",
        ];
        for pass in passwords {
            let result = validate_sudo_password(pass);
            // Just verify it doesn't panic
            let _ = result;
        }
    }

    #[test]
    #[ignore = "Uses sudo with wrong password - may lock user out. Run with --ignored"]
    /// What: Test password validation function signature.
    ///
    /// Inputs:
    /// - Various password strings.
    ///
    /// Output:
    /// - Returns Result<bool, String> as expected.
    ///
    /// Details:
    /// - Verifies the function returns the correct type.
    /// - Marked as ignored to prevent user lockout from failed sudo attempts.
    fn test_validate_sudo_password_signature() {
        let result: Result<bool, String> = validate_sudo_password("test");
        // Verify it returns the correct type
        let _ = result;
    }
}
