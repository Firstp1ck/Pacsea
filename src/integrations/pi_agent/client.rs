//! Piped LF-JSONL transport and private workspace ownership for Pi RPC scans.

use std::fmt;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::process::{
    PiLaunchSpec, PiProcess, create_private_runtime_dir, launch_pi, materialize_descriptor,
    materialize_extension,
};
use super::protocol::{CommandCorrelator, LineFramer, ProtocolError};
use super::restricted_tools::SnapshotRegistry;

/// Sequence for collision-resistant per-process workspace names.
static WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// What: Transport metadata recorded in scan provenance.
///
/// Inputs: Supplied by a production or deterministic fake transport.
///
/// Output: Verified extension and tool-contract identities.
///
/// Details:
/// - Production values come from the launched [`PiProcess`], never model output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportMetadata {
    /// Verified extension asset SHA-256.
    pub extension_sha256: String,
    /// Restricted tool contract version.
    pub tool_contract_version: String,
}

/// What: Bounded transport failure independent of logical scan policy.
///
/// Inputs: Produced by pipe I/O, strict framing, timeout, or process teardown.
///
/// Output: Actionable execution failure without raw provider or source content.
///
/// Details:
/// - Raw stderr, prompts, model output, and source bodies are never retained in variants.
#[derive(Debug)]
pub enum TransportError {
    /// Strict LF JSONL framing failed.
    Protocol(ProtocolError),
    /// A pipe or process operation failed.
    Io(String),
    /// Pi closed stdout before the required response arrived.
    Closed,
    /// The supplied deadline elapsed.
    Timeout,
    /// Sticky caller cancellation interrupted a read.
    Cancelled,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(formatter, "Pi RPC framing failed: {error}"),
            Self::Io(error) => write!(formatter, "Pi RPC transport failed: {error}"),
            Self::Closed => write!(
                formatter,
                "Pi closed its RPC output before the scan settled"
            ),
            Self::Timeout => write!(formatter, "Pi RPC operation exceeded its deadline"),
            Self::Cancelled => write!(formatter, "Pi RPC operation was cancelled"),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<ProtocolError> for TransportError {
    fn from(value: ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

/// What: Injectable raw LF-JSONL child transport used by the logical engine.
///
/// Inputs: Strict encoded records, deadlines, cancellation, and command correlation.
///
/// Output: Strict framed record bytes plus bounded teardown.
///
/// Details:
/// - The seam is intentionally below RPC interpretation so fake tests verify exact framing.
/// - Implementations must reap on both [`RpcTransport::reap`] and
///   [`RpcTransport::abort_and_reap`].
pub trait RpcTransport {
    /// Write and flush exactly one LF-terminated command record.
    ///
    /// # Errors
    /// - Returns a framing, pipe, or closed-transport failure.
    fn write_record(&mut self, record: &[u8]) -> Result<(), TransportError>;

    /// Read exactly one record without its LF terminator before the deadline.
    ///
    /// # Errors
    /// - Returns a framing, pipe, timeout, cancellation, or closed-transport failure.
    fn read_record(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>, TransportError>;

    /// Return total bounded wire bytes observed by the transport.
    fn bytes_exchanged(&self) -> u64;

    /// Return trusted transport provenance.
    fn metadata(&self) -> TransportMetadata;

    /// Send correlated abort controls, terminate the group, reap, and clean the workspace.
    ///
    /// # Errors
    /// - Returns a correlation, pipe, process-control, reap, or cleanup failure.
    fn abort_and_reap(&mut self, correlator: &mut CommandCorrelator) -> Result<(), TransportError>;

    /// Reap a normally completed or failed child and clean its workspace.
    ///
    /// # Errors
    /// - Returns a process-control, reap, or cleanup failure.
    fn reap(&mut self) -> Result<(), TransportError>;
}

/// What: Production piped Pi child transport with private workspace ownership.
///
/// Inputs: Created through [`PiRpcClient::launch`].
///
/// Output: [`RpcTransport`] implementation for one logical scan.
///
/// Details:
/// - Dedicated reader threads drain stdout and stderr concurrently, preventing pipe deadlock.
/// - Stderr is discarded and never persisted; stdout is strictly framed before delivery.
/// - Dropping an unreaped client performs best-effort group termination and cleanup.
pub struct PiRpcClient {
    /// Launched isolated Pi process.
    process: Option<PiProcess>,
    /// Piped stdin.
    stdin: Option<std::process::ChildStdin>,
    /// Strict framed stdout receiver.
    records: Receiver<Result<Vec<u8>, TransportError>>,
    /// Stdout reader thread.
    stdout_reader: Option<JoinHandle<()>>,
    /// Stderr draining thread.
    stderr_reader: Option<JoinHandle<()>>,
    /// Private top-level session workspace.
    workspace: PathBuf,
    /// Total bytes written plus framed bytes received.
    bytes_exchanged: u64,
    /// Trusted metadata copied before process ownership changes.
    metadata: TransportMetadata,
    /// Whether teardown already completed.
    reaped: bool,
}

impl PiRpcClient {
    /// What: Prepare private directories/descriptor/extension and launch isolated Pi.
    ///
    /// Inputs:
    /// - `workspace_parent`: Existing or creatable parent for the ephemeral private session.
    /// - `executable`: Resolved absolute Pi executable.
    /// - `registry`: Already acquired immutable snapshot roots.
    ///
    /// Output:
    /// - Running piped RPC client owning all private artifacts.
    ///
    /// Details:
    /// - Creates separate mode-0700 runtime and neutral directories. The descriptor and
    ///   verified mode-0600 extension live only in the runtime directory.
    /// - No package code, URL, shell, or source acquisition is executed here.
    ///
    /// # Errors
    /// - Returns an error after cleaning partial artifacts when preparation or launch fails.
    pub fn launch(
        workspace_parent: &Path,
        executable: &Path,
        registry: &SnapshotRegistry,
    ) -> Result<Self, TransportError> {
        if !executable.is_absolute() {
            return Err(TransportError::Io(
                "the resolved Pi executable path must be absolute".to_string(),
            ));
        }
        let session = create_private_runtime_dir(workspace_parent, &workspace_name())
            .map_err(|error| TransportError::Io(error.to_string()))?;
        match Self::launch_in_workspace(session.clone(), executable, registry) {
            Ok(client) => Ok(client),
            Err(error) => {
                let _ = std::fs::remove_dir_all(session);
                Err(error)
            }
        }
    }

    /// Launch after the top-level private workspace exists.
    fn launch_in_workspace(
        workspace: PathBuf,
        executable: &Path,
        registry: &SnapshotRegistry,
    ) -> Result<Self, TransportError> {
        let runtime = create_private_runtime_dir(&workspace, "runtime")
            .map_err(|error| TransportError::Io(error.to_string()))?;
        let neutral = create_private_runtime_dir(&workspace, "neutral")
            .map_err(|error| TransportError::Io(error.to_string()))?;
        let extension = materialize_extension(&runtime)
            .map_err(|error| TransportError::Io(error.to_string()))?;
        materialize_descriptor(&runtime, registry)
            .map_err(|error| TransportError::Io(error.to_string()))?;
        let mut process = launch_pi(&PiLaunchSpec {
            executable: executable.to_path_buf(),
            neutral_cwd: neutral,
            extension_path: extension,
        })
        .map_err(|error| TransportError::Io(error.to_string()))?;
        let stdin = process
            .child
            .stdin
            .take()
            .ok_or_else(|| TransportError::Io("Pi stdin was not piped".to_string()))?;
        let stdout = process
            .child
            .stdout
            .take()
            .ok_or_else(|| TransportError::Io("Pi stdout was not piped".to_string()))?;
        let stderr = process
            .child
            .stderr
            .take()
            .ok_or_else(|| TransportError::Io("Pi stderr was not piped".to_string()))?;
        let metadata = TransportMetadata {
            extension_sha256: process.extension_sha256.clone(),
            tool_contract_version: process.tool_contract_version.to_string(),
        };
        let (sender, records) = mpsc::sync_channel(1);
        let stdout_reader = std::thread::spawn(move || read_stdout(stdout, &sender));
        let stderr_reader = std::thread::spawn(move || drain_stderr(stderr));
        Ok(Self {
            process: Some(process),
            stdin: Some(stdin),
            records,
            stdout_reader: Some(stdout_reader),
            stderr_reader: Some(stderr_reader),
            workspace,
            bytes_exchanged: 0,
            metadata,
            reaped: false,
        })
    }

    /// Finish process-level teardown and remove private artifacts.
    fn finish(
        &mut self,
        cancelled: bool,
        correlator: &mut CommandCorrelator,
    ) -> Result<(), TransportError> {
        if self.reaped {
            return Ok(());
        }
        let mut result = Ok(());
        if let Some(mut process) = self.process.take() {
            if cancelled {
                if let Some(stdin) = self.stdin.as_mut() {
                    result = process
                        .abort_and_terminate(
                            stdin,
                            correlator,
                            super::process::default_abort_grace(),
                            super::process::default_shutdown_deadline(),
                        )
                        .map(|_| ())
                        .map_err(|error| TransportError::Io(error.to_string()));
                }
            } else {
                self.stdin.take();
                result = process
                    .terminate_group(Duration::from_millis(500), Duration::from_secs(5))
                    .map(|_| ())
                    .map_err(|error| TransportError::Io(error.to_string()));
            }
        }
        self.stdin.take();
        join_reader(self.stdout_reader.take());
        join_reader(self.stderr_reader.take());
        if let Err(error) = std::fs::remove_dir_all(&self.workspace)
            && result.is_ok()
        {
            result = Err(TransportError::Io(format!(
                "could not remove the private Pi workspace: {}",
                error.kind()
            )));
        }
        self.reaped = true;
        result
    }
}

impl RpcTransport for PiRpcClient {
    fn write_record(&mut self, record: &[u8]) -> Result<(), TransportError> {
        if record.last() != Some(&b'\n')
            || record[..record.len().saturating_sub(1)].contains(&b'\n')
        {
            return Err(TransportError::Io(
                "outbound Pi command was not exactly one LF record".to_string(),
            ));
        }
        let stdin = self.stdin.as_mut().ok_or(TransportError::Closed)?;
        stdin
            .write_all(record)
            .and_then(|()| stdin.flush())
            .map_err(|error| TransportError::Io(error.kind().to_string()))?;
        self.bytes_exchanged = self.bytes_exchanged.saturating_add(record.len() as u64);
        Ok(())
    }

    fn read_record(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>, TransportError> {
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err(TransportError::Cancelled);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(TransportError::Timeout);
            }
            let wait = deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(25));
            match self.records.recv_timeout(wait) {
                Ok(Ok(record)) => {
                    self.bytes_exchanged =
                        self.bytes_exchanged.saturating_add(record.len() as u64 + 1);
                    return Ok(record);
                }
                Ok(Err(error)) => return Err(error),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(TransportError::Closed),
            }
        }
    }

    fn bytes_exchanged(&self) -> u64 {
        self.bytes_exchanged
    }

    fn metadata(&self) -> TransportMetadata {
        self.metadata.clone()
    }

    fn abort_and_reap(&mut self, correlator: &mut CommandCorrelator) -> Result<(), TransportError> {
        self.finish(true, correlator)
    }

    fn reap(&mut self) -> Result<(), TransportError> {
        self.finish(false, &mut CommandCorrelator::new())
    }
}

impl Drop for PiRpcClient {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.finish(false, &mut CommandCorrelator::new());
        }
    }
}

