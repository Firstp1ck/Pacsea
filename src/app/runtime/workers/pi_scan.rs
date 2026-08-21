//! Sequential runtime worker for optional Pi-backed AUR scanning.
//!
//! The worker is default-off and does not acquire sources or launch Pi. It consumes frozen
//! WS1 identities, publishes an execution dispatch for the deferred acquisition/execution
//! owner, and can be given a WS2 process target so cancellation always uses correlated RPC
//! abort followed by bounded process-group termination and reap.

use crate::pi_agent::process::{
    PiProcess, ProcessError, default_abort_grace, default_shutdown_deadline,
};
use crate::pi_agent::protocol::CommandCorrelator;
use crate::state::pi_scan::{
    PiScanActiveItem, PiScanActualUsage, PiScanBudgetAdjustment, PiScanBudgetAdjustmentResult,
    PiScanConsentState, PiScanJobRequest, PiScanPersistedState, PiScanPersistenceError,
    PiScanQueueKey, PiScanRuntimeState, PiScanStartBlock, PiScanTerminalRecord, load_pi_scan_state,
    save_pi_scan_state_atomic,
};
use std::fmt;
use std::path::PathBuf;
use std::process::ChildStdin;
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Worker-side deadline kept below the ten-second total cleanup bound.
const WORKER_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(9);
/// Budget-pause revalidation interval while Pacsea remains open.
const BUDGET_REVALIDATION_INTERVAL: Duration = Duration::from_secs(30);

/// What: Runtime configuration supplied by central integration or WS4.
///
/// Inputs:
/// - Default-off feature gate, dry-run flag, and durable paths.
///
/// Output:
/// - Startup policy for one sequential worker.
///
/// Details:
/// - `enabled` is effective only on Linux. Dry-run never loads, writes, or launches
///   durable/model state and instead emits inert previews.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanRuntimeOptions {
    /// Explicit runtime enablement; defaults to false.
    pub enabled: bool,
    /// Session dry-run mode.
    pub dry_run: bool,
    /// Versioned runtime state path.
    pub state_path: PathBuf,
    /// Private quarantine directory.
    pub quarantine_dir: PathBuf,
    /// Production adapter/service settings, absent for inert tests and default-off startup.
    pub production: Option<crate::pi_scan_production::ProductionRuntimeSettings>,
}

impl Default for PiScanRuntimeOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            dry_run: false,
            state_path: PathBuf::new(),
            quarantine_dir: PathBuf::new(),
            production: None,
        }
    }
}

impl PiScanRuntimeOptions {
    /// Return whether explicit enablement and the compiled platform gate both allow runtime use.
    #[must_use]
    pub const fn effective_enabled(&self) -> bool {
        self.enabled && pi_scan_runtime_supported()
    }
}

/// Return whether this binary target supports Arch/Linux Pi runtime execution.
#[must_use]
pub const fn pi_scan_runtime_supported() -> bool {
    cfg!(target_os = "linux")
}

