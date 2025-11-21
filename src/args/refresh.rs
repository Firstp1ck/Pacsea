//! Command-line refresh functionality.
#[cfg(not(target_os = "windows"))]
use crate::args::{i18n, utils};
#[cfg(not(target_os = "windows"))]
use pacsea::theme;

/// What: Refresh package database by running pacman and AUR helper sync, logging results.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some(true)` if refresh completed successfully, `Some(false)` if it failed, `None` if not run.
///
/// Details:
/// - Runs `sudo pacman -Sy` first to sync official package database.
/// - Then runs `yay -Sy` or `paru -Sy` (prefers paru) if available.
/// - Logs success/failure of each step to `refresh.log` in the config logs directory.
/// - Informs user of final status and log file path.
/// - Unlike `handle_update`, this does not exit - it allows the program to continue to TUI.
/// - Returns the success status so the TUI can display a popup notification.
#[cfg(not(target_os = "windows"))]
pub fn handle_refresh() -> Option<bool> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    tracing::info!("Package database refresh requested from CLI");

    // Get logs directory and create refresh.log path
    let logs_dir = theme::logs_dir();
    let log_file_path = logs_dir.join("refresh.log");

    // Helper function to write to log file
    let write_log = |message: &str| {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
        {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| pacsea::util::ts_to_date(Some(d.as_secs() as i64)))
                .unwrap_or_else(|_| "unknown".to_string());
            let _ = writeln!(file, "[{}] {}", timestamp, message);
        }
    };

    let mut all_succeeded = true;
    let mut failed_commands = Vec::new();

    // Step 1: Refresh pacman database (sudo pacman -Sy)
    println!("{}", i18n::t("app.cli.refresh.starting"));
    write_log("Starting package database refresh: pacman -Sy");

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
                write_log("SUCCESS: pacman -Sy completed successfully");
                if !output.stdout.is_empty() {
                    write_log(&format!(
                        "Output: {}",
                        String::from_utf8_lossy(&output.stdout)
                    ));
                }
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                println!("{}", i18n::t("app.cli.refresh.pacman_failed"));
                eprintln!(
                    "{}",
                    i18n::t_fmt1("app.cli.refresh.error_prefix", &error_msg)
                );
                write_log(&format!(
                    "FAILED: pacman -Sy failed with exit code {:?}",
                    output.status.code()
                ));
                write_log(&format!("Error: {}", error_msg));
                if !output.stdout.is_empty() {
                    write_log(&format!(
                        "Output: {}",
                        String::from_utf8_lossy(&output.stdout)
                    ));
                }
                all_succeeded = false;
                failed_commands.push("pacman -Sy".to_string());
            }
        }
        Err(e) => {
            println!("{}", i18n::t("app.cli.refresh.pacman_exec_failed"));
            eprintln!("{}", i18n::t_fmt1("app.cli.refresh.error_prefix", &e));
            write_log(&format!("FAILED: Could not execute pacman -Sy: {}", e));
            all_succeeded = false;
            failed_commands.push("pacman -Sy".to_string());
        }
    }

    // Step 2: Refresh AUR helper database (yay/paru -Sy)
    let aur_helper = utils::get_aur_helper();
    if let Some(helper) = aur_helper {
        println!("\n{}", i18n::t_fmt1("app.cli.refresh.aur_starting", helper));
        write_log(&format!("Starting AUR database refresh: {} -Sy", helper));

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
                    write_log(&format!("SUCCESS: {} -Sy completed successfully", helper));
                    if !output.stdout.is_empty() {
                        write_log(&format!(
                            "Output: {}",
                            String::from_utf8_lossy(&output.stdout)
                        ));
                    }
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    println!("{}", i18n::t_fmt1("app.cli.refresh.aur_failed", helper));
                    eprintln!(
                        "{}",
                        i18n::t_fmt1("app.cli.refresh.error_prefix", &error_msg)
                    );
                    write_log(&format!(
                        "FAILED: {} -Sy failed with exit code {:?}",
                        helper,
                        output.status.code()
                    ));
                    write_log(&format!("Error: {}", error_msg));
                    if !output.stdout.is_empty() {
                        write_log(&format!(
                            "Output: {}",
                            String::from_utf8_lossy(&output.stdout)
                        ));
                    }
                    all_succeeded = false;
                    failed_commands.push(format!("{} -Sy", helper));
                }
            }
            Err(e) => {
                println!(
                    "{}",
                    i18n::t_fmt1("app.cli.refresh.aur_exec_failed", helper)
                );
                eprintln!("{}", i18n::t_fmt1("app.cli.refresh.error_prefix", &e));
                write_log(&format!("FAILED: Could not execute {} -Sy: {}", helper, e));
                all_succeeded = false;
                failed_commands.push(format!("{} -Sy", helper));
            }
        }
    } else {
        println!("\n{}", i18n::t("app.cli.refresh.no_aur_helper"));
        write_log("SKIPPED: No AUR helper (paru/yay) available");
    }

    // Final summary
    println!("\n{}", i18n::t("app.cli.refresh.separator"));
    if all_succeeded {
        println!("{}", i18n::t("app.cli.refresh.all_success"));
        write_log("SUMMARY: All database refreshes completed successfully");
    } else {
        println!("{}", i18n::t("app.cli.refresh.completed_with_errors"));
        write_log(&format!(
            "SUMMARY: Database refresh failed. Failed commands: {:?}",
            failed_commands
        ));
    }
    println!(
        "{}",
        i18n::t_fmt1("app.cli.refresh.log_file", log_file_path.display())
    );
    write_log(&format!(
        "Refresh process finished. Log file: {}",
        log_file_path.display()
    ));

    if all_succeeded {
        tracing::info!("Package database refresh completed successfully");
    } else {
        tracing::error!("Package database refresh completed with errors");
    }

    Some(all_succeeded)
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
    /// - Sets PACSEA_TEST_SKIP_COMMANDS to skip actual command execution during tests.
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
