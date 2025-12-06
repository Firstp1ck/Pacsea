//! Command execution utilities for service resolution.

use std::process::Command;
use tracing::{debug, warn};

/// What: Execute a command and capture stdout as UTF-8.
///
/// Inputs:
/// - `program`: Binary to execute.
/// - `args`: Command-line arguments.
/// - `display_label`: Human-friendly command description for logging.
///
/// Output:
/// - Stdout as a `String` on success; error description otherwise.
///
/// Details:
/// - Annotates errors with the supplied `display` string for easier debugging.
pub(super) fn run_command(
    program: &str,
    args: &[&str],
    display_label: &str,
) -> Result<String, String> {
    debug!(
        command = program,
        args = ?args,
        display = display_label,
        "executing service command"
    );

    let output = Command::new(program).args(args).output().map_err(|err| {
        warn!(
            command = program,
            args = ?args,
            display = display_label,
            error = %err,
            "failed to spawn command"
        );
        format!("failed to spawn `{display_label}`: {err}")
    })?;

    let status_code = output.status.code();
    let stdout_len = output.stdout.len();
    let stderr_len = output.stderr.len();

    if !output.status.success() {
        warn!(
            command = program,
            args = ?args,
            display = display_label,
            status = ?output.status,
            status_code,
            stdout_len,
            stderr_len,
            "command exited with non-zero status"
        );
        return Err(format!(
            "`{display_label}` exited with status {}",
            output.status
        ));
    }

    debug!(
        command = program,
        args = ?args,
        display = display_label,
        status = ?output.status,
        status_code,
        stdout_len,
        stderr_len,
        "command completed successfully"
    );

    String::from_utf8(output.stdout).map_err(|err| {
        warn!(
            command = program,
            args = ?args,
            display = display_label,
            error = %err,
            "command produced invalid UTF-8"
        );
        format!("`{display_label}` produced invalid UTF-8: {err}")
    })
}
