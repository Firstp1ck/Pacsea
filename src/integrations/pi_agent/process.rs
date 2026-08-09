//! Neutral direct-argv Pi startup, trusted-asset verification, and bounded teardown.
//!
//! Startup rules enforced here:
//!
//! - direct `argv` only; never `sh -c`, never a helper, never shell interpolation;
//! - a neutral, empty, private working directory;
//! - the exact disable flags plus the exact four-tool allowlist;
//! - a positive environment allowlist plus the fixed offline/telemetry controls;
//! - the embedded extension is materialized atomically at mode 0600 inside a mode-0700
//!   runtime directory and its SHA-256 is re-verified from disk immediately before launch.
//!
//! Teardown rules enforced here: RPC abort first, then a bounded grace, then
//! `SIGTERM` to the whole child process group, then `SIGKILL`, then a reap. The child
//! is started with `Command::process_group(0)` so no `unsafe` `pre_exec` is needed.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use super::{RESTRICTED_TOOL_NAMES, TOOL_CONTRACT_VERSION, limits, sha256, to_hex};

/// Trusted restricted-tool extension compiled into the Pacsea binary.
pub const EMBEDDED_EXTENSION_SOURCE: &str = include_str!("assets/pacsea-scan-tools.ts");

/// Build-reviewed SHA-256 of [`EMBEDDED_EXTENSION_SOURCE`].
///
/// The asset-integrity unit test forces an explicit digest update whenever the trusted
/// extension changes, rather than silently deriving both sides of the launch check from
/// the same runtime bytes.
pub const EMBEDDED_EXTENSION_SHA256: &str =
    "459204d0870be81e2eb39dc494da2975201550eb640edfdcda914802583318af";

/// File name used when the trusted extension is materialized at runtime.
pub const EMBEDDED_EXTENSION_FILE_NAME: &str = "pacsea-scan-tools.ts";

/// File name of the private snapshot descriptor the extension reads.
///
/// The extension resolves this file relative to its own module URL, so snapshot roots
/// never travel through the environment, the command line, or model input.
pub const SNAPSHOT_DESCRIPTOR_FILE_NAME: &str = "pacsea-scan-descriptor.json";

/// Name of the Pacsea-supplied extension command registered for capability probing.
pub const PACSEA_EXTENSION_COMMAND: &str = "pacsea-scan-tools";

/// Environment names inherited verbatim for executable lookup and standard Pi state.
///
/// This is a positive allowlist. Provider keys, proxies, credential helpers, SSH agent
/// sockets, sudo state, and any future variable are excluded by construction.
pub const PASSTHROUGH_ENVIRONMENT: [&str; 9] = [
    "PATH",
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "PI_CODING_AGENT_DIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
];

/// Environment values the scanner always sets on the Pi child.
pub const FIXED_ENVIRONMENT: [(&str, &str); 3] = [
    ("PI_OFFLINE", "1"),
    ("PI_TELEMETRY", "0"),
    ("PI_SKIP_VERSION_CHECK", "1"),
];

/// Fallback `PATH` used when the parent process has none.
const FALLBACK_PATH: &str = "/usr/bin:/bin";

/// Exact CLI disable flags placed before the extension and tool allowlist.
const ISOLATION_FLAGS: [&str; 12] = [
    "--mode",
    "rpc",
    "--no-session",
    "--offline",
    "--no-builtin-tools",
    "--no-extensions",
    "--no-skills",
    "--no-prompt-templates",
    "--no-context-files",
    "--no-themes",
    "--no-approve",
    "--extension",
];

