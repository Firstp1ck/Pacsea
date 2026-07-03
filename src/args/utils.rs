//! Shared utilities for argument processing.

use crate::args::i18n;

/// What: Read and parse package names from a file.
///
/// Inputs:
/// - `file_path`: Path to the file containing package names.
///
/// Output:
/// - Vector of package names, or exits on error.
///
/// Details:
/// - Reads file line by line.
/// - Ignores empty lines.
/// - Ignores lines starting with "#" and ignores text after "#" in any line.
/// - Skips (with a warning) lines that contain spaces between words.
/// - Trims whitespace from package names.
/// - Shared by the `-I` (install-from-file) and `-R` (remove-from-file) flags.
pub fn read_packages_from_file(file_path: &str) -> Vec<String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "{}",
                i18n::t_fmt(
                    "app.cli.install.file_open_error",
                    &[&file_path as &dyn std::fmt::Display, &e]
                )
            );
            tracing::error!(file = %file_path, error = %e, "Failed to open file");
            std::process::exit(1);
        }
    };

    let reader = BufReader::new(file);
    let mut packages = Vec::new();
    let mut warnings = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let original_line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "{}",
                    i18n::t_fmt(
                        "app.cli.install.file_read_error",
                        &[
                            &(line_num + 1) as &dyn std::fmt::Display,
                            &file_path as &dyn std::fmt::Display,
                            &e,
                        ]
                    )
                );
                tracing::error!(
                    file = %file_path,
                    line = line_num + 1,
                    error = %e,
                    "Failed to read line from file"
                );
                continue;
            }
        };

        // Remove comments (everything after "#")
        let line = original_line.split('#').next().unwrap_or("").trim();

        // Skip empty lines and lines starting with "#"
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check if line contains spaces between words (package names should not have spaces)
        if line.contains(' ') {
            warnings.push((line_num + 1, original_line.trim().to_string()));
            tracing::warn!(
                file = %file_path,
                line = line_num + 1,
                content = %original_line.trim(),
                "Line contains spaces between words"
            );
            continue;
        }

        packages.push(line.to_string());
    }

    // Display warnings if any
    if !warnings.is_empty() {
        eprintln!("\n{}", i18n::t("app.cli.install.lines_with_spaces"));
        for (line_num, content) in &warnings {
            eprintln!(
                "{}",
                i18n::t_fmt2("app.cli.install.line_item", line_num, content)
            );
        }
        eprintln!();
    }

    packages
}

/// What: Determine the log level based on command-line arguments and environment variables.
///
/// Inputs:
/// - `args`: Parsed command-line arguments.
///
/// Output:
/// - Log level string (trace, debug, info, warn, error).
///
/// Details:
/// - Verbose flag overrides `log_level` argument.
/// - `PACSEA_PREFLIGHT_TRACE=1` enables TRACE level for detailed preflight timing.
pub fn determine_log_level(args: &crate::args::Args) -> String {
    if args.verbose {
        "debug".to_string()
    } else if std::env::var("PACSEA_PREFLIGHT_TRACE").ok().as_deref() == Some("1") {
        "trace".to_string()
    } else {
        args.log_level.clone()
    }
}

/// What: Check whether an AUR helper binary responds to `--version`.
///
/// Inputs:
/// - `name`: Helper binary name ("paru" or "yay").
///
/// Output:
/// - `true` when the helper can be executed.
///
/// Details:
/// - Silences stdin/stdout/stderr; only the spawn result matters.
fn helper_available(name: &str) -> bool {
    use std::process::{Command, Stdio};

    Command::new(name)
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

/// What: Resolve the AUR helper to use, honoring the `aur_helper` key from `settings.conf`.
///
/// Inputs:
/// - None (reads `pacsea::theme::settings().aur_helper`).
///
/// Output:
/// - `Some("paru")` / `Some("yay")` for the resolved helper, `None` if neither is available.
///
/// Details:
/// - When `aur_helper` is "paru" or "yay" and that helper is installed, it is used.
/// - When the preferred helper is missing (or the key is "auto"), falls back to
///   auto-detection: paru first, then yay.
pub fn get_aur_helper() -> Option<&'static str> {
    let preference = pacsea::theme::settings().aur_helper;
    match preference.as_str() {
        "paru" if helper_available("paru") => return Some("paru"),
        "yay" if helper_available("yay") => return Some("yay"),
        "paru" | "yay" => {
            tracing::warn!(
                preferred = %preference,
                "Preferred AUR helper from settings.conf is not installed; auto-detecting"
            );
        }
        _ => {}
    }

    if helper_available("paru") {
        return Some("paru");
    }
    if helper_available("yay") {
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

    print!("{message} [Y/n]: ");
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

    print!("{message} [y/N]: ");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// What: Build a unique temp file path for file-reading tests.
    ///
    /// Inputs:
    /// - `label`: Short label distinguishing the calling test.
    ///
    /// Output:
    /// - Unique path under the system temp directory.
    ///
    /// Details:
    /// - Combines process id and a nanosecond timestamp for uniqueness.
    fn temp_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_args_utils_{label}_{}_{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        path
    }

    /// What: Verify comments, blank lines, and spaced lines are filtered out.
    #[test]
    fn read_packages_from_file_filters_comments_and_spaces() {
        let path = temp_path("filters");
        std::fs::write(
            &path,
            "# full comment line\nripgrep\n\n  fd # trailing comment\nbad line with spaces\nbat  \n",
        )
        .expect("write temp file");

        let packages = read_packages_from_file(path.to_str().expect("temp path is valid UTF-8"));
        std::fs::remove_file(&path).ok();

        assert_eq!(packages, vec!["ripgrep", "fd", "bat"]);
    }

    /// What: Verify an empty file yields no package names.
    #[test]
    fn read_packages_from_file_empty_file_returns_empty() {
        let path = temp_path("empty");
        std::fs::write(&path, "").expect("write temp file");

        let packages = read_packages_from_file(path.to_str().expect("temp path is valid UTF-8"));
        std::fs::remove_file(&path).ok();

        assert!(packages.is_empty());
    }

    /// What: Verify comma-separated and space-separated parsing of package lists.
    #[test]
    fn parse_package_names_splits_commas_and_trims() {
        let input = vec![
            "ripgrep, fd".to_string(),
            "bat".to_string(),
            " ,".to_string(),
        ];
        assert_eq!(parse_package_names(&input), vec!["ripgrep", "fd", "bat"]);
    }
}
