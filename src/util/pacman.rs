//! Pacman command execution utilities.
//!
//! This module provides functions for executing pacman commands and handling
//! common error cases.
use std::process::Command;
use tracing::{debug, warn};

/// Result type alias for pacman command operations.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Execute `pacman` with the provided arguments and capture stdout.
///
/// Inputs:
/// - `args`: Slice of CLI arguments passed directly to the pacman binary.
///
/// Output:
/// - Returns the command's stdout as a UTF-8 string or propagates execution/parsing errors.
///
/// # Errors
/// - Returns `Err` when `pacman` command execution fails (I/O error or pacman not found)
/// - Returns `Err` when `pacman` exits with non-zero status
/// - Returns `Err` when stdout cannot be decoded as UTF-8
///
/// Details:
/// - Used internally by index and logic helpers to keep command invocation boilerplate centralized.
pub fn run_pacman(args: &[&str]) -> Result<String> {
    debug!(command = "pacman", args = ?args, "executing pacman command");

    let out = match Command::new("pacman").args(args).output() {
        Ok(output) => output,
        Err(err) => {
            warn!(command = "pacman", args = ?args, error = %err, "failed to spawn pacman");
            return Err(err.into());
        }
    };

    let status_code = out.status.code();
    let stdout_len = out.stdout.len();
    let stderr_len = out.stderr.len();

    if !out.status.success() {
        warn!(
            command = "pacman",
            args = ?args,
            status = ?out.status,
            status_code,
            stdout_len,
            stderr_len,
            "pacman exited with non-zero status"
        );
        return Err(format!("pacman {:?} exited with {:?}", args, out.status).into());
    }

    debug!(
        command = "pacman",
        args = ?args,
        status = ?out.status,
        status_code,
        stdout_len,
        stderr_len,
        "pacman command completed successfully"
    );

    Ok(String::from_utf8(out.stdout)?)
}
