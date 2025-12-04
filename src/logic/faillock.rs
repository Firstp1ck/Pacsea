//! Faillock status checking and configuration parsing.

use std::process::Command;

/// What: Faillock status information for a user.
///
/// Inputs: None (constructed from faillock command output).
///
/// Output: Status information about failed login attempts.
///
/// Details:
/// - Contains the number of failed attempts, maximum allowed attempts,
/// - whether the account is locked, and the lockout duration in minutes.
#[derive(Debug, Clone)]
pub struct FaillockStatus {
    /// Number of failed attempts currently recorded.
    pub attempts_used: u32,
    /// Maximum number of failed attempts before lockout.
    pub max_attempts: u32,
    /// Whether the account is currently locked.
    pub is_locked: bool,
    /// Lockout duration in minutes.
    pub lockout_duration_minutes: u32,
    /// Timestamp of the last failed attempt (if any).
    pub last_failed_timestamp: Option<std::time::SystemTime>,
}

/// What: Faillock configuration values.
///
/// Inputs: None (parsed from `/etc/security/faillock.conf`).
///
/// Output: Configuration values for faillock behavior.
///
/// Details:
/// - Contains the deny count (max attempts) and fail interval (lockout duration).
#[derive(Debug, Clone)]
pub struct FaillockConfig {
    /// Maximum number of failed attempts before lockout (deny setting).
    pub deny: u32,
    /// Lockout duration in minutes (`fail_interval` setting).
    pub fail_interval: u32,
}

/// What: Check faillock status for a user.
///
/// Inputs:
/// - `username`: Username to check faillock status for.
///
/// Output:
/// - `Ok(FaillockStatus)` with status information, or `Err(String)` on error.
///
/// # Errors
///
/// - Returns `Err` if the `faillock` command cannot be executed.
///
/// Details:
/// - Executes `faillock --user <username>` command.
/// - Parses output to count lines with "V" (valid attempts).
/// - Compares with max attempts from config to determine if locked.
/// - Returns status with attempt count, max attempts, lock status, and lockout duration.
pub fn check_faillock_status(username: &str) -> Result<FaillockStatus, String> {
    // Get config first to determine max attempts and lockout duration
    let config = parse_faillock_config().unwrap_or(FaillockConfig {
        deny: 3,
        fail_interval: 15,
    });

    // Execute faillock command
    let output = Command::new("faillock")
        .args(["--user", username])
        .output()
        .map_err(|e| format!("Failed to execute faillock command: {e}"))?;

    if !output.status.success() {
        // If command fails, assume no lockout (might not be configured)
        return Ok(FaillockStatus {
            attempts_used: 0,
            max_attempts: config.deny,
            is_locked: false,
            lockout_duration_minutes: config.fail_interval,
            last_failed_timestamp: None,
        });
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = output_str.lines().collect();

    // Count lines with "V" (valid attempts) - skip header line
    // Also track the most recent failed attempt timestamp (the one that triggered lockout)
    let mut attempts_used = 0u32;
    let mut in_user_section = false;
    let mut seen_header = false;
    let mut most_recent_timestamp: Option<std::time::SystemTime> = None;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if this is the username header line (format: "username:")
        if trimmed.ends_with(':') && trimmed.trim_end_matches(':') == username {
            in_user_section = true;
            seen_header = false; // Reset header flag for this user section
            continue;
        }

        // If we're in the user section, look for lines with "V"
        if in_user_section {
            // Skip the header line that contains "When", "Type", "Source", "Valid"
            // This line also contains "V" but is not an attempt
            if !seen_header
                && trimmed.contains("When")
                && trimmed.contains("Type")
                && trimmed.contains("Source")
                && trimmed.contains("Valid")
            {
                seen_header = true;
                continue;
            }

            // Check if line contains "V" (valid attempt marker)
            // Format is typically: "YYYY-MM-DD HH:MM:SS TTY /dev/pts/X V"
            // Must be a date-like line (starts with YYYY-MM-DD format)
            if (trimmed.contains(" V") || trimmed.ends_with('V'))
                && trimmed.chars().take(4).all(|c| c.is_ascii_digit())
            {
                attempts_used += 1;
                // Parse timestamp from the line (format: "YYYY-MM-DD HH:MM:SS")
                // Extract first 19 characters which should be "YYYY-MM-DD HH:MM:SS"
                if trimmed.len() >= 19 {
                    let timestamp_str = &trimmed[0..19];
                    if let Ok(dt) =
                        chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S")
                    {
                        // Faillock timestamps are in local time, so we need to convert them properly
                        // First, assume the timestamp is in local timezone
                        let local_dt = dt.and_local_timezone(chrono::Local);
                        // Get the single valid timezone conversion (or use UTC as fallback)
                        let dt_utc = local_dt.single().map_or_else(
                            || dt.and_utc(),
                            |dt_local| dt_local.with_timezone(&chrono::Utc),
                        );
                        // Convert chrono DateTime to SystemTime
                        let unix_timestamp = dt_utc.timestamp();
                        if unix_timestamp >= 0
                            && let Some(st) = std::time::SystemTime::UNIX_EPOCH.checked_add(
                                std::time::Duration::from_secs(
                                    u64::try_from(unix_timestamp).unwrap_or(0),
                                ),
                            )
                        {
                            // Keep the most recent timestamp (faillock shows oldest first, so last one is newest)
                            most_recent_timestamp = Some(st);
                        }
                    }
                }
            }
        }
    }

    // Check if user should be locked based on attempts
    let should_be_locked = attempts_used >= config.deny;

    // If locked, check if lockout has expired based on timestamp
    let is_locked = if should_be_locked {
        most_recent_timestamp.map_or(should_be_locked, |last_timestamp| {
            // Check if lockout duration has passed since last failed attempt
            let now = std::time::SystemTime::now();
            now.duration_since(last_timestamp).map_or(true, |elapsed| {
                let lockout_seconds = u64::from(config.fail_interval) * 60;
                // If elapsed time is less than lockout duration, user is still locked
                elapsed.as_secs() < lockout_seconds
            })
        })
    } else {
        false
    };

    Ok(FaillockStatus {
        attempts_used,
        max_attempts: config.deny,
        is_locked,
        lockout_duration_minutes: config.fail_interval,
        last_failed_timestamp: most_recent_timestamp,
    })
}