/// What: Failure modes of Pi startup, asset verification, and teardown.
///
/// Inputs: Produced by this module.
///
/// Output: Implements `Display`/`Error` with actionable, user-facing wording.
///
/// Details:
/// - Every variant means the Pi process must not be considered running or usable.
#[derive(Debug)]
pub enum ProcessError {
    /// The materialized extension did not match the compiled trusted asset.
    ExtensionHashMismatch {
        /// SHA-256 hex digest of the compiled asset.
        expected: String,
        /// SHA-256 hex digest observed on disk.
        observed: String,
    },
    /// The runtime directory, extension file, or neutral working directory is unusable.
    RuntimeDirectory {
        /// Path that could not be prepared.
        path: PathBuf,
        /// Underlying I/O failure.
        source: io::Error,
    },
    /// A launch path violated the absolute/private/empty startup contract.
    InvalidLaunchSpec {
        /// Actionable invariant that failed.
        reason: String,
    },
    /// The Pi executable could not be spawned.
    Spawn {
        /// Executable that failed to start.
        executable: PathBuf,
        /// Underlying I/O failure.
        source: io::Error,
    },
    /// A standard stream was not piped as requested.
    MissingStream {
        /// Stream name.
        stream: &'static str,
    },
    /// Termination did not complete inside the compiled deadline.
    TerminationTimeout {
        /// Deadline that elapsed.
        deadline: Duration,
    },
    /// The correlated RPC abort commands could not be written before process teardown.
    AbortRpc {
        /// Bounded protocol or pipe failure rendering.
        reason: String,
    },
    /// A signal or wait syscall failed.
    Signal {
        /// Operation that failed.
        operation: &'static str,
        /// Underlying error rendering.
        reason: String,
    },
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtensionHashMismatch { expected, observed } => write!(
                f,
                "the Pacsea scanner extension on disk ({observed}) does not match the compiled \
                 trusted asset ({expected}); Pi was not launched. Re-run the scan; if this \
                 repeats, reinstall Pacsea"
            ),
            Self::RuntimeDirectory { path, source } => write!(
                f,
                "could not prepare the private Pi runtime directory {}: {source}. Check that the \
                 Pacsea cache directory is writable",
                path.display()
            ),
            Self::InvalidLaunchSpec { reason } => write!(
                f,
                "the isolated Pi launch specification is invalid: {reason}; create a fresh private scanner runtime and retry"
            ),
            Self::Spawn { executable, source } => write!(
                f,
                "could not start the Pi executable {}: {source}. Verify the configured \
                 pi_scan_binary path or disable Pi scanning",
                executable.display()
            ),
            Self::MissingStream { stream } => {
                write!(f, "the Pi child process did not provide a piped {stream}")
            }
            Self::TerminationTimeout { deadline } => write!(
                f,
                "the Pi process group did not terminate within {deadline:?}; the scan was \
                 abandoned to protect shutdown"
            ),
            Self::AbortRpc { reason } => write!(
                f,
                "could not send the Pi RPC abort before the process group was reaped: {reason}"
            ),
            Self::Signal { operation, reason } => {
                write!(f, "Pi process control failed during {operation}: {reason}")
            }
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuntimeDirectory { source, .. } | Self::Spawn { source, .. } => Some(source),
            Self::ExtensionHashMismatch { .. }
            | Self::InvalidLaunchSpec { .. }
            | Self::MissingStream { .. }
            | Self::TerminationTimeout { .. }
            | Self::AbortRpc { .. }
            | Self::Signal { .. } => None,
        }
    }
}

/// What: Return the comma-delimited restricted tool allowlist passed to `--tools`.
///
/// Inputs: None.
///
/// Output:
/// - Stable sorted comma-delimited tool names.
///
/// Details:
/// - Sorted so argv, extension registration, and probe comparison stay identical.
#[must_use]
pub fn restricted_tool_csv() -> String {
    RESTRICTED_TOOL_NAMES.join(",")
}

/// What: Compute the SHA-256 of the compiled trusted extension asset.
///
/// Inputs: None.
///
/// Output:
/// - Lowercase hex digest of [`EMBEDDED_EXTENSION_SOURCE`].
///
/// Details:
/// - Computed from the compiled bytes rather than stored as a literal, so the constant
///   can never drift away from the asset it is supposed to describe.
#[must_use]
pub fn embedded_extension_sha256() -> String {
    EMBEDDED_EXTENSION_SHA256.to_string()
}

/// What: Build the exact Pi argv for a scanner or probe session.
///
/// Inputs:
/// - `extension_path`: Absolute path to the verified trusted extension.
///
/// Output:
/// - The full argv tail after the executable, in fixed order.
///
/// Details:
/// - Direct argv only. No fragment is shell-interpreted, and no package-derived data
///   ever appears here.
/// - Order is fixed so tests and the dry-run preview can compare it byte for byte.
#[must_use]
pub fn pi_argv(extension_path: &Path) -> Vec<OsString> {
    let mut argv: Vec<OsString> = ISOLATION_FLAGS.iter().map(OsString::from).collect();
    argv.push(extension_path.as_os_str().to_os_string());
    argv.push(OsString::from("--tools"));
    argv.push(OsString::from(restricted_tool_csv()));
    argv
}

