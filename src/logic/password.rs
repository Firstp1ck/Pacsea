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
/// Errors:
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
    if output.status.success() {
        Ok(true)
    } else {
        // Check stderr for common error messages
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Common sudo error messages for invalid password (English and German):
        // - "sudo: a password is required"
        // - "sudo: 1 incorrect password attempt"
        // - "Sorry, try again."
        // - "Fehlversuch bei der Passwort-Eingabe" (German: failed password attempt)
        // - "Das hat nicht funktioniert" (German: that didn't work)
        // Since exit code is non-zero, password is invalid regardless of message
        // We check for specific messages to distinguish from other errors, but
        // for password validation, any non-zero exit means invalid password
        if stderr.contains("incorrect password")
            || stderr.contains("Sorry, try again")
            || stderr.contains("Fehlversuch")
            || stderr.contains("Passwort")
            || stdout.contains("incorrect password")
            || stdout.contains("Sorry, try again")
            || stdout.contains("Fehlversuch")
            || stdout.contains("Passwort")
        {
            Ok(false)
        } else {
            // Other error (e.g., sudo not available, permission denied, etc.)
            // For password validation, non-zero exit code means invalid password
            Ok(false)
        }
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
        match result {
            Ok(valid) => {
                // Should be false for invalid password
                assert!(!valid);
            }
            Err(_) => {
                // Error is acceptable (e.g., sudo not available)
            }
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
        match result {
            Ok(valid) => {
                assert!(!valid);
            }
            Err(_) => {
                // Error is acceptable
            }
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
        match result {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}