/// What: Typed commands accepted by the sequential scanner worker.
///
/// Inputs:
/// - Queue, consent, pause, service validation, completion, or budget-revalidation data.
///
/// Output:
/// - Progress and result messages on the paired channels.
///
/// Details:
/// - These commands never contain source bodies, prompts, credentials, or Pi wire records.
#[derive(Debug)]
pub enum PiScanRequestMessage {
    /// Probe and publish exact setup facts without changing consent.
    ProbeSetup,
    /// Observe only the explicitly selected unresolved package names in the foreground.
    ManualObservation {
        /// Exact installed package names selected in the Targets view.
        package_names: Vec<String>,
    },
    /// Append one frozen package-base and commit request.
    Enqueue(PiScanJobRequest),
    /// Replace independent observation and paid-execution consent.
    SetConsent(PiScanConsentState),
    /// Persist runtime consent plus material-bound setup confirmations after explicit UI change.
    SetConsentDetails {
        /// Independent observation and paid-execution consent.
        consent: PiScanConsentState,
        /// Provider/privacy/cost/coverage disclosure confirmation.
        disclosure_confirmed: bool,
        /// Ordered fallback confirmation.
        fallback_confirmed: bool,
        /// Independent paid background-execution confirmation.
        background_paid_execution_confirmed: bool,
        /// Readiness-warning confirmation.
        readiness_warning_confirmed: bool,
    },
    /// Apply or clear the user-owned sticky pause.
    SetUserPaused(bool),
    /// Apply a service/security/readiness pause.
    PauseForService,
    /// Clear service pause only when the named validation succeeded.
    ClearServicePause {
        /// Whether the caller completed the required validation successfully.
        validation_succeeded: bool,
    },
    /// Complete one exact active correlation and identity.
    Complete {
        /// Runtime correlation id.
        correlation_id: u64,
        /// Exact package-base and commit identity returned by execution.
        key: PiScanQueueKey,
        /// Actual or conservative bounded usage.
        usage: PiScanActualUsage,
        /// Completion timestamp in Unix seconds.
        finished_at_unix: u64,
    },
    /// Replace typed update candidates captured by the current update cycle.
    UpdateCandidates(Vec<crate::pi_scan_orchestrator::UpdateCandidate>),
    /// Accept one complete current-HEAD result as the independent observation baseline.
    AcceptBaseline {
        /// Canonical package base.
        package_base: crate::logic::pi_scan::identity::PackageBase,
        /// Exact current-HEAD commit.
        commit_oid: crate::logic::pi_scan::identity::CommitOid,
        /// Stored result scan id.
        scan_id: String,
        /// Exact result binding recorded as baseline evidence.
        result_binding: String,
    },
    /// Re-resolve official AUR HEAD before linked install/update continuation.
    ValidateContinuation {
        /// Canonical package base bound to the validated result.
        package_base: crate::logic::pi_scan::identity::PackageBase,
        /// HEAD identity observed and scanned.
        observed_head_oid: crate::logic::pi_scan::identity::CommitOid,
        /// Mutable Git refs resolved during advisory acquisition.
        mutable_sources: Vec<crate::logic::pi_scan::acquisition::MutableSourceIdentity>,
        /// Exact result/acknowledgement binding awaiting continuation.
        result_binding: String,
    },
    /// Adjust every budget exceeded by the next queued background reservation.
    AdjustBudgets {
        /// Exact checked Double or affected-only Unlimited policy.
        adjustment: PiScanBudgetAdjustment,
        /// Unix timestamp used to recompute and revalidate rolling windows.
        now_unix: u64,
        /// Request-owned typed acknowledgement destination.
        acknowledge: mpsc::UnboundedSender<PiScanBudgetAdjustmentAcknowledgement>,
    },
    /// Revalidate rolling windows at a deterministic timestamp.
    RevalidateBudgets {
        /// Unix timestamp used for rolling windows.
        now_unix: u64,
    },
}

/// Typed cancellation request for one exact active correlation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanCancelMessage {
    /// Runtime correlation id to abort and suppress.
    pub correlation_id: u64,
    /// Cancellation timestamp in Unix seconds.
    pub requested_at_unix: u64,
}

/// What: Bounded worker shutdown request with an acknowledgement channel.
///
/// Inputs:
/// - One synchronous acknowledgement sender owned by cleanup.
///
/// Output:
/// - Exactly one [`PiScanShutdownAck`] when the worker reaches its durability boundary.
///
/// Details:
/// - Active Pi work is aborted/reaped first and recovered as interrupted with full
///   reservation consumption before persistence is attempted.
pub struct PiScanShutdownMessage {
    /// One-shot bounded acknowledgement sender.
    pub acknowledge: std_mpsc::SyncSender<PiScanShutdownAck>,
}

impl fmt::Debug for PiScanShutdownMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PiScanShutdownMessage")
            .finish_non_exhaustive()
    }
}

/// Worker shutdown acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanShutdownAck {
    /// Whether durable state reached the atomic persistence boundary.
    pub persisted: bool,
    /// Whether an active item was recovered as interrupted.
    pub active_interrupted: bool,
    /// Actionable persistence or abort warning, when any.
    pub warning: Option<String>,
}

/// Source class for one typed runtime notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanNoticeSource {
    /// Direct response to a foreground user action.
    Foreground,
    /// Observation or unattended work not initiated by the current action.
    Background,
    /// Runtime lifecycle or recovery information.
    System,
}

/// Foreground action attached to runtime notice provenance when available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanRuntimeAction {
    /// Request the sticky durable user pause.
    Pause,
    /// Request clearing the sticky durable user pause.
    Resume,
}

/// What: Typed provenance retained with every new runtime notice protocol message.
///
/// Inputs:
/// - Source class, optional foreground action, and optional active correlation.
///
/// Output:
/// - Projection-safe attribution that never depends on the latest UI action.
///
/// Details:
/// - Background and system producers leave `action` absent unless a later typed action exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanNoticeProvenance {
    /// Foreground, background, or system producer class.
    pub source: PiScanNoticeSource,
    /// Exact typed action when this notice acknowledges one.
    pub action: Option<PiScanRuntimeAction>,
    /// Active runtime correlation when the notice is tied to one run.
    pub correlation_id: Option<u64>,
}

/// Durable policy acknowledgement state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanPolicyAcknowledgement {
    /// Persistence is queued behind the active execution under the orchestrator lock.
    Queued,
    /// Persistence completed before another execution may start.
    Persisted,
    /// Persistence failed and the policy must not be presented as applied.
    Failed {
        /// Actionable persistence/recovery guidance.
        reason: String,
    },
}

