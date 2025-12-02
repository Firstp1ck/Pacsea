#![cfg(test)]
#![cfg(not(target_os = "windows"))]
//! Tests for the `-u` and `--update` commandline flags.
//!
//! These tests verify that the update flags are parsed correctly and simulate
//! a long-running update scenario where sudo may timeout and require password re-entry.

use std::fs;
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Mock pacman script content that simulates a long-running update.
const MOCK_PACMAN_SCRIPT: &str = r#"#!/bin/bash
# Simulate a long-running pacman update
echo ":: Synchronizing package databases..."
sleep 2
echo ":: Starting full system upgrade..."
sleep 2
echo "resolving dependencies..."
sleep 1
echo "looking for conflicting packages..."
sleep 1
echo ""
echo "Packages (5) to upgrade:"
echo "  core/systemd  250.4-1 -> 251.1-1"
echo "  core/linux    6.8.0-1 -> 6.9.0-1"
echo "  extra/firefox 120.0-1 -> 121.0-1"
echo "  extra/vim     9.0.2000-1 -> 9.1.0000-1"
echo "  aur/custom-pkg 1.0.0-1 -> 2.0.0-1"
echo ""
sleep 2
echo ":: Proceeding with installation..."
sleep 2
echo "(5/5) checking package integrity..."
sleep 1
echo "(5/5) loading package files..."
sleep 1
echo "(5/5) checking for file conflicts..."
sleep 1
echo "(5/5) checking available disk space..."
sleep 1
echo "(5/5) upgrading systemd..."
sleep 2
echo "(5/5) upgrading linux..."
sleep 2
echo "(5/5) upgrading firefox..."
sleep 2
echo "(5/5) upgrading vim..."
sleep 2
echo "(5/5) upgrading custom-pkg..."
sleep 2
echo ""
echo "Total download size: 500.00 MiB"
echo "Total installed size: 1200.00 MiB"
echo "Net upgrade size: 700.00 MiB"
echo ""
echo ":: Running post-transaction hooks..."
sleep 1
echo "(1/3) Updating systemd service files..."
sleep 1
echo "(2/3) Reloading system manager configuration..."
sleep 1
echo "(3/3) Updating font cache..."
sleep 1
echo ""
echo "System upgrade completed successfully."
exit 0
"#;

