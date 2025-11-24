//! Command-line update functionality.

use crate::args::i18n;
#[cfg(not(target_os = "windows"))]
use crate::args::utils;
#[cfg(not(target_os = "windows"))]
use pacsea::install::shell_single_quote;
#[cfg(not(target_os = "windows"))]
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

/// What: Ensure color flags are present in command arguments to preserve colored output.
///
/// Inputs:
/// - `program`: The program name (e.g., "pacman", "yay", "paru").
/// - `args`: Command arguments slice.
///
/// Output:
/// - A vector of arguments with color flags added if not already present.
///
/// Details:
/// - For pacman: adds `--color=always` if no `--color` flag is present.
/// - For yay/paru: adds `--color=always` if no `--color` flag is present (both support this flag).
/// - Preserves all original arguments and their order.
#[cfg(not(target_os = "windows"))]
fn ensure_color_flags(program: &str, args: &[&str]) -> Vec<String> {
    let mut result: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();

    // Check if program is pacman (either directly or via sudo) or yay/paru
    let needs_color_flag = program == "pacman"
        || (program == "sudo" && args.first() == Some(&"pacman"))
        || program == "yay"
        || program == "paru";

    if needs_color_flag {
        // Check if --color flag is already present
        let has_color_flag = args.iter().any(|arg| arg.starts_with("--color"));

        if !has_color_flag {
            // Insert --color=always after the program name but before other flags
            // When program is "sudo", args[0] is "pacman", so insert after args[0]
            let insert_pos = match program {
                "sudo" => 1,
                _ => 0,
            };
            result.insert(insert_pos, "--color=always".to_string());
        }
    }

    result
}

/// What: Execute a command with output both displayed in real-time and logged to file using script.
///
/// Inputs:
/// - `program`: The program to execute.
/// - `args`: Command arguments.
/// - `log_file_path`: Path to the log file where output should be written.
/// - `refresh_sudo`: If true and program is "sudo", refresh sudo timestamp before running command.
///
/// Output:
/// - `Ok((status, output))` if command executed, `Err(e)` if execution failed.
///
/// Details:
/// - Uses `script` command to create a PTY, making programs think they're writing to a real terminal.
/// - This preserves all colors and formatting since programs detect they're writing to a TTY.
/// - Output is displayed in real-time on the terminal and simultaneously captured by script.
/// - Returns the command output for parsing failed packages.
/// - Output is parsed using locale-aware i18n patterns.
/// - Sets `LC_ALL=C` and `LANG=C` for consistent English output.
/// - Automatically adds `--color=always` to pacman/yay/paru commands to ensure colors are enabled.
/// - Captured output is appended to the log file after command completion.
/// - When `refresh_sudo` is true and program is "sudo", prepends `sudo -v &&` to refresh timestamp in same terminal context.
#[cfg(not(target_os = "windows"))]
fn run_command_with_logging(
    program: &str,
    args: &[&str],
    log_file_path: &Path,
    refresh_sudo: bool,
) -> Result<(std::process::ExitStatus, String), std::io::Error> {
    use std::process::{Command, Stdio};

    // Ensure color flags are present for colored output
    let args_with_color = ensure_color_flags(program, args);
    let args_refs: Vec<&str> = args_with_color
        .iter()
        .map(std::string::String::as_str)
        .collect();

    let args_str = args_refs
        .iter()
        .map(|a| shell_single_quote(a))
        .collect::<Vec<_>>()
        .join(" ");

    // Use `script` command to create a PTY and capture output
    // This preserves all colors since programs think they're writing to a real terminal
    let temp_output =
        std::env::temp_dir().join(format!("pacsea_update_output_{}.txt", std::process::id()));
    let temp_output_str = temp_output.to_string_lossy();

    // Build the command - if refresh_sudo is true and program is "sudo" or an AUR helper,
    // prepend "sudo -v &&" to refresh timestamp in the same terminal context
    // AUR helpers (yay/paru) call sudo internally, so they also need fresh timestamp
    let full_command =
        if refresh_sudo && (program == "sudo" || program == "yay" || program == "paru") {
            format!("sudo -v && {program} {args_str}")
        } else {
            format!("{program} {args_str}")
        };

    // Use `script` command to create a PTY, making programs think they're writing to a real terminal
    // This preserves all colors and formatting. The script command logs everything to a file
    // while also displaying it on the terminal.
    // We use `script -qefc` where:
    //   -q: quiet mode (don't print script started/ended messages)
    //   -e: return exit code of the command
    //   -f: flush output immediately (for real-time display)
    //   -c: command to execute (must be a single quoted string)
    // The output is logged to temp_output, then we append it to the log file
    // Note: stdin must be inherited so password prompts work correctly and are hidden
    let escaped_cmd = full_command.replace('\'', "'\"'\"'");
    let script_cmd = format!("script -qefc '{escaped_cmd}' {temp_output_str}");

    let status = Command::new("bash")
        .arg("-c")
        .arg(&script_cmd)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::inherit()) // Inherit stdin so password prompts work and are hidden
        .stdout(Stdio::inherit()) // Inherit stdout so output goes directly to terminal
        .stderr(Stdio::inherit()) // Inherit stderr so output goes directly to terminal
        .status()?;

    // Read the captured output from script
    let output = std::fs::read_to_string(&temp_output).unwrap_or_else(|_| String::new());

    // Append the captured output to the log file
    if let Ok(mut log_file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
    {
        let _ = std::io::Write::write_all(&mut log_file, output.as_bytes());
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_output);

    Ok((status, output))
}

