//! Pacman command execution utilities.
//!
//! This module provides functions for executing pacman commands and handling
//! common error cases.

use crate::util::command::run_capture;

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
/// - Thin wrapper over [`crate::util::command::run_capture`], which handles the
///   spawn/exit tracing; failures are boxed into the local `Result` alias.
/// - Used internally by index and logic helpers to keep command invocation boilerplate centralized.
pub fn run_pacman(args: &[&str]) -> Result<String> {
    Ok(run_capture("pacman", args)?)
}
