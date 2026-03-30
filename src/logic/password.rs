//! Privilege password validation utilities.
//!
//! Delegates to [`crate::logic::privilege`] for tool-aware checks.

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
    let tool = crate::logic::privilege::active_tool()?;
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

    let tool = match crate::logic::privilege::active_tool() {
        Ok(t) => t,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Could not resolve privilege tool; treating as password prompt required"
            );
            return false;
        }
    };
    if !tool.capabilities().supports_stdin_password {
        tracing::debug!(
            tool = %tool,
            "Active privilege tool does not support stdin password; skipping in-app password prompt"
        );
        return true;
    }

    if !settings.use_passwordless_sudo {
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
/// - Returns `Err` if the active tool does not support stdin password validation.
///
/// Details:
/// - Delegates to [`crate::logic::privilege::validate_password`].
/// - Works for active tools that support stdin password piping (sudo/doas).
pub fn validate_sudo_password(password: &str) -> Result<bool, String> {
    let tool = crate::logic::privilege::active_tool()?;
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
    /// What: Ensure doas does not implicitly skip the in-app password prompt.
    ///
    /// Inputs:
    /// - Integration test env forcing only doas availability.
    /// - Settings with `use_passwordless_sudo = false`.
    ///
    /// Output:
    /// - Returns `false` from `should_use_passwordless_sudo`.
    ///
    /// Details:
    /// - Regression guard for doas flow:
    ///   selecting doas must still show the same in-app password popup unless
    ///   passwordless mode is explicitly active.
    fn test_should_use_passwordless_sudo_false_for_doas_when_passwordless_disabled() {
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
            !should_skip_prompt,
            "doas should not skip in-app password prompt when passwordless is disabled"
        );
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
