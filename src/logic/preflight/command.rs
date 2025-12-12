//! Command execution abstraction for preflight operations.
//!
//! This module provides the [`CommandRunner`] trait and implementations for
//! executing system commands, enabling testability through dependency injection.

use std::fmt;

/// What: Abstract command execution interface used for spawning helper
/// binaries such as `pacman`.
///
/// Inputs:
/// - `program`: Executable name to run (for example, `"pacman"`).
/// - `args`: Slice of positional arguments passed to the executable.
///
/// Output:
/// - `Ok(String)` containing UTF-8 stdout on success.
/// - `Err(CommandError)` when the invocation fails or stdout is not valid UTF-8.
///
/// # Errors
/// - Returns `Err(CommandError::Io)` when command spawning or execution fails
/// - Returns `Err(CommandError::Utf8)` when stdout cannot be decoded as UTF-8
/// - Returns `Err(CommandError::Failed)` when the command exits with a non-zero status
///
/// Details:
/// - Implementations may stub command results to enable deterministic unit
///   testing.
/// - Production code relies on [`SystemCommandRunner`].
pub trait CommandRunner {
    /// # Errors
    /// - Returns `Err(CommandError::Io)` when command spawning or execution fails
    /// - Returns `Err(CommandError::Utf8)` when stdout cannot be decoded as UTF-8
    /// - Returns `Err(CommandError::Failed)` when the command exits with a non-zero status
    fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError>;
}

/// What: Real command runner backed by `std::process::Command`.
///
/// Inputs: Satisfies the [`CommandRunner`] trait without additional parameters.
///
/// Output:
/// - Executes commands on the host system and captures stdout.
///
/// # Errors
/// - Returns `Err(CommandError::Io)` when command spawning or execution fails
/// - Returns `Err(CommandError::Utf8)` when stdout cannot be decoded as UTF-8
/// - Returns `Err(CommandError::Failed)` when the command exits with a non-zero status
///
/// Details:
/// - Errors from `std::process::Command::output` are surfaced as
///   [`CommandError::Io`].
#[derive(Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError> {
        let output = std::process::Command::new(program).args(args).output()?;
        if !output.status.success() {
            return Err(CommandError::Failed {
                program: program.to_string(),
                args: args.iter().map(ToString::to_string).collect(),
                status: output.status,
            });
        }
        Ok(String::from_utf8(output.stdout)?)
    }
}

/// What: Error type capturing command spawning, execution, and decoding
/// failures.
///
/// Inputs: Generated internally by helper routines.
///
/// Output: Implements `Display`/`Error` for ergonomic propagation.
///
/// Details:
/// - Represents various failure modes when executing system commands.
/// - Wraps I/O errors, UTF-8 conversion failures, parsing issues, and
///   non-success exit statuses.
#[derive(Debug)]
pub enum CommandError {
    /// I/O error occurred.
    Io(std::io::Error),
    /// UTF-8 decoding error occurred.
    Utf8(std::string::FromUtf8Error),
    /// Command execution failed.
    Failed {
        /// Program name that failed.
        program: String,
        /// Command arguments.
        args: Vec<String>,
        /// Exit status of the failed command.
        status: std::process::ExitStatus,
    },
    /// Parse error when processing command output.
    Parse {
        /// Program name that produced invalid output.
        program: String,
        /// Field name that failed to parse.
        field: String,
    },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Utf8(err) => write!(f, "UTF-8 decoding error: {err}"),
            Self::Failed {
                program,
                args,
                status,
            } => {
                write!(f, "{program:?} {args:?} exited with status {status}")
            }
            Self::Parse { program, field } => {
                write!(
                    f,
                    "{program} output did not contain expected field \"{field}\""
                )
            }
        }
    }
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Utf8(err) => Some(err),
            Self::Failed { .. } | Self::Parse { .. } => None,
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::string::FromUtf8Error> for CommandError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::Utf8(value)
    }
}