/// What: Execute combined commands in a single script session to share sudo timestamp.
///
/// Inputs:
/// - `command`: The combined command string to execute.
/// - `log_file_path`: Path to the log file where output should be written.
///
/// Output:
/// - `Ok((status, output))` if command executed, `Err(e)` if execution failed.
///
/// Details:
/// - Uses `script` command to create a PTY and capture output.
/// - Runs commands in a single script session so they share the same terminal context and sudo timestamp.
/// - Writes command to a temporary bash script file to avoid shell escaping issues with special characters like $?.
#[cfg(not(target_os = "windows"))]
fn run_combined_commands_with_logging(
    command: &str,
    log_file_path: &Path,
) -> Result<(std::process::ExitStatus, String), std::io::Error> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let temp_output =
        std::env::temp_dir().join(format!("pacsea_update_output_{}.txt", std::process::id()));
    let temp_output_str = temp_output.to_string_lossy();

    // Write command to a temporary bash script to avoid escaping issues
    let temp_script =
        std::env::temp_dir().join(format!("pacsea_update_script_{}.sh", std::process::id()));
    fs::write(&temp_script, format!("#!/bin/bash\n{command}\n"))?;

    // Make script executable
    let mut perms = fs::metadata(&temp_script)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&temp_script, perms)?;

    let script_path_str = temp_script.to_string_lossy();
    let script_cmd = format!("script -qefc 'bash {script_path_str}' {temp_output_str}");

    let status = Command::new("bash")
        .arg("-c")
        .arg(&script_cmd)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    let output = std::fs::read_to_string(&temp_output).unwrap_or_else(|_| String::new());

    if let Ok(mut log_file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
    {
        let _ = std::io::Write::write_all(&mut log_file, output.as_bytes());
    }

    // Clean up temp files
    let _ = std::fs::remove_file(&temp_output);
    let _ = std::fs::remove_file(&temp_script);

    Ok((status, output))
}

/// What: Update execution results.
///
/// Details:
/// - Holds the results of update execution for both pacman and AUR helper.
#[cfg(not(target_os = "windows"))]
struct UpdateResults {
    /// Whether all updates succeeded.
    all_succeeded: bool,
    /// Pacman update status.
    pacman_succeeded: Option<bool>,
    /// AUR helper update status.
    aur_succeeded: Option<bool>,
    /// Name of AUR helper used (if any).
    aur_helper_name: Option<&'static str>,
    /// List of failed commands.
    failed_commands: Vec<String>,
    /// List of failed packages.
    failed_packages: Vec<String>,
}

