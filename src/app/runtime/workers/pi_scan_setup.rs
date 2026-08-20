//! Correlated setup controller and transactional runtime-transfer seam for Pi Scan.
//!
//! Probe and validation are write-free. Apply re-probes exact metadata, prepares a queue-inert
//! runtime, atomically commits only Pi Scan settings plus compatible consent, and transfers the
//! prepared owner to central integration. Production channels are not created until integration
//! has durably shut down the previous owner and explicitly activates the transfer.

use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tokio::sync::mpsc;

use crate::app::runtime::workers::pi_scan::{
    PiScanRuntimeChannels, PiScanRuntimeOptions, PiScanShutdownMessage,
};
use crate::pi_agent::session::ModelChoice;
use crate::pi_agent::setup_probe::{
    PiSetupAdvertisedRoute, PiSetupProbeRequest, PiSetupProbeSnapshot,
    SETUP_PROBE_MAXIMUM_PRICING_AGE, SETUP_PROBE_RESERVATION_TOKENS,
};
use crate::pi_scan_orchestrator::{PiScanSetupConsentState, SetupSnapshot};
use crate::state::pi_scan::{PiScanBudgetLimits, PiScanConsentState, PiScanReservation};
use crate::state::pi_scan_setup::PiScanSetupConfirmations;
use crate::theme::PiScanSettings;

/// Bounded production shutdown wait used by rollback.
const TRANSFER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum controller wait for one no-model setup probe.
const SETUP_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum controller wait for candidate validation blocking work.
const SETUP_VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum controller wait for apply preparation blocking work.
const SETUP_APPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// What: Configuration for one setup-only controller instance.
///
/// Inputs:
/// - Session dry-run flag plus the exact durable paths a committed apply owns.
///
/// Output:
/// - Startup policy for [`spawn_pi_scan_setup_controller`].
///
/// Details:
/// - Dry-run controllers never probe Pi, write configuration/consent, or activate a runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanSetupControllerOptions {
    /// Session dry-run mode.
    pub dry_run: bool,
    /// Absolute `settings.conf` path patched only on committed apply.
    pub settings_path: PathBuf,
    /// Durable consent document path persisted only on committed apply.
    pub consent_path: PathBuf,
    /// Versioned runtime state path used by candidate activation.
    pub state_path: PathBuf,
    /// Private quarantine directory used by candidate activation.
    pub quarantine_dir: PathBuf,
}

/// Transaction stage reported by typed setup failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanSetupStage {
    /// Pi binary/version/tool/model/pricing discovery.
    Probe,
    /// Candidate normalization and reviewed-binding validation.
    CandidateValidation,
    /// Candidate production runtime construction and health check.
    Activation,
    /// Atomic settings/consent persistence.
    Persistence,
}

/// What: Typed requests accepted by the setup-only controller.
///
/// Inputs:
/// - Correlated wizard state: binary choice, candidate settings, confirmations.
///
/// Output:
/// - Exactly one correlated [`PiScanSetupEvent`] per accepted request.
///
/// Details:
/// - Requests contain no credentials, prompts, source bodies, or Pi wire records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanSetupRequest {
    /// Verify the Pi binary and enumerate exact routes without a model call.
    BeginSetupProbe {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Pi executable name or absolute path to verify.
        binary: String,
    },
    /// Validate the full candidate configuration without durable changes.
    ValidateSetupCandidate {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Complete candidate settings under review.
        candidate: PiScanSettings,
        /// Candidate independent observation/paid-background choices.
        consent: PiScanConsentState,
        /// Independent explicit confirmations.
        confirmations: PiScanSetupConfirmations,
    },
    /// Revalidate, commit, and prepare runtime ownership transfer.
    ApplySetupCandidate {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Complete reviewed candidate settings.
        candidate: PiScanSettings,
        /// Candidate independent observation/paid-background choices.
        consent: PiScanConsentState,
        /// Independent explicit confirmations.
        confirmations: PiScanSetupConfirmations,
        /// Exact validation binding echoed from `CandidateValidated`.
        validation_binding: String,
    },
}

impl PiScanSetupRequest {
    /// Return the exact request correlation.
    const fn correlation_id(&self) -> u64 {
        match self {
            Self::BeginSetupProbe { correlation_id, .. }
            | Self::ValidateSetupCandidate { correlation_id, .. }
            | Self::ApplySetupCandidate { correlation_id, .. } => *correlation_id,
        }
    }

    /// Return the stage used when request correlation is stale.
    const fn stage(&self) -> PiScanSetupStage {
        match self {
            Self::BeginSetupProbe { .. } => PiScanSetupStage::Probe,
            Self::ValidateSetupCandidate { .. } | Self::ApplySetupCandidate { .. } => {
                PiScanSetupStage::CandidateValidation
            }
        }
    }
}

/// What: Correlated events published by the setup-only controller.
///
/// Inputs:
/// - Produced in response to exactly one [`PiScanSetupRequest`].
///
/// Output:
/// - Wizard-facing verified facts, validation outcome, prepared apply, or typed failure.
///
/// Details:
/// - `Applied` means settings and consent are durable and the matching transfer is available;
///   production Channels are still queue-inert and uncreated until integration activates them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanSetupEvent {
    /// Exact no-model capability facts verified for the requested binary.
    CapabilitiesVerified {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Exact verified facts for wizard display and route selection.
        snapshot: Box<SetupSnapshot>,
    },
    /// Candidate validation succeeded without durable changes.
    CandidateValidated {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Binding over normalized candidate, facts, file fingerprint, consent, and versions.
        validation_binding: String,
    },
    /// Durable files committed and a correlated prepared-runtime transfer is available.
    Applied {
        /// Wizard request correlation and transfer identity.
        correlation_id: u64,
        /// Exact effective settings now committed pending transfer acceptance.
        effective: Box<PiScanSettings>,
        /// Exact setup snapshot rebound immediately before commit.
        snapshot: Box<SetupSnapshot>,
    },
    /// One transaction stage failed without replacing production ownership.
    Failed {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Failing transaction stage.
        stage: PiScanSetupStage,
        /// Actionable retry guidance, including rollback failure when applicable.
        reason: String,
    },
}

/// What: Correlated controller deadline expiry retained separately from legacy failure projection.
///
/// Inputs:
/// - Exact request correlation, setup stage, and enforced operation deadline.
///
/// Output:
/// - Typed timeout protocol for actionable Retry and stale-response rejection.
///
/// Details:
/// - The controller also emits the existing `Failed` event so current projection remains safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanSetupTimeout {
    /// Request whose operation exceeded its deadline.
    pub correlation_id: u64,
    /// Setup stage that timed out.
    pub stage: PiScanSetupStage,
    /// Enforced controller/driver boundary.
    pub deadline: Duration,
}

/// Typed channel endpoints owned by central integration for guided setup.
pub struct PiScanSetupChannels {
    /// Correlated request sender.
    pub request_tx: mpsc::UnboundedSender<PiScanSetupRequest>,
    /// Correlated event receiver.
    pub event_rx: mpsc::UnboundedReceiver<PiScanSetupEvent>,
    /// Prepared runtime transfers, emitted only after durable commit.
    pub transfer_rx: mpsc::UnboundedReceiver<PiScanRuntimeTransfer>,
    /// Correlated deadline expiries for later actionable UI projection.
    pub timeout_rx: mpsc::UnboundedReceiver<PiScanSetupTimeout>,
}

/// Explicit rollback result retained for later workspace transaction projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanRollbackOutcome {
    /// Candidate teardown and durable restoration both completed.
    Succeeded,
    /// At least one teardown or restoration step failed visibly.
    Failed {
        /// Actionable combined failure text.
        reason: String,
    },
}

impl PiScanRollbackOutcome {
    /// Convert the typed projection primitive back to the existing result contract.
    fn into_result(self) -> Result<(), String> {
        match self {
            Self::Succeeded => Ok(()),
            Self::Failed { reason } => Err(reason),
        }
    }
}

/// Correlated explicit rollback report for an abandoned setup transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanRollbackReport {
    /// Apply correlation retaining transaction ownership after the wizard closes.
    pub correlation_id: u64,
    /// Explicit successful or failed rollback outcome.
    pub outcome: PiScanRollbackOutcome,
}

/// What: A durable, correlated candidate awaiting exclusive runtime ownership.
///
/// Inputs:
/// - Created only after settings and consent commit successfully.
///
/// Output:
/// - [`PiScanActivatedRuntime`] after central integration shuts down the previous owner.
///
/// Details:
/// - Dropping an unaccepted transfer tears down its inert candidate and restores both files.
/// - Call [`Self::activate`] only after the old runtime acknowledges durable shutdown.
pub struct PiScanRuntimeTransfer {
    /// Apply correlation used to pair event and transfer.
    correlation_id: u64,
    /// Queue-inert prepared runtime factory.
    candidate: Option<Box<dyn PreparedRuntime>>,
    /// Committed files kept rollback-capable until activation is accepted.
    commit: Option<DurableSetupCommit>,
    /// Effective normalized settings.
    effective: PiScanSettings,
    /// Fresh setup projection used by the new runtime.
    snapshot: SetupSnapshot,
}

