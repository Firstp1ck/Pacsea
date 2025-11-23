//! Command-line refresh functionality.
#[cfg(not(target_os = "windows"))]
use crate::args::{i18n, utils};
#[cfg(not(target_os = "windows"))]
use pacsea::theme;

/// What: Write a message to the refresh log file with timestamp.
///
/// Inputs:
/// - `log_file_path`: Path to the log file.
/// - `message`: Message to write to the log.
///
/// Output:
/// - None (writes to file, ignores errors).
///
/// Details:
/// - Creates or appends to the log file.
/// - Adds a timestamp to each log entry.
#[cfg(not(target_os = "windows"))]
fn write_log(log_file_path: &std::path::Path, message: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
    {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
            |_| "unknown".to_string(),
            |d| pacsea::util::ts_to_date(Some(i64::try_from(d.as_secs()).unwrap_or(0))),
        );
        let _ = writeln!(file, "[{timestamp}] {message}");
    }
}

/// What: Execute pacman database refresh and handle the result.
///
/// Inputs:
/// - `log_file_path`: Path to the log file.
///
/// Output:
/// - `(success, failed_command)`: Tuple with success status and optional failed command name.
///
/// Details:
/// - Runs `sudo pacman -Sy` to sync the official package database.
/// - Logs success/failure to the log file.
/// - Prints user-friendly messages.
#[cfg(not(target_os = "windows"))]
fn refresh_pacman(log_file_path: &std::path::Path) -> (bool, Option<String>) {
    use std::process::Command;

    println!("{}", i18n::t("app.cli.refresh.starting"));
    write_log(
        log_file_path,
        "Starting package database refresh: pacman -Sy",
    );

    // Skip actual command execution during tests to avoid requiring sudo
    #[cfg(test)]
    let pacman_output = if std::env::var("PACSEA_TEST_SKIP_COMMANDS").is_ok() {
        // Return a mock success response during tests
        use std::os::unix::process::ExitStatusExt;
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: b"test output".to_vec(),
            stderr: Vec::new(),
        })
    } else {
        Command::new("sudo").arg("pacman").args(["-Sy"]).output()
    };
    #[cfg(not(test))]
    let pacman_output = Command::new("sudo").arg("pacman").args(["-Sy"]).output();

    match pacman_output {
        Ok(output) => {
            if output.status.success() {
                println!("{}", i18n::t("app.cli.refresh.pacman_success"));
                write_log(log_file_path, "SUCCESS: pacman -Sy completed successfully");
                if !output.stdout.is_empty() {
                    write_log(
                        log_file_path,
                        &format!("Output: {}", String::from_utf8_lossy(&output.stdout)),
                    );
                }
                (true, None)
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                println!("{}", i18n::t("app.cli.refresh.pacman_failed"));
                eprintln!(
                    "{}",
                    i18n::t_fmt1("app.cli.refresh.error_prefix", &error_msg)
                );
                write_log(
                    log_file_path,
                    &format!(
                        "FAILED: pacman -Sy failed with exit code {:?}",
                        output.status.code()
                    ),
                );
                write_log(log_file_path, &format!("Error: {error_msg}"));
                if !output.stdout.is_empty() {
                    write_log(
                        log_file_path,
                        &format!("Output: {}", String::from_utf8_lossy(&output.stdout)),
                    );
                }
                (false, Some("pacman -Sy".to_string()))
            }
        }
        Err(e) => {
            println!("{}", i18n::t("app.cli.refresh.pacman_exec_failed"));
            eprintln!("{}", i18n::t_fmt1("app.cli.refresh.error_prefix", &e));
            write_log(
                log_file_path,
                &format!("FAILED: Could not execute pacman -Sy: {e}"),
            );
            (false, Some("pacman -Sy".to_string()))
        }
    }
}

/// What: Execute AUR helper database refresh and handle the result.
///
/// Inputs:
/// - `log_file_path`: Path to the log file.
///
/// Output:
/// - `(success, failed_command)`: Tuple with success status and optional failed command name.
///
/// Details:
/// - Detects available AUR helper (paru or yay).
/// - Runs `{helper} -Sy` to sync the AUR database.
/// - Logs success/failure to the log file.
/// - Prints user-friendly messages.
#[cfg(not(target_os = "windows"))]
fn refresh_aur_helper(log_file_path: &std::path::Path) -> (bool, Option<String>) {
    use std::process::Command;

    let aur_helper = utils::get_aur_helper();
    aur_helper.map_or_else(
        || {
            println!("\n{}", i18n::t("app.cli.refresh.no_aur_helper"));
            write_log(log_file_path, "SKIPPED: No AUR helper (paru/yay) available");
            (true, None)
        },
        |helper| {
            println!("\n{}", i18n::t_fmt1("app.cli.refresh.aur_starting", helper));
            write_log(
                log_file_path,
                &format!("Starting AUR database refresh: {helper} -Sy"),
            );

            // Skip actual command execution during tests to avoid requiring sudo
            #[cfg(test)]
            let aur_output = if std::env::var("PACSEA_TEST_SKIP_COMMANDS").is_ok() {
                // Return a mock success response during tests
                use std::os::unix::process::ExitStatusExt;
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: b"test output".to_vec(),
                    stderr: Vec::new(),
                })
            } else {
                Command::new(helper).args(["-Sy"]).output()
            };
            #[cfg(not(test))]
            let aur_output = Command::new(helper).args(["-Sy"]).output();

            match aur_output {
                Ok(output) => {
                    if output.status.success() {
                        println!("{}", i18n::t_fmt1("app.cli.refresh.aur_success", helper));
                        write_log(
                            log_file_path,
                            &format!("SUCCESS: {helper} -Sy completed successfully"),
                        );
                        if !output.stdout.is_empty() {
                            write_log(
                                log_file_path,
                                &format!("Output: {}", String::from_utf8_lossy(&output.stdout)),
                            );
                        }
                        (true, None)
                    } else {
                        let error_msg = String::from_utf8_lossy(&output.stderr);
                        println!("{}", i18n::t_fmt1("app.cli.refresh.aur_failed", helper));
                        eprintln!(
                            "{}",
                            i18n::t_fmt1("app.cli.refresh.error_prefix", &error_msg)
                        );
                        write_log(
                            log_file_path,
                            &format!(
                                "FAILED: {} -Sy failed with exit code {:?}",
                                helper,
                                output.status.code()
                            ),
                        );
                        write_log(log_file_path, &format!("Error: {error_msg}"));
                        if !output.stdout.is_empty() {
                            write_log(
                                log_file_path,
                                &format!("Output: {}", String::from_utf8_lossy(&output.stdout)),
                            );
                        }
                        (false, Some(format!("{helper} -Sy")))
                    }
                }
                Err(e) => {
                    println!(
                        "{}",
                        i18n::t_fmt1("app.cli.refresh.aur_exec_failed", helper)
                    );
                    eprintln!("{}", i18n::t_fmt1("app.cli.refresh.error_prefix", &e));
                    write_log(
                        log_file_path,
                        &format!("FAILED: Could not execute {helper} -Sy: {e}"),
                    );
                    (false, Some(format!("{helper} -Sy")))
                }
            }
        },
    )
}