/// What: Typed runtime notice consumed by later foreground/background UI projection.
///
/// Inputs:
/// - Immutable provenance and truthful policy acknowledgement state.
///
/// Output:
/// - Notice protocol independent from the legacy result channel.
///
/// Details:
/// - The dedicated channel lets Wave C add projection without changing existing result matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanRuntimeNotice {
    /// Attribution and correlation supplied by the runtime producer.
    pub provenance: PiScanNoticeProvenance,
    /// Requested sticky user-pause value.
    pub user_paused: bool,
    /// Queued, persisted, or failed durability state.
    pub acknowledgement: PiScanPolicyAcknowledgement,
}

/// Typed progress update published by the runtime worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanProgressMessage {
    /// Exact no-model setup facts verified for display before consent.
    SetupVerified(crate::pi_scan_orchestrator::SetupSnapshot),
    /// Durable queue/terminal/full-target projection restored after restart.
    RestoredRuntime(Box<crate::pi_scan_orchestrator::OrchestrationState>),
    /// Durable runtime/setup consent restored under the current material configuration binding.
    RestoredConsent {
        /// Independent observation and paid-execution consent.
        consent: PiScanConsentState,
        /// Material-bound setup confirmations.
        setup: crate::pi_scan_orchestrator::PiScanSetupConsentState,
    },
    /// Semantically validated persisted results restored for reopen after restart.
    RestoredResults {
        /// Canonical stored result documents.
        documents: Vec<crate::logic::pi_scan::result_store::StoredScanResult>,
    },
    /// Production observation resolved full immutable targets for UI selection.
    Observed {
        /// Full identities returned by the central orchestrator.
        targets: Vec<crate::pi_scan_orchestrator::FrozenScanIdentity>,
    },
    /// One request was appended without coalescing.
    Queued {
        /// Exact immutable request now present in the durable queue.
        request: PiScanJobRequest,
        /// Pending queue length.
        queue_len: usize,
    },
    /// One request became active and is ready for deferred acquisition/execution.
    Started(PiScanActiveItem),
    /// Authoritative durable runtime projection after rolling-budget state changed.
    BudgetRevalidated(Box<PiScanRuntimeState>),
    /// Correlation-owned transient execution phase from the production adapter.
    PhaseChanged(crate::state::PiScanExecutionProgress),
    /// Work remains queued behind a policy gate.
    Paused(PiScanStartBlock),
    /// Dry-run preview; no queue, budget, consent, pause, or durable state changed.
    DryRunPreview(PiScanJobRequest),
    /// A correlated WS2 cancellation target was attached to the active item.
    SessionRegistered {
        /// Correlation now owning the process target.
        correlation_id: u64,
    },
    /// Cancellation was accepted and late completion is suppressed.
    Cancelling {
        /// Correlation being aborted.
        correlation_id: u64,
    },
    /// Worker reached its shutdown durability boundary.
    Shutdown(PiScanShutdownAck),
}

/// What: Typed acknowledgement for one authoritative budget adjustment request.
///
/// Inputs:
/// - Selected adjustment, exact scheduler result, and durability/dry-run facts.
///
/// Output:
/// - Projection-safe success or preview consumed by WS2 without reclassifying limits.
///
/// Details:
/// - `durable` is true only after both settings and owner state cross their persistence boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanBudgetAdjustmentAcknowledgement {
    /// Authoritative Apply found no exceeded budget and performed no mutation or wake.
    NoLongerBlocked {
        /// Requested adjustment policy.
        adjustment: PiScanBudgetAdjustment,
        /// Exact empty-affected scheduler result.
        result: PiScanBudgetAdjustmentResult,
        /// Whether this acknowledgement came from inert dry-run evaluation.
        dry_run: bool,
    },
    /// Adjustment was durably applied or inertly previewed.
    Applied {
        /// Requested adjustment policy.
        adjustment: PiScanBudgetAdjustment,
        /// Exact affected/residual scheduler result.
        result: PiScanBudgetAdjustmentResult,
        /// Whether the applied policy is durable.
        durable: bool,
        /// Whether this is an inert dry-run preview.
        dry_run: bool,
    },
    /// Adjustment was rejected without an execution wake.
    Rejected {
        /// Requested adjustment policy.
        adjustment: PiScanBudgetAdjustment,
        /// Actionable rejection reason.
        reason: String,
    },
}