impl fmt::Debug for PiScanRuntimeTransfer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PiScanRuntimeTransfer")
            .field("correlation_id", &self.correlation_id)
            .field("effective", &self.effective)
            .finish_non_exhaustive()
    }
}

impl PiScanRuntimeTransfer {
    /// Return the apply correlation paired with [`PiScanSetupEvent::Applied`].
    #[must_use]
    pub const fn correlation_id(&self) -> u64 {
        self.correlation_id
    }

    /// What: Activate the prepared runtime after the previous owner is durably stopped.
    ///
    /// Inputs:
    /// - Exclusive ownership of this transfer.
    ///
    /// Output:
    /// - Rollback-capable activated channels awaiting the integration owner's atomic swap.
    ///
    /// Details:
    /// - Activation failure restores exact prior settings and consent before returning.
    ///
    /// # Errors
    /// - Returns activation and any rollback failure explicitly.
    pub fn activate(mut self) -> Result<PiScanActivatedRuntime, PiScanRuntimeActivationError> {
        let candidate = self
            .candidate
            .take()
            .ok_or_else(|| PiScanRuntimeActivationError {
                reason: "Pi Scan candidate runtime was already consumed".to_string(),
                rollback_failure: None,
            })?;
        let commit = self
            .commit
            .take()
            .ok_or_else(|| PiScanRuntimeActivationError {
                reason: "Pi Scan durable setup commit is unavailable".to_string(),
                rollback_failure: None,
            })?;
        match candidate.activate() {
            Ok(channels) => Ok(PiScanActivatedRuntime {
                correlation_id: self.correlation_id,
                channels: Some(channels),
                commit: Some(commit),
                effective: self.effective.clone(),
                snapshot: self.snapshot.clone(),
            }),
            Err(reason) => {
                let rollback_failure = commit.rollback().err();
                Err(PiScanRuntimeActivationError {
                    reason,
                    rollback_failure,
                })
            }
        }
    }

    /// Tear down an unactivated candidate and restore the exact prior durable files.
    ///
    /// # Errors
    /// - Combines candidate teardown and durable rollback failures explicitly.
    pub fn rollback(self) -> Result<(), String> {
        self.rollback_with_outcome().outcome.into_result()
    }

    /// What: Explicitly roll back this transfer and retain its correlated outcome.
    ///
    /// Inputs:
    /// - Unactivated transfer still owning candidate teardown and durable file restoration.
    ///
    /// Output:
    /// - Correlated success or combined failure suitable for workspace notice projection.
    ///
    /// Details:
    /// - This is the reporting path for later abandonment handling; `Drop` remains only a
    ///   fail-safe cleanup guard and cannot claim an outcome.
    #[must_use]
    pub fn rollback_with_outcome(mut self) -> PiScanRollbackReport {
        let teardown = self
            .candidate
            .take()
            .map_or_else(|| Ok(()), PreparedRuntime::teardown);
        let rollback = self
            .commit
            .take()
            .map_or(Ok(()), DurableSetupCommit::rollback);
        let outcome = match combine_results(teardown, rollback) {
            Ok(()) => PiScanRollbackOutcome::Succeeded,
            Err(reason) => PiScanRollbackOutcome::Failed { reason },
        };
        PiScanRollbackReport {
            correlation_id: self.correlation_id,
            outcome,
        }
    }
}

impl Drop for PiScanRuntimeTransfer {
    fn drop(&mut self) {
        if let Some(candidate) = self.candidate.take() {
            drop(candidate.teardown());
        }
        if let Some(commit) = self.commit.take() {
            drop(commit.rollback());
        }
    }
}

/// Activation failure plus an explicit durable rollback failure, when any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanRuntimeActivationError {
    /// Actionable activation failure.
    pub reason: String,
    /// Exact rollback failure requiring manual recovery.
    pub rollback_failure: Option<String>,
}

impl fmt::Display for PiScanRuntimeActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(rollback) = &self.rollback_failure {
            write!(
                formatter,
                "Pi Scan activation failed: {}; rollback also failed: {rollback}",
                self.reason
            )
        } else {
            write!(formatter, "Pi Scan activation failed: {}", self.reason)
        }
    }
}

impl std::error::Error for PiScanRuntimeActivationError {}

/// What: Activated candidate channels still guarded by durable rollback.
///
/// Inputs:
/// - Returned by [`PiScanRuntimeTransfer::activate`].
///
/// Output:
/// - Commit into production ownership or explicit bounded rollback.
///
/// Details:
/// - The integration owner swaps Channels only by consuming [`Self::commit`].
pub struct PiScanActivatedRuntime {
    /// Apply correlation retaining transaction ownership through activation.
    correlation_id: u64,
    /// Candidate runtime channels.
    channels: Option<PiScanRuntimeChannels>,
    /// Durable rollback guard.
    commit: Option<DurableSetupCommit>,
    /// Effective normalized settings.
    effective: PiScanSettings,
    /// Fresh setup projection.
    snapshot: SetupSnapshot,
}

impl PiScanActivatedRuntime {
    /// Borrow effective settings for final UI projection before ownership swap.
    #[must_use]
    pub const fn effective(&self) -> &PiScanSettings {
        &self.effective
    }

    /// Borrow fresh verified setup facts for final UI projection.
    #[must_use]
    pub const fn snapshot(&self) -> &SetupSnapshot {
        &self.snapshot
    }

    /// Permanently accept durable files and return the new production Channels owner.
    ///
    /// # Errors
    /// - Returns only if an internally consumed activated handle is reused.
    pub fn commit(mut self) -> Result<PiScanRuntimeChannels, String> {
        let channels = self.channels.take().ok_or_else(|| {
            "activated Pi Scan runtime channels were already consumed".to_string()
        })?;
        self.commit = None;
        Ok(channels)
    }

    /// What: Tear down this candidate and restore exact prior durable files.
    ///
    /// Inputs:
    /// - Activated candidate not yet installed into the shared Channels owner.
    ///
    /// Output:
    /// - Successful bounded shutdown and rollback.
    ///
    /// Details:
    /// - Used when central integration cannot complete its non-fallible channel swap.
    ///
    /// # Errors
    /// - Combines runtime shutdown and file rollback failures explicitly.
    pub fn rollback(self) -> Result<(), String> {
        self.rollback_with_outcome().outcome.into_result()
    }

    /// What: Shut down an activated candidate and explicitly report correlated rollback.
    ///
    /// Inputs:
    /// - Activated candidate not yet committed into shared runtime ownership.
    ///
    /// Output:
    /// - Correlated success or combined shutdown/restoration failure.
    ///
    /// Details:
    /// - The report lets later workspace transaction state surface abandonment after the wizard
    ///   closes without relying on implicit `Drop` cleanup.
    #[must_use]
    pub fn rollback_with_outcome(mut self) -> PiScanRollbackReport {
        let shutdown = self.channels.take().map_or(Ok(()), shutdown_candidate);
        let rollback = self
            .commit
            .take()
            .map_or(Ok(()), DurableSetupCommit::rollback);
        let outcome = match combine_results(shutdown, rollback) {
            Ok(()) => PiScanRollbackOutcome::Succeeded,
            Err(reason) => PiScanRollbackOutcome::Failed { reason },
        };
        PiScanRollbackReport {
            correlation_id: self.correlation_id,
            outcome,
        }
    }
}

impl Drop for PiScanActivatedRuntime {
    fn drop(&mut self) {
        if let Some(channels) = self.channels.take() {
            drop(request_candidate_shutdown(&channels));
        }
        if let Some(commit) = self.commit.take() {
            drop(commit.rollback());
        }
    }
}

/// Failure returned by one controller-bounded driver operation.
enum DriverCallError {
    /// Driver completed with an actionable failure.
    Failed(String),
    /// Controller deadline expired after owned resources were cleaned up.
    TimedOut(Duration),
}