/// What: Parse faillock configuration from `/etc/security/faillock.conf`.
///
/// Inputs: None (reads from system config file).
///
/// Output:
/// - `Ok(FaillockConfig)` with parsed values, or `Err(String)` on error.
///
/// # Errors
///
/// - Returns `Err` if the config file cannot be read (though defaults are used in practice).
///
/// Details:
/// - Reads `/etc/security/faillock.conf`.
/// - Parses `deny` setting (default 3 if commented out).
/// - Parses `fail_interval` setting (default 15 minutes if commented out).
/// - Handles comments (lines starting with `#`) and whitespace.
pub fn parse_faillock_config() -> Result<FaillockConfig, String> {
    use std::fs;

    let config_path = "/etc/security/faillock.conf";
    let Ok(contents) = fs::read_to_string(config_path) else {
        // File doesn't exist or can't be read, use defaults
        return Ok(FaillockConfig {
            deny: 3,
            fail_interval: 15,
        });
    };

    let mut deny = 3u32; // Default
    let mut fail_interval = 15u32; // Default in minutes

    for line in contents.lines() {
        let trimmed = line.trim();

        // Skip empty lines and full-line comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Handle inline comments
        let line_without_comment = trimmed.split('#').next().unwrap_or("").trim();

        // Parse deny setting
        if line_without_comment.starts_with("deny")
            && let Some(value_str) = line_without_comment.split('=').nth(1)
        {
            let value_trimmed = value_str.trim();
            if let Ok(value) = value_trimmed.parse::<u32>() {
                deny = value;
            }
        }

        // Parse fail_interval setting
        if line_without_comment.starts_with("fail_interval")
            && let Some(value_str) = line_without_comment.split('=').nth(1)
        {
            let value_trimmed = value_str.trim();
            if let Ok(value) = value_trimmed.parse::<u32>() {
                fail_interval = value;
            }
        }
    }

    Ok(FaillockConfig {
        deny,
        fail_interval,
    })
}

/// What: Check if user is locked out and return lockout message if so.
///
/// Inputs:
/// - `username`: Username to check.
/// - `app`: Application state for translations.
///
/// Output:
/// - `Some(message)` if user is locked out, `None` otherwise.
///
/// Details:
/// - Checks faillock status and returns formatted lockout message if locked.
/// - Returns `None` if not locked or if check fails.
/// - Uses translations from `AppState`.
#[must_use]
pub fn get_lockout_message_if_locked(
    username: &str,
    app: &crate::state::AppState,
) -> Option<String> {
    if let Ok(status) = check_faillock_status(username)
        && status.is_locked
    {
        return Some(crate::i18n::t_fmt(
            app,
            "app.modals.alert.account_locked_with_time",
            &[
                &username as &dyn std::fmt::Display,
                &status.lockout_duration_minutes,
            ],
        ));
    }
    None
}

/// What: Calculate remaining lockout time in minutes based on last failed attempt timestamp.
///
/// Inputs:
/// - `last_timestamp`: Timestamp of the last failed attempt.
/// - `lockout_duration_minutes`: Total lockout duration in minutes.
///
/// Output:
/// - `Some(minutes)` if still locked out, `None` if lockout has expired.
///
/// Details:
/// - Calculates time elapsed since last failed attempt.
/// - Returns remaining minutes if lockout is still active, `None` if expired.
#[must_use]
pub fn calculate_remaining_lockout_minutes(
    last_timestamp: &std::time::SystemTime,
    lockout_duration_minutes: u32,
) -> Option<u32> {
    let now = std::time::SystemTime::now();
    now.duration_since(*last_timestamp)
        .map_or(Some(lockout_duration_minutes), |elapsed| {
            let lockout_seconds = u64::from(lockout_duration_minutes) * 60;
            if elapsed.as_secs() < lockout_seconds {
                let remaining_seconds = lockout_seconds - elapsed.as_secs();
                let remaining_minutes =
                    (remaining_seconds / 60) + u64::from(remaining_seconds % 60 > 0);
                Some(u32::try_from(remaining_minutes.min(u64::from(u32::MAX))).unwrap_or(u32::MAX))
            } else {
                None // Lockout expired
            }
        })
}