/// What: Write final summary of the refresh operation.
///
/// Inputs:
/// - `log_file_path`: Path to the log file.
/// - `all_succeeded`: Whether all refresh operations succeeded.
/// - `failed_commands`: List of failed command names.
///
/// Output:
/// - None.
///
/// Details:
/// - Prints summary message to console.
/// - Logs summary to file.
/// - Logs tracing messages based on success status.
#[cfg(not(target_os = "windows"))]
fn write_summary(log_file_path: &std::path::Path, all_succeeded: bool, failed_commands: &[String]) {
    println!("\n{}", i18n::t("app.cli.refresh.separator"));
    if all_succeeded {
        println!("{}", i18n::t("app.cli.refresh.all_success"));
        write_log(
            log_file_path,
            "SUMMARY: All database refreshes completed successfully",
        );
    } else {
        println!("{}", i18n::t("app.cli.refresh.completed_with_errors"));
        write_log(
            log_file_path,
            &format!("SUMMARY: Database refresh failed. Failed commands: {failed_commands:?}"),
        );
    }
    println!(
        "{}",
        i18n::t_fmt1("app.cli.refresh.log_file", log_file_path.display())
    );
    write_log(
        log_file_path,
        &format!(
            "Refresh process finished. Log file: {}",
            log_file_path.display()
        ),
    );

    if all_succeeded {
        tracing::info!("Package database refresh completed successfully");
    } else {
        tracing::error!("Package database refresh completed with errors");
    }
}

/// What: Refresh package database by running pacman and AUR helper sync, logging results.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `true` if refresh completed successfully, `false` if it failed.
///
/// Details:
/// - Runs `sudo pacman -Sy` first to sync official package database.
/// - Then runs `yay -Sy` or `paru -Sy` (prefers paru) if available.
/// - Logs success/failure of each step to `refresh.log` in the config logs directory.
/// - Informs user of final status and log file path.
/// - Unlike `handle_update`, this does not exit - it allows the program to continue to TUI.
/// - Returns the success status so the TUI can display a popup notification.
#[cfg(not(target_os = "windows"))]
pub fn handle_refresh() -> bool {
    tracing::info!("Package database refresh requested from CLI");

    // Get logs directory and create refresh.log path
    let logs_dir = theme::logs_dir();
    let log_file_path = logs_dir.join("refresh.log");

    let mut all_succeeded = true;
    let mut failed_commands = Vec::new();

    // Step 1: Refresh pacman database
    let (pacman_success, pacman_failed) = refresh_pacman(&log_file_path);
    if !pacman_success {
        all_succeeded = false;
        if let Some(cmd) = pacman_failed {
            failed_commands.push(cmd);
        }
    }

    // Step 2: Refresh AUR helper database
    let (aur_success, aur_failed) = refresh_aur_helper(&log_file_path);
    if !aur_success {
        all_succeeded = false;
        if let Some(cmd) = aur_failed {
            failed_commands.push(cmd);
        }
    }

    // Final summary
    write_summary(&log_file_path, all_succeeded, &failed_commands);

    all_succeeded
}

#[cfg(test)]
#[cfg(not(target_os = "windows"))]
mod tests {
    use super::*;

    /// What: Validate that `handle_refresh` does not panic when called.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Test passes if function completes without panicking.
    ///
    /// Details:
    /// - This is a smoke test to ensure the function can be called.
    /// - Sets `PACSEA_TEST_SKIP_COMMANDS` to skip actual command execution during tests.
    #[test]
    fn handle_refresh_does_not_panic() {
        // Set environment variable to skip actual command execution
        unsafe {
            std::env::set_var("PACSEA_TEST_SKIP_COMMANDS", "1");
        }
        // This test just ensures the function can be called without panicking
        let _ = handle_refresh();
        // Clean up
        unsafe {
            std::env::remove_var("PACSEA_TEST_SKIP_COMMANDS");
        }
    }
}
