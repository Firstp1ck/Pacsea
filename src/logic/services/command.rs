//! Command execution utilities for service resolution.

use crate::util::command::{CommandError, run_capture};
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
/// - Delegates execution to [`crate::util::command::run_capture`], which emits
///   the detailed spawn/exit tracing.
/// - Annotates errors with the supplied `display_label` string for easier
///   debugging, preserving the historical message formats.
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

    run_capture(program, args).map_err(|err| {
        warn!(
            command = program,
            args = ?args,
            display = display_label,
            error = %err,
            "service command failed"
        );
        match err {
            CommandError::Io(io_err) => format!("failed to spawn `{display_label}`: {io_err}"),
            CommandError::Failed { status, .. } => {
                format!("`{display_label}` exited with status {status}")
            }
            CommandError::Utf8(utf8_err) => {
                format!("`{display_label}` produced invalid UTF-8: {utf8_err}")
            }
            CommandError::Parse { .. } => format!("`{display_label}` failed: {err}"),
        }
    })
}
