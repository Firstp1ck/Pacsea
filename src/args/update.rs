//! Command-line update functionality.

use crate::args::i18n;
#[cfg(not(target_os = "windows"))]
use crate::args::utils;
#[cfg(not(target_os = "windows"))]
use pacsea::install::shell_single_quote;
#[cfg(not(target_os = "windows"))]
use pacsea::theme;
use std::path::Path;

/// What: Format text with ANSI color codes if colors are enabled.
///
/// Inputs:
/// - `text`: The text to format.
/// - `color_code`: ANSI color code (e.g., "32" for green, "31" for red).
/// - `no_color`: If true, returns text without color codes.
///
/// Output:
/// - Colored text string if colors enabled, plain text otherwise.
///
/// Details:
/// - Uses ANSI escape sequences for terminal colors.
/// - Respects the `no_color` flag to disable coloring.
#[cfg(not(target_os = "windows"))]
fn colorize(text: &str, color_code: &str, no_color: bool) -> String {
    if no_color {
        text.to_string()
    } else {
        format!("\x1b[{color_code}m{text}\x1b[0m")
    }
}

/// What: Format success messages in green.
///
/// Inputs:
/// - `text`: The text to format.
/// - `no_color`: If true, returns text without color codes.
///
/// Output:
/// - Green colored text string if colors enabled, plain text otherwise.
#[cfg(not(target_os = "windows"))]
fn success_color(text: &str, no_color: bool) -> String {
    colorize(text, "32", no_color) // Green
}

/// What: Format error messages in red.
///
/// Inputs:
/// - `text`: The text to format.
/// - `no_color`: If true, returns text without color codes.
///
/// Output:
/// - Red colored text string if colors enabled, plain text otherwise.
#[cfg(not(target_os = "windows"))]
fn error_color(text: &str, no_color: bool) -> String {
    colorize(text, "31", no_color) // Red
}

/// What: Format info messages in cyan.
///
/// Inputs:
/// - `text`: The text to format.
/// - `no_color`: If true, returns text without color codes.
///
/// Output:
/// - Cyan colored text string if colors enabled, plain text otherwise.
#[cfg(not(target_os = "windows"))]
fn info_color(text: &str, no_color: bool) -> String {
    colorize(text, "36", no_color) // Cyan
}

/// What: Format warning messages in yellow.
///
/// Inputs:
/// - `text`: The text to format.
/// - `no_color`: If true, returns text without color codes.
///
/// Output:
/// - Yellow colored text string if colors enabled, plain text otherwise.
#[cfg(not(target_os = "windows"))]
fn warning_color(text: &str, no_color: bool) -> String {
    colorize(text, "33", no_color) // Yellow
}

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
#[cfg(not(target_os = "windows"))]
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
                .map_or_else(|| path.to_path_buf(), |cwd| cwd.join(path))
        }
    };
    let path_str = absolute_path.to_string_lossy();
    let file_url = format!("file://{path_str}");
    format!("\x1b]8;;{file_url}\x1b\\{path_str}\x1b]8;;\x1b\\")
}