/// Strictly frame stdout chunks and send complete bounded records.
fn read_stdout(
    mut stdout: std::process::ChildStdout,
    sender: &mpsc::SyncSender<Result<Vec<u8>, TransportError>>,
) {
    let mut framer = LineFramer::default();
    let mut chunk = [0u8; 8192];
    loop {
        match stdout.read(&mut chunk) {
            Ok(0) => {
                if framer.pending_len() > 0 {
                    let _ = sender.send(Err(TransportError::Io(
                        "Pi closed stdout with an incomplete JSONL record".to_string(),
                    )));
                }
                break;
            }
            Ok(count) => {
                if let Err(error) = framer.push(&chunk[..count]) {
                    let _ = sender.send(Err(TransportError::Protocol(error)));
                    break;
                }
                while let Some(record) = framer.next_record() {
                    if sender.send(Ok(record)).is_err() {
                        return;
                    }
                }
            }
            Err(error) => {
                let _ = sender.send(Err(TransportError::Io(error.kind().to_string())));
                break;
            }
        }
    }
}

/// Drain diagnostics without retaining or logging potentially sensitive text.
fn drain_stderr(mut stderr: std::process::ChildStderr) {
    let mut chunk = [0u8; 8192];
    while let Ok(count) = stderr.read(&mut chunk) {
        if count == 0 {
            break;
        }
    }
}

/// Join a reader after process teardown; panics become inert teardown failures.
fn join_reader(reader: Option<JoinHandle<()>>) {
    if let Some(reader) = reader {
        let _ = reader.join();
    }
}

/// Build a unique, non-hostile private workspace name.
fn workspace_name() -> String {
    let sequence = WORKSPACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("scan-{}-{nanos}-{sequence}", std::process::id())
}
