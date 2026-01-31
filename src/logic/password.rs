//! Sudo password validation utilities.

use std::process::Command;

/// What: Returns true only when running in integration test context.
///
/// Inputs:
/// - None (reads env var `PACSEA_INTEGRATION_TEST`).
///
/// Output:
/// - `true` if `PACSEA_INTEGRATION_TEST=1` is set, `false` otherwise.
///
/// Details:
/// - Used to gate `PACSEA_TEST_SUDO_PASSWORDLESS` so production never honors it.
/// - Integration tests set this env var so the test override is applied.
fn is_integration_test_context() -> bool {
    std::env::var("PACSEA_INTEGRATION_TEST").is_ok_and(|v| v == "1")
}

/// What: Check if passwordless sudo is available for the current user.
///
/// Inputs:
/// - None (checks system configuration).
///
/// Output:
/// - `Ok(true)` if passwordless sudo is available, `Ok(false)` if not, or `Err(String)` on error.
///
/// # Errors
///
/// - Returns `Err` if the check cannot be executed (e.g., sudo not installed).
///
/// Details:
/// - Uses `sudo -n true` to test if sudo can run without password.
/// - `-n`: Non-interactive mode (fails if password required).
/// - `true`: Simple command that always succeeds if sudo works.
/// - Returns `Ok(false)` if sudo is not available or requires a password.
/// - **Testing**: Only when `PACSEA_INTEGRATION_TEST=1` is set (test harness), the env var
///   `PACSEA_TEST_SUDO_PASSWORDLESS` is honored: "1" = available, "0" = unavailable.
///   In production (when `PACSEA_INTEGRATION_TEST` is not set), `PACSEA_TEST_SUDO_PASSWORDLESS`
///   is ignored so the only way to enable passwordless sudo is via `use_passwordless_sudo` in settings.
pub fn check_passwordless_sudo_available() -> Result<bool, String> {
    // Honor test override only in integration test context so production never honors it
    if is_integration_test_context()
        && let Ok(val) = std::env::var("PACSEA_TEST_SUDO_PASSWORDLESS")
    {
        tracing::debug!(
            "Using test override for passwordless sudo check: PACSEA_TEST_SUDO_PASSWORDLESS={}",
            val
        );
        return Ok(val == "1");
    }

    let status = Command::new("sudo")
        .args(["-n", "true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("Failed to check passwordless sudo: {e}"))?;

    Ok(status.success())
}

/// What: Check if passwordless sudo should be used based on settings and system availability.
///
/// Inputs:
/// - `settings`: Reference to the application settings.
///
/// Output:
/// - `true` if passwordless sudo should be used, `false` otherwise.
///
/// Details:
/// - First checks if `use_passwordless_sudo` is enabled in settings (safety barrier).
/// - If not enabled, returns `false` immediately without checking system availability.
/// - If enabled, checks if passwordless sudo is actually available on the system.
/// - Returns `true` only if both conditions are met.
/// - Logs the decision for debugging purposes.
/// - **Testing**: Only when `PACSEA_INTEGRATION_TEST=1` is set (test harness), the env var
///   `PACSEA_TEST_SUDO_PASSWORDLESS` is honored. In production this override is disabled.
#[must_use]
pub fn should_use_passwordless_sudo(settings: &crate::theme::Settings) -> bool {
    // Honor test override only in integration test context so production never honors it
    if is_integration_test_context()
        && let Ok(val) = std::env::var("PACSEA_TEST_SUDO_PASSWORDLESS")
    {
        tracing::debug!(
            "Using test override for should_use_passwordless_sudo: PACSEA_TEST_SUDO_PASSWORDLESS={}",
            val
        );
        return val == "1";
    }

    // Check if passwordless sudo is enabled in settings (safety barrier)
    if !settings.use_passwordless_sudo {
        tracing::debug!("Passwordless sudo disabled in settings, requiring password prompt");
        return false;
    }

    // Check if passwordless sudo is available on the system
    match check_passwordless_sudo_available() {
        Ok(true) => {
            tracing::info!("Passwordless sudo enabled in settings and available on system");
            true
        }
        Ok(false) => {
            tracing::debug!(
                "Passwordless sudo enabled in settings but not available on system, requiring password prompt"
            );
            false
        }
        Err(e) => {
            tracing::debug!(
                "Passwordless sudo check failed ({}), requiring password prompt",
                e
            );
            false
        }
    }
}

/// What: Validate a sudo password without executing any command.
///
/// Inputs:
/// - `password`: Password to validate.
///
/// Output:
/// - `Ok(true)` if password is valid, `Ok(false)` if invalid, or `Err(String)` on error.
///
/// # Errors
///
/// - Returns `Err` if the validation command cannot be executed (e.g., sudo not available).
///
/// Details:
/// - First invalidates cached sudo credentials with `sudo -k` to ensure fresh validation.
/// - Then executes `printf '%s\n' '<password>' | sudo -S -v` to test password validity.
/// - Uses `printf` instead of `echo` for more reliable password handling.
/// - Uses `sudo -v` which validates credentials without executing a command.
/// - Returns `Ok(true)` if password is valid, `Ok(false)` if invalid.
/// - Handles errors appropriately (e.g., if sudo is not available).
pub fn validate_sudo_password(password: &str) -> Result<bool, String> {
    use crate::install::shell_single_quote;

    // Escape password for shell safety
    let escaped_password = shell_single_quote(password);

    // Build command: sudo -k ; printf '%s\n' '<password>' | sudo -S -v
    // First, sudo -k invalidates any cached credentials to ensure fresh validation.
    // Without this, cached credentials could cause validation to succeed even with wrong password.
    // Use printf instead of echo for more reliable password handling.
    // sudo -v validates credentials without executing a command.
    let cmd = format!("sudo -k ; printf '%s\\n' {escaped_password} | sudo -S -v 2>&1");

    // Execute command
    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| format!("Failed to execute sudo validation: {e}"))?;

    // Check exit code
    // Exit code 0 means password is valid
    // Non-zero exit code means password is invalid or other error
    // This approach is language-independent as it relies on exit codes, not error messages
    if output.status.success() {
        Ok(true)
    } else {
        Ok(false)
    }
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
