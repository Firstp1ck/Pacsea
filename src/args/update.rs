//! Command-line update functionality.

use crate::args::{i18n, utils};
use pacsea::theme;

/// What: Handle system update by running pacman and AUR helper updates, logging results.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Exits the process with appropriate exit code.
///
/// Details:
/// - Runs `sudo pacman -Syyu` first to update official packages.
/// - Then runs `yay -Syyu` or `paru -Syyu` (prefers paru) if available.
/// - Logs success/failure of each step to `update.log` in the config logs directory.
/// - Informs user of final status and log file path.
pub fn handle_update() -> ! {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    tracing::info!("System update requested from CLI");

    // Get logs directory and create update.log path
    let logs_dir = theme::logs_dir();
    let log_file_path = logs_dir.join("update.log");

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

    // Step 1: Update pacman (sudo pacman -Syyu)
    println!("{}", i18n::t("app.cli.update.starting"));
    write_log("Starting system update: pacman -Syyu");

    let pacman_output = Command::new("sudo").arg("pacman").args(["-Syyu"]).output();

    match pacman_output {
        Ok(output) => {
            if output.status.success() {
                println!("{}", i18n::t("app.cli.update.pacman_success"));
                write_log("SUCCESS: pacman -Syyu completed successfully");
                if !output.stdout.is_empty() {
                    write_log(&format!(
                        "Output: {}",
                        String::from_utf8_lossy(&output.stdout)
                    ));
                }
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                println!("{}", i18n::t("app.cli.update.pacman_failed"));
                eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &error_msg));
                write_log(&format!(
                    "FAILED: pacman -Syyu failed with exit code {:?}",
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
                failed_commands.push("pacman -Syyu".to_string());
            }
        }
        Err(e) => {
            println!("{}", i18n::t("app.cli.update.pacman_exec_failed"));
            eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
            write_log(&format!("FAILED: Could not execute pacman -Syyu: {}", e));
            all_succeeded = false;
            failed_commands.push("pacman -Syyu".to_string());
        }
    }

    // Step 2: Update AUR packages (yay/paru -Syyu)
    let aur_helper = utils::get_aur_helper();
    if let Some(helper) = aur_helper {
        println!("\n{}", i18n::t_fmt1("app.cli.update.aur_starting", helper));
        write_log(&format!("Starting AUR update: {} -Syyu", helper));

        let aur_output = Command::new(helper).args(["-Syyu"]).output();

        match aur_output {
            Ok(output) => {
                if output.status.success() {
                    println!("{}", i18n::t_fmt1("app.cli.update.aur_success", helper));
                    write_log(&format!("SUCCESS: {} -Syyu completed successfully", helper));
                    if !output.stdout.is_empty() {
                        write_log(&format!(
                            "Output: {}",
                            String::from_utf8_lossy(&output.stdout)
                        ));
                    }
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    println!("{}", i18n::t_fmt1("app.cli.update.aur_failed", helper));
                    eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &error_msg));
                    write_log(&format!(
                        "FAILED: {} -Syyu failed with exit code {:?}",
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
                    failed_commands.push(format!("{} -Syyu", helper));
                }
            }
            Err(e) => {
                println!("{}", i18n::t_fmt1("app.cli.update.aur_exec_failed", helper));
                eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
                write_log(&format!(
                    "FAILED: Could not execute {} -Syyu: {}",
                    helper, e
                ));
                all_succeeded = false;
                failed_commands.push(format!("{} -Syyu", helper));
            }
        }
    } else {
        println!("\n{}", i18n::t("app.cli.update.no_aur_helper"));
        write_log("SKIPPED: No AUR helper (paru/yay) available");
    }

    // Final summary
    println!("\n{}", i18n::t("app.cli.update.separator"));
    if all_succeeded {
        println!("{}", i18n::t("app.cli.update.all_success"));
        write_log("SUMMARY: All updates completed successfully");
    } else {
        println!("{}", i18n::t("app.cli.update.completed_with_errors"));
        write_log(&format!(
            "SUMMARY: Update failed. Failed commands: {:?}",
            failed_commands
        ));
    }
    println!("{}", i18n::t_fmt1("app.cli.update.log_file", log_file_path.display()));
    write_log(&format!(
        "Update process finished. Log file: {}",
        log_file_path.display()
    ));

    if all_succeeded {
        tracing::info!("System update completed successfully");
        std::process::exit(0);
    } else {
        tracing::error!("System update completed with errors");
        std::process::exit(1);
    }
}