/// What: Process pacman command results from combined output.
///
/// Inputs:
/// - `pacman_output`: The pacman command output.
/// - `pacman_exit_code_opt`: Optional exit code from pacman command.
/// - `state`: Mutable state for update processing.
/// - `write_log`: Function to write log messages.
///
/// Output:
/// - Returns `bool` indicating pacman success status.
///
/// Details:
/// - Checks if pacman command actually ran.
/// - Evaluates success based on exit code and output content.
/// - Always returns a boolean (false if command didn't run).
#[cfg(not(target_os = "windows"))]
fn process_pacman_result(
    pacman_output: &str,
    pacman_exit_code_opt: Option<i32>,
    state: &mut UpdateState<'_>,
    write_log: &dyn Fn(&str),
) -> bool {
    // Check if pacman command actually ran
    // It ran if we have the exit code marker OR if there's meaningful output
    let pacman_ran = pacman_exit_code_opt.is_some()
        || (!pacman_output.trim().is_empty()
            && (pacman_output.contains("Synchronizing")
                || pacman_output.contains("Starting")
                || pacman_output.contains("error:")
                || pacman_output.contains("target not found")));

    if pacman_ran {
        // Pacman command ran - check its exit code
        let pacman_exit_code = pacman_exit_code_opt.unwrap_or_else(|| {
            // If exit code marker not found but command ran, check output for errors
            i32::from(
                pacman_output.contains("error:")
                    || pacman_output.contains("failed")
                    || pacman_output.contains("target not found"),
            )
        });

        // Check pacman success using individual exit code
        // Also check for "nothing to do" messages as success indicators
        if pacman_exit_code == 0
            || pacman_output.contains("Es gibt nichts zu tun")
            || pacman_output.contains("there is nothing to do")
        {
            println!("{}", i18n::t("app.cli.update.pacman_success"));
            write_log("SUCCESS: pacman -Syyu --noconfirm completed");
            true
        } else {
            println!("{}", i18n::t("app.cli.update.pacman_failed"));
            write_log(&format!(
                "FAILED: pacman -Syyu --noconfirm failed with exit code {pacman_exit_code}"
            ));
            let packages = extract_failed_packages(pacman_output, "pacman");
            state.failed_packages.extend(packages);
            *state.all_succeeded = false;
            state.failed_commands.push("pacman -Syyu".to_string());
            false
        }
    } else {
        // Pacman command didn't run (likely sudo -v failed or command structure issue)
        println!("{}", i18n::t("app.cli.update.pacman_failed"));
        write_log("FAILED: pacman -Syyu --noconfirm did not execute (command chain stopped early)");
        *state.all_succeeded = false;
        state.failed_commands.push("pacman -Syyu".to_string());
        false
    }
}

/// What: Mutable state for update processing.
///
/// Details:
/// - Holds mutable references to state that needs to be updated during processing.
#[cfg(not(target_os = "windows"))]
struct UpdateState<'a> {
    /// Overall success flag.
    all_succeeded: &'a mut bool,
    /// Failed commands list.
    failed_commands: &'a mut Vec<String>,
    /// Failed packages list.
    failed_packages: &'a mut Vec<String>,
}

/// What: Parameters for AUR result processing.
///
/// Details:
/// - Holds all parameters needed to process AUR helper results.
#[cfg(not(target_os = "windows"))]
struct AurProcessParams<'a> {
    /// AUR helper command output.
    aur_output: &'a str,
    /// Optional exit code from AUR helper command.
    aur_exit_code_opt: Option<i32>,
    /// Full combined output for checking section markers.
    output: &'a str,
    /// Marker string indicating AUR section start.
    aur_section_marker: &'a str,
    /// AUR helper name.
    helper: &'a str,
    /// Whether pacman succeeded (affects AUR failure reporting).
    pacman_succeeded: Option<bool>,
}