/// What: Replace inherited process state with the bounded Pi environment.
///
/// Inputs:
/// - `command`: Command under construction.
///
/// Output:
/// - The command keeps only allowlisted path/state/locale variables plus fixed controls.
///
/// Details:
/// - `env_clear` first, then a positive allowlist. A denylist would silently admit any
///   future credential variable.
/// - No proxy, SSH-agent, Git-credential, sudo-password, or provider-key variable can pass.
pub fn configure_environment(command: &mut Command) {
    command.env_clear();
    for name in PASSTHROUGH_ENVIRONMENT {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    if std::env::var_os("PATH").is_none() {
        command.env("PATH", FALLBACK_PATH);
    }
    for (name, value) in FIXED_ENVIRONMENT {
        command.env(name, value);
    }
}

/// What: Create the private mode-0700 runtime directory for one Pi session.
///
/// Inputs:
/// - `parent`: Directory that will contain the session directory.
/// - `name`: Session directory name.
///
/// Output:
/// - The created directory path.
///
/// Details:
/// - Uses `create_new` semantics via `DirBuilder` so an attacker-planted directory or
///   symlink cannot be reused.
///
/// # Errors
/// - Returns `Err` when the directory already exists or cannot be created.
pub fn create_private_runtime_dir(parent: &Path, name: &str) -> Result<PathBuf, ProcessError> {
    let path = parent.join(name);
    std::fs::create_dir_all(parent).map_err(|source| ProcessError::RuntimeDirectory {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(&path)
        .map_err(|source| ProcessError::RuntimeDirectory {
            path: path.clone(),
            source,
        })?;
    Ok(path)
}

/// What: Materialize the trusted extension atomically at mode 0600.
///
/// Inputs:
/// - `runtime_dir`: Private mode-0700 session directory.
///
/// Output:
/// - Path to the written extension file.
///
/// Details:
/// - `create_new(true)` plus mode 0600 avoids symlink-following TOCTOU races and keeps
///   the asset unreadable by other users.
/// - Writing does not imply trust: [`verify_extension_asset`] must still pass before launch.
///
/// # Errors
/// - Returns `Err` when the file already exists or cannot be created or written.
pub fn materialize_extension(runtime_dir: &Path) -> Result<PathBuf, ProcessError> {
    use std::io::Write as _;

    let path = runtime_dir.join(EMBEDDED_EXTENSION_FILE_NAME);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|source| ProcessError::RuntimeDirectory {
            path: path.clone(),
            source,
        })?;
    file.write_all(EMBEDDED_EXTENSION_SOURCE.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|source| ProcessError::RuntimeDirectory {
            path: path.clone(),
            source,
        })?;
    Ok(path)
}

/// What: Write the private snapshot descriptor next to the materialized extension.
///
/// Inputs:
/// - `runtime_dir`: Private mode-0700 session directory.
/// - `registry`: Snapshot ids and roots Pacsea approved for this scan.
///
/// Output:
/// - Path to the written descriptor file.
///
/// Details:
/// - Written with `create_new(true)` at mode 0600 for the same TOCTOU and privacy reasons
///   as the extension itself.
/// - The descriptor is per-scan data, so it is deliberately not part of the verified asset
///   hash; its integrity comes from the private mode-0700 directory and mode-0600 file.
///
/// # Errors
/// - Returns `Err` when the file already exists or cannot be created or written.
pub fn materialize_descriptor(
    runtime_dir: &Path,
    registry: &super::restricted_tools::SnapshotRegistry,
) -> Result<PathBuf, ProcessError> {
    use std::io::Write as _;

    let path = runtime_dir.join(SNAPSHOT_DESCRIPTOR_FILE_NAME);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|source| ProcessError::RuntimeDirectory {
            path: path.clone(),
            source,
        })?;
    file.write_all(registry.to_descriptor_json().as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|source| ProcessError::RuntimeDirectory {
            path: path.clone(),
            source,
        })?;
    Ok(path)
}

/// What: Verify that the on-disk extension still matches the compiled trusted asset.
///
/// Inputs:
/// - `path`: Materialized extension file.
///
/// Output:
/// - The verified lowercase hex digest.
///
/// Details:
/// - Re-reads the file rather than trusting the write, so tampering, truncation, or a
///   swapped replacement between materialization and launch is detected.
/// - This is the mandatory pre-launch gate; [`launch_pi`] refuses to spawn without it.
///
/// # Errors
/// - Returns `Err` when the file is unreadable or its digest differs from the compiled asset.
pub fn verify_extension_asset(path: &Path) -> Result<String, ProcessError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|source| ProcessError::RuntimeDirectory {
            path: path.to_path_buf(),
            source,
        })?;
    if !metadata.file_type().is_file() {
        return Err(ProcessError::InvalidLaunchSpec {
            reason: "the trusted extension path is not a regular file".to_string(),
        });
    }
    let bytes = std::fs::read(path).map_err(|source| ProcessError::RuntimeDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    let observed = to_hex(&sha256(&bytes));
    let expected = embedded_extension_sha256();
    if observed == expected {
        Ok(observed)
    } else {
        Err(ProcessError::ExtensionHashMismatch { expected, observed })
    }
}

/// What: Fully described, verified launch inputs for one Pi session.
///
/// Inputs: Assembled by the runtime before spawning.
///
/// Output: Consumed by [`launch_pi`].
///
/// Details:
/// - `neutral_cwd` must be an empty private directory so Pi discovers no ambient
///   project context, and it is never the Pacsea repository or the user's cwd.
#[derive(Debug, Clone)]
pub struct PiLaunchSpec {
    /// Absolute path to the resolved Pi executable.
    pub executable: PathBuf,
    /// Empty private working directory for the child.
    pub neutral_cwd: PathBuf,
    /// Materialized trusted extension path.
    pub extension_path: PathBuf,
}