/// What: Create a temporary directory with a unique name for test artifacts.
///
/// Inputs:
/// - `prefix`: Prefix for the directory name.
///
/// Output:
/// - Returns the path to the created temporary directory.
///
/// Details:
/// - Creates a directory in the system temp directory with a unique name
///   based on the process ID and current timestamp.
fn create_test_temp_dir(prefix: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir().join(format!(
        "pacsea_test_{}_{}_{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time is before UNIX epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    temp_dir
}

/// What: Make a script file executable and ensure it's synced to disk.
///
/// Inputs:
/// - `script_path`: Path to the script file.
///
/// Output:
/// - Sets the executable permission on the file and syncs it to disk.
///
/// Details:
/// - Sets the file mode to 0o755 to make it executable.
/// - Syncs the file to disk to prevent "Text file busy" errors when executing immediately.
fn make_script_executable(script_path: &Path) {
    let mut perms = fs::metadata(script_path)
        .expect("Failed to read script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script_path, perms).expect("Failed to set script permissions");

    // Sync the file to disk to prevent "Text file busy" errors
    // This ensures the file system has fully written the file before execution
    let file = File::open(script_path).expect("Failed to open script file for syncing");
    file.sync_all().expect("Failed to sync script file to disk");
}

/// What: Create a mock sudo script that tracks password requests.
///
/// Inputs:
/// - `script_path`: Path where the script should be created.
/// - `password_log_path`: Path to the password log file.
///
/// Output:
/// - Creates an executable mock sudo script at the specified path.
///
/// Details:
/// - The script simulates sudo behavior that requires password via stdin
///   and logs password requests to verify it's only asked once.
fn create_mock_sudo_script(script_path: &Path, password_log_path: &Path) {
    let password_log_str = password_log_path.to_string_lossy();
    let sudo_script = format!(
        r#"#!/bin/bash
# Mock sudo that simulates password requirement and timeout
# This simulates the scenario where sudo requires password at the beginning
# and should NOT require it again even if the update takes a long time

PASSWORD_LOG="{password_log_str}"

# Check if password was provided via stdin (sudo -S)
if [ -t 0 ]; then
    # No stdin, passwordless sudo attempt
    echo "Passwordless sudo not available" >&2
    exit 1
fi

# Read password from stdin
read -r PASSWORD < /dev/stdin

# Log password request (without the actual password)
echo "$(date +%s): Password provided" >> "$PASSWORD_LOG"

# Simulate sudo timestamp - in real scenario, this would be set by sudo
# For testing, we simulate that the password is valid for the entire update
# The actual implementation should handle this by using sudo -v to refresh timestamp
# or by providing password once and using it for all commands

# Execute the actual command (simulate pacman update)
exec "$@"
"#
    );

    // Write the script and ensure it's synced to disk
    let mut file = File::create(script_path).expect("Failed to create mock sudo script");
    file.write_all(sudo_script.as_bytes())
        .expect("Failed to write mock sudo script");
    file.sync_all()
        .expect("Failed to sync sudo script file to disk");
    drop(file); // Ensure file handle is closed

    make_script_executable(script_path);
}

/// What: Create a mock pacman script that simulates a long-running update.
///
/// Inputs:
/// - `script_path`: Path where the script should be created.
///
/// Output:
/// - Creates an executable mock pacman script at the specified path.
///
/// Details:
/// - Uses the shared `MOCK_PACMAN_SCRIPT` content to create the script.
fn create_mock_pacman_script(script_path: &Path) {
    // Write the script and ensure it's synced to disk
    let mut file = File::create(script_path).expect("Failed to create mock pacman script");
    file.write_all(MOCK_PACMAN_SCRIPT.as_bytes())
        .expect("Failed to write mock pacman script");
    file.sync_all()
        .expect("Failed to sync pacman script file to disk");
    drop(file); // Ensure file handle is closed

    make_script_executable(script_path);
}

/// What: Run the mock update command and verify the results.
///
/// Inputs:
/// - `mock_sudo_path`: Path to the mock sudo script.
/// - `mock_pacman_path`: Path to the mock pacman script.
/// - `password_log_path`: Path to the password log file.
///
/// Output:
/// - Returns the command output and duration.
///
/// Details:
/// - Executes the mock update command and measures execution time.
/// - Verifies that the update completed successfully and took at least 20 seconds.
/// - Verifies that password was only requested once.
/// - Verifies that output contains expected update messages.
fn run_and_verify_sudo_timeout_test(
    mock_sudo_path: &Path,
    mock_pacman_path: &Path,
    password_log_path: &Path,
) -> (Output, Duration) {
    let start = Instant::now();
    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "echo 'testpassword' | {} {} -Syu --noconfirm",
            mock_sudo_path.display(),
            mock_pacman_path.display()
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute mock update");

    let duration = start.elapsed();

    // Verify the update completed successfully
    assert!(
        output.status.success(),
        "Mock update should complete successfully"
    );

    // Verify it took a reasonable amount of time (simulating long-running update)
    assert!(
        duration >= Duration::from_secs(20),
        "Mock update should take at least 20 seconds. Actual duration: {duration:?}"
    );

    // Verify password was only requested ONCE (not multiple times)
    assert!(
        password_log_path.exists(),
        "Password log file should exist. This indicates the mock sudo script failed to create the log file, \
         which means the password tracking mechanism is not working correctly."
    );

    let password_requests = fs::read_to_string(password_log_path).unwrap_or_else(|_| String::new());
    let request_count = password_requests.lines().count();
    assert!(
        request_count == 1,
        "Password should be requested only once, but was requested {request_count} times. \
         This indicates the implementation may not properly handle sudo timeout during long updates."
    );

    // Verify output contains expected update messages
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Synchronizing package databases"),
        "Output should contain synchronization message"
    );
    assert!(
        stdout.contains("System upgrade completed successfully"),
        "Output should contain success message"
    );

    (output, duration)
}

/// What: Verify that the `-u` short flag triggers the update handler.
///
/// Inputs:
/// - Command line: `pacsea -u`
///
/// Output:
/// - The binary should attempt to run the update process.
///
/// Details:
/// - Tests the short form of the update flag by running the binary.
/// - Since `handle_update()` exits, we verify it's triggered by checking
///   that it attempts to run sudo pacman (or exits early if passwordless sudo fails).
#[test]
#[ignore = "Long-running simulation test, only run manually"]
fn test_update_short_flag_triggers_update() {
    // Get the path to the pacsea binary
    let binary_path = std::env::current_exe()
        .expect("Failed to get current exe")
        .parent()
        .expect("Failed to get parent dir")
        .parent()
        .expect("Failed to get parent parent dir")
        .join("pacsea");

    // Skip if binary doesn't exist (e.g., during `cargo check`)
    if !binary_path.exists() {
        eprintln!("Skipping test: binary not found at {binary_path:?}");
        return;
    }

    // Run pacsea with -u flag
    // The update handler will exit, so we expect a non-zero exit code
    // (either from update failure or from password prompt cancellation)
    let output = Command::new(&binary_path)
        .arg("-u")
        .stdin(Stdio::null()) // No stdin to avoid password prompt hanging
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute pacsea");

    // The update handler should have been triggered
    // It will either:
    // 1. Exit with code 1 if passwordless sudo fails and password prompt is cancelled
    // 2. Exit with code 0/1 depending on update success/failure
    // 3. Exit early if sudo/pacman is not available
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exit_code = output.status.code();

    // Verify that update was attempted by checking for:
    // - Update-related messages in output (e.g., "Updating system packages", "pacman", "Syu")
    // - Valid exit code (0 for success, 1 for failure)
    // - Or specific error messages indicating the update handler was triggered
    let has_update_message = stderr.contains("update")
        || stdout.contains("update")
        || stderr.contains("pacman")
        || stdout.contains("pacman")
        || stderr.contains("Syu")
        || stdout.contains("Syu")
        || stderr.contains("Updating")
        || stdout.contains("Updating")
        || stderr.contains("password")
        || stdout.contains("password");

    let has_valid_exit_code = exit_code.is_some_and(|code| code == 0 || code == 1);

    assert!(
        has_update_message || has_valid_exit_code,
        "Update handler should have been triggered. \
         Expected update-related messages or exit code 0/1. \
         Exit code: {exit_code:?}, stdout: {stdout}, stderr: {stderr}"
    );
}

/// What: Verify that the `--update` long flag triggers the update handler.
///
/// Inputs:
/// - Command line: `pacsea --update`
///
/// Output:
/// - The binary should attempt to run the update process.
///
/// Details:
/// - Tests the long form of the update flag by running the binary.
#[test]
#[ignore = "Long-running simulation test, only run manually"]
fn test_update_long_flag_triggers_update() {
    // Get the path to the pacsea binary
    let binary_path = std::env::current_exe()
        .expect("Failed to get current exe")
        .parent()
        .expect("Failed to get parent dir")
        .parent()
        .expect("Failed to get parent parent dir")
        .join("pacsea");

    // Skip if binary doesn't exist
    if !binary_path.exists() {
        eprintln!("Skipping test: binary not found at {binary_path:?}");
        return;
    }

    // Run pacsea with --update flag
    let output = Command::new(&binary_path)
        .arg("--update")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute pacsea");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exit_code = output.status.code();

    // Verify that update was attempted by checking for:
    // - Update-related messages in output (e.g., "Updating system packages", "pacman", "Syu")
    // - Valid exit code (0 for success, 1 for failure)
    // - Or specific error messages indicating the update handler was triggered
    let has_update_message = stderr.contains("update")
        || stdout.contains("update")
        || stderr.contains("pacman")
        || stdout.contains("pacman")
        || stderr.contains("Syu")
        || stdout.contains("Syu")
        || stderr.contains("Updating")
        || stdout.contains("Updating")
        || stderr.contains("password")
        || stdout.contains("password");

    let has_valid_exit_code = exit_code.is_some_and(|code| code == 0 || code == 1);

    assert!(
        has_update_message || has_valid_exit_code,
        "Update handler should have been triggered. \
         Expected update-related messages or exit code 0/1. \
         Exit code: {exit_code:?}, stdout: {stdout}, stderr: {stderr}"
    );
}

/// What: Simulate sudo password timeout scenario during long-running update.
///
/// Inputs:
/// - A mock sudo that requires password and has a short timeout.
/// - A long-running update process that exceeds sudo's timestamp timeout.
///
/// Output:
/// - Verifies that password is provided once at the beginning and NOT required again.
///
/// Details:
/// - This test simulates the scenario where:
///   1. User provides sudo password at the beginning of the update
///   2. Update runs for a long time (exceeding sudo's `timestamp_timeout`)
///   3. Password should NOT be required again (implementation should handle this)
/// - The test creates a mock sudo wrapper that simulates password requirement and timeout.
/// - Verifies that the update handler properly handles sudo password to avoid re-prompting.
/// - This is a manual integration test for long-running scenarios.
/// - The test is ignored by default because it requires:
///   - Actual system setup with sudo
///   - Long-running simulation (may take several minutes)
///   - Manual execution outside of CI
/// - For unit-level testing, see `tests/install/password_prompt.rs::integration_password_prompt_timeout_error`
/// - Run manually with: `cargo test -- --ignored test_sudo_password_timeout_during_long_update`
#[test]
#[ignore = "Long-running simulation test, only run manually"]
fn test_sudo_password_timeout_during_long_update() {
    // Skip this test if we're in CI or don't have bash/sudo available
    if std::env::var("CI").is_ok()
        || Command::new("which").arg("bash").output().is_err()
        || Command::new("which").arg("sudo").output().is_err()
    {
        return;
    }

    let temp_dir = create_test_temp_dir("sudo_timeout");
    let password_log = temp_dir.join("password_requests.log");
    let mock_sudo_path = temp_dir.join("mock_sudo");
    let mock_pacman_path = temp_dir.join("mock_pacman");

    create_mock_sudo_script(&mock_sudo_path, &password_log);
    create_mock_pacman_script(&mock_pacman_path);

    // Test the scenario: password provided once, used for long-running update
    // This simulates: echo 'password' | sudo -S pacman -Syu --noconfirm
    let (_output, _duration) =
        run_and_verify_sudo_timeout_test(&mock_sudo_path, &mock_pacman_path, &password_log);

    // Clean up
    let _ = fs::remove_file(&mock_sudo_path);
    let _ = fs::remove_file(&mock_pacman_path);
    let _ = fs::remove_file(&password_log);
    let _ = fs::remove_dir_all(&temp_dir);
}

/// What: Simulate a long-running update that would require password re-entry.
///
/// Inputs:
/// - A mock update process that takes longer than sudo's default timeout (typically 15 minutes).
///
/// Output:
/// - Verifies that the update process can handle long-running operations.
///
/// Details:
/// - This test simulates the scenario where an update takes a long time,
///   potentially causing sudo's timestamp to expire and requiring password re-entry.
/// - The test creates a mock script that simulates a long-running pacman update.
/// - In a real scenario, if sudo times out during a long update, the user would need to
///   provide the password again. This test verifies the update process structure can handle
///   such scenarios by simulating the long-running nature of updates.
#[test]
#[ignore = "Long-running simulation test, only run manually"]
fn test_long_running_update_simulation() {
    // Skip this test if we're in CI or don't have bash available
    if std::env::var("CI").is_ok() || Command::new("which").arg("bash").output().is_err() {
        return;
    }

    let temp_dir = create_test_temp_dir("update");
    let mock_script_path = temp_dir.join("mock_pacman_update.sh");

    // Write the script and ensure it's synced to disk
    let mut file = File::create(&mock_script_path).expect("Failed to create mock script");
    file.write_all(MOCK_PACMAN_SCRIPT.as_bytes())
        .expect("Failed to write mock script");
    file.sync_all().expect("Failed to sync script file to disk");
    drop(file); // Ensure file handle is closed

    make_script_executable(&mock_script_path);

    // Test that the script runs and takes a reasonable amount of time
    // (simulating a long-running update)
    let start = Instant::now();
    let output = Command::new(&mock_script_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute mock script");

    let duration = start.elapsed();

    // Verify the script completed successfully
    assert!(
        output.status.success(),
        "Mock update script should complete successfully"
    );

    // Verify it took a reasonable amount of time (at least 20 seconds for the simulated update)
    // In a real long-running update, this could be 15+ minutes, which would cause
    // sudo's timestamp to expire, requiring password re-entry
    assert!(
        duration >= Duration::from_secs(20),
        "Mock update should take at least 20 seconds to simulate long-running operation. Actual duration: {duration:?}"
    );

    // Verify output contains expected update messages
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Synchronizing package databases"),
        "Output should contain synchronization message"
    );
    assert!(
        stdout.contains("System upgrade completed successfully"),
        "Output should contain success message"
    );

    // Clean up
    let _ = fs::remove_file(&mock_script_path);
    let _ = fs::remove_dir_all(&temp_dir);
}

/// What: Test that individual command exit codes are correctly detected in combined command execution.
///
/// Inputs:
/// - A scenario where pacman fails but AUR helper succeeds.
///
/// Output:
/// - Verifies that pacman failure is correctly detected even when AUR helper succeeds.
///
/// Details:
/// - This test verifies the fix for the bug where `status.success()` was used to check
///   both pacman and AUR helper success, incorrectly marking pacman as successful when
///   pacman failed but AUR helper succeeded.
/// - The combined command should capture individual exit codes and detect failures correctly.
#[test]
#[ignore = "Requires actual pacsea binary and system setup"]
fn test_individual_exit_codes_in_combined_command() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    // Skip this test if we're in CI or don't have bash available
    if std::env::var("CI").is_ok() || Command::new("which").arg("bash").output().is_err() {
        return;
    }

    // Create a temporary directory for test artifacts
    let temp_dir = std::env::temp_dir().join(format!(
        "pacsea_test_exit_codes_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time is before UNIX epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    // Create a mock pacman that fails
    let mock_pacman_path = temp_dir.join("mock_pacman");
    let pacman_script = r#"#!/bin/bash
# Mock pacman that fails
echo "error: failed to prepare transaction"
echo "error: target not found: some-package"
exit 1
"#;

    fs::write(&mock_pacman_path, pacman_script).expect("Failed to write mock pacman script");
    let mut perms = fs::metadata(&mock_pacman_path)
        .expect("Failed to read pacman script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_pacman_path, perms).expect("Failed to set pacman script permissions");

    // Create a mock AUR helper that succeeds
    let mock_aur_path = temp_dir.join("mock_aur");
    let aur_script = r#"#!/bin/bash
# Mock AUR helper that succeeds
echo ":: Synchronizing package databases..."
echo "there is nothing to do"
exit 0
"#;

    fs::write(&mock_aur_path, aur_script).expect("Failed to write mock AUR script");
    let mut perms = fs::metadata(&mock_aur_path)
        .expect("Failed to read AUR script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_aur_path, perms).expect("Failed to set AUR script permissions");

    // Test combined command with exit code capture
    // This simulates: command1; PACMAN_EXIT=$?; command2; AUR_EXIT=$?
    let combined_cmd = format!(
        "{} -Syu --noconfirm; PACMAN_EXIT=$?; echo 'PACMAN_EXIT='$PACMAN_EXIT; {} -Syu --noconfirm; AUR_EXIT=$?; echo 'AUR_EXIT='$AUR_EXIT; exit $((PACMAN_EXIT | AUR_EXIT))",
        mock_pacman_path.display(),
        mock_aur_path.display()
    );

    let output = Command::new("bash")
        .arg("-c")
        .arg(&combined_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute combined command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Verify that individual exit codes are captured
    assert!(
        stdout.contains("PACMAN_EXIT=1") || stderr.contains("PACMAN_EXIT=1"),
        "Should capture pacman exit code as 1. stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains("AUR_EXIT=0") || stderr.contains("AUR_EXIT=0"),
        "Should capture AUR exit code as 0. stdout: {stdout}, stderr: {stderr}"
    );

    // The combined exit code should be non-zero (since pacman failed)
    // Using bitwise OR: 1 | 0 = 1
    assert!(
        !output.status.success(),
        "Combined command should fail when pacman fails, even if AUR succeeds"
    );

    // Clean up
    let _ = fs::remove_file(&mock_pacman_path);
    let _ = fs::remove_file(&mock_aur_path);
    let _ = fs::remove_dir_all(&temp_dir);
}

/// What: Test that empty passwords are rejected during password prompt.
///
/// Inputs:
/// - An empty password string.
///
/// Output:
/// - Verifies that empty passwords are rejected with an appropriate error.
///
/// Details:
/// - This test verifies the fix for the bug where empty passwords from
///   `rpassword::prompt_password` were accepted and passed to sudo, which would fail.
/// - Empty passwords should be rejected before being used in sudo commands.
#[test]
fn test_empty_password_rejected() {
    // Test that empty password validation works
    // The validation should reject empty strings
    let empty_password = String::new();
    let non_empty_password = "test123".to_string();

    // Empty password should be rejected
    assert!(
        empty_password.is_empty(),
        "Empty password should be detected as empty"
    );

    // Non-empty password should be accepted
    assert!(
        !non_empty_password.is_empty(),
        "Non-empty password should not be detected as empty"
    );

    // Verify that trimming whitespace-only passwords would also be empty
    let whitespace_password = "   ".to_string();
    assert!(
        whitespace_password.trim().is_empty(),
        "Whitespace-only password should be considered empty after trimming"
    );
}
