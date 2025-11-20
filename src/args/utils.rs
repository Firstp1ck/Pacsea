//! Shared utilities for argument processing.

/// What: Determine the log level based on command-line arguments and environment variables.
///
/// Inputs:
/// - `args`: Parsed command-line arguments.
///
/// Output:
/// - Log level string (trace, debug, info, warn, error).
///
/// Details:
/// - Verbose flag overrides log_level argument.
/// - PACSEA_PREFLIGHT_TRACE=1 enables TRACE level for detailed preflight timing.
pub fn determine_log_level(args: &crate::args::Args) -> String {
    if args.verbose {
        "debug".to_string()
    } else if std::env::var("PACSEA_PREFLIGHT_TRACE").ok().as_deref() == Some("1") {
        "trace".to_string()
    } else {
        args.log_level.clone()
    }
}

/// What: Check if paru or yay is available.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some("paru")` if paru is available, `Some("yay")` if only yay is available, `None` if neither.
///
/// Details:
/// - Checks for paru first, then falls back to yay.
pub fn get_aur_helper() -> Option<&'static str> {
    use std::process::{Command, Stdio};

    if Command::new("paru")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
    {
        return Some("paru");
    }

    if Command::new("yay")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
    {
        return Some("yay");
    }

    None
}

/// What: Parse package names from input, handling both comma-separated and space-separated formats.
///
/// Inputs:
/// - `packages`: Vector of package strings (may contain comma-separated values).
///
/// Output:
/// - Vector of individual package names.
///
/// Details:
/// - Splits each input string by commas and trims whitespace.
/// - Filters out empty strings.
pub fn parse_package_names(packages: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for pkg in packages {
        for name in pkg.split(',') {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                result.push(trimmed.to_string());
            }
        }
    }
    result
}

/// What: Prompt the user for yes/no confirmation.
///
/// Inputs:
/// - `message`: The prompt message to display.
///
/// Output:
/// - `true` if user confirms (default), `false` if user explicitly declines (n/N/no).
///
/// Details:
/// - Reads a single line from stdin.
/// - Defaults to "yes" (empty input or Enter key).
/// - Returns `false` only if user explicitly enters 'n', 'N', or 'no'.
/// - Trims whitespace before checking.
pub fn prompt_user(message: &str) -> bool {
    use std::io::{self, Write};

    print!("{} [Y/n]: ", message);
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        // Default to yes (empty input), only return false for explicit 'n' or 'no'
        !(trimmed.eq_ignore_ascii_case("n") || trimmed.eq_ignore_ascii_case("no"))
    } else {
        true // Default to yes on read error
    }
}

/// What: Prompt the user for yes/no confirmation with "No" as default.
///
/// Inputs:
/// - `message`: The prompt message to display.
///
/// Output:
/// - `true` if user explicitly confirms (y/Y/yes), `false` otherwise (default).
///
/// Details:
/// - Reads a single line from stdin.
/// - Defaults to "no" (empty input or Enter key).
/// - Returns `true` only if user explicitly enters 'y', 'Y', or 'yes'.
/// - Trims whitespace before checking.
pub fn prompt_user_no_default(message: &str) -> bool {
    use std::io::{self, Write};

    print!("{} [y/N]: ", message);
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim();
        // Default to no (empty input), only return true for explicit 'y' or 'yes'
        trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes")
    } else {
        false // Default to no on read error
    }
}