/// What: A running Pi child plus the information needed to terminate its whole group.
///
/// Inputs: Produced by [`launch_pi`].
///
/// Output: Bounded teardown through [`PiProcess::terminate_group`].
///
/// Details:
/// - The child is its own process-group leader, so a forked grandchild that ignores
///   `SIGTERM` is still reachable by `killpg`.
#[derive(Debug)]
pub struct PiProcess {
    /// The spawned Pi child.
    pub child: Child,
    /// Verified extension digest recorded into scan provenance.
    pub extension_sha256: String,
    /// Tool contract version recorded into scan provenance.
    pub tool_contract_version: &'static str,
}

/// What: Verify absolute launch paths and the empty private neutral directory.
///
/// Inputs:
/// - `spec`: Candidate launch specification.
///
/// Output:
/// - `Ok(())` only when every startup path invariant holds.
///
/// Details:
/// - The executable, extension, and cwd must be absolute; the cwd must be a real mode-0700
///   directory with no entries, so Pi cannot discover project or user context there.
///
/// # Errors
/// - Returns `Err` for relative paths, symlink/non-directory cwd, non-private mode, or entries.
fn validate_launch_spec(spec: &PiLaunchSpec) -> Result<(), ProcessError> {
    if !spec.executable.is_absolute()
        || !spec.extension_path.is_absolute()
        || !spec.neutral_cwd.is_absolute()
    {
        return Err(ProcessError::InvalidLaunchSpec {
            reason: "the executable, extension, and neutral cwd paths must all be absolute"
                .to_string(),
        });
    }
    let metadata = std::fs::symlink_metadata(&spec.neutral_cwd).map_err(|source| {
        ProcessError::RuntimeDirectory {
            path: spec.neutral_cwd.clone(),
            source,
        }
    })?;
    if !metadata.file_type().is_dir() {
        return Err(ProcessError::InvalidLaunchSpec {
            reason: "the neutral cwd must be a real directory, not a symlink".to_string(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(ProcessError::InvalidLaunchSpec {
                reason: "the neutral cwd must have mode 0700".to_string(),
            });
        }
    }
    let mut entries =
        std::fs::read_dir(&spec.neutral_cwd).map_err(|source| ProcessError::RuntimeDirectory {
            path: spec.neutral_cwd.clone(),
            source,
        })?;
    if entries.next().is_some() {
        return Err(ProcessError::InvalidLaunchSpec {
            reason: "the neutral cwd must be empty".to_string(),
        });
    }
    Ok(())
}

/// What: Launch Pi with the exact isolated scanner contract.
///
/// Inputs:
/// - `spec`: Verified executable, neutral cwd, and materialized extension.
///
/// Output:
/// - The running child with piped stdio.
///
/// Details:
/// - Verifies the extension digest first and returns before spawning on mismatch, so a
///   tampered asset can never reach a `Command::spawn` call.
/// - On Unix the child becomes its own process-group leader through the safe
///   `Command::process_group(0)` API; no `unsafe` `pre_exec` or raw libc call is used.
///
/// # Errors
/// - Returns `Err` on digest mismatch, unreadable asset, spawn failure, or unpiped stdio.
pub fn launch_pi(spec: &PiLaunchSpec) -> Result<PiProcess, ProcessError> {
    let extension_sha256 = verify_extension_asset(&spec.extension_path)?;
    validate_launch_spec(spec)?;

    let mut command = Command::new(&spec.executable);
    command
        .current_dir(&spec.neutral_cwd)
        .args(pi_argv(&spec.extension_path))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_environment(&mut command);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }

    let child = command.spawn().map_err(|source| ProcessError::Spawn {
        executable: spec.executable.clone(),
        source,
    })?;
    if child.stdin.is_none() {
        return Err(ProcessError::MissingStream { stream: "stdin" });
    }
    if child.stdout.is_none() {
        return Err(ProcessError::MissingStream { stream: "stdout" });
    }
    if child.stderr.is_none() {
        return Err(ProcessError::MissingStream { stream: "stderr" });
    }
    Ok(PiProcess {
        child,
        extension_sha256,
        tool_contract_version: TOOL_CONTRACT_VERSION,
    })
}

/// What: How a Pi process group was finally stopped.
///
/// Inputs: Returned by [`PiProcess::terminate_group`].
///
/// Output: Provenance for cancellation and shutdown reporting.
///
/// Details:
/// - `Exited` means the child settled inside the grace period without a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationOutcome {
    /// The child exited on its own before any signal was sent.
    Exited(ExitStatus),
    /// The group was terminated with `SIGTERM` and reaped.
    Terminated,
    /// The group ignored `SIGTERM` and was killed with `SIGKILL` and reaped.
    Killed,
}