/// What: Extract failed package names from pacman error output.
///
/// Inputs:
/// - `output`: The pacman command output text to parse.
///
/// Output:
/// - Vector of failed package names.
///
/// Details:
/// - Parses various pacman error patterns including "target not found", transaction failures, etc.
/// - Handles both English and German error messages.
#[cfg(not(target_os = "windows"))]
#[allow(clippy::similar_names)]
fn extract_failed_packages_from_pacman(output: &str) -> Vec<String> {
    let mut failed = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    let mut in_error_section = false;
    let mut in_conflict_section = false;

    // Get locale-specific error patterns from i18n
    let target_not_found = i18n::t("app.cli.update.pacman_errors.target_not_found").to_lowercase();
    let failed_to_commit = i18n::t("app.cli.update.pacman_errors.failed_to_commit").to_lowercase();
    let failed_to_prepare =
        i18n::t("app.cli.update.pacman_errors.failed_to_prepare").to_lowercase();
    let error_prefix = i18n::t("app.cli.update.pacman_errors.error_prefix").to_lowercase();
    let resolving = i18n::t("app.cli.update.pacman_errors.resolving").to_lowercase();
    let looking_for = i18n::t("app.cli.update.pacman_errors.looking_for").to_lowercase();
    let package_word = i18n::t("app.cli.update.pacman_errors.package").to_lowercase();
    let packages_word = i18n::t("app.cli.update.pacman_errors.packages").to_lowercase();
    let error_word = i18n::t("app.cli.update.pacman_errors.error").to_lowercase();
    let failed_word = i18n::t("app.cli.update.pacman_errors.failed").to_lowercase();
    let transaction_word = i18n::t("app.cli.update.pacman_errors.transaction").to_lowercase();
    let conflicting_word = i18n::t("app.cli.update.pacman_errors.conflicting").to_lowercase();
    let files_word = i18n::t("app.cli.update.pacman_errors.files").to_lowercase();

    for line in &lines {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        // Pattern 1: "error: target not found: package-name"
        if lower.contains(&target_not_found) {
            // Extract package name after "not found:" or similar
            if let Some(colon_pos) = trimmed.rfind(':') {
                let after_colon = &trimmed[colon_pos + 1..].trim();
                // Package name should be alphanumeric with dashes/underscores
                if after_colon
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '/')
                {
                    // Remove any trailing punctuation
                    let pkg = after_colon.trim_end_matches(|c: char| {
                        !c.is_alphanumeric() && c != '-' && c != '_' && c != '/'
                    });
                    if !pkg.is_empty() && pkg.len() > 1 {
                        failed.push(pkg.to_string());
                    }
                }
            }
            in_error_section = true;
        }
        // Pattern 2: "error: failed to commit transaction" or similar
        else if lower.contains(&failed_to_commit) || lower.contains(&failed_to_prepare) {
            in_error_section = true;
            in_conflict_section = true;
        }
        // Pattern 3: Look for package names in error context
        else if in_error_section || in_conflict_section {
            // Look for lines that might contain package names
            // Skip common error message text
            if !trimmed.is_empty()
                && !lower.starts_with(&format!("{error_prefix}:"))
                && !lower.contains(&resolving)
                && !lower.contains(&looking_for)
                && !lower.contains("::")
            {
                // Check if line looks like it contains package names
                // Package names are typically: alphanumeric, dashes, underscores, slashes
                let words: Vec<&str> = trimmed.split_whitespace().collect();
                for word in words {
                    let clean_word = word.trim_matches(|c: char| {
                        !c.is_alphanumeric() && c != '-' && c != '_' && c != '/' && c != ':'
                    });
                    // Valid package name: 2+ chars, alphanumeric with dashes/underscores/slashes
                    if clean_word.len() >= 2
                        && clean_word.chars().all(|c| {
                            c.is_alphanumeric() || c == '-' || c == '_' || c == '/' || c == ':'
                        })
                        && clean_word.contains(|c: char| c.is_alphanumeric())
                    {
                        // Avoid common false positives using locale-specific words
                        if !clean_word.eq_ignore_ascii_case(&package_word)
                            && !clean_word.eq_ignore_ascii_case(&packages_word)
                            && !clean_word.eq_ignore_ascii_case(&error_word)
                            && !clean_word.eq_ignore_ascii_case(&failed_word)
                            && !clean_word.eq_ignore_ascii_case(&transaction_word)
                            && !clean_word.eq_ignore_ascii_case(&conflicting_word)
                            && !clean_word.eq_ignore_ascii_case(&files_word)
                        {
                            failed.push(clean_word.to_string());
                        }
                    }
                }
            }
            // Reset error section on empty lines or new error messages
            if trimmed.is_empty() || lower.starts_with(&format!("{error_prefix}:")) {
                in_error_section = false;
                in_conflict_section = false;
            }
        }
        // Pattern 4: Look for package names after "::" separator (pacman format: repo::package)
        else if trimmed.contains("::") {
            let parts: Vec<&str> = trimmed.split("::").collect();
            if parts.len() == 2 {
                let pkg_part = parts[1].split_whitespace().next().unwrap_or("");
                if pkg_part
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                    && pkg_part.len() >= 2
                {
                    failed.push(pkg_part.to_string());
                }
            }
        }
    }
    failed
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
#[cfg(not(target_os = "windows"))]
fn extract_failed_packages(output: &str, helper: &str) -> Vec<String> {
    let mut failed = if helper == "pacman" {
        extract_failed_packages_from_pacman(output)
    } else {
        // For yay/paru, primarily rely on the universal " - exit status" pattern
        // This pattern appears to be locale-independent
        let mut failed_aur = Vec::new();
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
                    failed_aur.push(pkg.to_string());
                }
            }
        }

        // If we didn't find any via exit status pattern, try to find packages
        // in a section that follows common structural markers
        if failed_aur.is_empty() {
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
                    if after_arrow.chars().all(|c| !c.is_whitespace() && c != ':') {
                        // Might be a package name
                        if after_arrow
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                        {
                            failed_aur.push((*after_arrow).to_string());
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
                        failed_aur.push(trimmed.to_string());
                    } else if trimmed.is_empty() || trimmed.starts_with("==>") {
                        // Empty line or new section marker ends the list
                        in_package_list = false;
                    }
                }
            }
        }
        failed_aur
    };

    // Deduplicate and return
    failed.sort();
    failed.dedup();

    // Additional cleanup: remove very short strings and common false positives
    failed.retain(|pkg| {
        pkg.len() >= 2
            && !pkg.eq_ignore_ascii_case("package")
            && !pkg.eq_ignore_ascii_case("packages")
            && !pkg.eq_ignore_ascii_case("error")
            && !pkg.eq_ignore_ascii_case("failed")
    });

    failed
}

