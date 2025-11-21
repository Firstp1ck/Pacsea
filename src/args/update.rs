//! Command-line update functionality.

use crate::args::{i18n, utils};
use pacsea::install::shell_single_quote;
use pacsea::theme;
use std::path::Path;

/// What: Format a file path as a clickable hyperlink in the terminal using OSC 8 escape sequences.
///
/// Inputs:
/// - `path`: The file path to make clickable.
///
/// Output:
/// - A string containing the path formatted as a clickable hyperlink.
///
/// Details:
/// - Uses OSC 8 escape sequences to create clickable links in modern terminals.
/// - Converts the path to an absolute file:// URL.
/// - Handles paths that may not exist yet by using absolute path resolution.
fn format_clickable_path(path: &Path) -> String {
    // Try to get absolute path - canonicalize if file exists, otherwise resolve relative to current dir
    let absolute_path = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| cwd.join(path).canonicalize().ok())
                .unwrap_or_else(|| path.to_path_buf())
        })
    } else {
        // File doesn't exist yet, try to resolve relative to current directory
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .ok()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|| path.to_path_buf())
        }
    };
    let path_str = absolute_path.to_string_lossy();
    let file_url = format!("file://{}", path_str);
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", file_url, path_str)
}

/// What: Extract failed package names from command output.
///
/// Inputs:
/// - `output`: The command output text to parse.
/// - `helper`: The AUR helper name (yay/paru) or "pacman" for official packages.
///
/// Output:
/// - Vector of failed package names.
///
/// Details:
/// - Parses yay/paru output for lines like "package - exit status X".
/// - Uses locale-independent pattern matching (exit status pattern is universal).
/// - Does not rely on locale-specific error messages.
fn extract_failed_packages(output: &str, helper: &str) -> Vec<String> {
    let mut failed = Vec::new();

    if helper == "pacman" {
        // For pacman, look for package names in error messages
        // This is less common but we can try to extract them
        // Pacman errors are usually less structured
    } else {
        // For yay/paru, primarily rely on the universal " - exit status" pattern
        // This pattern appears to be locale-independent
        let lines: Vec<&str> = output.lines().collect();

        // Look for lines with "exit status" pattern - this is the most reliable indicator
        // Format: "package - exit status X" (works across locales)
        for line in &lines {
            if line.contains(" - exit status")
                && let Some(pkg) = line.split(" - exit status").next()
            {
                let pkg = pkg.trim();
                // Remove common prefixes like "->" that yay/paru use
                let pkg = pkg.strip_prefix("->").unwrap_or(pkg).trim();
                if !pkg.is_empty() {
                    failed.push(pkg.to_string());
                }
            }
        }

        // If we didn't find any via exit status pattern, try to find packages
        // in a section that follows common structural markers
        if failed.is_empty() {
            // Look for sections that typically contain failed packages
            // These sections usually have markers like "->" followed by package lists
            let mut in_package_list = false;
            for line in &lines {
                let trimmed = line.trim();

                // Detect start of package list section (common markers)
                // Look for lines with "->" that might indicate a list section
                if trimmed.starts_with("->") && trimmed.len() > 2 {
                    // Check if the rest looks like it might be a header/description
                    // If it contains common words, it's probably a header, not a package
                    let after_arrow = &trimmed[2..].trim();
                    if !after_arrow.chars().any(|c| c.is_whitespace() || c == ':') {
                        // Might be a package name
                        if after_arrow
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                        {
                            failed.push(after_arrow.to_string());
                            in_package_list = true;
                        }
                    } else {
                        in_package_list = true;
                    }
                } else if in_package_list {
                    // In package list, look for package-like strings
                    if !trimmed.is_empty()
                        && !trimmed.starts_with("==>")
                        && !trimmed.contains("exit status")
                        && trimmed
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                    {
                        failed.push(trimmed.to_string());
                    } else if trimmed.is_empty() || trimmed.starts_with("==>") {
                        // Empty line or new section marker ends the list
                        in_package_list = false;
                    }
                }
            }
        }
    }

    // Deduplicate and return
    failed.sort();
    failed.dedup();
    failed
}

/// What: Execute a command with output both displayed in real-time and logged to file using tee.
///
/// Inputs:
/// - `program`: The program to execute.
/// - `args`: Command arguments.
/// - `log_file_path`: Path to the log file where output should be written.
///
/// Output:
/// - `Ok((status, output))` if command executed, `Err(e)` if execution failed.
///
/// Details:
/// - Uses a shell wrapper with `tee` to duplicate output to both terminal and log file.
/// - Preserves real-time output display while logging everything.
/// - Returns the command output for parsing failed packages.
fn run_command_with_logging(
    program: &str,
    args: &[&str],
    log_file_path: &Path,
) -> Result<(std::process::ExitStatus, String), std::io::Error> {
    use std::process::Command;

    let log_file_str = log_file_path.to_string_lossy();
    let args_str = args
        .iter()
        .map(|a| shell_single_quote(a))
        .collect::<Vec<_>>()
        .join(" ");

    // Use bash -c with tee to both display and log output
    // Redirect both stdout and stderr through tee
    // Use PIPESTATUS[0] to get the exit status of the command, not tee
    // Also capture output to a temp file so we can read it back
    let temp_output =
        std::env::temp_dir().join(format!("pacsea_update_output_{}.txt", std::process::id()));
    let temp_output_str = temp_output.to_string_lossy();

    // Use tee twice: first logs to file, second captures to tempfile and displays
    // command 2>&1 | tee -a logfile | tee tempfile > /dev/tty
    // This way: output is displayed once, logged to file, and captured to tempfile
    let shell_cmd = format!(
        "{} {} 2>&1 | tee -a {} | tee {} > /dev/tty; exit ${{PIPESTATUS[0]}}",
        program, args_str, log_file_str, temp_output_str
    );

    let status = Command::new("bash").arg("-c").arg(&shell_cmd).status()?;

    // Read the captured output
    let output = std::fs::read_to_string(&temp_output).unwrap_or_else(|_| String::new());

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_output);

    Ok((status, output))
}