/// What: Encode and write the ordered `abort_retry` and `abort` RPC controls.
///
/// Inputs:
/// - `rpc_stdin`: Pi stdin writer.
/// - `correlator`: Session command correlator.
///
/// Output:
/// - `Ok(())` after both LF records are flushed.
///
/// Details:
/// - Uses the same strict encoder and monotonic ids as every other command.
///
/// # Errors
/// - Returns `Err` for correlation, encoding, write, or flush failure.
fn write_abort_commands<W: std::io::Write>(
    rpc_stdin: &mut W,
    correlator: &mut super::protocol::CommandCorrelator,
) -> Result<(), ProcessError> {
    let fields = serde_json::Map::new();
    for command in ["abort_retry", "abort"] {
        let id = correlator
            .issue(command)
            .map_err(|error| ProcessError::AbortRpc {
                reason: error.to_string(),
            })?;
        let encoded = super::protocol::encode_command(&id, command, &fields).map_err(|error| {
            ProcessError::AbortRpc {
                reason: error.to_string(),
            }
        })?;
        rpc_stdin
            .write_all(&encoded)
            .map_err(|error| ProcessError::AbortRpc {
                reason: error.to_string(),
            })?;
    }
    rpc_stdin.flush().map_err(|error| ProcessError::AbortRpc {
        reason: error.to_string(),
    })
}

impl PiProcess {
    /// What: Send correlated abort controls, suppress pending responses, and reap the group.
    ///
    /// Inputs:
    /// - `rpc_stdin`: Pi's piped stdin.
    /// - `correlator`: Current session correlation state.
    /// - `grace`: Time allowed for the RPC abort to make Pi exit voluntarily.
    /// - `deadline`: Total abort/termination deadline.
    ///
    /// Output:
    /// - How the process group was stopped.
    ///
    /// Details:
    /// - Writes `abort_retry` before `abort`, clears every outstanding command regardless of
    ///   pipe success, then performs bounded process-group TERM/KILL/reap.
    /// - A pipe failure never skips process teardown; it is reported only after the child is
    ///   safely reaped.
    ///
    /// # Errors
    /// - Returns `Err` for RPC encoding/write failure or process teardown failure.
    pub fn abort_and_terminate<W: std::io::Write>(
        &mut self,
        rpc_stdin: &mut W,
        correlator: &mut super::protocol::CommandCorrelator,
        grace: Duration,
        deadline: Duration,
    ) -> Result<TerminationOutcome, ProcessError> {
        let rpc_result = write_abort_commands(rpc_stdin, correlator);
        correlator.clear();
        let outcome = self.terminate_group(grace, deadline)?;
        match rpc_result {
            Ok(()) => Ok(outcome),
            Err(error) => Err(error),
        }
    }

    /// What: Stop the whole Pi process group within a bounded deadline and reap it.
    ///
    /// Inputs:
    /// - `grace`: Time allowed after `SIGTERM` before escalating to `SIGKILL`.
    /// - `deadline`: Total time allowed for the whole termination sequence.
    ///
    /// Output:
    /// - How the group was stopped.
    ///
    /// Details:
    /// - Callers must send the RPC `abort` and drop correlation state before calling this;
    ///   this function performs only the process-level half of cancellation.
    /// - Signals target the negative pgid through `nix::sys::signal::killpg`, so a forked
    ///   grandchild cannot survive as an orphan.
    /// - The child is always waited on, so no zombie remains even after `SIGKILL`.
    ///
    /// # Errors
    /// - Returns `Err` when the group is still alive after `deadline`, or when a wait fails.
    pub fn terminate_group(
        &mut self,
        grace: Duration,
        deadline: Duration,
    ) -> Result<TerminationOutcome, ProcessError> {
        let started = Instant::now();
        if let Some(status) = self.poll_until(started, grace.min(deadline))? {
            return Ok(TerminationOutcome::Exited(status));
        }
        self.signal_group(Signal::Term)?;
        if self
            .poll_until(started, deadline.min(started.elapsed() + grace))?
            .is_some()
        {
            return Ok(TerminationOutcome::Terminated);
        }
        self.signal_group(Signal::Kill)?;
        if self.poll_until(started, deadline)?.is_some() {
            return Ok(TerminationOutcome::Killed);
        }
        Err(ProcessError::TerminationTimeout { deadline })
    }