/// What: Execute a command with output both displayed in real-time and logged to file using tee.
///
/// Inputs:
/// - `program`: The program to execute.
/// - `args`: Command arguments.
/// - `log_file_path`: Path to the log file where output should be written.
/// - `password`: Optional sudo password; when provided, uses `sudo -S` with password piping.
///
/// Output:
/// - `Ok((status, output))` if command executed, `Err(e)` if execution failed.
///
/// Details:
/// - Uses a shell wrapper with `tee` to duplicate output to both terminal and log file.
/// - Preserves real-time output display while logging everything.
/// - Returns the command output for parsing failed packages.
/// - Output is parsed using locale-aware i18n patterns.
/// - Sets `LC_ALL=C` and `LANG=C` for consistent English output.
/// - Handles TTY detection and falls back to stdout if no TTY available.
/// - Uses `set -o pipefail` for reliable exit status capture.
/// - Configures stdin/stdout/stderr explicitly to prevent interactive prompts.
#[cfg(not(target_os = "windows"))]
fn run_command_with_logging(
    program: &str,
    args: &[&str],
    log_file_path: &Path,
    password: Option<&str>,
) -> Result<(std::process::ExitStatus, String), std::io::Error> {
    use std::io::IsTerminal;
    use std::process::{Command, Stdio};

    let log_file_str = log_file_path.to_string_lossy();
    let args_str = args
        .iter()
        .map(|a| shell_single_quote(a))
        .collect::<Vec<_>>()
        .join(" ");

    // Check if stdout is a TTY for /dev/tty redirection
    let has_tty = std::io::stdout().is_terminal();
    let tty_redirect = if has_tty {
        "> /dev/tty"
    } else {
        "> /dev/stdout"
    };

    // Use bash -c with tee to both display and log output
    // Redirect both stdout and stderr through tee
    // Use set -o pipefail for reliable exit status capture
    // Also capture output to a temp file so we can read it back
    let temp_output =
        std::env::temp_dir().join(format!("pacsea_update_output_{}.txt", std::process::id()));
    let temp_output_str = temp_output.to_string_lossy();

    // Build the command with optional password piping for sudo
    // When program is "sudo", we need to handle it specially:
    // - With password: echo 'password' | sudo -S pacman args...
    // - Without password: sudo pacman args...
    // When program is not "sudo" (e.g., paru/yay), use it directly
    let full_command = if program == "sudo" {
        password.map_or_else(
            || format!("sudo {args_str}"),
            |pass| {
                // Use shell_single_quote for consistent password escaping
                let escaped = shell_single_quote(pass);
                // args[0] is the actual command (e.g., "pacman"), args[1..] are its arguments
                args.first().map_or_else(
                    || format!("echo {escaped} | sudo -S {args_str}"),
                    |cmd| {
                        let cmd_args = &args[1..];
                        let cmd_args_str = cmd_args
                            .iter()
                            .map(|a| shell_single_quote(a))
                            .collect::<Vec<_>>()
                            .join(" ");
                        if cmd_args_str.is_empty() {
                            format!("echo {escaped} | sudo -S {cmd}")
                        } else {
                            format!("echo {escaped} | sudo -S {cmd} {cmd_args_str}")
                        }
                    },
                )
            },
        )
    } else {
        // Non-sudo command (e.g., paru, yay), use directly
        format!("{program} {args_str}")
    };

    // Use tee twice: first logs to file, second captures to tempfile and displays
    // set -o pipefail ensures exit status reflects command failure, not tee
    // command 2>&1 | tee -a logfile | tee tempfile > /dev/tty
    // This way: output is displayed once, logged to file, and captured to tempfile
    let log_file_escaped = shell_single_quote(&log_file_str);
    let temp_output_escaped = shell_single_quote(&temp_output_str);
    let shell_cmd = format!(
        "set -o pipefail; {full_command} 2>&1 | tee -a {log_file_escaped} | tee {temp_output_escaped} {tty_redirect}"
    );

    let status = Command::new("bash")
        .arg("-c")
        .arg(&shell_cmd)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()?;

    // Read the captured output
    let output = std::fs::read_to_string(&temp_output).unwrap_or_else(|_| String::new());

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_output);

    Ok((status, output))
}