/// What: Handle system update by running pacman and AUR helper updates, logging results.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Exits the process with appropriate exit code.
///
/// Details:
/// - Runs `sudo pacman -Syyu --noconfirm` first to update official packages.
/// - Then runs `yay -Syyu --noconfirm` or `paru -Syyu --noconfirm` (prefers paru) if available.
/// - Displays update progress output in real-time to the terminal.
/// - Logs all command output and status messages to `update.log` in the config logs directory.
/// - Informs user of final status and log file path.
pub fn handle_update() -> ! {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    tracing::info!("System update requested from CLI");

    // Get logs directory and create update.log path
    let logs_dir = theme::logs_dir();
    let log_file_path = logs_dir.join("update.log");

    // Ensure log file exists and is writable
    if let Some(parent) = log_file_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Helper function to write status messages to log file
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
    let mut failed_packages = Vec::new();

    // Step 1: Update pacman (sudo pacman -Syyu --noconfirm)
    println!("{}", i18n::t("app.cli.update.starting"));
    write_log("Starting system update: pacman -Syyu --noconfirm");

    let pacman_result =
        run_command_with_logging("sudo", &["pacman", "-Syyu", "--noconfirm"], &log_file_path);

    match pacman_result {
        Ok((status, output)) => {
            if status.success() {
                println!("{}", i18n::t("app.cli.update.pacman_success"));
                write_log("SUCCESS: pacman -Syyu --noconfirm completed successfully");
            } else {
                println!("{}", i18n::t("app.cli.update.pacman_failed"));
                write_log(&format!(
                    "FAILED: pacman -Syyu --noconfirm failed with exit code {:?}",
                    status.code()
                ));
                let packages = extract_failed_packages(&output, "pacman");
                failed_packages.extend(packages);
                all_succeeded = false;
                failed_commands.push("pacman -Syyu".to_string());
            }
        }
        Err(e) => {
            println!("{}", i18n::t("app.cli.update.pacman_exec_failed"));
            eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
            write_log(&format!(
                "FAILED: Could not execute pacman -Syyu --noconfirm: {}",
                e
            ));
            all_succeeded = false;
            failed_commands.push("pacman -Syyu --noconfirm".to_string());
        }
    }

    // Step 2: Update AUR packages (yay/paru -Syyu --noconfirm)
    let aur_helper = utils::get_aur_helper();
    if let Some(helper) = aur_helper {
        println!("\n{}", i18n::t_fmt1("app.cli.update.aur_starting", helper));
        write_log(&format!(
            "Starting AUR update: {} -Syyu --noconfirm",
            helper
        ));

        let aur_result =
            run_command_with_logging(helper, &["-Syyu", "--noconfirm"], &log_file_path);

        match aur_result {
            Ok((status, output)) => {
                if status.success() {
                    println!("{}", i18n::t_fmt1("app.cli.update.aur_success", helper));
                    write_log(&format!(
                        "SUCCESS: {} -Syyu --noconfirm completed successfully",
                        helper
                    ));
                } else {
                    println!("{}", i18n::t_fmt1("app.cli.update.aur_failed", helper));
                    write_log(&format!(
                        "FAILED: {} -Syyu --noconfirm failed with exit code {:?}",
                        helper,
                        status.code()
                    ));
                    let packages = extract_failed_packages(&output, helper);
                    failed_packages.extend(packages);
                    all_succeeded = false;
                    failed_commands.push(format!("{} -Syyu --noconfirm", helper));
                }
            }
            Err(e) => {
                println!("{}", i18n::t_fmt1("app.cli.update.aur_exec_failed", helper));
                eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
                write_log(&format!(
                    "FAILED: Could not execute {} -Syyu --noconfirm: {}",
                    helper, e
                ));
                all_succeeded = false;
                failed_commands.push(format!("{} -Syyu --noconfirm", helper));
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
        if !failed_packages.is_empty() {
            println!("\n{}", i18n::t("app.cli.update.failed_packages"));
            for pkg in &failed_packages {
                println!("  - {}", pkg);
            }
            write_log(&i18n::t_fmt1(
                "app.cli.update.failed_packages_log",
                format!("{:?}", failed_packages),
            ));
        }
    }
    let log_file_format = i18n::t("app.cli.update.log_file");
    let clickable_path = format_clickable_path(&log_file_path);
    // Replace {} placeholder with clickable path
    let log_file_message = log_file_format.replace("{}", &clickable_path);
    println!("{}", log_file_message);
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