    /// What: Poll the child until it exits or the elapsed budget runs out.
    ///
    /// Inputs:
    /// - `started`: Start of the whole termination sequence.
    /// - `budget`: Maximum elapsed time since `started`.
    ///
    /// Output:
    /// - `Some(status)` when the child was reaped, `None` on timeout.
    ///
    /// Details:
    /// - Polls at a short fixed interval so shutdown stays responsive without busy waiting.
    ///
    /// # Errors
    /// - Returns `Err` when the wait syscall fails.
    fn poll_until(
        &mut self,
        started: Instant,
        budget: Duration,
    ) -> Result<Option<ExitStatus>, ProcessError> {
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(Some(status)),
                Ok(None) => {}
                Err(error) => {
                    return Err(ProcessError::Signal {
                        operation: "wait",
                        reason: error.to_string(),
                    });
                }
            }
            if started.elapsed() >= budget {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// What: Send one signal to the child's entire process group.
    ///
    /// Inputs:
    /// - `signal`: Termination signal to deliver.
    ///
    /// Output:
    /// - `Ok(())` when delivered, or when the group already vanished.
    ///
    /// Details:
    /// - Uses the safe `nix` wrapper. `ESRCH` is treated as success because a group that
    ///   already exited needs no signal.
    /// - On non-Unix targets this is a no-op; the scanner is Arch/Linux-only at runtime and
    ///   Windows builds only need to compile.
    ///
    /// # Errors
    /// - Returns `Err` when signal delivery fails for a reason other than a missing group.
    #[allow(
        clippy::unused_self,
        reason = "non-Unix builds keep the same method shape so callers stay portable"
    )]
    fn signal_group(&self, signal: Signal) -> Result<(), ProcessError> {
        #[cfg(unix)]
        {
            use nix::errno::Errno;
            use nix::sys::signal::{Signal as NixSignal, killpg};
            use nix::unistd::Pid;

            let raw = i32::try_from(self.child.id()).map_err(|error| ProcessError::Signal {
                operation: "pgid conversion",
                reason: error.to_string(),
            })?;
            let nix_signal = match signal {
                Signal::Term => NixSignal::SIGTERM,
                Signal::Kill => NixSignal::SIGKILL,
            };
            match killpg(Pid::from_raw(raw), nix_signal) {
                Ok(()) | Err(Errno::ESRCH) => Ok(()),
                Err(errno) => Err(ProcessError::Signal {
                    operation: "killpg",
                    reason: errno.to_string(),
                }),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = signal;
            Ok(())
        }
    }
}

/// What: The two signals used by bounded Pi teardown.
///
/// Inputs: Chosen by [`PiProcess::terminate_group`].
///
/// Output: Mapped to `nix` signals on Unix.
///
/// Details:
/// - Kept as a tiny local enum so no platform type leaks into the module's public API.
#[derive(Debug, Clone, Copy)]
enum Signal {
    /// Polite termination request.
    Term,
    /// Unconditional kill after the grace period.
    Kill,
}

/// What: Default abort grace derived from the compiled bound.
///
/// Inputs: None.
///
/// Output:
/// - Five-second grace period.
///
/// Details:
/// - Exposed so the runtime and tests share one source of truth.
#[must_use]
pub const fn default_abort_grace() -> Duration {
    Duration::from_secs(limits::ABORT_GRACE_SECONDS)
}