/// What: Prompt user for sudo password and validate it is not empty.
///
/// Inputs:
/// - `write_log`: Function to write log messages.
///
/// Output:
/// - `Some(password)` if password is valid and non-empty, `None` if passwordless sudo works.
/// - Exits the process with code 1 if password is empty or cannot be read.
///
/// Details:
/// - Prompts user for password using `rpassword::prompt_password`.
/// - Validates that password is not empty (after trimming whitespace).
/// - Empty passwords are rejected early to prevent sudo failures.
#[cfg(not(target_os = "windows"))]
fn prompt_and_validate_password(write_log: &dyn Fn(&str)) -> Option<String> {
    use std::io::IsTerminal;
    use std::process::Command;

    // Check if passwordless sudo is available
    if Command::new("sudo")
        .args(["-n", "true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        // Passwordless sudo works, no password needed
        write_log("Passwordless sudo detected, skipping password prompt");
        return None;
    }

    // Password required, but check if stdin is available for interactive input
    if !std::io::stdin().is_terminal() {
        // Not in an interactive terminal (e.g., in tests or non-interactive environment)
        let error_msg =
            "Password required but stdin is not a terminal. Cannot prompt for password.";
        eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", error_msg));
        write_log("FAILED: Password required but stdin is not a terminal");
        tracing::error!("Password required but stdin is not a terminal");
        std::process::exit(1);
    }

    // Password required, prompt user
    // Get username to mimic sudo's password prompt format
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let password_prompt = i18n::t_fmt1("app.cli.update.password_prompt", &username);
    match rpassword::prompt_password(&password_prompt) {
        Ok(pass) => {
            // Validate that password is not empty
            // Empty passwords will cause sudo to fail, so reject them early
            if pass.trim().is_empty() {
                let error_msg = "Empty password provided. Password cannot be empty.";
                eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", error_msg));
                write_log("FAILED: Empty password provided");
                tracing::error!("Empty password provided");
                std::process::exit(1);
            }
            write_log("Password obtained from user (not logged)");
            Some(pass)
        }
        Err(e) => {
            eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
            write_log(&format!("FAILED: Could not read password: {e}"));
            tracing::error!("Failed to read sudo password: {e}");
            std::process::exit(1);
        }
    }
}

/// What: Handle system update by running pacman and AUR helper updates, logging results.
///
/// Inputs:
/// - `no_color`: If true, disables colored output.
///
/// Output:
/// - Exits the process with appropriate exit code.
///
/// Details:
/// - Runs `sudo pacman -Syu --noconfirm` first to update official packages.
/// - Then runs `yay -Syu --noconfirm` or `paru -Syu --noconfirm` (prefers paru) if available.
/// - Displays update progress output in real-time to the terminal.
/// - Logs all command output and status messages to `update.log` in the config logs directory.
/// - Informs user of final status and log file path.
/// - Uses colored output for success (green), error (red), info (cyan), and warning (yellow) messages.
#[cfg(not(target_os = "windows"))]
#[allow(clippy::too_many_lines)]
pub fn handle_update(no_color: bool) -> ! {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::Command;
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
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
                |_| "unknown".to_string(),
                |d| pacsea::util::ts_to_date(Some(i64::try_from(d.as_secs()).unwrap_or(0))),
            );
            let _ = writeln!(file, "[{timestamp}] {message}");
        }
    };

    // Prompt for password and validate it
    let password = prompt_and_validate_password(&write_log);

    let mut all_succeeded = true;
    let mut failed_commands = Vec::new();
    let mut failed_packages = Vec::new();
    #[allow(unused_assignments)]
    let mut pacman_succeeded = Option::<bool>::None;
    let mut aur_succeeded = Option::<bool>::None;
    let mut aur_helper_name = Option::<&str>::None;

    // Step 1: Update pacman (sudo pacman -Syu --noconfirm)
    println!(
        "{}",
        info_color(&i18n::t("app.cli.update.starting"), no_color)
    );
    write_log("Starting system update: pacman -Syu --noconfirm");

    let pacman_result = run_command_with_logging(
        "sudo",
        &["pacman", "-Syu", "--noconfirm"],
        &log_file_path,
        password.as_deref(),
    );

    match pacman_result {
        Ok((status, output)) => {
            if status.success() {
                println!(
                    "{}",
                    success_color(&i18n::t("app.cli.update.pacman_success"), no_color)
                );
                write_log("SUCCESS: pacman -Syu --noconfirm completed successfully");
                pacman_succeeded = Some(true);
            } else {
                println!(
                    "{}",
                    error_color(&i18n::t("app.cli.update.pacman_failed"), no_color)
                );
                write_log(&format!(
                    "FAILED: pacman -Syu --noconfirm failed with exit code {:?}",
                    status.code()
                ));
                let packages = extract_failed_packages(&output, "pacman");
                failed_packages.extend(packages);
                all_succeeded = false;
                failed_commands.push("pacman -Syu".to_string());
                pacman_succeeded = Some(false);
            }
        }
        Err(e) => {
            println!(
                "{}",
                error_color(&i18n::t("app.cli.update.pacman_exec_failed"), no_color)
            );
            eprintln!(
                "{}",
                error_color(&i18n::t_fmt1("app.cli.update.error_prefix", &e), no_color)
            );
            write_log(&format!(
                "FAILED: Could not execute pacman -Syu --noconfirm: {e}"
            ));
            all_succeeded = false;
            failed_commands.push("pacman -Syu --noconfirm".to_string());
            pacman_succeeded = Some(false);
        }
    }

    // Refresh sudo timestamp after pacman command so AUR helper can use it
    // This prevents a second password prompt when the AUR helper calls sudo internally
    if let Some(pass) = password {
        // Use shell_single_quote for consistent password escaping
        #[allow(clippy::needless_borrow)]
        let escaped = shell_single_quote(&pass);
        let refresh_cmd = format!("echo {escaped} | sudo -S -v");
        let _ = Command::new("bash")
            .arg("-c")
            .arg(&refresh_cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        write_log("Refreshed sudo timestamp for AUR helper");
    }

    // Step 2: Update AUR packages (yay/paru -Syu --noconfirm)
    let aur_helper = utils::get_aur_helper();
    if let Some(helper) = aur_helper {
        aur_helper_name = Some(helper);
        println!(
            "\n{}",
            info_color(
                &i18n::t_fmt1("app.cli.update.aur_starting", helper),
                no_color
            )
        );
        write_log(&format!("Starting AUR update: {helper} -Syu --noconfirm"));

        let aur_result = run_command_with_logging(
            helper,
            &["-Syu", "--noconfirm"],
            &log_file_path,
            None, // AUR helpers handle sudo internally, no password needed
        );

        match aur_result {
            Ok((status, output)) => {
                if status.success() {
                    println!(
                        "{}",
                        success_color(
                            &i18n::t_fmt1("app.cli.update.aur_success", helper),
                            no_color
                        )
                    );
                    write_log(&format!(
                        "SUCCESS: {helper} -Syu --noconfirm completed successfully"
                    ));
                    aur_succeeded = Some(true);
                } else {
                    println!(
                        "{}",
                        error_color(&i18n::t_fmt1("app.cli.update.aur_failed", helper), no_color)
                    );
                    write_log(&format!(
                        "FAILED: {} -Syu --noconfirm failed with exit code {:?}",
                        helper,
                        status.code()
                    ));
                    let packages = extract_failed_packages(&output, helper);
                    failed_packages.extend(packages);
                    all_succeeded = false;
                    failed_commands.push(format!("{helper} -Syu --noconfirm"));
                    aur_succeeded = Some(false);
                }
            }
            Err(e) => {
                println!(
                    "{}",
                    error_color(
                        &i18n::t_fmt1("app.cli.update.aur_exec_failed", helper),
                        no_color
                    )
                );
                eprintln!(
                    "{}",
                    error_color(&i18n::t_fmt1("app.cli.update.error_prefix", &e), no_color)
                );
                write_log(&format!(
                    "FAILED: Could not execute {helper} -Syu --noconfirm: {e}"
                ));
                all_succeeded = false;
                failed_commands.push(format!("{helper} -Syu --noconfirm"));
                aur_succeeded = Some(false);
            }
        }
    } else {
        println!(
            "\n{}",
            warning_color(&i18n::t("app.cli.update.no_aur_helper"), no_color)
        );
        write_log("SKIPPED: No AUR helper (paru/yay) available");
    }

    // Final summary
    println!(
        "\n{}",
        info_color(&i18n::t("app.cli.update.separator"), no_color)
    );

    // Show individual status for pacman and AUR helper
    if pacman_succeeded == Some(true) {
        println!(
            "{}",
            success_color(&i18n::t("app.cli.update.pacman_success"), no_color)
        );
    } else if pacman_succeeded == Some(false) {
        println!(
            "{}",
            error_color(&i18n::t("app.cli.update.pacman_failed"), no_color)
        );
    }

    if let Some(helper) = aur_helper_name {
        if aur_succeeded == Some(true) {
            println!(
                "{}",
                success_color(
                    &i18n::t_fmt1("app.cli.update.aur_success", helper),
                    no_color
                )
            );
        } else if aur_succeeded == Some(false) {
            println!(
                "{}",
                error_color(&i18n::t_fmt1("app.cli.update.aur_failed", helper), no_color)
            );
        }
    }

    // Show overall summary
    if all_succeeded {
        println!(
            "\n{}",
            success_color(&i18n::t("app.cli.update.all_success"), no_color)
        );
        write_log("SUMMARY: All updates completed successfully");
    } else {
        println!(
            "\n{}",
            error_color(&i18n::t("app.cli.update.completed_with_errors"), no_color)
        );
        write_log(&format!(
            "SUMMARY: Update failed. Failed commands: {failed_commands:?}"
        ));
        if !failed_packages.is_empty() {
            println!(
                "\n{}",
                warning_color(&i18n::t("app.cli.update.failed_packages"), no_color)
            );
            for pkg in &failed_packages {
                println!("  - {}", error_color(pkg, no_color));
            }
            write_log(&i18n::t_fmt1(
                "app.cli.update.failed_packages_log",
                format!("{failed_packages:?}"),
            ));
        }
    }
    let log_file_format = i18n::t("app.cli.update.log_file");
    let clickable_path = format_clickable_path(&log_file_path);
    // Replace {} placeholder with clickable path
    let log_file_message = log_file_format.replace("{}", &clickable_path);
    println!("{log_file_message}");
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