/// Driver boundary for production metadata probing and inert runtime preparation.
trait SetupDriver: Send {
    /// Create an independent operation driver so a timed-out read-only call cannot retain the controller.
    fn fork(&self) -> Box<dyn SetupDriver>;
    /// Return the controller-enforced deadline for one stage.
    fn operation_timeout(&self, stage: PiScanSetupStage) -> Duration {
        match stage {
            PiScanSetupStage::Probe => SETUP_PROBE_TIMEOUT,
            PiScanSetupStage::CandidateValidation => SETUP_VALIDATION_TIMEOUT,
            PiScanSetupStage::Activation | PiScanSetupStage::Persistence => SETUP_APPLY_TIMEOUT,
        }
    }
    /// Deterministic current Unix time.
    fn now_unix_seconds(&self) -> u64;
    /// Run one exact no-model setup probe.
    fn probe(
        &mut self,
        request: &PiSetupProbeRequest,
    ) -> Result<PiSetupProbeSnapshot, DriverCallError>;
    /// Prepare and health-check a runtime without creating Channels or accepting queue work.
    fn prepare_runtime(
        &mut self,
        options: &PiScanSetupControllerOptions,
        settings: &PiScanSettings,
        snapshot: &PiSetupProbeSnapshot,
        models: Vec<ModelChoice>,
        reservation: PiScanReservation,
    ) -> Result<Box<dyn PreparedRuntime>, DriverCallError>;

    /// Deterministic test seam immediately before durable commit.
    fn before_commit(&mut self, _options: &PiScanSetupControllerOptions) -> Result<(), String> {
        Ok(())
    }

