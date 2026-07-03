//! Shared command execution abstraction.
//!
//! This module is the single home for synchronous, capture-only command
//! execution in Pacsea. It provides the [`CommandRunner`] trait for
//! dependency-injected execution, the production [`SystemCommandRunner`],
//! the [`run_capture`] convenience wrapper, and the [`binary_available`]
//! capability probe.
//!
//! Interactive, PTY-based, and terminal-spawning command paths (installs,
//! privilege escalation, clipboard, browsers) intentionally do **not** go
//! through this module.

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
/// - Emits `tracing::debug!` before spawning and on success, and
///   `tracing::warn!` on spawn failure or non-zero exit, so callers get
///   consistent diagnostics without duplicating log statements.
#[derive(Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError> {
        tracing::debug!(
            command = program,
            args = ?args,
            arg_count = args.len(),
            "executing command"
        );

        let output = match std::process::Command::new(program).args(args).output() {
            Ok(output) => output,
            Err(err) => {
                tracing::warn!(
                    command = program,
                    args = ?args,
                    error = %err,
                    "failed to spawn command"
                );
                return Err(CommandError::Io(err));
            }
        };

        let status_code = output.status.code();
        let stdout_len = output.stdout.len();
        let stderr_len = output.stderr.len();

        if !output.status.success() {
            tracing::warn!(
                command = program,
                args = ?args,
                status = ?output.status,
                status_code,
                stdout_len,
                stderr_len,
                "command exited with non-zero status"
            );
            return Err(CommandError::Failed {
                program: program.to_string(),
                args: args.iter().map(ToString::to_string).collect(),
                status: output.status,
            });
        }

        tracing::debug!(
            command = program,
            args = ?args,
            status = ?output.status,
            status_code,
            stdout_len,
            stderr_len,
            "command completed successfully"
        );

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

/// What: Run a command via [`SystemCommandRunner`] and capture stdout as UTF-8.
///
/// Inputs:
/// - `program`: Executable name to run (for example, `"pacman"`).
/// - `args`: Slice of positional arguments passed to the executable.
///
/// Output:
/// - `Ok(String)` containing UTF-8 stdout on success.
/// - `Err(CommandError)` when spawning fails, the command exits non-zero, or
///   stdout is not valid UTF-8.
///
/// # Errors
/// - Returns `Err(CommandError::Io)` when command spawning or execution fails
/// - Returns `Err(CommandError::Utf8)` when stdout cannot be decoded as UTF-8
/// - Returns `Err(CommandError::Failed)` when the command exits with a non-zero status
///
/// Details:
/// - Thin convenience wrapper around [`SystemCommandRunner::run`]; use the
///   trait directly when dependency injection is needed for testing.
pub fn run_capture(program: &str, args: &[&str]) -> Result<String, CommandError> {
    SystemCommandRunner.run(program, args)
}

/// What: Check whether an external binary is available on the system.
///
/// Inputs:
/// - `name`: Binary name to probe (for example, `"paru"` or `"fakeroot"`).
///
/// Output:
/// - `true` when spawning `<name> --version` succeeds, `false` otherwise.
///
/// Details:
/// - Runs `<name> --version` with stdin, stdout, and stderr attached to the
///   null device, so nothing leaks to the terminal.
/// - Only spawn success is checked (`output().is_ok()`); the exit status is
///   intentionally ignored, matching the historical capability probes.
#[must_use]
pub fn binary_available(name: &str) -> bool {
    use std::process::{Command, Stdio};

    Command::new(name)
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Verify `run_capture` returns stdout for a successful command.
    ///
    /// Inputs:
    /// - `echo hi` executed on Unix hosts.
    ///
    /// Output:
    /// - Captured stdout equals `"hi\n"`.
    ///
    /// Details:
    /// - Guards the success path of [`SystemCommandRunner::run`].
    #[cfg(unix)]
    #[test]
    fn run_capture_returns_stdout_on_success() {
        let out = run_capture("echo", &["hi"]).expect("echo should succeed");
        assert_eq!(out, "hi\n");
    }

    /// What: Verify a non-zero exit maps to `CommandError::Failed`.
    ///
    /// Inputs:
    /// - `false` executed on Unix hosts (always exits with status 1).
    ///
    /// Output:
    /// - `Err(CommandError::Failed)` carrying the program name.
    ///
    /// Details:
    /// - Guards the exit-status branch of [`SystemCommandRunner::run`].
    #[cfg(unix)]
    #[test]
    fn run_capture_maps_nonzero_exit_to_failed() {
        let err = run_capture("false", &[]).expect_err("false should fail");
        match err {
            CommandError::Failed { program, .. } => assert_eq!(program, "false"),
            other => panic!("expected CommandError::Failed, got {other:?}"),
        }
    }

    /// What: Verify a missing binary maps to `CommandError::Io`.
    ///
    /// Inputs:
    /// - A program name that cannot exist in `PATH`.
    ///
    /// Output:
    /// - `Err(CommandError::Io)`.
    ///
    /// Details:
    /// - Guards the spawn-failure branch of [`SystemCommandRunner::run`].
    #[test]
    fn run_capture_maps_missing_binary_to_io() {
        let err = run_capture("pacsea-definitely-not-a-real-binary-xyz", &[])
            .expect_err("missing binary should fail");
        assert!(matches!(err, CommandError::Io(_)));
    }

    /// What: Verify `binary_available` returns `true` for an existing binary.
    ///
    /// Inputs:
    /// - `sh`, which is present on all Unix hosts.
    ///
    /// Output:
    /// - `binary_available("sh")` is `true`.
    ///
    /// Details:
    /// - Guards the positive probe path.
    #[cfg(unix)]
    #[test]
    fn binary_available_true_for_sh() {
        assert!(binary_available("sh"));
    }

    /// What: Verify `binary_available` returns `false` for a missing binary.
    ///
    /// Inputs:
    /// - A nonsense binary name that cannot exist in `PATH`.
    ///
    /// Output:
    /// - `binary_available(...)` is `false`.
    ///
    /// Details:
    /// - Guards the negative probe path.
    #[test]
    fn binary_available_false_for_missing_binary() {
        assert!(!binary_available("pacsea-definitely-not-a-real-binary-xyz"));
    }
}