/// Typed terminal or rejection message published by the runtime worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanResultMessage {
    /// Bounded acquisition-only dry-run completed without Pi or durable state.
    DryRunAcquired {
        /// Exact acquired package/commit key.
        key: PiScanQueueKey,
        /// Complete, incomplete, or failed acquisition status.
        status: String,
        /// Canonical manifest count produced.
        manifest_count: usize,
        /// Explicit coverage limitations.
        coverage_notes: Vec<String>,
    },
    /// Canonical validated production result accepted and persisted by the orchestrator.
    Validated(Box<crate::pi_scan_orchestrator::ExecutionReceipt>),
    /// Explicit complete current-HEAD baseline was accepted and persisted.
    BaselineAccepted {
        /// Exact result binding accepted as baseline evidence.
        result_binding: String,
    },
    /// Exact continuation staleness recheck completed without invoking a model.
    ContinuationValidated {
        /// Canonical package base that was rechecked.
        package_base: crate::logic::pi_scan::identity::PackageBase,
        /// Exact result binding supplied with the request.
        result_binding: String,
        /// Whether official AUR HEAD changed since the scan identity was frozen.
        stale: bool,
    },
    /// Strictly correlated completion accepted.
    Completed(PiScanTerminalRecord),
    /// Cancellation or shutdown interruption completed; the record retains its exact status.
    Cancelled {
        /// Terminal cancellation or interruption record.
        record: PiScanTerminalRecord,
        /// Actionable abort/reap warning.
        warning: Option<String>,
    },
    /// Active execution failed and its exact terminal transition was persisted.
    Failed {
        /// Correlated failed terminal record.
        record: PiScanTerminalRecord,
        /// Actionable execution failure reason.
        reason: String,
    },
    /// Request was rejected without accepting a stale result or mutating durable state.
    Rejected {
        /// Actionable reason.
        reason: String,
    },
}

/// What: A cancellation target registered by the deferred Pi execution owner.
///
/// Inputs:
/// - Exact active correlation and owned process/session resources.
///
/// Output:
/// - Correlated RPC abort plus bounded process-group reap.
///
/// Details:
/// - Tests may provide an inert recorder. Production uses [`CorrelatedPiAbortTarget`].
pub trait PiScanAbortTarget: Send {
    /// Return the exact active runtime correlation.
    fn correlation_id(&self) -> u64;

    /// Abort retries, abort the request, terminate the group if needed, and reap it.
    fn abort_and_reap(&mut self) -> Result<(), String>;
}

/// Registration message for a started Pi process cancellation target.
pub struct PiScanSessionRegistration {
    /// Exact active runtime correlation.
    pub correlation_id: u64,
    /// Owned cancellation target.
    pub target: Box<dyn PiScanAbortTarget>,
}

impl fmt::Debug for PiScanSessionRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PiScanSessionRegistration")
            .field("correlation_id", &self.correlation_id)
            .finish_non_exhaustive()
    }
}

/// What: Concrete WS2 cancellation target for one correlated Pi session.
///
/// Inputs:
/// - Runtime correlation, Pi process, piped stdin, and command correlator.
///
/// Output:
/// - [`PiScanAbortTarget`] implementation using WS2's bounded abort/group-reap path.
///
/// Details:
/// - The process is never created here; the deferred execution owner must register it only
///   after matching the worker's `Started` dispatch.
pub struct CorrelatedPiAbortTarget {
    /// Runtime correlation bound to this process.
    correlation_id: u64,
    /// Owned Pi child process group.
    process: PiProcess,
    /// Owned RPC stdin used for correlated abort commands.
    rpc_stdin: ChildStdin,
    /// Monotonic command correlation state.
    correlator: CommandCorrelator,
}

impl CorrelatedPiAbortTarget {
    /// What: Bind a launched WS2 process to one active runtime correlation.
    ///
    /// Inputs:
    /// - `correlation_id`: Exact `Started` correlation.
    /// - `process`: Launched isolated Pi process with piped stdin.
    /// - `correlator`: Current session command correlator.
    ///
    /// Output:
    /// - Owned cancellation target.
    ///
    /// Details:
    /// - Takes the child's stdin exactly once so later cancellation cannot use an unrelated pipe.
    ///
    /// # Errors
    /// - Returns [`ProcessError::MissingStream`] when stdin was already taken.
    pub fn new(
        correlation_id: u64,
        mut process: PiProcess,
        correlator: CommandCorrelator,
    ) -> Result<Self, ProcessError> {
        let rpc_stdin = process
            .child
            .stdin
            .take()
            .ok_or(ProcessError::MissingStream { stream: "stdin" })?;
        Ok(Self {
            correlation_id,
            process,
            rpc_stdin,
            correlator,
        })
    }
}

impl PiScanAbortTarget for CorrelatedPiAbortTarget {
    fn correlation_id(&self) -> u64 {
        self.correlation_id
    }