/// What: Process AUR helper command results from combined output.
///
/// Inputs:
/// - `params`: Parameters for AUR result processing.
/// - `state`: Mutable state for update processing.
/// - `write_log`: Function to write log messages.
///
/// Output:
/// - Returns `Option<bool>` indicating AUR helper success status (None if didn't run).
///
/// Details:
/// - Checks if AUR helper command actually ran.
/// - Evaluates success based on exit code and output content.
/// - Handles case where AUR didn't run due to pacman failure.
#[cfg(not(target_os = "windows"))]
fn process_aur_result(
    params: &AurProcessParams<'_>,
    state: &mut UpdateState<'_>,
    write_log: &dyn Fn(&str),
) -> Option<bool> {
    // Check if AUR helper command actually ran
    // It ran if we have the exit code marker OR if the section marker exists with output
    let aur_ran = params.aur_exit_code_opt.is_some()
        || (params.output.contains(params.aur_section_marker)
            && (!params.aur_output.trim().is_empty() || params.output.contains("AUR_EXIT=")));

    if aur_ran {
        // AUR helper command ran - check its exit code
        let aur_exit_code = params.aur_exit_code_opt.unwrap_or_else(|| {
            // If exit code marker not found but command ran, check output for errors
            i32::from(
                params.aur_output.contains("error:")
                    || params.aur_output.contains("failed")
                    || params.aur_output.contains(" - exit status"),
            )
        });

        // Check AUR helper success using individual exit code
        if aur_exit_code == 0 || params.aur_output.contains("there is nothing to do") {
            println!(
                "{}",
                i18n::t_fmt1("app.cli.update.aur_success", params.helper)
            );
            write_log(&format!(
                "SUCCESS: {} -Syyu --noconfirm completed",
                params.helper
            ));
            Some(true)
        } else {
            println!(
                "{}",
                i18n::t_fmt1("app.cli.update.aur_failed", params.helper)
            );
            write_log(&format!(
                "FAILED: {} -Syyu --noconfirm failed with exit code {aur_exit_code}",
                params.helper
            ));
            let packages = extract_failed_packages(params.aur_output, params.helper);
            state.failed_packages.extend(packages);
            *state.all_succeeded = false;
            state
                .failed_commands
                .push(format!("{} -Syyu --noconfirm", params.helper));
            Some(false)
        }
    } else {
        // AUR helper command didn't run (likely pacman failed early and chain stopped)
        // Only report this as a failure if pacman also failed
        // If pacman succeeded, AUR should have run, so this is unexpected
        if params.pacman_succeeded == Some(false) {
            // Pacman failed, so AUR not running is expected (chain stopped)
            write_log(&format!(
                "SKIPPED: {} -Syyu --noconfirm did not execute (pacman failed early)",
                params.helper
            ));
            None // Mark as not run, not failed
        } else {
            // Pacman succeeded but AUR didn't run - this is unexpected
            println!(
                "{}",
                i18n::t_fmt1("app.cli.update.aur_failed", params.helper)
            );
            write_log(&format!(
                "FAILED: {} -Syyu --noconfirm did not execute (unexpected)",
                params.helper
            ));
            *state.all_succeeded = false;
            state
                .failed_commands
                .push(format!("{} -Syyu --noconfirm", params.helper));
            Some(false)
        }
    }
}

