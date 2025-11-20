//! Command execution utilities for service resolution.

use std::process::Command;

/// What: Execute a command and capture stdout as UTF-8.
///
/// Inputs:
/// - `program`: Binary to execute.
/// - `args`: Command-line arguments.
/// - `display`: Human-friendly command description for logging.
///
/// Output:
/// - Stdout as a `String` on success; error description otherwise.
///
/// Details:
/// - Annotates errors with the supplied `display` string for easier debugging.
pub(crate) fn run_command(program: &str, args: &[&str], display: &str) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to spawn `{}`: {}", display, err))?;

    if !output.status.success() {
        return Err(format!(
            "`{}` exited with status {}",
            display, output.status
        ));
    }

    String::from_utf8(output.stdout)
        .map_err(|err| format!("`{}` produced invalid UTF-8: {}", display, err))
}
