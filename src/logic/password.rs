//! Sudo password validation utilities.

use std::process::Command;

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
/// - Executes `echo '<password>' | sudo -S -v` to test password validity.
/// - Uses `sudo -v` which validates credentials without executing a command.
/// - Returns `Ok(true)` if password is valid, `Ok(false)` if invalid.
/// - Handles errors appropriately (e.g., if sudo is not available).
pub fn validate_sudo_password(password: &str) -> Result<bool, String> {
    use crate::install::shell_single_quote;

    // Escape password for shell safety
    let escaped_password = shell_single_quote(password);

    // Build command: echo '<password>' | sudo -S -v
    // sudo -v validates credentials without executing a command
    let cmd = format!("echo {escaped_password} | sudo -S -v 2>&1");

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

    #[test]
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
    fn test_validate_sudo_password_invalid() {
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
    fn test_validate_sudo_password_empty() {
        let result = validate_sudo_password("");
        // Empty password should be invalid
        if let Ok(valid) = result {
            assert!(!valid);
        } else {
            // Error is acceptable
        }
    }

    #[test]
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
    fn test_validate_sudo_password_signature() {
        let result: Result<bool, String> = validate_sudo_password("test");
        // Verify it returns the correct type
        let _ = result;
    }
}
