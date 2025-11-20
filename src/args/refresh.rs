//! Command-line refresh functionality.

use crate::args::{i18n, utils};
use pacsea::theme;

/// What: Refresh package database by running pacman and AUR helper sync, logging results.
///
/// Inputs:
/// - None.
///
/// Output:
/// - None (does not exit, allows program to continue to TUI).
///
/// Details:
/// - Runs `sudo pacman -Sy` first to sync official package database.
/// - Then runs `yay -Sy` or `paru -Sy` (prefers paru) if available.
/// - Logs success/failure of each step to `refresh.log` in the config logs directory.
/// - Informs user of final status and log file path.
/// - Unlike `handle_update`, this does not exit - it allows the program to continue to TUI.
pub fn handle_refresh() {
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
}

#[cfg(test)]
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
    /// - Actual execution will attempt to run system commands, which may fail in test environment.
    #[test]
    fn handle_refresh_does_not_panic() {
        // This test just ensures the function can be called without panicking
        // In a real test environment, we might want to mock the Command calls
        handle_refresh();
    }
}