/// What: Default total shutdown deadline derived from the compiled bound.
///
/// Inputs: None.
///
/// Output:
/// - Ten-second deadline.
///
/// Details:
/// - Exposed so the runtime and tests share one source of truth.
#[must_use]
pub const fn default_shutdown_deadline() -> Duration {
    Duration::from_secs(limits::SHUTDOWN_DEADLINE_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::{
        EMBEDDED_EXTENSION_FILE_NAME, EMBEDDED_EXTENSION_SHA256, EMBEDDED_EXTENSION_SOURCE,
        FIXED_ENVIRONMENT, PASSTHROUGH_ENVIRONMENT, PiLaunchSpec, ProcessError,
        configure_environment, create_private_runtime_dir, default_abort_grace,
        default_shutdown_deadline, embedded_extension_sha256, launch_pi, materialize_extension,
        pi_argv, restricted_tool_csv, verify_extension_asset,
    };
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// Verify the argv is the exact fixed isolation contract in the exact order.
    #[test]
    fn argv_is_the_exact_isolation_contract() {
        let argv: Vec<String> = pi_argv(Path::new("/run/pacsea/ext.ts"))
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            argv,
            vec![
                "--mode",
                "rpc",
                "--no-session",
                "--offline",
                "--no-builtin-tools",
                "--no-extensions",
                "--no-skills",
                "--no-prompt-templates",
                "--no-context-files",
                "--no-themes",
                "--no-approve",
                "--extension",
                "/run/pacsea/ext.ts",
                "--tools",
                "pacsea_scan_find,pacsea_scan_grep,pacsea_scan_ls,pacsea_scan_read",
            ]
        );
        assert!(
            !argv.iter().any(|arg| arg.contains("-c") && arg.len() == 2),
            "argv must never contain a shell -c form"
        );
        assert_eq!(
            restricted_tool_csv(),
            "pacsea_scan_find,pacsea_scan_grep,pacsea_scan_ls,pacsea_scan_read"
        );
    }

    /// Verify the environment is a positive allowlist that excludes injected secrets.
    #[test]
    #[cfg(unix)]
    fn environment_is_a_positive_allowlist() {
        let mut command = Command::new("/usr/bin/env");
        command.env("PACSEA_UNLISTED_SECRET", "must-not-survive");
        command.env("HTTPS_PROXY", "http://attacker.invalid");
        command.env("SSH_AUTH_SOCK", "/tmp/agent.sock");
        command.env("ANTHROPIC_API_KEY", "sk-must-not-survive");
        configure_environment(&mut command);
        let output = command.output().expect("bounded env probe must launch");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("env output must be UTF-8");
        let names: Vec<&str> = stdout
            .lines()
            .filter_map(|line| line.split_once('=').map(|(name, _)| name))
            .collect();
        for forbidden in [
            "PACSEA_UNLISTED_SECRET",
            "HTTPS_PROXY",
            "SSH_AUTH_SOCK",
            "ANTHROPIC_API_KEY",
        ] {
            assert!(!names.contains(&forbidden), "{forbidden} leaked into Pi");
        }
        assert!(names.iter().all(|name| {
            PASSTHROUGH_ENVIRONMENT.contains(name)
                || FIXED_ENVIRONMENT.iter().any(|(fixed, _)| fixed == name)
        }));
        for (name, value) in FIXED_ENVIRONMENT {
            assert!(stdout.lines().any(|line| line == format!("{name}={value}")));
        }
    }

    /// Verify the embedded source matches the build-reviewed compiled digest.
    #[test]
    fn embedded_asset_matches_the_reviewed_digest() {
        assert_eq!(
            crate::pi_agent::to_hex(&crate::pi_agent::sha256(
                EMBEDDED_EXTENSION_SOURCE.as_bytes()
            )),
            EMBEDDED_EXTENSION_SHA256
        );
        assert_eq!(embedded_extension_sha256(), EMBEDDED_EXTENSION_SHA256);
    }

    /// Verify materialization writes the compiled asset with private permissions.
    #[test]
    fn materialized_extension_matches_the_compiled_asset() {
        let temp = tempfile::tempdir().expect("temp dir");
        let runtime = create_private_runtime_dir(temp.path(), "session-1").expect("runtime dir");
        let extension = materialize_extension(&runtime).expect("materializes");
        assert_eq!(
            extension.file_name().and_then(|n| n.to_str()),
            Some(EMBEDDED_EXTENSION_FILE_NAME)
        );
        assert_eq!(
            std::fs::read_to_string(&extension).expect("readable"),
            EMBEDDED_EXTENSION_SOURCE
        );
        assert_eq!(
            verify_extension_asset(&extension).expect("verifies"),
            embedded_extension_sha256()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let file_mode = std::fs::metadata(&extension)
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(file_mode & 0o777, 0o600, "extension must be private");
            let dir_mode = std::fs::metadata(&runtime)
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(dir_mode & 0o777, 0o700, "runtime dir must be private");
        }
        assert!(
            materialize_extension(&runtime).is_err(),
            "create_new must refuse to overwrite an existing path"
        );
    }

    /// Verify tampering, truncation, and replacement are all detected before launch.
    #[test]
    fn tampered_extension_fails_verification_and_blocks_launch() {
        for (label, replacement) in [
            (
                "appended",
                format!("{EMBEDDED_EXTENSION_SOURCE}\n// tamper"),
            ),
            (
                "truncated",
                EMBEDDED_EXTENSION_SOURCE[..EMBEDDED_EXTENSION_SOURCE.len() / 2].to_string(),
            ),
            ("replaced", "export default function () {}\n".to_string()),
        ] {
            let temp = tempfile::tempdir().expect("temp dir");
            let runtime = create_private_runtime_dir(temp.path(), "session").expect("runtime dir");
            let extension = materialize_extension(&runtime).expect("materializes");
            std::fs::write(&extension, replacement).expect("rewrite");
            let error = verify_extension_asset(&extension)
                .expect_err(&format!("{label} asset must fail verification"));
            let ProcessError::ExtensionHashMismatch { expected, observed } = &error else {
                panic!("{label}: unexpected error {error:?}");
            };
            assert_eq!(expected, &embedded_extension_sha256());
            assert_ne!(expected, observed);

            // A mismatch must prevent process creation entirely: point at an executable
            // that would fail loudly if it were ever reached.
            let spec = PiLaunchSpec {
                executable: PathBuf::from("/nonexistent/pacsea-pi-must-not-run"),
                neutral_cwd: temp.path().to_path_buf(),
                extension_path: extension.clone(),
            };
            let launch_error = launch_pi(&spec).expect_err("launch must fail closed");
            assert!(
                matches!(launch_error, ProcessError::ExtensionHashMismatch { .. }),
                "hash verification must run before spawn, got {launch_error:?}"
            );
        }
    }

    /// Verify a valid asset cannot launch from a non-empty neutral cwd.
    #[test]
    fn nonempty_neutral_cwd_blocks_launch() {
        let temp = tempfile::tempdir().expect("temp dir");
        let runtime = create_private_runtime_dir(temp.path(), "runtime").expect("runtime");
        let extension = materialize_extension(&runtime).expect("extension");
        let neutral = create_private_runtime_dir(temp.path(), "neutral").expect("neutral");
        std::fs::write(neutral.join("ambient-project-file"), "hostile").expect("fixture");
        let spec = PiLaunchSpec {
            executable: PathBuf::from("/nonexistent/pacsea-pi-must-not-run"),
            neutral_cwd: neutral,
            extension_path: extension,
        };
        assert!(matches!(
            launch_pi(&spec).expect_err("ambient cwd must block spawn"),
            ProcessError::InvalidLaunchSpec { .. }
        ));
    }

    /// Verify a missing extension file also fails closed rather than launching.
    #[test]
    fn missing_extension_blocks_launch() {
        let temp = tempfile::tempdir().expect("temp dir");
        let spec = PiLaunchSpec {
            executable: PathBuf::from("/nonexistent/pacsea-pi-must-not-run"),
            neutral_cwd: temp.path().to_path_buf(),
            extension_path: temp.path().join("absent.ts"),
        };
        assert!(matches!(
            launch_pi(&spec).expect_err("must fail"),
            ProcessError::RuntimeDirectory { .. }
        ));
    }

    /// Verify the compiled bounds are exposed unchanged.
    #[test]
    fn compiled_teardown_bounds_are_exposed() {
        assert_eq!(default_abort_grace().as_secs(), 5);
        assert_eq!(default_shutdown_deadline().as_secs(), 10);
    }

    /// Verify a forked child that ignores SIGTERM is still group-killed and reaped.
    #[test]
    #[cfg(unix)]
    fn stubborn_process_group_is_killed_and_reaped() {
        use super::{PiProcess, TerminationOutcome};
        use std::os::unix::process::CommandExt as _;
        use std::process::Stdio;
        use std::time::{Duration, Instant};

        let Ok(shell) = which::which("sh") else {
            eprintln!("skipping: no POSIX shell available");
            return;
        };
        // Leader ignores TERM and forks a grandchild that also ignores TERM. Only a
        // process-group KILL can stop the pair.
        let mut command = Command::new(shell);
        command
            .arg("-c")
            .arg("trap '' TERM; (trap '' TERM; while :; do sleep 1; done) & while :; do sleep 1; done")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.process_group(0);
        let child = command.spawn().expect("stubborn child must start");
        let mut process = PiProcess {
            child,
            extension_sha256: embedded_extension_sha256(),
            tool_contract_version: super::TOOL_CONTRACT_VERSION,
        };
        let mut rpc = Vec::new();
        let mut correlator = crate::pi_agent::protocol::CommandCorrelator::new();
        correlator.issue("prompt").expect("pending prompt");
        let started = Instant::now();
        let outcome = process
            .abort_and_terminate(
                &mut rpc,
                &mut correlator,
                Duration::from_millis(300),
                Duration::from_secs(5),
            )
            .expect("RPC abort and group teardown must succeed");
        assert_eq!(outcome, TerminationOutcome::Killed);
        assert_eq!(correlator.pending_len(), 0);
        let records: Vec<_> = rpc
            .split(|byte| *byte == b'\n')
            .filter(|record| !record.is_empty())
            .map(|record| {
                crate::pi_agent::protocol::decode_record(record).expect("strict abort record")
            })
            .collect();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["type"], "abort_retry");
        assert_eq!(records[1]["type"], "abort");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(
            process.child.try_wait().expect("reaped").is_some(),
            "the child must already be reaped"
        );
    }

    /// Verify a cooperative child exits inside the grace period without a signal.
    #[test]
    #[cfg(unix)]
    fn cooperative_process_exits_within_grace() {
        use super::{PiProcess, TerminationOutcome};
        use std::os::unix::process::CommandExt as _;
        use std::process::Stdio;
        use std::time::Duration;

        let Ok(shell) = which::which("sh") else {
            eprintln!("skipping: no POSIX shell available");
            return;
        };
        let mut command = Command::new(shell);
        command
            .arg("-c")
            .arg("exit 0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.process_group(0);
        let child = command.spawn().expect("child must start");
        let mut process = PiProcess {
            child,
            extension_sha256: embedded_extension_sha256(),
            tool_contract_version: super::TOOL_CONTRACT_VERSION,
        };
        let outcome = process
            .terminate_group(Duration::from_secs(2), Duration::from_secs(5))
            .expect("teardown must succeed");
        assert!(matches!(outcome, TerminationOutcome::Exited(_)));
    }
}