/// What: Handle combined update execution (pacman and AUR helper in single session).
///
/// Inputs:
/// - `helper`: The AUR helper name (`yay`/`paru`).
/// - `log_file_path`: Path to the log file.
/// - `write_log`: Function to write log messages.
///
/// Output:
/// - Returns `UpdateResults` with execution results.
///
/// Details:
/// - Runs both pacman and AUR helper in a single script session to share sudo timestamp.
/// - Captures individual exit codes for each command.
/// - Handles cases where commands may not run due to early failures.
#[cfg(not(target_os = "windows"))]
fn handle_combined_update(
    helper: &'static str,
    log_file_path: &Path,
    write_log: &dyn Fn(&str),
) -> UpdateResults {
    let mut all_succeeded = true;
    let mut failed_commands = Vec::new();
    let mut failed_packages = Vec::new();

    write_log("Starting combined update: pacman and AUR helper in single session");

    // Build combined command: sudo -v && echo message && sudo pacman ... && echo message && yay ...
    // Messages are echoed within the script session to avoid duplicates
    // Use ensure_color_flags to add --color=always for consistency with separate command execution
    let pacman_args = ensure_color_flags("sudo", &["pacman", "-Syyu", "--noconfirm"]);
    let pacman_args_str = pacman_args
        .iter()
        .map(|a| shell_single_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    let pacman_cmd = format!("sudo {pacman_args_str}");

    let aur_args = ensure_color_flags(helper, &["-Syyu", "--noconfirm"]);
    let aur_args_str = aur_args
        .iter()
        .map(|a| shell_single_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    let aur_cmd = format!("{helper} {aur_args_str}");

    let starting_msg = i18n::t("app.cli.update.starting");
    // Use shell_single_quote to safely handle special characters in the message
    let escaped_msg = shell_single_quote(&starting_msg);
    // Build combined command with individual exit code capture
    // Use ; between pacman and AUR sections so both commands always run regardless of individual success/failure
    // Capture exit codes immediately after each command using a subshell to avoid issues with $? interpretation
    // Format: sudo -v && echo ... && (pacman_cmd; pacman_exit=$?; echo "PACMAN_EXIT=$pacman_exit"); ...
    // Using lowercase variable names and explicit assignment to avoid shell interpretation issues
    let combined_cmd = format!(
        "sudo -v && echo {escaped_msg} && ({pacman_cmd}; pacman_exit=$?; echo \"PACMAN_EXIT=$pacman_exit\"); echo ''; echo 'Updating AUR packages ({helper} -Syyu)...'; ({aur_cmd}; aur_exit=$?; echo \"AUR_EXIT=$aur_exit\")"
    );

    let combined_result = run_combined_commands_with_logging(&combined_cmd, log_file_path);

    // Parse the output to determine individual command success
    let (pacman_succeeded, aur_succeeded) = match combined_result {
        Ok((_status, output)) => {
            // Split output for package extraction first
            let aur_section_marker = format!("Updating AUR packages ({helper} -Syyu)...");
            let pacman_output = output.split(&aur_section_marker).next().unwrap_or("");
            let aur_output = output.split(&aur_section_marker).nth(1).unwrap_or("");

            // Extract individual exit codes from output
            // Look for "PACMAN_EXIT=N" and "AUR_EXIT=N" patterns
            // These markers are only present if the commands actually ran
            let pacman_exit_code_opt = output.lines().find_map(|line| {
                if line.contains("PACMAN_EXIT=") {
                    line.split("PACMAN_EXIT=")
                        .nth(1)
                        .and_then(|s| s.trim().parse::<i32>().ok())
                } else {
                    None
                }
            });

            let aur_exit_code_opt = output.lines().find_map(|line| {
                if line.contains("AUR_EXIT=") {
                    line.split("AUR_EXIT=")
                        .nth(1)
                        .and_then(|s| s.trim().parse::<i32>().ok())
                } else {
                    None
                }
            });

            let mut state = UpdateState {
                all_succeeded: &mut all_succeeded,
                failed_commands: &mut failed_commands,
                failed_packages: &mut failed_packages,
            };
            let pacman_succeeded =
                process_pacman_result(pacman_output, pacman_exit_code_opt, &mut state, write_log);
            let aur_params = AurProcessParams {
                aur_output,
                aur_exit_code_opt,
                output: &output,
                aur_section_marker: &aur_section_marker,
                helper,
                pacman_succeeded: Some(pacman_succeeded),
            };
            let aur_succeeded = process_aur_result(&aur_params, &mut state, write_log);

            (Some(pacman_succeeded), aur_succeeded)
        }
        Err(e) => {
            println!("{}", i18n::t("app.cli.update.pacman_exec_failed"));
            eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
            write_log(&format!("FAILED: Could not execute combined update: {e}"));
            all_succeeded = false;
            failed_commands.push("combined update".to_string());
            (Some(false), Some(false))
        }
    };

    UpdateResults {
        all_succeeded,
        pacman_succeeded,
        aur_succeeded,
        aur_helper_name: Some(helper),
        failed_commands,
        failed_packages,
    }
}

/// What: Handle separate update execution (pacman and AUR helper run separately).
///
/// Inputs:
/// - `has_passwordless_sudo`: Whether passwordless sudo is available.
/// - `log_file_path`: Path to the log file.
/// - `write_log`: Function to write log messages.
///
/// Output:
/// - Returns `UpdateResults` with execution results.
///
/// Details:
/// - Runs pacman update first, then AUR helper update separately.
/// - Each command runs in its own script session.
#[cfg(not(target_os = "windows"))]
fn handle_separate_updates(
    has_passwordless_sudo: bool,
    log_file_path: &Path,
    write_log: &dyn Fn(&str),
) -> UpdateResults {
    let mut all_succeeded = true;
    let mut failed_commands = Vec::new();
    let mut failed_packages = Vec::new();
    let pacman_succeeded: Option<bool>;
    let mut aur_succeeded: Option<bool> = None;
    let mut aur_helper_name: Option<&'static str> = None;

    // Run commands separately (original behavior)
    // Step 1: Update pacman (sudo pacman -Syyu --noconfirm)
    // Print starting message only if passwordless sudo (already printed for password prompt)
    if has_passwordless_sudo {
        println!("{}", i18n::t("app.cli.update.starting"));
    }
    write_log("Starting system update: pacman -Syyu --noconfirm");

    let pacman_result = run_command_with_logging(
        "sudo",
        &["pacman", "-Syyu", "--noconfirm"],
        log_file_path,
        !has_passwordless_sudo,
    );

    match pacman_result {
        Ok((status, output)) => {
            if status.success() {
                println!("{}", i18n::t("app.cli.update.pacman_success"));
                write_log("SUCCESS: pacman -Syyu --noconfirm completed successfully");
                pacman_succeeded = Some(true);
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
                pacman_succeeded = Some(false);
            }
        }
        Err(e) => {
            println!("{}", i18n::t("app.cli.update.pacman_exec_failed"));
            eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
            write_log(&format!(
                "FAILED: Could not execute pacman -Syyu --noconfirm: {e}"
            ));
            all_succeeded = false;
            failed_commands.push("pacman -Syyu --noconfirm".to_string());
            pacman_succeeded = Some(false);
        }
    }

    // Step 2: Update AUR packages (yay/paru -Syyu --noconfirm)
    let aur_helper_separate = utils::get_aur_helper();
    if let Some(helper) = aur_helper_separate {
        aur_helper_name = Some(helper);
        println!("\n{}", i18n::t_fmt1("app.cli.update.aur_starting", helper));
        write_log(&format!("Starting AUR update: {helper} -Syyu --noconfirm"));

        // AUR helpers like yay/paru call sudo internally for pacman operations
        // The sudo timestamp will be refreshed in the script context if needed
        let aur_result = run_command_with_logging(
            helper,
            &["-Syyu", "--noconfirm"],
            log_file_path,
            !has_passwordless_sudo,
        );

        match aur_result {
            Ok((status, output)) => {
                if status.success() {
                    println!("{}", i18n::t_fmt1("app.cli.update.aur_success", helper));
                    write_log(&format!(
                        "SUCCESS: {helper} -Syyu --noconfirm completed successfully"
                    ));
                    aur_succeeded = Some(true);
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
                    failed_commands.push(format!("{helper} -Syyu --noconfirm"));
                    aur_succeeded = Some(false);
                }
            }
            Err(e) => {
                println!("{}", i18n::t_fmt1("app.cli.update.aur_exec_failed", helper));
                eprintln!("{}", i18n::t_fmt1("app.cli.update.error_prefix", &e));
                write_log(&format!(
                    "FAILED: Could not execute {helper} -Syyu --noconfirm: {e}"
                ));
                all_succeeded = false;
                failed_commands.push(format!("{helper} -Syyu --noconfirm"));
                aur_succeeded = Some(false);
            }
        }
    } else {
        println!("\n{}", i18n::t("app.cli.update.no_aur_helper"));
        write_log("SKIPPED: No AUR helper (paru/yay) available");
    }

    UpdateResults {
        all_succeeded,
        pacman_succeeded,
        aur_succeeded,
        aur_helper_name,
        failed_commands,
        failed_packages,
    }
}

/// What: Print update summary and exit with appropriate code.
///
/// Inputs:
/// - `results`: `UpdateResults` containing all update execution results.
/// - `log_file_path`: Path to the log file.
/// - `write_log`: Function to write log messages.
///
/// Output:
/// - Exits the process with appropriate exit code.
///
/// Details:
/// - Prints final summary to console.
/// - Logs summary to file.
/// - Exits with code 0 on success, 1 on failure.
#[cfg(not(target_os = "windows"))]
fn print_update_summary(
    results: &UpdateResults,
    log_file_path: &Path,
    write_log: &dyn Fn(&str),
) -> ! {
    // Final summary
    println!("\n{}", i18n::t("app.cli.update.separator"));

    // Show individual status for pacman and AUR helper
    if results.pacman_succeeded == Some(true) {
        println!("{}", i18n::t("app.cli.update.pacman_success"));
    } else if results.pacman_succeeded == Some(false) {
        println!("{}", i18n::t("app.cli.update.pacman_failed"));
    }

    if let Some(helper) = results.aur_helper_name {
        if results.aur_succeeded == Some(true) {
            println!("{}", i18n::t_fmt1("app.cli.update.aur_success", helper));
        } else if results.aur_succeeded == Some(false) {
            println!("{}", i18n::t_fmt1("app.cli.update.aur_failed", helper));
        }
    }

    // Show overall summary
    if results.all_succeeded {
        println!("\n{}", i18n::t("app.cli.update.all_success"));
        write_log("SUMMARY: All updates completed successfully");
    } else {
        println!("\n{}", i18n::t("app.cli.update.completed_with_errors"));
        write_log(&format!(
            "SUMMARY: Update failed. Failed commands: {:?}",
            results.failed_commands
        ));
        if !results.failed_packages.is_empty() {
            println!("\n{}", i18n::t("app.cli.update.failed_packages"));
            for pkg in &results.failed_packages {
                println!("  - {pkg}");
            }
            write_log(&i18n::t_fmt1(
                "app.cli.update.failed_packages_log",
                format!("{:?}", results.failed_packages),
            ));
        }
    }
    let log_file_format = i18n::t("app.cli.update.log_file");
    let clickable_path = format_clickable_path(log_file_path);
    // Replace {} placeholder with clickable path
    let log_file_message = log_file_format.replace("{}", &clickable_path);
    println!("{log_file_message}");
    write_log(&format!(
        "Update process finished. Log file: {}",
        log_file_path.display()
    ));

    if results.all_succeeded {
        tracing::info!("System update completed successfully");
        std::process::exit(0);
    } else {
        tracing::error!("System update completed with errors");
        std::process::exit(1);
    }
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
#[cfg(not(target_os = "windows"))]
pub fn handle_update() -> ! {
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

    // Check if passwordless sudo is available
    // If not, we'll let sudo prompt for password naturally
    // We'll use sudo -v to refresh the timestamp before running commands
    let has_passwordless_sudo = Command::new("sudo")
        .args(["-n", "true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if has_passwordless_sudo {
        write_log("Passwordless sudo detected");
    } else {
        write_log("Passwordless sudo not available - sudo will prompt in script context");
        // Sudo timestamp will be refreshed in each script context before commands
        // The first sudo -v in script will prompt for password once per script session
        println!("{}", i18n::t("app.cli.update.starting"));
    }

    // Check if AUR helper is available - if so, we'll run both commands in a single script session
    // to share the sudo timestamp and avoid multiple password prompts
    let aur_helper = utils::get_aur_helper();
    let run_together = aur_helper.is_some() && !has_passwordless_sudo;

    let results = if run_together {
        let helper = aur_helper.expect("AUR helper should be available when run_together is true");
        handle_combined_update(helper, &log_file_path, &write_log)
    } else {
        handle_separate_updates(has_passwordless_sudo, &log_file_path, &write_log)
    };

    print_update_summary(&results, &log_file_path, &write_log);
}