    /// Deterministic test seam immediately before consent persistence.
    fn before_consent_commit(
        &mut self,
        _options: &PiScanSetupControllerOptions,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Deterministic test seam after consent persistence but before settings replacement.
    fn before_settings_commit(
        &mut self,
        _options: &PiScanSetupControllerOptions,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Queue-inert runtime candidate consumed by transfer activation or teardown.
trait PreparedRuntime: Send {
    /// Create production channels after exclusive ownership is available.
    fn activate(self: Box<Self>) -> Result<PiScanRuntimeChannels, String>;
    /// Tear down pre-activation resources after persistence or transfer failure.
    fn teardown(self: Box<Self>) -> Result<(), String>;
}

/// Production driver using WS2A's isolated no-model probe.
#[derive(Debug, Default)]
struct ProductionSetupDriver;

impl SetupDriver for ProductionSetupDriver {
    fn fork(&self) -> Box<dyn SetupDriver> {
        Box::new(Self)
    }

    fn now_unix_seconds(&self) -> u64 {
        unix_now()
    }

    fn probe(
        &mut self,
        request: &PiSetupProbeRequest,
    ) -> Result<PiSetupProbeSnapshot, DriverCallError> {
        const PRODUCTION_PROBE_TOTAL_TIMEOUT: Duration = Duration::from_secs(29);
        crate::pi_agent::setup_probe::probe_pi_setup_with_deadline(
            request,
            PRODUCTION_PROBE_TOTAL_TIMEOUT,
        )
        .map_err(|error| match error {
            crate::pi_agent::setup_probe::PiSetupProbeError::DeadlineExceeded { .. } => {
                DriverCallError::TimedOut(SETUP_PROBE_TIMEOUT)
            }
            other => DriverCallError::Failed(other.to_string()),
        })
    }

    fn prepare_runtime(
        &mut self,
        options: &PiScanSetupControllerOptions,
        settings: &PiScanSettings,
        _snapshot: &PiSetupProbeSnapshot,
        models: Vec<ModelChoice>,
        reservation: PiScanReservation,
    ) -> Result<Box<dyn PreparedRuntime>, DriverCallError> {
        validate_runtime_paths(options).map_err(DriverCallError::Failed)?;
        let production = production_runtime_settings(settings, models, reservation)
            .map_err(DriverCallError::Failed)?;
        let root = options.state_path.parent().ok_or_else(|| {
            DriverCallError::Failed(
                "Pi Scan state path has no private parent; fix configuration and retry".to_string(),
            )
        })?;
        drop(
            crate::pi_scan_production::resolve_production_adapter_config(
                &production.binary,
                root.join("workspaces"),
                production.models.clone(),
                &production.thinking,
                production.model_attempt_timeout,
                production.logical_timeout,
                production.head_query_timeout,
                production.observation_deadline,
                production.reservation,
                &production.https_proxy,
                false,
            )
            .map_err(DriverCallError::Failed)?,
        );
        Ok(Box::new(ProductionPreparedRuntime {
            options: PiScanRuntimeOptions {
                enabled: true,
                dry_run: false,
                state_path: options.state_path.clone(),
                quarantine_dir: options.quarantine_dir.clone(),
                production: Some(production),
            },
        }))
    }
}

/// Production candidate retaining only validated construction data.
#[derive(Debug)]
struct ProductionPreparedRuntime {
    /// Exact runtime options activated only after old-owner shutdown.
    options: PiScanRuntimeOptions,
}

impl PreparedRuntime for ProductionPreparedRuntime {
    fn activate(self: Box<Self>) -> Result<PiScanRuntimeChannels, String> {
        crate::pi_scan_production::spawn_production_pi_scan_worker(&self.options)
    }

    fn teardown(self: Box<Self>) -> Result<(), String> {
        Ok(())
    }
}

/// Exact prior file state retained in memory for rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SetupFileSnapshot {
    /// Whether the path existed before the transaction.
    existed: bool,
    /// Exact prior bytes.
    bytes: Vec<u8>,
    /// Domain-separated SHA-256 fingerprint.
    fingerprint: String,
}

/// Setup transaction file error.
#[derive(Debug)]
enum SetupFileError {
    /// Unsafe or semantically invalid path/content.
    Invalid(String),
    /// Exact filesystem operation failure.
    Io {
        /// Path involved in the failure.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
}

impl fmt::Display for SetupFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for SetupFileError {}

/// Snapshot one exact file without creating it.
fn snapshot_config_file(path: &Path) -> Result<SetupFileSnapshot, SetupFileError> {
    validate_setup_write_path(path)?;
    match fs::read(path) {
        Ok(bytes) => Ok(SetupFileSnapshot {
            existed: true,
            fingerprint: file_fingerprint(b"pacsea:setup-file:v1:present\0", &bytes),
            bytes,
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(SetupFileSnapshot {
            existed: false,
            bytes: Vec::new(),
            fingerprint: file_fingerprint(b"pacsea:setup-file:v1:missing", &[]),
        }),
        Err(source) => Err(SetupFileError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// What: Snapshot one validation input under the controller's blocking-operation deadline.
///
/// Inputs:
/// - `path`: Exact settings file whose fingerprint becomes part of validation.
/// - `deadline`: Maximum controller wait before Retry is released.
///
/// Output:
/// - Exact snapshot, actionable file failure, or typed timeout.
///
/// Details:
/// - Snapshotting is read-only. Any late response is dropped and cannot update validation state.
fn snapshot_config_file_with_deadline(
    path: PathBuf,
    deadline: Duration,
) -> Result<SetupFileSnapshot, DriverCallError> {
    let (sender, receiver) = std_mpsc::sync_channel(1);
    std::thread::spawn(move || {
        drop(sender.send(snapshot_config_file(&path)));
    });
    match receiver.recv_timeout(deadline) {
        Ok(Ok(snapshot)) => Ok(snapshot),
        Ok(Err(error)) => Err(DriverCallError::Failed(format!(
            "could not fingerprint settings.conf: {error}"
        ))),
        Err(std_mpsc::RecvTimeoutError::Timeout) => Err(DriverCallError::TimedOut(deadline)),
        Err(std_mpsc::RecvTimeoutError::Disconnected) => Err(DriverCallError::Failed(
            "settings fingerprint worker stopped unexpectedly; retry validation".to_string(),
        )),
    }
}

/// Atomically patch every and only Pi Scan settings key after fingerprint comparison.
fn patch_pi_scan_settings_atomic(
    path: &Path,
    expected_fingerprint: &str,
    settings: &PiScanSettings,
) -> Result<SetupFileSnapshot, SetupFileError> {
    let original = snapshot_config_file(path)?;
    if original.fingerprint != expected_fingerprint {
        return Err(SetupFileError::Invalid(
            "settings.conf changed after setup validation".to_string(),
        ));
    }
    let existing = std::str::from_utf8(&original.bytes).map_err(|_| {
        SetupFileError::Invalid(
            "settings.conf is not UTF-8; repair it before applying Pi Scan setup".to_string(),
        )
    })?;
    let content = render_pi_scan_settings(existing, settings)?;
    atomic_write_setup_file(path, content.as_bytes())?;
    Ok(original)
}

/// Atomically replace one private setup document and retain its prior snapshot.
fn replace_private_file_atomic(
    path: &Path,
    expected_fingerprint: &str,
    content: &str,
) -> Result<SetupFileSnapshot, SetupFileError> {
    let original = snapshot_config_file(path)?;
    if original.fingerprint != expected_fingerprint {
        return Err(SetupFileError::Invalid(
            "Pi Scan consent changed during setup Apply; review and retry".to_string(),
        ));
    }
    atomic_write_setup_file(path, content.as_bytes())?;
    Ok(original)
}

/// Restore an exact prior snapshot, including prior absence.
fn restore_config_file(path: &Path, snapshot: &SetupFileSnapshot) -> Result<(), SetupFileError> {
    validate_setup_write_path(path)?;
    if snapshot.existed {
        atomic_write_setup_file(path, &snapshot.bytes)
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(SetupFileError::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }
}

/// Render all canonical Pi Scan key/value pairs while preserving unrelated lines.
fn render_pi_scan_settings(
    existing: &str,
    settings: &PiScanSettings,
) -> Result<String, SetupFileError> {
    let values = pi_scan_setting_values(settings);
    if values.iter().any(|(_, value)| value.contains(['\n', '\r'])) {
        return Err(SetupFileError::Invalid(
            "Pi Scan setting values cannot contain line breaks".to_string(),
        ));
    }
    let mut lines: Vec<String> = existing.lines().map(ToString::to_string).collect();
    for (key, value) in values {
        let mut replaced = false;
        for line in &mut lines {
            let Some((found, _)) = line.trim().split_once('=') else {
                continue;
            };
            if normalize_config_key(found) == key {
                *line = format!("{key} = {value}");
                replaced = true;
            }
        }
        if !replaced {
            lines.push(format!("{key} = {value}"));
        }
    }
    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    Ok(content)
}

/// Return every canonical Pi Scan settings key exactly once.
fn pi_scan_setting_values(settings: &PiScanSettings) -> [(&'static str, String); 18] {
    [
        ("pi_scan_enabled", settings.enabled.to_string()),
        (
            "pi_scan_background_enabled",
            settings.background_enabled.to_string(),
        ),
        ("pi_scan_binary", settings.binary.clone()),
        ("pi_scan_provider", settings.provider.clone()),
        ("pi_scan_model", settings.model.clone()),
        ("pi_scan_fallback_models", settings.fallback_models.clone()),
        ("pi_scan_thinking", settings.thinking.clone()),
        (
            "pi_scan_observation_interval_seconds",
            settings.observation_interval_seconds.to_string(),
        ),
        (
            "pi_scan_head_query_timeout_seconds",
            settings.head_query_timeout_seconds.to_string(),
        ),
        (
            "pi_scan_observation_deadline_seconds",
            settings.observation_deadline_seconds.to_string(),
        ),
        (
            "pi_scan_model_attempt_timeout_seconds",
            settings.model_attempt_timeout_seconds.to_string(),
        ),
        (
            "pi_scan_logical_timeout_seconds",
            settings.logical_timeout_seconds.to_string(),
        ),
        (
            "pi_scan_background_starts_per_hour",
            settings.background_starts_per_hour.to_string(),
        ),
        (
            "pi_scan_background_token_cap_24h",
            settings.background_token_cap_24h.to_string(),
        ),
        (
            "pi_scan_background_cost_cap_24h",
            settings.background_cost_cap_24h.clone(),
        ),
        (
            "pi_scan_result_retention_days",
            settings.result_retention_days.to_string(),
        ),
        (
            "pi_scan_show_raw_output",
            settings.show_raw_output.to_string(),
        ),
        ("pi_scan_https_proxy", settings.https_proxy.clone()),
    ]
}

/// Normalize a settings key under the existing config parser convention.
fn normalize_config_key(key: &str) -> String {
    key.trim()
        .to_ascii_lowercase()
        .replace(['.', '-', ' '], "_")
}

/// Atomically write one private same-directory setup file.
fn atomic_write_setup_file(path: &Path, bytes: &[u8]) -> Result<(), SetupFileError> {
    validate_setup_write_path(path)?;
    let parent = path.parent().ok_or_else(|| {
        SetupFileError::Invalid(format!("setup path {} has no parent", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| SetupFileError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let temporary = setup_temporary_path(path);
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|source| SetupFileError::Io {
            path: temporary.clone(),
            source,
        })?;
    if let Err(source) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(fs::remove_file(&temporary));
        return Err(SetupFileError::Io {
            path: temporary,
            source,
        });
    }
    fs::rename(&temporary, path).map_err(|source| {
        drop(fs::remove_file(&temporary));
        SetupFileError::Io {
            path: path.to_path_buf(),
            source,
        }
    })?;
    sync_setup_parent(parent)
}

/// Persist one completed same-directory rename in the parent directory metadata.
fn sync_setup_parent(parent: &Path) -> Result<(), SetupFileError> {
    #[cfg(unix)]
    {
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|source| SetupFileError::Io {
                path: parent.to_path_buf(),
                source,
            })
    }
    #[cfg(not(unix))]
    {
        let _ = parent;
        Ok(())
    }
}

/// Create a collision-resistant sibling temporary path.
fn setup_temporary_path(path: &Path) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let name = path
        .file_name()
        .map_or_else(|| "setup".into(), |name| name.to_string_lossy());
    path.with_file_name(format!(".{name}.tmp.{}.{suffix}", std::process::id()))
}

/// Reject relative or lexical-parent write paths.
fn validate_setup_write_path(path: &Path) -> Result<(), SetupFileError> {
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err(SetupFileError::Invalid(format!(
            "setup write path {} must be absolute and traversal-free",
            path.display()
        )));
    }
    Ok(())
}

/// Compute one domain-separated SHA-256 file fingerprint.
fn file_fingerprint(domain: &[u8], bytes: &[u8]) -> String {
    let mut material = Vec::with_capacity(domain.len() + bytes.len());
    material.extend_from_slice(domain);
    material.extend_from_slice(bytes);
    crate::pi_agent::to_hex(&crate::pi_agent::sha256(&material))
}

/// One successful write pair retained for exact rollback until Channels ownership swaps.
#[derive(Debug)]
struct DurableSetupCommit {
    /// Settings destination.
    settings_path: PathBuf,
    /// Exact pre-apply settings state.
    settings_original: SetupFileSnapshot,
    /// Consent destination.
    consent_path: PathBuf,
    /// Exact pre-apply consent state.
    consent_original: SetupFileSnapshot,
}

impl DurableSetupCommit {
    /// Restore consent first, then settings, reporting every failure.
    fn rollback(self) -> Result<(), String> {
        let consent = restore_config_file(&self.consent_path, &self.consent_original)
            .map_err(|error| format!("could not restore prior Pi Scan consent: {error}"));
        let settings = restore_config_file(&self.settings_path, &self.settings_original)
            .map_err(|error| format!("could not restore prior settings.conf: {error}"));
        combine_results(consent, settings)
    }
}

/// Full write-free validation record accepted only by one later Apply.
#[derive(Clone)]
struct ValidationRecord {
    /// Cryptographic reviewed binding.
    binding: String,
    /// Normalized candidate.
    candidate: PiScanSettings,
    /// Candidate observation/background choice values.
    consent: PiScanConsentState,
    /// Independent confirmations.
    confirmations: PiScanSetupConfirmations,
    /// Exact pre-apply settings fingerprint.
    settings_fingerprint: String,
    /// Exact full facts reviewed during validation.
    snapshot: PiSetupProbeSnapshot,
}

/// Sequential setup-only owner state.
struct SetupController {
    /// Exact paths and dry-run policy.
    options: PiScanSetupControllerOptions,
    /// Injectable process/runtime boundary.
    driver: Box<dyn SetupDriver>,
    /// Latest accepted request correlation.
    last_correlation: Option<u64>,
    /// Latest successful full probe.
    probe: Option<PiSetupProbeSnapshot>,
    /// Latest successful write-free validation.
    validation: Option<ValidationRecord>,
    /// Wizard events.
    event_tx: mpsc::UnboundedSender<PiScanSetupEvent>,
    /// Prepared runtime ownership transfers.
    transfer_tx: mpsc::UnboundedSender<PiScanRuntimeTransfer>,
    /// Correlated controller deadline notifications.
    timeout_tx: mpsc::UnboundedSender<PiScanSetupTimeout>,
}

impl SetupController {
    /// Consume setup requests sequentially on one blocking owner.
    fn run(mut self, mut request_rx: mpsc::UnboundedReceiver<PiScanSetupRequest>) {
        while let Some(request) = request_rx.blocking_recv() {
            let correlation_id = request.correlation_id();
            let event = if self.options.dry_run {
                PiScanSetupEvent::Failed {
                    correlation_id,
                    stage: request.stage(),
                    reason: "dry-run setup is inert: Pi is not launched and settings, consent, and runtime remain unchanged".to_string(),
                }
            } else if self
                .last_correlation
                .is_some_and(|last| correlation_id <= last)
            {
                PiScanSetupEvent::Failed {
                    correlation_id,
                    stage: request.stage(),
                    reason: "stale Pi Scan setup correlation was rejected; retry the current wizard action".to_string(),
                }
            } else {
                self.last_correlation = Some(correlation_id);
                self.handle(request)
            };
            if self.event_tx.send(event).is_err() {
                break;
            }
        }
    }

    /// Dispatch one fresh request.
    fn handle(&mut self, request: PiScanSetupRequest) -> PiScanSetupEvent {
        match request {
            PiScanSetupRequest::BeginSetupProbe {
                correlation_id,
                binary,
            } => self.handle_probe(correlation_id, binary),
            PiScanSetupRequest::ValidateSetupCandidate {
                correlation_id,
                candidate,
                consent,
                confirmations,
            } => self.handle_validation(correlation_id, candidate, consent, confirmations),
            PiScanSetupRequest::ApplySetupCandidate {
                correlation_id,
                candidate,
                consent,
                confirmations,
                validation_binding,
            } => self.handle_apply(
                correlation_id,
                candidate,
                consent,
                confirmations,
                &validation_binding,
            ),
        }
    }

    /// Run and retain one exact no-model probe.
    fn handle_probe(&mut self, correlation_id: u64, binary: String) -> PiScanSetupEvent {
        self.probe = None;
        self.validation = None;
        let request = self.probe_request(binary);
        match self.probe_with_deadline(request, PiScanSetupStage::Probe) {
            Ok(snapshot) => {
                let projection = setup_projection(&snapshot, None);
                self.probe = Some(snapshot);
                PiScanSetupEvent::CapabilitiesVerified {
                    correlation_id,
                    snapshot: Box::new(projection),
                }
            }
            Err(DriverCallError::Failed(reason)) => PiScanSetupEvent::Failed {
                correlation_id,
                stage: PiScanSetupStage::Probe,
                reason,
            },
            Err(DriverCallError::TimedOut(deadline)) => {
                self.timeout_failure(correlation_id, PiScanSetupStage::Probe, deadline)
            }
        }
    }

    /// Normalize and bind a candidate without writes or runtime construction.
    fn handle_validation(
        &mut self,
        correlation_id: u64,
        candidate: PiScanSettings,
        consent: PiScanConsentState,
        confirmations: PiScanSetupConfirmations,
    ) -> PiScanSetupEvent {
        self.validation = None;
        let result = self.validate_candidate(correlation_id, candidate, consent, confirmations);
        match result {
            Ok(record) => {
                let validation_binding = record.binding.clone();
                self.validation = Some(record);
                PiScanSetupEvent::CandidateValidated {
                    correlation_id,
                    validation_binding,
                }
            }
            Err(DriverCallError::Failed(reason)) => PiScanSetupEvent::Failed {
                correlation_id,
                stage: PiScanSetupStage::CandidateValidation,
                reason,
            },
            Err(DriverCallError::TimedOut(deadline)) => self.timeout_failure(
                correlation_id,
                PiScanSetupStage::CandidateValidation,
                deadline,
            ),
        }
    }

    /// Re-probe, prepare, commit, and publish one ownership transfer.
    fn handle_apply(
        &mut self,
        correlation_id: u64,
        candidate: PiScanSettings,
        consent: PiScanConsentState,
        confirmations: PiScanSetupConfirmations,
        validation_binding: &str,
    ) -> PiScanSetupEvent {
        let Some(reviewed) = self.validation.take() else {
            return failed_binding(correlation_id, "no current write-free validation exists");
        };
        let normalized = match normalize_candidate(candidate) {
            Ok(candidate) => candidate,
            Err(reason) => return failed_binding(correlation_id, &reason),
        };
        if reviewed.binding != validation_binding
            || reviewed.candidate != normalized
            || reviewed.consent != consent
            || reviewed.confirmations != confirmations
        {
            return failed_binding(
                correlation_id,
                "candidate, confirmations, consent, or validation binding changed after review",
            );
        }
        if let Err(reason) =
            require_current_fingerprint(&self.options.settings_path, &reviewed.settings_fingerprint)
        {
            return failed_binding(correlation_id, &reason);
        }
        let fresh = match self.reprobe_reviewed(&reviewed) {
            Ok(snapshot) => snapshot,
            Err(DriverCallError::Failed(reason)) => {
                return PiScanSetupEvent::Failed {
                    correlation_id,
                    stage: PiScanSetupStage::Probe,
                    reason,
                };
            }
            Err(DriverCallError::TimedOut(deadline)) => {
                return self.timeout_failure(correlation_id, PiScanSetupStage::Probe, deadline);
            }
        };
        let (models, reservation) = match selected_models_and_reservation(&normalized, &fresh) {
            Ok(value) => value,
            Err(reason) => return failed_binding(correlation_id, &reason),
        };
        let configuration_binding =
            match material_consent_binding(&normalized, &models, reservation) {
                Ok(binding) => binding,
                Err(reason) => return failed_binding(correlation_id, &reason),
            };
        let candidate_runtime = match self.prepare_with_deadline(
            normalized.clone(),
            fresh.clone(),
            models,
            reservation,
        ) {
            Ok(runtime) => runtime,
            Err(DriverCallError::Failed(reason)) => {
                return PiScanSetupEvent::Failed {
                    correlation_id,
                    stage: PiScanSetupStage::Activation,
                    reason,
                };
            }
            Err(DriverCallError::TimedOut(deadline)) => {
                return self.timeout_failure(
                    correlation_id,
                    PiScanSetupStage::Activation,
                    deadline,
                );
            }
        };
        if let Err(reason) = self.driver.before_commit(&self.options) {
            let teardown = candidate_runtime.teardown().err();
            return persistence_failure(correlation_id, &reason, teardown);
        }
        let fresh_projection =
            setup_projection(&fresh, Some((&normalized.provider, &normalized.model)));
        let consent_json =
            match setup_consent_json(&configuration_binding, consent, confirmations, &fresh) {
                Ok(json) => json,
                Err(reason) => {
                    let teardown = candidate_runtime.teardown().err();
                    return persistence_failure(correlation_id, &reason, teardown);
                }
            };
        let commit = match commit_setup_files(
            &mut *self.driver,
            &self.options,
            &reviewed.settings_fingerprint,
            &normalized,
            &consent_json,
        ) {
            Ok(commit) => commit,
            Err(reason) => {
                let teardown = candidate_runtime.teardown().err();
                return persistence_failure(correlation_id, &reason, teardown);
            }
        };
        let transfer = PiScanRuntimeTransfer {
            correlation_id,
            candidate: Some(candidate_runtime),
            commit: Some(commit),
            effective: normalized.clone(),
            snapshot: fresh_projection.clone(),
        };
        if let Err(error) = self.transfer_tx.send(transfer) {
            drop(error.0);
            return PiScanSetupEvent::Failed {
                correlation_id,
                stage: PiScanSetupStage::Activation,
                reason: "central runtime transfer channel closed; candidate was torn down and durable setup was rolled back".to_string(),
            };
        }
        PiScanSetupEvent::Applied {
            correlation_id,
            effective: Box::new(normalized),
            snapshot: Box::new(fresh_projection),
        }
    }

    /// Validate the full candidate against the current exact probe and settings fingerprint.
    fn validate_candidate(
        &self,
        correlation_id: u64,
        candidate: PiScanSettings,
        consent: PiScanConsentState,
        confirmations: PiScanSetupConfirmations,
    ) -> Result<ValidationRecord, DriverCallError> {
        let candidate = normalize_candidate(candidate).map_err(DriverCallError::Failed)?;
        validate_confirmations(&candidate, consent, confirmations)
            .map_err(DriverCallError::Failed)?;
        let snapshot = self.probe.as_ref().ok_or_else(|| {
            DriverCallError::Failed(
                "run the no-model Pi setup probe before validating the candidate".to_string(),
            )
        })?;
        snapshot
            .validate_pricing_freshness(self.driver.now_unix_seconds())
            .map_err(|error| DriverCallError::Failed(error.to_string()))?;
        let reviewed_binary = resolve_reviewed_binary(snapshot, &candidate.binary)
            .map_err(DriverCallError::Failed)?;
        if snapshot.executable != reviewed_binary {
            return Err(DriverCallError::Failed(
                "Pi executable changed after the verified probe; probe again".to_string(),
            ));
        }
        drop(
            selected_models_and_reservation(&candidate, snapshot)
                .map_err(DriverCallError::Failed)?,
        );
        let settings = snapshot_config_file_with_deadline(
            self.options.settings_path.clone(),
            self.driver
                .operation_timeout(PiScanSetupStage::CandidateValidation),
        )?;
        let binding = validation_binding(
            correlation_id,
            &candidate,
            consent,
            confirmations,
            &settings.fingerprint,
            snapshot,
        );
        Ok(ValidationRecord {
            binding,
            candidate,
            consent,
            confirmations,
            settings_fingerprint: settings.fingerprint,
            snapshot: snapshot.clone(),
        })
    }

    /// What: Run one no-model probe on a replaceable driver under a controller deadline.
    ///
    /// Inputs:
    /// - `request`: Exact isolated setup probe request.
    /// - `stage`: Correlated stage controlling the configured deadline.
    ///
    /// Output:
    /// - Fresh probe facts, a driver failure, or typed timeout.
    ///
    /// Details:
    /// - A late read-only response has no event sender and is therefore stale; the controller
    ///   immediately remains available for a higher-correlation Retry.
    fn probe_with_deadline(
        &self,
        request: PiSetupProbeRequest,
        stage: PiScanSetupStage,
    ) -> Result<PiSetupProbeSnapshot, DriverCallError> {
        let deadline = self.driver.operation_timeout(stage);
        let mut driver = self.driver.fork();
        let (sender, receiver) = std_mpsc::sync_channel(1);
        std::thread::spawn(move || {
            drop(sender.send(driver.probe(&request)));
        });
        match receiver.recv_timeout(deadline) {
            Ok(Ok(snapshot)) => Ok(snapshot),
            Ok(Err(error)) => Err(error),
            Err(std_mpsc::RecvTimeoutError::Timeout) => Err(DriverCallError::TimedOut(deadline)),
            Err(std_mpsc::RecvTimeoutError::Disconnected) => Err(DriverCallError::Failed(
                "Pi setup probe worker stopped unexpectedly; retry the probe".to_string(),
            )),
        }
    }

    /// What: Prepare one queue-inert runtime under the Apply operation deadline.
    ///
    /// Inputs:
    /// - Normalized settings, fresh probe facts, exact models, and bounded reservation.
    ///
    /// Output:
    /// - Prepared runtime, driver failure, or typed timeout.
    ///
    /// Details:
    /// - Production preparation launches no runtime Channels. A late prepared value is dropped
    ///   without transfer and cannot replace production ownership.
    fn prepare_with_deadline(
        &self,
        settings: PiScanSettings,
        snapshot: PiSetupProbeSnapshot,
        models: Vec<ModelChoice>,
        reservation: PiScanReservation,
    ) -> Result<Box<dyn PreparedRuntime>, DriverCallError> {
        let deadline = self.driver.operation_timeout(PiScanSetupStage::Activation);
        let mut driver = self.driver.fork();
        let options = self.options.clone();
        let (sender, receiver) = std_mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result =
                driver.prepare_runtime(&options, &settings, &snapshot, models, reservation);
            drop(sender.send(result));
        });
        match receiver.recv_timeout(deadline) {
            Ok(Ok(runtime)) => Ok(runtime),
            Ok(Err(error)) => Err(error),
            Err(std_mpsc::RecvTimeoutError::Timeout) => Err(DriverCallError::TimedOut(deadline)),
            Err(std_mpsc::RecvTimeoutError::Disconnected) => Err(DriverCallError::Failed(
                "Pi runtime preparation worker stopped unexpectedly; retry Apply".to_string(),
            )),
        }
    }

    /// Build one legacy failure plus a typed correlated timeout notification.
    fn timeout_failure(
        &self,
        correlation_id: u64,
        stage: PiScanSetupStage,
        deadline: Duration,
    ) -> PiScanSetupEvent {
        let _ = self.timeout_tx.send(PiScanSetupTimeout {
            correlation_id,
            stage,
            deadline,
        });
        PiScanSetupEvent::Failed {
            correlation_id,
            stage,
            reason: format!(
                "Pi Scan setup operation exceeded its {} second deadline; Retry is available and any late response will be ignored",
                deadline.as_secs_f64()
            ),
        }
    }

    /// Re-run WS2A and require every material reviewed fact to remain exact.
    fn reprobe_reviewed(
        &self,
        reviewed: &ValidationRecord,
    ) -> Result<PiSetupProbeSnapshot, DriverCallError> {
        let request = self.probe_request(reviewed.candidate.binary.clone());
        let fresh = self.probe_with_deadline(request, PiScanSetupStage::Probe)?;
        fresh
            .validate_pricing_freshness(self.driver.now_unix_seconds())
            .map_err(|error| DriverCallError::Failed(error.to_string()))?;
        if !same_material_probe(&reviewed.snapshot, &fresh) {
            return Err(DriverCallError::Failed(
                "Pi version, tool contract, advertised routes, or exact pricing changed after review; probe and validate again"
                    .to_string(),
            ));
        }
        drop(
            selected_models_and_reservation(&reviewed.candidate, &fresh)
                .map_err(DriverCallError::Failed)?,
        );
        Ok(fresh)
    }

    /// Build one deterministic WS2A request.
    fn probe_request(&self, binary: String) -> PiSetupProbeRequest {
        let workspace_parent = self
            .options
            .state_path
            .parent()
            .map_or_else(|| self.options.quarantine_dir.clone(), Path::to_path_buf)
            .join("setup-probe");
        PiSetupProbeRequest {
            binary,
            workspace_parent,
            reservation_tokens: SETUP_PROBE_RESERVATION_TOKENS,
            now_unix_seconds: self.driver.now_unix_seconds(),
            maximum_pricing_age: SETUP_PROBE_MAXIMUM_PRICING_AGE,
        }
    }
}

/// What: Spawn the setup-only controller used while production scanning is off or being replaced.
///
/// Inputs:
/// - `options`: Dry-run flag and exact durable paths.
///
/// Output:
/// - Request/events plus the post-commit runtime-transfer receiver.
///
/// Details:
/// - All process and filesystem operations execute sequentially on one blocking owner.
#[must_use]
pub fn spawn_pi_scan_setup_controller(
    options: PiScanSetupControllerOptions,
) -> PiScanSetupChannels {
    spawn_pi_scan_setup_controller_with_driver(options, Box::<ProductionSetupDriver>::default())
}

/// Spawn one controller with an injected deterministic driver.
fn spawn_pi_scan_setup_controller_with_driver(
    options: PiScanSetupControllerOptions,
    driver: Box<dyn SetupDriver>,
) -> PiScanSetupChannels {
    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (transfer_tx, transfer_rx) = mpsc::unbounded_channel();
    let (timeout_tx, timeout_rx) = mpsc::unbounded_channel();
    tokio::task::spawn_blocking(move || {
        SetupController {
            options,
            driver,
            last_correlation: None,
            probe: None,
            validation: None,
            event_tx,
            transfer_tx,
            timeout_tx,
        }
        .run(request_rx);
    });
    PiScanSetupChannels {
        request_tx,
        event_rx,
        transfer_rx,
        timeout_rx,
    }
}

/// Normalize candidate text and exact fallback representation without raising limits.
fn normalize_candidate(mut candidate: PiScanSettings) -> Result<PiScanSettings, String> {
    candidate.enabled = true;
    candidate.binary = candidate.binary.trim().to_string();
    candidate.provider = candidate.provider.trim().to_string();
    candidate.model = candidate.model.trim().to_string();
    candidate.thinking = candidate.thinking.trim().to_ascii_lowercase();
    candidate.https_proxy = candidate.https_proxy.trim().to_string();
    candidate.background_cost_cap_24h = candidate.background_cost_cap_24h.trim().to_string();
    let fallback = parse_fallback_models(&candidate)?;
    candidate.fallback_models = fallback
        .iter()
        .map(|choice| format!("{}/{}", choice.provider, choice.model))
        .collect::<Vec<_>>()
        .join(",");
    let mut issues = candidate.validation_issues();
    if !matches!(
        candidate.thinking.as_str(),
        "off" | "low" | "medium" | "high"
    ) {
        issues.push("pi_scan_thinking must be off, low, medium, or high".to_string());
    }
    for value in [
        &candidate.binary,
        &candidate.provider,
        &candidate.model,
        &candidate.fallback_models,
        &candidate.thinking,
        &candidate.https_proxy,
    ] {
        if value.contains(['\n', '\r']) || value.chars().any(char::is_control) {
            issues.push("Pi Scan text settings cannot contain control characters".to_string());
            break;
        }
    }
    if issues.is_empty() {
        Ok(candidate)
    } else {
        Err(issues.join("; "))
    }
}

/// Parse exact fallback routes, inheriting the primary provider only for legacy model-only entries.
fn parse_fallback_models(candidate: &PiScanSettings) -> Result<Vec<ModelChoice>, String> {
    let mut models = Vec::new();
    for fallback in candidate
        .fallback_models
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let (provider, model) = fallback.split_once('/').map_or_else(
            || (candidate.provider.as_str(), fallback),
            |(provider, model)| (provider, model),
        );
        if provider.is_empty()
            || model.is_empty()
            || provider.trim() != provider
            || model.trim() != model
        {
            return Err(format!(
                "fallback route {fallback:?} is malformed; select exact advertised provider/model routes"
            ));
        }
        let choice = ModelChoice {
            provider: provider.to_string(),
            model: model.to_string(),
        };
        if models.contains(&choice) {
            return Err(format!(
                "fallback route {provider}/{model} is duplicated; keep one exact ordered route"
            ));
        }
        models.push(choice);
    }
    Ok(models)
}

/// Enforce independent confirmation semantics before a binding can be created.
fn validate_confirmations(
    candidate: &PiScanSettings,
    consent: PiScanConsentState,
    confirmations: PiScanSetupConfirmations,
) -> Result<(), String> {
    if !confirmations.disclosure_confirmed {
        return Err("confirm the provider/privacy/cost/coverage disclosure".to_string());
    }
    if !confirmations.foreground_paid_confirmed {
        return Err("confirm foreground paid execution independently".to_string());
    }
    let has_fallback = !candidate.fallback_models.is_empty();
    if has_fallback != confirmations.fallback_confirmed {
        return Err("ordered fallback and its independent confirmation do not match".to_string());
    }
    if candidate.background_enabled != consent.paid_execution {
        return Err(
            "paid background execution setting and its independent confirmation do not match"
                .to_string(),
        );
    }
    Ok(())
}

/// Resolve exact selected model order and conservative maximum reservation.
fn selected_models_and_reservation(
    candidate: &PiScanSettings,
    snapshot: &PiSetupProbeSnapshot,
) -> Result<(Vec<ModelChoice>, PiScanReservation), String> {
    let mut models = vec![ModelChoice {
        provider: candidate.provider.clone(),
        model: candidate.model.clone(),
    }];
    models.extend(parse_fallback_models(candidate)?);
    let mut reservation = PiScanReservation {
        tokens: SETUP_PROBE_RESERVATION_TOKENS,
        cost_microusd: 0,
    };
    for choice in &models {
        let route = snapshot
            .exact_route(&choice.provider, &choice.model)
            .map_err(|error| error.to_string())?;
        reservation.cost_microusd = reservation
            .cost_microusd
            .max(route.reservation.cost_microusd);
    }
    Ok((models, reservation))
}

/// Return the already canonical executable retained by the reviewed snapshot.
fn resolve_reviewed_binary(
    snapshot: &PiSetupProbeSnapshot,
    configured: &str,
) -> Result<PathBuf, String> {
    if configured.is_empty() {
        return Err("Pi executable is empty; choose it and probe again".to_string());
    }
    Ok(snapshot.executable.clone())
}

/// Compare all material probe facts while allowing only a newer observation timestamp/binding.
fn same_material_probe(reviewed: &PiSetupProbeSnapshot, fresh: &PiSetupProbeSnapshot) -> bool {
    reviewed.executable == fresh.executable
        && reviewed.pi_version == fresh.pi_version
        && reviewed.isolation == fresh.isolation
        && reviewed.routes == fresh.routes
        && reviewed.maximum_pricing_age == fresh.maximum_pricing_age
}

/// Convert WS2A's complete snapshot into the existing bounded wizard/runtime projection.
fn setup_projection(
    snapshot: &PiSetupProbeSnapshot,
    selected: Option<(&str, &str)>,
) -> SetupSnapshot {
    let route = selected
        .and_then(|(provider, model)| snapshot.exact_route(provider, model).ok())
        .or_else(|| snapshot.routes.first());
    let (selected_provider, selected_model, reservation) = route.map_or_else(
        || {
            (
                String::new(),
                String::new(),
                PiScanReservation {
                    tokens: 0,
                    cost_microusd: 0,
                },
            )
        },
        |route| {
            (
                route.provider.clone(),
                route.model.clone(),
                route.reservation,
            )
        },
    );
    SetupSnapshot {
        pi_version: snapshot.pi_version.to_string(),
        available_models: snapshot
            .routes
            .iter()
            .map(|route| (route.provider.clone(), route.model.clone()))
            .collect(),
        selected_provider,
        selected_model,
        reservation,
        route_reservations: snapshot
            .routes
            .iter()
            .map(|route| {
                (
                    route.provider.clone(),
                    route.model.clone(),
                    route.reservation,
                )
            })
            .collect(),
        pricing_binding: snapshot.pricing_binding.clone(),
        pricing_observed_at_unix_seconds: snapshot.pricing_observed_at_unix_seconds,
        maximum_pricing_age_seconds: snapshot.maximum_pricing_age.as_secs(),
        pricing_summary: snapshot.routes.iter().map(pricing_summary).collect(),
    }
}

/// Produce a bounded exact pricing summary without raw provider responses.
fn pricing_summary(route: &PiSetupAdvertisedRoute) -> String {
    format!(
        "{}/{} · input={} output={} micro-USD/million · {:?} · {}",
        route.provider,
        route.model,
        route.pricing.rates.input_microusd_per_million,
        route.pricing.rates.output_microusd_per_million,
        route.pricing.accounting,
        route.pricing_provenance
    )
}

/// Build the complete validation binding over reviewed typed material.
fn validation_binding(
    correlation_id: u64,
    candidate: &PiScanSettings,
    consent: PiScanConsentState,
    confirmations: PiScanSetupConfirmations,
    settings_fingerprint: &str,
    snapshot: &PiSetupProbeSnapshot,
) -> String {
    let value = serde_json::json!({
        "contract": "pacsea-pi-scan-setup-validation-v1",
        "correlation_id": correlation_id,
        "candidate": settings_value(candidate),
        "settings_file_fingerprint": settings_fingerprint,
        "consent": {
            "background_observation": consent.background_observation,
            "paid_background_execution": consent.paid_execution,
        },
        "confirmations": {
            "disclosure": confirmations.disclosure_confirmed,
            "foreground_paid": confirmations.foreground_paid_confirmed,
            "fallback": confirmations.fallback_confirmed,
            "readiness_warning": confirmations.readiness_warning_confirmed,
        },
        "verified": {
            "executable": snapshot.executable,
            "pi_version": snapshot.pi_version.to_string(),
            "pricing_binding": snapshot.pricing_binding,
            "pricing_observed_at": snapshot.pricing_observed_at_unix_seconds,
            "pricing_maximum_age_seconds": snapshot.maximum_pricing_age.as_secs(),
            "tool_contract_version": snapshot.isolation.tool_contract_version,
            "extension_sha256": snapshot.isolation.extension_sha256,
            "active_tools": snapshot.isolation.active_tools,
            "isolation_argv": snapshot.isolation.argv,
        },
        "prompt_version": crate::logic::pi_scan::prompt::PROMPT_VERSION,
        "result_schema": crate::logic::pi_scan::prompt::SCHEMA_VERSION,
    });
    hash_json(&value)
}

/// Represent every normalized candidate field in deterministic binding order.
fn settings_value(settings: &PiScanSettings) -> Value {
    serde_json::json!({
        "enabled": settings.enabled,
        "background_enabled": settings.background_enabled,
        "binary": settings.binary,
        "provider": settings.provider,
        "model": settings.model,
        "fallback_models": settings.fallback_models,
        "thinking": settings.thinking,
        "observation_interval_seconds": settings.observation_interval_seconds,
        "head_query_timeout_seconds": settings.head_query_timeout_seconds,
        "observation_deadline_seconds": settings.observation_deadline_seconds,
        "model_attempt_timeout_seconds": settings.model_attempt_timeout_seconds,
        "logical_timeout_seconds": settings.logical_timeout_seconds,
        "background_starts_per_hour": settings.background_starts_per_hour,
        "background_token_cap_24h": settings.background_token_cap_24h,
        "background_cost_cap_24h": settings.background_cost_cap_24h,
        "result_retention_days": settings.result_retention_days,
        "show_raw_output": settings.show_raw_output,
        "https_proxy": settings.https_proxy,
    })
}

/// Build the production-compatible material configuration binding.
fn material_consent_binding(
    settings: &PiScanSettings,
    models: &[ModelChoice],
    reservation: PiScanReservation,
) -> Result<String, String> {
    let production = production_runtime_settings(settings, models.to_vec(), reservation)?;
    Ok(crate::pi_scan_production::production_consent_binding(
        &production,
    ))
}

/// Hash deterministic JSON with the repository's SHA-256 helper.
fn hash_json(value: &Value) -> String {
    crate::pi_agent::to_hex(&crate::pi_agent::sha256(value.to_string().as_bytes()))
}

/// Serialize consent through the exact schema consumed by the production orchestrator.
fn setup_consent_json(
    binding: &str,
    consent: PiScanConsentState,
    confirmations: PiScanSetupConfirmations,
    snapshot: &PiSetupProbeSnapshot,
) -> Result<String, String> {
    let runtime = PiScanConsentState {
        background_observation: consent.background_observation,
        paid_execution: confirmations.foreground_paid_confirmed,
    };
    crate::pi_scan_orchestrator::serialize_setup_consent_document(
        binding,
        runtime,
        PiScanSetupConsentState {
            configuration_binding: binding.to_string(),
            disclosure_confirmed: confirmations.disclosure_confirmed,
            fallback_confirmed: confirmations.fallback_confirmed,
            background_paid_execution: consent.paid_execution,
            readiness_warning_confirmed: confirmations.readiness_warning_confirmed,
            confirmed_pi_version: snapshot.pi_version.to_string(),
            confirmed_pricing_binding: snapshot.pricing_binding.clone(),
        },
    )
}

/// Commit consent before settings so interruption leaves the prior configuration authoritative.
fn commit_setup_files(
    driver: &mut dyn SetupDriver,
    options: &PiScanSetupControllerOptions,
    settings_fingerprint: &str,
    settings: &PiScanSettings,
    consent_json: &str,
) -> Result<DurableSetupCommit, String> {
    let settings_original = snapshot_config_file(&options.settings_path)
        .map_err(|error| format!("could not snapshot prior Pi Scan settings: {error}"))?;
    if settings_original.fingerprint != settings_fingerprint {
        return Err("settings.conf changed after setup validation".to_string());
    }
    let consent_original = snapshot_config_file(&options.consent_path)
        .map_err(|error| format!("could not snapshot prior Pi Scan consent: {error}"))?;
    driver.before_consent_commit(options)?;
    let actual_consent_original = replace_private_file_atomic(
        &options.consent_path,
        &consent_original.fingerprint,
        consent_json,
    )
    .map_err(|error| format!("could not atomically save Pi Scan consent: {error}"))?;
    if let Err(error) = driver.before_settings_commit(options) {
        let rollback = restore_config_file(&options.consent_path, &actual_consent_original).err();
        return Err(rollback.map_or_else(
            || error.clone(),
            |rollback| format!("{error}; consent rollback also failed: {rollback}"),
        ));
    }
    match patch_pi_scan_settings_atomic(&options.settings_path, settings_fingerprint, settings) {
        Ok(actual_settings_original) => Ok(DurableSetupCommit {
            settings_path: options.settings_path.clone(),
            settings_original: actual_settings_original,
            consent_path: options.consent_path.clone(),
            consent_original: actual_consent_original,
        }),
        Err(error) => {
            let settings_rollback =
                restore_config_file(&options.settings_path, &settings_original).err();
            let consent_rollback =
                restore_config_file(&options.consent_path, &actual_consent_original).err();
            let rollback = combine_optional_errors(settings_rollback, consent_rollback);
            let mut reason = format!("could not atomically save Pi Scan settings: {error}");
            if let Some(rollback) = rollback {
                let _ = write!(reason, "; rollback also failed: {rollback}");
            }
            Err(reason)
        }
    }
}

/// Re-read only the fingerprint and reject external settings drift.
fn require_current_fingerprint(path: &Path, expected: &str) -> Result<(), String> {
    let current = snapshot_config_file(path)
        .map_err(|error| format!("could not recheck settings.conf: {error}"))?;
    if current.fingerprint == expected {
        Ok(())
    } else {
        Err(
            "settings.conf changed after validation; review the current file and validate again"
                .to_string(),
        )
    }
}

/// Construct production settings from normalized candidate and fresh exact pricing.
fn production_runtime_settings(
    settings: &PiScanSettings,
    models: Vec<ModelChoice>,
    reservation: PiScanReservation,
) -> Result<crate::pi_scan_production::ProductionRuntimeSettings, String> {
    let cost_microusd = decimal_dollars_to_microusd(&settings.background_cost_cap_24h)
        .ok_or_else(|| "Pi Scan background cost cap is not a valid decimal".to_string())?;
    Ok(crate::pi_scan_production::ProductionRuntimeSettings {
        binary: settings.binary.clone(),
        models,
        background_execution: settings.background_enabled,
        thinking: settings.thinking.clone(),
        observation_interval_seconds: settings.observation_interval_seconds,
        model_attempt_timeout: Duration::from_secs(settings.model_attempt_timeout_seconds),
        logical_timeout: Duration::from_secs(settings.logical_timeout_seconds),
        head_query_timeout: Duration::from_secs(settings.head_query_timeout_seconds),
        observation_deadline: Duration::from_secs(settings.observation_deadline_seconds),
        result_retention_days: settings.result_retention_days,
        reservation,
        budget_limits: PiScanBudgetLimits {
            starts_per_hour: settings.background_starts_per_hour,
            tokens_per_24h: settings.background_token_cap_24h,
            cost_microusd_per_24h: cost_microusd,
        },
        https_proxy: settings.https_proxy.clone(),
    })
}

/// Validate path coupling required by the existing production runtime.
fn validate_runtime_paths(options: &PiScanSetupControllerOptions) -> Result<(), String> {
    let root = options.state_path.parent().ok_or_else(|| {
        "Pi Scan state path has no parent; choose an absolute private path".to_string()
    })?;
    let expected_consent = root.join("consent-v1.json");
    if options.consent_path != expected_consent {
        return Err(format!(
            "Pi Scan consent path must be {} so the prepared runtime reads the committed document",
            expected_consent.display()
        ));
    }
    if !options.settings_path.is_absolute()
        || !options.state_path.is_absolute()
        || !options.quarantine_dir.is_absolute()
    {
        return Err("Pi Scan setup paths must be absolute".to_string());
    }
    Ok(())
}

/// Convert decimal dollars to integer micro-USD without floating-point drift.
fn decimal_dollars_to_microusd(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    let (whole, fraction) = trimmed.split_once('.').map_or((trimmed, ""), |parts| parts);
    if whole.is_empty()
        || !whole.chars().all(|character| character.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let dollars = whole.parse::<u64>().ok()?;
    let mut micros = fraction.to_string();
    micros.push_str(&"0".repeat(6usize.saturating_sub(micros.len())));
    dollars
        .checked_mul(1_000_000)?
        .checked_add(micros.parse::<u64>().ok()?)
}

/// Build a stale/review mismatch failure.
fn failed_binding(correlation_id: u64, reason: &str) -> PiScanSetupEvent {
    PiScanSetupEvent::Failed {
        correlation_id,
        stage: PiScanSetupStage::CandidateValidation,
        reason: format!("{reason}; validate the complete candidate again"),
    }
}

/// Build persistence failure with candidate teardown failure retained explicitly.
fn persistence_failure(
    correlation_id: u64,
    reason: &str,
    teardown: Option<String>,
) -> PiScanSetupEvent {
    let reason = teardown.map_or_else(
        || reason.to_string(),
        |teardown| format!("{reason}; candidate teardown also failed: {teardown}"),
    );
    PiScanSetupEvent::Failed {
        correlation_id,
        stage: PiScanSetupStage::Persistence,
        reason,
    }
}

/// Send bounded shutdown and wait for the candidate durability acknowledgement.
fn shutdown_candidate(channels: PiScanRuntimeChannels) -> Result<(), String> {
    let wait = request_candidate_shutdown(&channels)?;
    drop(channels);
    wait.recv_timeout(TRANSFER_SHUTDOWN_TIMEOUT)
        .map_err(|error| format!("candidate runtime shutdown acknowledgement failed: {error}"))?
        .warning
        .map_or(Ok(()), Err)
}

/// Request shutdown without waiting, returning its acknowledgement receiver.
fn request_candidate_shutdown(
    channels: &PiScanRuntimeChannels,
) -> Result<std_mpsc::Receiver<crate::app::runtime::workers::pi_scan::PiScanShutdownAck>, String> {
    let (ack_tx, ack_rx) = std_mpsc::sync_channel(1);
    channels
        .shutdown_tx
        .send(PiScanShutdownMessage {
            acknowledge: ack_tx,
        })
        .map_err(|error| format!("candidate runtime did not accept shutdown: {error}"))?;
    Ok(ack_rx)
}

/// Combine two rollback/shutdown results without hiding either failure.
fn combine_results(first: Result<(), String>, second: Result<(), String>) -> Result<(), String> {
    match (first, second) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(first), Err(second)) => Err(format!("{first}; {second}")),
    }
}

/// Combine optional rollback failures.
fn combine_optional_errors(
    first: Option<SetupFileError>,
    second: Option<SetupFileError>,
) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (Some(error), None) | (None, Some(error)) => Some(error.to_string()),
        (None, None) => None,
    }
}

/// Return current Unix seconds, using zero only for a pre-epoch clock.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
#[path = "../../../../tests/pi_scan/ws_setup_runtime.rs"]
mod tests;