    fn abort_and_reap(&mut self) -> Result<(), String> {
        self.process
            .abort_and_terminate(
                &mut self.rpc_stdin,
                &mut self.correlator,
                default_abort_grace(),
                WORKER_SHUTDOWN_DEADLINE.min(default_shutdown_deadline()),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// What: Typed channel endpoints owned by the event loop and future WS4 integration.
///
/// Inputs:
/// - Created by [`spawn_pi_scan_worker`].
///
/// Output:
/// - Request/cancel/session/shutdown senders and progress/result receivers.
///
/// Details:
/// - Channels are unbounded to match current runtime patterns; the worker itself remains
///   single-consumer and sequential. Queue persistence, not task fan-out, owns ordering.
pub struct PiScanRuntimeChannels {
    /// General runtime request sender.
    pub request_tx: mpsc::UnboundedSender<PiScanRequestMessage>,
    /// Cancellation sender.
    pub cancel_tx: mpsc::UnboundedSender<PiScanCancelMessage>,
    /// Started-session registration sender.
    pub session_tx: mpsc::UnboundedSender<PiScanSessionRegistration>,
    /// Bounded-shutdown sender.
    pub shutdown_tx: mpsc::UnboundedSender<PiScanShutdownMessage>,
    /// Progress receiver for event-loop/WS4 projection.
    pub progress_rx: mpsc::UnboundedReceiver<PiScanProgressMessage>,
    /// Terminal result receiver for event-loop/WS4 projection.
    pub result_rx: mpsc::UnboundedReceiver<PiScanResultMessage>,
    /// Typed provenance-bearing runtime notices for later UI projection.
    pub notice_rx: mpsc::UnboundedReceiver<PiScanRuntimeNotice>,
}

/// What: Spawn one sequential, default-off Pi scan runtime worker.
///
/// Inputs:
/// - `options`: Feature, platform, dry-run, and persistence configuration.
///
/// Output:
/// - Typed runtime channel endpoints.
///
/// Details:
/// - Enabled non-dry runs synchronously load and recover durable state before the task starts.
/// - Dry-run starts from empty in-memory state and never reads or writes durable scanner state.
/// - This function never launches Pi or performs network acquisition.
///
/// # Errors
/// - Returns an actionable load/recovery error and does not spawn when durable state is unavailable.
pub fn spawn_pi_scan_worker(
    options: PiScanRuntimeOptions,
) -> Result<PiScanRuntimeChannels, PiScanPersistenceError> {
    let now = unix_now();
    let persisted = if options.effective_enabled() && !options.dry_run {
        let recovered = load_pi_scan_state(&options.state_path, &options.quarantine_dir, now)?;
        if recovered.state.recovery_marker {
            save_pi_scan_state_atomic(&options.state_path, &recovered)?;
        }
        recovered
    } else {
        PiScanPersistedState::default()
    };
    Ok(spawn_with_state(options, persisted.state))
}

/// What: Spawn the inert default-off runtime used before WS4 supplies explicit configuration.
///
/// Inputs: None.
///
/// Output:
/// - Typed channels backed by an empty in-memory worker.
///
/// Details:
/// - This path cannot load or mutate durable state and rejects execution requests with
///   actionable enablement guidance.
#[must_use]
pub fn spawn_default_off_pi_scan_worker() -> PiScanRuntimeChannels {
    spawn_with_state(
        PiScanRuntimeOptions::default(),
        PiScanRuntimeState::default(),
    )
}

/// Build channels and start the single-owner worker from an already loaded state.
fn spawn_with_state(
    options: PiScanRuntimeOptions,
    state: PiScanRuntimeState,
) -> PiScanRuntimeChannels {
    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let (cancel_tx, cancel_rx) = mpsc::unbounded_channel();
    let (session_tx, session_rx) = mpsc::unbounded_channel();
    let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();
    let (progress_tx, progress_rx) = mpsc::unbounded_channel();
    let (result_tx, result_rx) = mpsc::unbounded_channel();
    let (_notice_tx, notice_rx) = mpsc::unbounded_channel();
    let worker = PiScanWorker {
        options,
        state,
        request_rx,
        cancel_rx,
        session_rx,
        shutdown_rx,
        progress_tx,
        result_tx,
        abort_target: None,
    };
    tokio::spawn(worker.run());
    PiScanRuntimeChannels {
        request_tx,
        cancel_tx,
        session_tx,
        shutdown_tx,
        progress_rx,
        result_rx,
        notice_rx,
    }
}

/// Single-owner worker state and channel receivers.
struct PiScanWorker {
    /// Runtime options.
    options: PiScanRuntimeOptions,
    /// Cohesive queue and policy state.
    state: PiScanRuntimeState,
    /// General request receiver.
    request_rx: mpsc::UnboundedReceiver<PiScanRequestMessage>,
    /// Cancellation receiver.
    cancel_rx: mpsc::UnboundedReceiver<PiScanCancelMessage>,
    /// Process-target registration receiver.
    session_rx: mpsc::UnboundedReceiver<PiScanSessionRegistration>,
    /// Shutdown receiver.
    shutdown_rx: mpsc::UnboundedReceiver<PiScanShutdownMessage>,
    /// Progress sender.
    progress_tx: mpsc::UnboundedSender<PiScanProgressMessage>,
    /// Result sender.
    result_tx: mpsc::UnboundedSender<PiScanResultMessage>,
    /// Cancellation target for the active correlation, when Pi has started.
    abort_target: Option<Box<dyn PiScanAbortTarget>>,
}

impl PiScanWorker {
    /// Run the single sequential command loop until shutdown or all senders close.
    async fn run(mut self) {
        let mut interval = tokio::time::interval(BUDGET_REVALIDATION_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                Some(request) = self.request_rx.recv() => self.handle_request(request),
                Some(cancel) = self.cancel_rx.recv() => self.handle_cancel(cancel),
                Some(registration) = self.session_rx.recv() => self.register_session(registration),
                Some(shutdown) = self.shutdown_rx.recv() => {
                    self.handle_shutdown(&shutdown);
                    break;
                }
                _ = interval.tick() => self.revalidate_and_dispatch(unix_now()),
                else => break,
            }
        }
    }

    /// Apply one typed request and attempt a non-preemptive dispatch.
    fn handle_request(&mut self, request: PiScanRequestMessage) {
        if self.options.dry_run {
            self.handle_dry_run(request);
            return;
        }
        if !self.options.effective_enabled() {
            self.reject("Pi scanning is disabled or unsupported on this platform; enable it explicitly on Arch Linux before queueing work");
            return;
        }
        let now = request_timestamp(&request).unwrap_or_else(unix_now);
        if let PiScanRequestMessage::AdjustBudgets {
            adjustment,
            now_unix,
            acknowledge,
        } = request
        {
            self.handle_budget_adjustment(adjustment, now_unix, &acknowledge);
            return;
        }
        let mutation = match request {
            PiScanRequestMessage::ProbeSetup | PiScanRequestMessage::ManualObservation { .. } => {
                Err(
                    "setup probing and manual observation require the production Pi orchestrator"
                        .to_string(),
                )
            }
            PiScanRequestMessage::Enqueue(job) => self.enqueue(job),
            PiScanRequestMessage::SetConsent(consent)
            | PiScanRequestMessage::SetConsentDetails { consent, .. } => {
                self.state.set_consent(consent);
                Ok(())
            }
            PiScanRequestMessage::SetUserPaused(paused) => {
                self.state.set_user_paused(paused);
                Ok(())
            }
            PiScanRequestMessage::PauseForService => {
                self.state.pause_for_service();
                Ok(())
            }
            PiScanRequestMessage::ClearServicePause {
                validation_succeeded,
            } => {
                self.state.clear_service_pause(validation_succeeded);
                Ok(())
            }
            PiScanRequestMessage::Complete {
                correlation_id,
                key,
                usage,
                finished_at_unix,
            } => self.complete(correlation_id, &key, usage, finished_at_unix),
            PiScanRequestMessage::UpdateCandidates(_) => {
                Err("typed update candidates require the production Pi orchestrator".to_string())
            }
            PiScanRequestMessage::AcceptBaseline { .. } => {
                Err("baseline acceptance requires the production Pi orchestrator".to_string())
            }
            PiScanRequestMessage::ValidateContinuation { .. } => Err(
                "linked continuation validation requires the production Pi orchestrator"
                    .to_string(),
            ),
            PiScanRequestMessage::AdjustBudgets { .. } => unreachable!("handled above"),
            PiScanRequestMessage::RevalidateBudgets { now_unix } => {
                self.state.revalidate_budget_pause(now_unix);
                Ok(())
            }
        };
        if let Err(error) = mutation {
            self.reject(&error);
            return;
        }
        if let Err(error) = self.persist() {
            self.state.pause_for_service();
            self.reject(&error.to_string());
            return;
        }
        self.dispatch(now);
    }

    /// Emit dry-run preview or no-op acknowledgement without mutating state.
    fn handle_dry_run(&self, request: PiScanRequestMessage) {
        match request {
            PiScanRequestMessage::Enqueue(job) => {
                let _ = self
                    .progress_tx
                    .send(PiScanProgressMessage::DryRunPreview(job));
            }
            PiScanRequestMessage::AdjustBudgets {
                adjustment,
                now_unix,
                acknowledge,
            } => {
                let mut preview = self.state.clone();
                let acknowledgement = match preview.adjust_exceeded_budgets(adjustment, now_unix)
                {
                    Ok(result) if result.affected.is_empty() => {
                        PiScanBudgetAdjustmentAcknowledgement::NoLongerBlocked {
                            adjustment,
                            result,
                            dry_run: true,
                        }
                    }
                    Ok(result) => PiScanBudgetAdjustmentAcknowledgement::Applied {
                        adjustment,
                        result,
                        durable: false,
                        dry_run: true,
                    },
                    Err(error) => PiScanBudgetAdjustmentAcknowledgement::Rejected {
                        adjustment,
                        reason: error.to_string(),
                    },
                };
                let _ = acknowledge.send(acknowledgement);
            }
            _ => self.reject(
                "dry-run preview did not mutate Pi scan consent, pause, budget, queue, result, or durable state",
            ),
        }
    }

    /// Apply one inert-owner adjustment with rollback on persistence failure.
    fn handle_budget_adjustment(
        &mut self,
        adjustment: PiScanBudgetAdjustment,
        now_unix: u64,
        acknowledge: &mpsc::UnboundedSender<PiScanBudgetAdjustmentAcknowledgement>,
    ) {
        let previous = self.state.clone();
        let result = match self.state.adjust_exceeded_budgets(adjustment, now_unix) {
            Ok(result) => result,
            Err(error) => {
                let reason = error.to_string();
                let _ = acknowledge.send(PiScanBudgetAdjustmentAcknowledgement::Rejected {
                    adjustment,
                    reason: reason.clone(),
                });
                self.reject(&reason);
                return;
            }
        };
        if result.affected.is_empty() {
            let _ = acknowledge.send(PiScanBudgetAdjustmentAcknowledgement::NoLongerBlocked {
                adjustment,
                result,
                dry_run: false,
            });
            return;
        }
        if let Err(error) = self.persist() {
            self.state = previous;
            let reason = error.to_string();
            let _ = acknowledge.send(PiScanBudgetAdjustmentAcknowledgement::Rejected {
                adjustment,
                reason: reason.clone(),
            });
            self.reject(&reason);
            return;
        }
        let _ = acknowledge.send(PiScanBudgetAdjustmentAcknowledgement::Applied {
            adjustment,
            result,
            durable: true,
            dry_run: false,
        });
        self.dispatch(now_unix);
    }

    /// Append a queue item and publish its exact queue depth.
    fn enqueue(&mut self, job: PiScanJobRequest) -> Result<(), String> {
        let request = job.clone();
        let queue_len = self.state.enqueue(job).map_err(|error| error.to_string())?;
        let _ = self
            .progress_tx
            .send(PiScanProgressMessage::Queued { request, queue_len });
        Ok(())
    }

    /// Accept a strictly correlated completion and publish a terminal result.
    fn complete(
        &mut self,
        correlation_id: u64,
        key: &PiScanQueueKey,
        usage: PiScanActualUsage,
        finished_at_unix: u64,
    ) -> Result<(), String> {
        let record = self
            .state
            .complete(correlation_id, key, usage, finished_at_unix)
            .map_err(|error| error.to_string())?;
        self.abort_target = None;
        let _ = self.result_tx.send(PiScanResultMessage::Completed(record));
        Ok(())
    }

    /// Register only a process target matching the current active correlation.
    fn register_session(&mut self, mut registration: PiScanSessionRegistration) {
        let matches_active = self.state.active.as_ref().is_some_and(|active| {
            active.correlation_id == registration.correlation_id
                && registration.target.correlation_id() == registration.correlation_id
        });
        if self.options.dry_run || !matches_active {
            let warning = registration.target.abort_and_reap().err();
            let reason = warning.map_or_else(
                || {
                    "rejected stale Pi process registration and reaped its process group"
                        .to_string()
                },
                |error| format!("rejected stale Pi process registration; reap warning: {error}"),
            );
            self.reject(&reason);
            return;
        }
        self.abort_target = Some(registration.target);
        let _ = self
            .progress_tx
            .send(PiScanProgressMessage::SessionRegistered {
                correlation_id: registration.correlation_id,
            });
    }

    /// Route cancellation through a matching target, then suppress and terminalize state.
    fn handle_cancel(&mut self, cancel: PiScanCancelMessage) {
        if self.options.dry_run || !self.options.effective_enabled() {
            self.reject(
                "Pi cancellation is inert because scanning is disabled or dry-run is active",
            );
            return;
        }
        let _ = self.progress_tx.send(PiScanProgressMessage::Cancelling {
            correlation_id: cancel.correlation_id,
        });
        let warning = self.abort_matching_target(cancel.correlation_id);
        match self
            .state
            .cancel_active(cancel.correlation_id, cancel.requested_at_unix)
        {
            Ok(record) => {
                if let Err(error) = self.persist() {
                    self.state.pause_for_service();
                    self.reject(&error.to_string());
                }
                let _ = self
                    .result_tx
                    .send(PiScanResultMessage::Cancelled { record, warning });
                self.dispatch(cancel.requested_at_unix);
            }
            Err(error) => self.reject(&error.to_string()),
        }
    }

    /// Abort and remove only the target bound to the requested correlation.
    fn abort_matching_target(&mut self, correlation_id: u64) -> Option<String> {
        let mut target = self.abort_target.take()?;
        if target.correlation_id() != correlation_id {
            self.abort_target = Some(target);
            return Some(
                "no Pi process was registered for the requested active correlation".to_string(),
            );
        }
        target.abort_and_reap().err()
    }

    /// Revalidate rolling budget pause and try to dispatch one next item.
    fn revalidate_and_dispatch(&mut self, now_unix: u64) {
        if self.options.dry_run || !self.options.effective_enabled() {
            return;
        }
        let was_budget_paused = self
            .state
            .pause_reasons
            .contains(&crate::state::pi_scan::PiScanPauseReason::Budget);
        let is_budget_paused = self.state.revalidate_budget_pause(now_unix);
        if was_budget_paused != is_budget_paused {
            let _ = self.persist();
        }
        self.dispatch(now_unix);
    }

    /// Dispatch at most one item; active work is never preempted.
    fn dispatch(&mut self, now_unix: u64) {
        match self
            .state
            .start_next(now_unix, self.options.effective_enabled())
        {
            Ok(Some(active)) => {
                if let Err(error) = self.persist() {
                    self.state.pause_for_service();
                    self.reject(&error.to_string());
                    return;
                }
                let _ = self
                    .progress_tx
                    .send(PiScanProgressMessage::Started(active));
            }
            Ok(None) => {}
            Err(block) => {
                let _ = self.progress_tx.send(PiScanProgressMessage::Paused(block));
            }
        }
    }

    /// Abort active work, recover it as interrupted, persist, and acknowledge shutdown.
    fn handle_shutdown(&mut self, shutdown: &PiScanShutdownMessage) {
        let active_correlation = self
            .state
            .active
            .as_ref()
            .map(|active| active.correlation_id);
        let abort_warning = active_correlation.and_then(|id| self.abort_matching_target(id));
        let recovery = if self.options.dry_run {
            Ok(None)
        } else {
            self.state.recover_interrupted(unix_now())
        };
        let active_interrupted = recovery.as_ref().is_ok_and(Option::is_some);
        let recovery_warning = recovery.err().map(|error| error.to_string());
        let persist_result = self.persist();
        let persisted =
            persist_result.is_ok() || self.options.dry_run || !self.options.effective_enabled();
        let persistence_warning = persist_result.err().map(|error| error.to_string());
        let warning = combine_warnings(
            abort_warning,
            combine_warnings(recovery_warning, persistence_warning),
        );
        let ack = PiScanShutdownAck {
            persisted,
            active_interrupted,
            warning,
        };
        let _ = self
            .progress_tx
            .send(PiScanProgressMessage::Shutdown(ack.clone()));
        let _ = shutdown.acknowledge.send(ack);
    }

    /// Persist current state unless disabled or dry-run.
    fn persist(&self) -> Result<(), PiScanPersistenceError> {
        if self.options.dry_run || !self.options.effective_enabled() {
            return Ok(());
        }
        save_pi_scan_state_atomic(
            &self.options.state_path,
            &PiScanPersistedState {
                schema_version: crate::state::pi_scan::PI_SCAN_RUNTIME_SCHEMA_VERSION,
                state: self.state.clone(),
            },
        )
    }

    /// Publish one actionable rejection.
    fn reject(&self, reason: &str) {
        let _ = self.result_tx.send(PiScanResultMessage::Rejected {
            reason: reason.to_string(),
        });
    }
}

/// Return a deterministic request timestamp when the message carries one.
const fn request_timestamp(request: &PiScanRequestMessage) -> Option<u64> {
    match request {
        PiScanRequestMessage::Complete {
            finished_at_unix, ..
        } => Some(*finished_at_unix),
        PiScanRequestMessage::AdjustBudgets { now_unix, .. }
        | PiScanRequestMessage::RevalidateBudgets { now_unix } => Some(*now_unix),
        PiScanRequestMessage::ProbeSetup
        | PiScanRequestMessage::ManualObservation { .. }
        | PiScanRequestMessage::Enqueue(_)
        | PiScanRequestMessage::SetConsent(_)
        | PiScanRequestMessage::SetConsentDetails { .. }
        | PiScanRequestMessage::SetUserPaused(_)
        | PiScanRequestMessage::PauseForService
        | PiScanRequestMessage::ClearServicePause { .. }
        | PiScanRequestMessage::UpdateCandidates(_)
        | PiScanRequestMessage::AcceptBaseline { .. }
        | PiScanRequestMessage::ValidateContinuation { .. } => None,
    }
}

/// Combine independently actionable abort and persistence warnings.
fn combine_warnings(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (Some(warning), None) | (None, Some(warning)) => Some(warning),
        (None, None) => None,
    }
}

/// Return current Unix time, falling back to zero only when the system clock predates the epoch.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
#[path = "../../../../tests/pi_scan/ws3_runtime.rs"]
mod ws3_runtime;