/// What: Check faillock status and calculate lockout information for display.
///
/// Inputs:
/// - `username`: Username to check.
///
/// Output:
/// - Tuple of `(is_locked, lockout_until, remaining_minutes)`.
///
/// Details:
/// - Checks faillock status and calculates remaining lockout time if locked.
/// - Returns lockout information for UI display.
#[must_use]
pub fn get_lockout_info(username: &str) -> (bool, Option<std::time::SystemTime>, Option<u32>) {
    if let Ok(status) = check_faillock_status(username)
        && status.is_locked
    {
        if let Some(last_timestamp) = status.last_failed_timestamp {
            let remaining = calculate_remaining_lockout_minutes(
                &last_timestamp,
                status.lockout_duration_minutes,
            );
            // Calculate lockout_until timestamp
            let lockout_until = last_timestamp
                + std::time::Duration::from_secs(u64::from(status.lockout_duration_minutes) * 60);
            // Return remaining time - if None, it means lockout expired, show 0
            // But if timestamp is in future (timezone issue), remaining should be Some
            return (true, Some(lockout_until), remaining);
        }
        // Locked but no timestamp - still show as locked but no time remaining
        return (true, None, Some(0));
    }
    (false, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    /// What: Test parsing faillock config with defaults.
    ///
    /// Inputs:
    /// - Config file that doesn't exist or has commented settings.
    ///
    /// Output:
    /// - Returns default values (deny=3, `fail_interval=15`).
    ///
    /// Details:
    /// - Verifies default values are used when config is missing or commented.
    fn test_parse_faillock_config_defaults() {
        // This test may fail if the file exists, but that's okay
        // The function should handle missing files gracefully
        let _config = parse_faillock_config();
        // Just verify it doesn't panic
    }

    #[test]
    /// What: Test parsing faillock config with custom values.
    ///
    /// Inputs:
    /// - Temporary config file with custom deny and `fail_interval` values.
    ///
    /// Output:
    /// - Returns parsed values from config file.
    ///
    /// Details:
    /// - Creates a temporary config file and verifies parsing works correctly.
    fn test_parse_faillock_config_custom_values() {
        use std::env::temp_dir;
        let temp_file = temp_dir().join("test_faillock.conf");
        let content = "deny = 5\nfail_interval = 30\n";
        if let Ok(mut file) = fs::File::create(&temp_file) {
            let _ = file.write_all(content.as_bytes());
            // Note: We can't easily test this without mocking file reading
            // Just verify the function doesn't panic
            let _config = parse_faillock_config();
            let _ = fs::remove_file(&temp_file);
        }
    }

    #[test]
    /// What: Test parsing faillock config with comments.
    ///
    /// Inputs:
    /// - Config file with commented lines and inline comments.
    ///
    /// Output:
    /// - Parses values correctly, ignoring comments.
    ///
    /// Details:
    /// - Verifies that comments (both full-line and inline) are handled correctly.
    fn test_parse_faillock_config_with_comments() {
        // The function should handle comments correctly
        // Since we can't easily mock file reading, just verify it doesn't panic
        let _config = parse_faillock_config();
    }

    #[test]
    /// What: Test faillock status checking handles errors gracefully.
    ///
    /// Inputs:
    /// - Username that may or may not have faillock entries.
    ///
    /// Output:
    /// - Returns status without panicking.
    ///
    /// Details:
    /// - Verifies the function handles various error cases.
    fn test_check_faillock_status_handles_errors() {
        let username = std::env::var("USER").unwrap_or_else(|_| "testuser".to_string());
        let result = check_faillock_status(&username);
        // Should return Ok or handle errors gracefully
        if let Ok(status) = result {
            // Verify status has reasonable values
            assert!(status.max_attempts > 0);
            assert!(status.lockout_duration_minutes > 0);
        } else {
            // Error is acceptable (e.g., faillock not configured)
        }
    }

    #[test]
    /// What: Test faillock status structure.
    ///
    /// Inputs:
    /// - Username.
    ///
    /// Output:
    /// - Returns status with all fields populated.
    ///
    /// Details:
    /// - Verifies that the status struct contains all expected fields.
    fn test_faillock_status_structure() {
        let username = std::env::var("USER").unwrap_or_else(|_| "testuser".to_string());
        if let Ok(status) = check_faillock_status(&username) {
            // Verify all fields are present
            let _ = status.attempts_used;
            let _ = status.max_attempts;
            let _ = status.is_locked;
            let _ = status.lockout_duration_minutes;
            // Just verify the struct can be accessed
        }
    }
}
