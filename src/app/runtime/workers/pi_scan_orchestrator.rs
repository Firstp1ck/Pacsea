//! Central single-owner orchestration for the optional Pi-backed AUR scanner.
//!
//! External discovery, observation, acquisition, and Pi execution are supplied through one
//! bounded adapter. The orchestrator owns setup gating, split-package deduplication, durable
//! commit ordering, frozen execution identity, canonical result persistence, budget
//! reconciliation, stale projection, and non-preemptive sequential execution.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::logic::pi_scan::acquisition::MutableSourceIdentity;
use crate::logic::pi_scan::baseline::{
    AcceptedBaselineEntry, AcceptedBaselineState, CommitBuildRelevance, load_versioned_state,
    save_versioned_state_atomic,
};
use crate::logic::pi_scan::identity::{CommitOid, PackageBase};
use crate::logic::pi_scan::manifest::CanonicalManifest;
use crate::logic::pi_scan::result::{MergedScanResult, ScanProvenance};
use crate::logic::pi_scan::result_store::{StoredScanResult, load_result, save_result_atomic};
use crate::state::pi_scan::{
    PiScanActualUsage, PiScanBudgetLimits, PiScanConsentState, PiScanJobRequest, PiScanPriority,
    PiScanQueueKey, PiScanReservation, PiScanRuntimeState, PiScanStartBlock, PiScanTerminalStatus,
};
use crate::state::{PiScanExecutionPhase, PiScanExecutionProgress};

/// Durable orchestration schema understood by this build.
pub const ORCHESTRATION_SCHEMA_VERSION: u32 = 1;
/// Independent consent schema understood by this build.
const CONSENT_SCHEMA_VERSION: u32 = 1;

/// What: Effective central-orchestrator configuration.
///
/// Inputs:
/// - Explicit feature/setup/background gates, dry-run, bounds, and private paths.
///
/// Output:
/// - Construction and execution policy for [`PiScanOrchestrator`].
///
/// Details:
/// - The feature and paid background execution are independently default-off.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Independent feature, setup, background, and dry-run gates are distinct decisions.
pub struct OrchestrationConfig {
    /// Explicit feature gate.
    pub enabled: bool,
    /// Explicit setup/disclosure confirmation.
    pub setup_confirmed: bool,
    /// Independent unattended model-execution consent.
    pub background_execution: bool,
    /// Explicit initial session consent used only when no durable state exists.
    pub initial_consent: PiScanConsentState,
    /// Material provider/model/privacy/pricing configuration binding.
    pub consent_binding: String,
    /// Independent versioned consent document path.
    pub consent_path: PathBuf,
    /// Private quarantine for corrupt/newer consent documents.
    pub consent_quarantine_dir: PathBuf,
    /// Session dry-run flag.
    pub dry_run: bool,
    /// Private versioned orchestration state path.
    pub state_path: PathBuf,
    /// Private canonical result-store root.
    pub results_root: PathBuf,
    /// Private quarantine for invalid stored result documents.
    pub result_quarantine_dir: PathBuf,
    /// Private quarantine for corrupt or unsupported orchestration documents.
    pub quarantine_dir: PathBuf,
    /// Independently persisted accepted current-HEAD baselines.
    pub baseline_path: PathBuf,
    /// Private quarantine for corrupt/newer baseline documents.
    pub baseline_quarantine_dir: PathBuf,
    /// Observation interval, with a 15-minute minimum.
    pub observation_interval_seconds: u64,
    /// Rolling unattended limits.
    pub budget_limits: PiScanBudgetLimits,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            setup_confirmed: false,
            background_execution: false,
            initial_consent: PiScanConsentState {
                background_observation: false,
                paid_execution: false,
            },
            consent_binding: String::new(),
            consent_path: PathBuf::new(),
            consent_quarantine_dir: PathBuf::new(),
            dry_run: false,
            state_path: PathBuf::new(),
            results_root: PathBuf::new(),
            result_quarantine_dir: PathBuf::new(),
            quarantine_dir: PathBuf::new(),
            baseline_path: PathBuf::new(),
            baseline_quarantine_dir: PathBuf::new(),
            observation_interval_seconds: 900,
            budget_limits: PiScanBudgetLimits::default(),
        }
    }
}

/// What: Exact no-model setup facts required before any target is accepted.
///
/// Inputs:
/// - Capability probe, available-model enumeration, exact selection, and exact pricing.
///
/// Output:
/// - Frozen model and worst-case reservation applied to discovered work.
///
/// Details:
/// - Contains no provider secret or authentication value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupSnapshot {
    /// Verified Pi version.
    pub pi_version: String,
    /// Exact provider/model pairs advertised by Pi.
    pub available_models: Vec<(String, String)>,
    /// Explicit selected provider.
    pub selected_provider: String,
    /// Explicit selected model.
    pub selected_model: String,
    /// Exact worst-case token and cost reservation for the selected route set.
    pub reservation: PiScanReservation,
    /// Exact reservation for every advertised provider/model route.
    pub route_reservations: Vec<(String, String, PiScanReservation)>,
    /// SHA-256 binding over exact Pi-reported pricing/provenance for configured routes.
    pub pricing_binding: String,
    /// Unix timestamp of the exact pricing observation.
    pub pricing_observed_at_unix_seconds: u64,
    /// Maximum accepted age of that pricing observation.
    pub maximum_pricing_age_seconds: u64,
    /// Human-readable exact configured route pricing/provenance disclosed before consent.
    pub pricing_summary: Vec<String>,
}

impl SetupSnapshot {
    /// Validate the selected model and reservation fail closed.
    fn validate(&self) -> Result<(), OrchestrationError> {
        let selected = self.available_models.iter().any(|(provider, model)| {
            provider == &self.selected_provider && model == &self.selected_model
        });
        if self.pi_version.trim().is_empty()
            || self.pricing_binding.trim().is_empty()
            || self.pricing_summary.is_empty()
            || !selected
        {
            return Err(OrchestrationError::Readiness(
                "Pi setup did not verify the selected provider/model; re-run setup and choose an advertised model"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// One typed update-candidate identity captured by the update worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCandidate {
    /// Package name.
    pub package_name: String,
    /// Installed/current version.
    pub current_version: String,
    /// Available candidate version.
    pub candidate_version: String,
}

/// What: One official-AUR package base resolved from installed foreign packages.
///
/// Inputs:
/// - Canonical base, every covered split-package name, and frozen installed/update versions.
///
/// Output:
/// - One deduplicated observation target.
///
/// Details:
/// - Installed version equality is retained only as metadata and never proves provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPackage {
    /// Canonical official AUR package base.
    pub package_base: PackageBase,
    /// Every installed split-package name covered by the base.
    pub installed_names: Vec<String>,
    /// Installed version recorded verbatim.
    pub installed_version: String,
    /// Frozen candidate version, when any.
    pub candidate_version: Option<String>,
}

/// One observed commit and its build-relevance classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationCommit {
    /// Full immutable AUR commit OID.
    pub oid: CommitOid,
    /// Deterministic build relevance.
    pub relevance: CommitBuildRelevance,
}

/// What: Sequential observation response for one official package base.
///
/// Inputs:
/// - Produced by the WS7 observation adapter.
///
/// Output:
/// - Oldest-first commit expansion and cursor/rebaseline facts.
///
/// Details:
/// - A rebaseline pause never advances the durable cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationPackage {
    /// Canonical observed package base.
    pub package_base: PackageBase,
    /// Current official AUR HEAD.
    pub head_oid: CommitOid,
    /// Every unseen commit, oldest first.
    pub commits: Vec<ObservationCommit>,
    /// Whether the bounded expansion must resume later.
    pub truncated: bool,
    /// Whether rewritten history requires explicit rebaseline.
    pub paused_for_rebaseline: bool,
}

/// What: Full immutable target accepted by the single runner.
///
/// Inputs:
/// - Package/install identity, recipe/head identity, cycle, exact model route, and reservation.
///
/// Output:
/// - Durable target used unchanged through acquisition, execution, stale checks, and storage.
///
/// Details:
/// - A target missing any identity/model/pricing component is rejected before queue mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenScanIdentity {
    /// Stable scan identifier and result filename stem.
    pub scan_id: String,
    /// Installed package whose immutable `.SRCINFO` membership must be proved.
    pub package_name: String,
    /// Canonical official AUR package base.
    pub package_base: PackageBase,
    /// Every installed split-package name covered by this scan.
    pub installed_names: Vec<String>,
    /// Installed version recorded verbatim.
    pub installed_version: String,
    /// Frozen update candidate version, when any.
    pub candidate_version: Option<String>,
    /// Exact recipe commit to acquire and scan.
    pub commit_oid: CommitOid,
    /// Official HEAD observed when this target was frozen.
    pub observed_head_oid: CommitOid,
    /// Observation cycle identifier.
    pub cycle_id: String,
    /// Exact confirmed provider route.
    pub provider: String,
    /// Exact confirmed model.
    pub model: String,
    /// Worst-case token/cost reservation.
    pub reservation: PiScanReservation,
    /// Foreground or unattended priority.
    pub priority: PiScanPriority,
}

impl FrozenScanIdentity {
    /// Validate that every required frozen identity and pricing component is present.
    fn validate(&self) -> Result<(), OrchestrationError> {
        let text_fields = [
            self.scan_id.as_str(),
            self.package_name.as_str(),
            self.installed_version.as_str(),
            self.cycle_id.as_str(),
            self.provider.as_str(),
            self.model.as_str(),
        ];
        if text_fields.iter().any(|value| value.trim().is_empty())
            || self.installed_names.is_empty()
        {
            return Err(OrchestrationError::InvalidTarget(
                "a Pi scan target requires full frozen package, commit, cycle, model, and pricing identity"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Convert the full identity to the WS3 queue request without re-deriving any field.
    fn queue_request(&self, request_id: u64) -> PiScanJobRequest {
        PiScanJobRequest {
            request_id,
            key: PiScanQueueKey {
                package_base: self.package_base.clone(),
                commit_oid: self.commit_oid.clone(),
            },
            priority: self.priority,
            reservation: self.reservation,
            manual_budget_override_confirmed: self.priority == PiScanPriority::Foreground,
        }
    }
}

/// What: Canonical output of WS8 acquisition plus WS6 execution and stale rechecks.
///
/// Inputs:
/// - Returned only by an adapter after strict model identity/evidence validation.
///
/// Output:
/// - Canonical result/provenance/manifests, bounded usage, and stale projection.
///
/// Details:
/// - No raw prompt, source body, thinking, or unvalidated response can be represented here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    /// Strictly validated merged result.
    pub result: MergedScanResult,
    /// Official AUR HEAD frozen when the target was created.
    pub observed_head_oid: CommitOid,
    /// Model/Pi/tool/schema provenance.
    pub provenance: ScanProvenance,
    /// Canonical recipe and source manifests.
    pub manifests: Vec<CanonicalManifest>,
    /// Actual or conservative bounded usage.
    pub usage: PiScanActualUsage,
    /// Whether exact HEAD or mutable-source recheck changed.
    pub stale: bool,
    /// Mutable Git refs resolved during advisory acquisition.
    pub mutable_sources: Vec<MutableSourceIdentity>,
}

/// Bounded acquisition-only evidence returned by a dry-run preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunAcquisitionReceipt {
    /// Exact package/commit identity that was acquired.
    pub key: PiScanQueueKey,
    /// Complete, incomplete, or failed acquisition status.
    pub status: String,
    /// Canonical manifest count produced by bounded acquisition.
    pub manifest_count: usize,
    /// Explicit coverage limitations retained for the preview.
    pub coverage_notes: Vec<String>,
}

/// Execution failure classification used by the central runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionFailure {
    /// Sticky cancellation suppressed correction, fallback, and result acceptance.
    Cancelled,
    /// Acquisition, Pi, validation, persistence, or recheck service failure.
    Service(String),
}

/// What: Correlation-aware transient phase publisher for one active execution.
///
/// Inputs:
/// - Exact active correlation ID and an in-process non-blocking publication callback.
///
/// Output:
/// - Typed [`PiScanExecutionProgress`] values bound to the active correlation.
///
/// Details:
/// - The reporter owns no durable state. It cannot report a phase for another correlation.
pub struct PiScanExecutionPhaseReporter<'a> {
    /// Exact active runtime correlation owning every report.
    correlation_id: u64,
    /// In-process callback supplied by the sequential runtime owner.
    publish: &'a dyn Fn(PiScanExecutionProgress),
}

impl<'a> PiScanExecutionPhaseReporter<'a> {
    /// What: Bind a phase callback to one exact active correlation.
    ///
    /// Inputs:
    /// - `correlation_id`: Exact active runtime correlation.
    /// - `publish`: In-process callback for typed progress values.
    ///
    /// Output:
    /// - Reporter that cannot emit an update for another correlation.
    ///
    /// Details:
    /// - Construction and reporting are synchronous and perform no durable mutation.
    #[must_use]
    pub fn new(correlation_id: u64, publish: &'a dyn Fn(PiScanExecutionProgress)) -> Self {
        Self {
            correlation_id,
            publish,
        }
    }

    /// What: Publish one observable execution phase for the bound correlation.
    ///
    /// Inputs:
    /// - `phase`: Current truthful phase at the caller's lifecycle boundary.
    ///
    /// Output:
    /// - Invokes the configured callback with a typed correlation-owned update.
    ///
    /// Details:
    /// - Ordering is the caller's synchronous invocation order; no persistence is performed.
    pub fn report(&self, phase: PiScanExecutionPhase) {
        (self.publish)(PiScanExecutionProgress {
            correlation_id: self.correlation_id,
            phase,
        });
    }
}

/// What: Injectable end-to-end platform adapter for deterministic orchestration.
///
/// Inputs:
/// - No-model setup, installed-package discovery, sequential WS7 observation, and WS8/WS6 run.
///
/// Output:
/// - Only bounded typed data accepted by the central owner.
///
/// Details:
/// - Production implementations must use direct-argv/read-only seams; tests use fakes and
///   never contact a network, Git repository, or provider.
pub trait OrchestrationAdapter {
    /// Run the no-model capability/model/pricing setup probe.
    ///
    /// # Errors
    /// - Returns actionable missing-tool, capability, model, or pricing guidance.
    fn probe_setup(&mut self) -> Result<SetupSnapshot, String>;

    /// Build inert configured setup identity for acquisition-only dry-run.
    ///
    /// # Errors
    /// - Returns when the configured dry-run identity is incomplete or invalid.
    fn dry_run_setup(&mut self) -> Result<SetupSnapshot, String> {
        self.probe_setup()
    }

    /// Enumerate foreign packages and resolve official package bases.
    ///
    /// # Errors
    /// - Returns actionable package-manager or official-AUR resolution guidance.
    fn enumerate_foreign(&mut self) -> Result<Vec<DiscoveredPackage>, String>;

    /// Enumerate only explicitly selected installed package names for a foreground request.
    ///
    /// # Errors
    /// - Returns actionable package-manager or official-AUR resolution guidance.
    fn enumerate_selected(
        &mut self,
        package_names: &BTreeSet<String>,
    ) -> Result<Vec<DiscoveredPackage>, String> {
        let mut packages = self.enumerate_foreign()?;
        packages.retain(|package| {
            package
                .installed_names
                .iter()
                .any(|name| package_names.contains(name))
        });
        Ok(packages)
    }

    /// Replace typed update candidates for the next observation cycle.
    fn set_update_candidates(&mut self, _candidates: Vec<UpdateCandidate>) {}

    /// Query and expand one package base sequentially from its durable cursor.
    ///
    /// # Errors
    /// - Returns an observation error without advancing the supplied cursor.
    fn observe_package(
        &mut self,
        package: &DiscoveredPackage,
        cursor: Option<&CommitOid>,
    ) -> Result<ObservationPackage, String>;

    /// Acquire immutable snapshots, execute WS6, and perform exact stale rechecks.
    ///
    /// # Errors
    /// - Returns sticky cancellation or a fail-closed acquisition/execution/recheck failure.
    fn execute(
        &mut self,
        target: &FrozenScanIdentity,
        cancelled: &AtomicBool,
    ) -> Result<ExecutionReceipt, ExecutionFailure>;

    /// Execute with an optional correlation-aware phase reporter.
    ///
    /// The default preserves existing adapter semantics and emits no detailed phases. Production
    /// adapters may override this seam when they can identify truthful internal boundaries.
    ///
    /// # Errors
    /// - Returns the same cancellation or fail-closed service failure as [`Self::execute`].
    fn execute_with_progress(
        &mut self,
        target: &FrozenScanIdentity,
        cancelled: &AtomicBool,
        _progress: &PiScanExecutionPhaseReporter<'_>,
    ) -> Result<ExecutionReceipt, ExecutionFailure> {
        self.execute(target, cancelled)
    }

    /// Re-run the service-specific setup and acquisition checks before clearing a sticky pause.
    ///
    /// # Errors
    /// - Returns when the service condition was not independently revalidated.
    fn revalidate_service(&mut self, _target: &FrozenScanIdentity) -> Result<(), String> {
        self.probe_setup().map(|_| ())
    }

    /// Perform bounded immutable acquisition only, without Pi or durable writes.
    ///
    /// # Errors
    /// - Returns when immutable acquisition cannot be completed under the configured bounds.
    fn dry_run_acquisition(
        &mut self,
        _target: &FrozenScanIdentity,
    ) -> Result<DryRunAcquisitionReceipt, String> {
        Err("this adapter does not provide acquisition-only dry-run".to_string())
    }

    /// Re-resolve mutable advisory source refs immediately before linked continuation.
    ///
    /// # Errors
    /// - Returns fail closed when any identity cannot be revalidated.
    fn recheck_mutable_sources(
        &mut self,
        sources: &[MutableSourceIdentity],
    ) -> Result<bool, String> {
        if sources.is_empty() {
            Ok(false)
        } else {
            Err(
                "mutable-source identity recheck is unavailable; do not continue installation"
                    .to_string(),
            )
        }
    }

    /// Re-resolve official AUR HEAD immediately before linked continuation.
    ///
    /// # Errors
    /// - Returns a fail-closed observation error when identity cannot be revalidated.
    fn recheck_continuation(
        &mut self,
        _package_base: &PackageBase,
        _observed_head_oid: &CommitOid,
    ) -> Result<bool, String> {
        Err(
            "continuation identity recheck is unavailable; do not continue installation"
                .to_string(),
        )
    }
}

/// Durable observation ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationLedgerEntry {
    /// Exact package-base and commit key.
    pub key: PiScanQueueKey,
    /// Deterministic relevance classification.
    pub relevance: CommitBuildRelevance,
    /// Unix second the commit was durably observed.
    pub observed_at_unix: u64,
}

/// Material-configuration-bound setup confirmations persisted with runtime consent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "disclosure, fallback, background payment, and readiness are independent durable decisions"
)]
pub struct PiScanSetupConsentState {
    /// SHA-256 binding over provider/model/privacy/pricing-relevant configuration.
    pub configuration_binding: String,
    /// Explicit provider/privacy/cost/coverage disclosure confirmation.
    pub disclosure_confirmed: bool,
    /// Explicit ordered-fallback confirmation.
    pub fallback_confirmed: bool,
    /// Independent paid background-execution confirmation.
    #[serde(default)]
    pub background_paid_execution: bool,
    /// Explicit readiness-warning confirmation.
    pub readiness_warning_confirmed: bool,
    /// Exact Pi version confirmed when consent was granted.
    pub confirmed_pi_version: String,
    /// Exact Pi-reported pricing/provenance binding confirmed with consent.
    pub confirmed_pricing_binding: String,
}

/// Independent versioned runtime/setup consent document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedConsentDocument {
    /// Document schema version.
    schema_version: u32,
    /// Material configuration binding.
    configuration_binding: String,
    /// Independent runtime consent.
    runtime: PiScanConsentState,
    /// Setup confirmations under the same binding.
    setup: PiScanSetupConsentState,
}

/// What: Serialize one setup transaction's consent in the production-compatible schema.
///
/// Inputs:
/// - `configuration_binding`: Exact material runtime binding.
/// - `runtime`: Independent observation and foreground-paid consent.
/// - `setup`: Disclosure, fallback, readiness, Pi, and pricing confirmations.
///
/// Output:
/// - Private consent JSON accepted by [`PiScanOrchestrator::new`].
///
/// Details:
/// - Only typed consent and cryptographic bindings are represented; credentials, prompts,
///   source content, Pi output, and provider responses cannot enter this document.
///
/// # Errors
/// - Rejects an empty or internally inconsistent material binding.
pub(crate) fn serialize_setup_consent_document(
    configuration_binding: &str,
    runtime: PiScanConsentState,
    setup: PiScanSetupConsentState,
) -> Result<String, String> {
    if configuration_binding.is_empty() || setup.configuration_binding != configuration_binding {
        return Err(
            "Pi setup consent binding is empty or inconsistent; validate setup again".to_string(),
        );
    }
    serde_json::to_string_pretty(&PersistedConsentDocument {
        schema_version: CONSENT_SCHEMA_VERSION,
        configuration_binding: configuration_binding.to_string(),
        runtime,
        setup,
    })
    .map_err(|error| format!("could not serialize Pi setup consent: {error}"))
}

/// What: Single-writer durable state across observation and execution.
///
/// Inputs:
/// - WS3 runtime, full frozen targets, ledger, cursors, request sequence, and observation time.
///
/// Output:
/// - Crash-recoverable orchestration document.
///
/// Details:
/// - Queue requests are always paired with a full target under the same request id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationState {
    /// Cohesive WS3 queue/budget state.
    pub runtime: PiScanRuntimeState,
    /// Full frozen target by queue request id.
    pub targets: BTreeMap<u64, FrozenScanIdentity>,
    /// Every observed commit, including no-recipe-delta commits.
    pub ledger: Vec<OrchestrationLedgerEntry>,
    /// Last durably inserted commit per package base.
    pub cursors: BTreeMap<String, CommitOid>,
    /// Last allocated request id.
    pub next_request_id: u64,
    /// Last successful observation cycle timestamp.
    pub last_observation_unix: Option<u64>,
    /// Explicit setup confirmations bound to material scanner configuration.
    #[serde(default)]
    pub setup_consent: PiScanSetupConsentState,
}

/// Versioned orchestration persistence envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedOrchestrationState {
    /// Supported schema version.
    schema_version: u32,
    /// Single-owner durable state.
    state: OrchestrationState,
}

/// Central orchestration error with an actionable fail-closed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestrationError {
    /// Feature, setup, or observation consent is disabled.
    Disabled(String),
    /// Setup/capability/model/pricing is unavailable.
    Readiness(String),
    /// A target lacks full frozen identity or pricing.
    InvalidTarget(String),
    /// Observation failed without taking down Pacsea.
    Observation(String),
    /// Queue policy or rolling budget blocked a start.
    Paused(String),
    /// User cancellation became terminal.
    Cancelled,
    /// Acquisition, execution, validation, stale check, or persistence failed.
    Execution(String),
    /// Durable state is corrupt, newer, or unavailable.
    Persistence(String),
}

impl fmt::Display for OrchestrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled(reason)
            | Self::Readiness(reason)
            | Self::InvalidTarget(reason)
            | Self::Observation(reason)
            | Self::Paused(reason)
            | Self::Execution(reason)
            | Self::Persistence(reason) => formatter.write_str(reason),
            Self::Cancelled => formatter.write_str("the active Pi scan was cancelled"),
        }
    }
}

impl std::error::Error for OrchestrationError {}

/// What: Single-owner default-off sequential scanner orchestrator.
///
/// Inputs:
/// - Effective configuration and one production or fake adapter.
///
/// Output:
/// - Startup/manual/periodic observation and one-at-a-time execution methods.
///
/// Details:
/// - All durable mutation occurs on this owner. External operations are sequential, while
///   callers place blocking methods on a bounded blocking task away from the async UI loop.
pub struct PiScanOrchestrator<A> {
    /// Effective policy and private paths.
    config: OrchestrationConfig,
    /// Single durable state owner.
    state: OrchestrationState,
    /// External platform adapter.
    adapter: A,
    /// Accepted current-HEAD baselines persisted independently from observation state.
    baselines: AcceptedBaselineState,
    /// Cached no-model setup facts for the current process.
    setup: Option<SetupSnapshot>,
}

impl<A: OrchestrationAdapter> PiScanOrchestrator<A> {
    /// What: Load a central orchestrator without weakening corrupt/newer-state handling.
    ///
    /// Inputs:
    /// - `config`: Effective gates, limits, and paths.
    /// - `adapter`: Production or deterministic fake external seams.
    ///
    /// Output:
    /// - Ready but still default-off orchestrator.
    ///
    /// Details:
    /// - Dry-run never reads durable state. Missing state starts empty; malformed/newer state
    ///   fails closed rather than being interpreted as empty.
    ///
    /// # Errors
    /// - Returns a persistence error for malformed, newer, or unreadable durable state.
    pub fn new(config: OrchestrationConfig, adapter: A) -> Result<Self, OrchestrationError> {
        if config.observation_interval_seconds < 900 {
            return Err(OrchestrationError::Disabled(
                "Pi scan observation interval must be at least 900 seconds; fix the setting and restart"
                    .to_string(),
            ));
        }
        let baselines = if config.dry_run {
            AcceptedBaselineState::default()
        } else {
            load_versioned_state(
                &config.baseline_path,
                1,
                &config.baseline_quarantine_dir,
                "baseline",
            )
            .map_err(|error| OrchestrationError::Persistence(error.to_string()))?
            .unwrap_or_default()
        };
        let consent_document: Option<PersistedConsentDocument> = if config.dry_run {
            None
        } else {
            load_versioned_state(
                &config.consent_path,
                CONSENT_SCHEMA_VERSION,
                &config.consent_quarantine_dir,
                "consent",
            )
            .map_err(|error| OrchestrationError::Persistence(error.to_string()))?
        };
        let state_exists = !config.dry_run && config.state_path.exists();
        let mut state = if config.dry_run {
            OrchestrationState::default()
        } else {
            load_state(&config.state_path, &config.quarantine_dir)?
        };
        let recovered_request = state
            .runtime
            .active
            .as_ref()
            .map(|active| active.request.request_id);
        if !config.dry_run && recovered_request.is_some() {
            state
                .runtime
                .recover_interrupted(current_unix())
                .map_err(|error| OrchestrationError::Persistence(error.to_string()))?;
            save_state(&config.state_path, &state)?;
        }
        state.runtime.budget_limits = config.budget_limits;
        let mut consent_needs_save = false;
        if let Some(document) = consent_document {
            if document.configuration_binding == config.consent_binding
                && document.setup.configuration_binding == config.consent_binding
            {
                state.runtime.set_consent(document.runtime);
                state.setup_consent = document.setup;
            } else {
                reset_consent_for_binding(&mut state, &config.consent_binding);
                consent_needs_save = true;
            }
        } else {
            let binding_changed =
                state_exists && state.setup_consent.configuration_binding != config.consent_binding;
            if !state_exists {
                state.runtime.set_consent(config.initial_consent);
                state.setup_consent = PiScanSetupConsentState {
                    configuration_binding: config.consent_binding.clone(),
                    disclosure_confirmed: config.setup_confirmed,
                    fallback_confirmed: false,
                    background_paid_execution: false,
                    readiness_warning_confirmed: false,
                    confirmed_pi_version: String::new(),
                    confirmed_pricing_binding: String::new(),
                };
            } else if binding_changed {
                reset_consent_for_binding(&mut state, &config.consent_binding);
                consent_needs_save = true;
            }
        }
        if consent_needs_save && !config.dry_run {
            save_state(&config.state_path, &state)?;
            save_consent_document(&config, &state)?;
        }
        Ok(Self {
            config,
            state,
            adapter,
            baselines,
            setup: None,
        })
    }

    /// Borrow the external adapter for deterministic evidence or projection.
    #[must_use]
    pub const fn adapter(&self) -> &A {
        &self.adapter
    }

    /// Mutably borrow the external adapter for a scripted next operation.
    pub const fn adapter_mut(&mut self) -> &mut A {
        &mut self.adapter
    }

    /// Borrow the single-owner state projection.
    #[must_use]
    pub const fn state(&self) -> &OrchestrationState {
        &self.state
    }

    /// Probe/cache current no-model setup facts before granting material-bound consent.
    ///
    /// # Errors
    /// - Returns disabled or readiness failures.
    pub fn setup_snapshot(&mut self) -> Result<SetupSnapshot, OrchestrationError> {
        if !self.config.enabled {
            return Err(OrchestrationError::Disabled(
                "Pi scanning is disabled; enable it before setup verification".to_string(),
            ));
        }
        self.ensure_setup().cloned()
    }

    /// Return durable runtime and material-bound setup consent projections.
    #[must_use]
    pub fn consent_snapshot(&self) -> (PiScanConsentState, PiScanSetupConsentState) {
        (self.state.runtime.consent, self.state.setup_consent.clone())
    }

    /// Persist explicit setup confirmations under the current material configuration binding.
    ///
    /// # Errors
    /// - Returns persistence failures.
    pub fn update_setup_consent(
        &mut self,
        mut setup: PiScanSetupConsentState,
    ) -> Result<(), OrchestrationError> {
        setup
            .configuration_binding
            .clone_from(&self.config.consent_binding);
        self.state.setup_consent = setup;
        self.persist()?;
        self.persist_consent()
    }

    /// Run the explicitly enabled startup observation cycle.
    ///
    /// # Errors
    /// - Returns disabled, readiness, observation, target, or persistence failures.
    pub fn startup_observation(
        &mut self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        self.observe(now_unix, false, None)
    }

    /// Run an explicitly requested broad manual observation cycle.
    ///
    /// # Errors
    /// - Returns disabled, readiness, observation, target, or persistence failures.
    pub fn manual_observation(
        &mut self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        self.observe(now_unix, true, None)
    }

    /// Observe only the installed package names explicitly selected in the Targets view.
    ///
    /// # Errors
    /// - Returns when the selection is empty or discovery, observation, target, or persistence fails.
    pub fn manual_observation_selected(
        &mut self,
        now_unix: u64,
        package_names: &BTreeSet<String>,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        if package_names.is_empty() {
            return Err(OrchestrationError::InvalidTarget(
                "select at least one unresolved target before observation".to_string(),
            ));
        }
        self.observe(now_unix, true, Some(package_names))
    }

    /// Perform bounded acquisition-only dry-run for one previously observed exact target.
    ///
    /// # Errors
    /// - Returns when dry-run is inactive, the target is unknown, or acquisition fails.
    pub fn dry_run_acquisition(
        &mut self,
        key: &PiScanQueueKey,
    ) -> Result<DryRunAcquisitionReceipt, OrchestrationError> {
        if !self.config.dry_run {
            return Err(OrchestrationError::Disabled(
                "acquisition-only preview is available only in dry-run mode".to_string(),
            ));
        }
        let target = self
            .state
            .targets
            .values()
            .find(|target| {
                target.package_base == key.package_base && target.commit_oid == key.commit_oid
            })
            .cloned()
            .ok_or_else(|| {
                OrchestrationError::InvalidTarget(
                    "run dry-run observation before requesting acquisition preview".to_string(),
                )
            })?;
        self.adapter
            .dry_run_acquisition(&target)
            .map_err(OrchestrationError::Execution)
    }

    /// Observe current HEAD for typed update candidates and queue explicit foreground scans.
    ///
    /// # Errors
    /// - Returns setup, discovery, observation, target, queue, or persistence failures.
    pub fn update_candidate_observation(
        &mut self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        self.ensure_enabled()?;
        let setup = self.ensure_setup()?.clone();
        let packages = deduplicate_packages(
            self.adapter
                .enumerate_foreign()
                .map_err(OrchestrationError::Observation)?,
        );
        let mut discovered = Vec::new();
        for package in packages
            .into_iter()
            .filter(|package| package.candidate_version.is_some())
        {
            let cursor = self.state.cursors.get(package.package_base.as_str());
            let observation = self
                .adapter
                .observe_package(&package, cursor)
                .map_err(OrchestrationError::Observation)?;
            let commit = ObservationCommit {
                oid: observation.head_oid.clone(),
                relevance: CommitBuildRelevance::BuildRelevant,
            };
            let Some(target) = self.target_for_commit(
                &package,
                &observation.head_oid,
                &commit,
                &setup,
                now_unix,
                true,
            )?
            else {
                continue;
            };
            let already_completed = self.state.runtime.terminal.iter().any(|record| {
                record.status == PiScanTerminalStatus::Completed
                    && record.request.key.package_base == target.package_base
                    && record.request.key.commit_oid == target.commit_oid
            });
            if already_completed {
                continue;
            }
            let request_id = self.state.next_request_id;
            match self.state.runtime.enqueue(target.queue_request(request_id)) {
                Ok(_) => {
                    self.state.targets.insert(request_id, target.clone());
                    discovered.push(target);
                }
                Err(crate::state::pi_scan::PiScanStateError::DuplicateIdentity(_)) => {}
                Err(error) => {
                    return Err(OrchestrationError::InvalidTarget(error.to_string()));
                }
            }
        }
        self.persist()?;
        Ok(discovered)
    }

    /// Run a due periodic observation at the configured 15-minute-or-longer interval.
    ///
    /// # Errors
    /// - Returns disabled, readiness, observation, target, or persistence failures when due.
    pub fn periodic_observation(
        &mut self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        let due = self.state.last_observation_unix.is_none_or(|last| {
            now_unix.saturating_sub(last) >= self.config.observation_interval_seconds
        });
        if !due {
            return Ok(Vec::new());
        }
        self.observe(now_unix, false, None)
    }

    /// What: Queue one manually selected full frozen identity.
    ///
    /// Inputs:
    /// - `target`: Exact package/commit/model/pricing identity.
    /// - `now_unix`: Durable insertion timestamp.
    ///
    /// Output:
    /// - Assigned queue request id.
    ///
    /// Details:
    /// - Manual work is foreground priority and therefore runs next without preempting active work.
    ///
    /// # Errors
    /// - Rejects disabled/setup-invalid, incomplete, duplicate, or unpersistable work.
    pub fn enqueue_frozen(
        &mut self,
        mut target: FrozenScanIdentity,
        now_unix: u64,
    ) -> Result<u64, OrchestrationError> {
        self.ensure_enabled()?;
        self.ensure_setup()?;
        target.priority = PiScanPriority::Foreground;
        target.validate()?;
        let request_id = self.allocate_request_id()?;
        self.state
            .runtime
            .enqueue(target.queue_request(request_id))
            .map_err(|error| OrchestrationError::InvalidTarget(error.to_string()))?;
        self.state.targets.insert(request_id, target);
        self.state.last_observation_unix.get_or_insert(now_unix);
        self.persist()?;
        Ok(request_id)
    }

    /// What: Promote an already observed queued target to explicit foreground execution.
    ///
    /// Inputs:
    /// - `key`: Exact package-base/commit identity selected by the user.
    ///
    /// Output:
    /// - The existing request id after durable priority promotion.
    ///
    /// Details:
    /// - Observation owns target construction. Selection cannot reconstruct or alter model,
    ///   package, commit, or reservation identity.
    ///
    /// # Errors
    /// - Returns when setup is disabled, the target is absent, or persistence fails.
    pub fn promote_queued(&mut self, key: &PiScanQueueKey) -> Result<u64, OrchestrationError> {
        self.ensure_enabled()?;
        self.ensure_setup()?;
        if self
            .state
            .runtime
            .pause_reasons
            .contains(&crate::state::pi_scan::PiScanPauseReason::Service)
        {
            let target = self
                .state
                .targets
                .values()
                .find(|target| {
                    target.package_base == key.package_base && target.commit_oid == key.commit_oid
                })
                .cloned()
                .ok_or_else(|| {
                    OrchestrationError::InvalidTarget(
                        "the paused service target no longer has a frozen identity".to_string(),
                    )
                })?;
            self.adapter
                .revalidate_service(&target)
                .map_err(OrchestrationError::Readiness)?;
            self.state.runtime.clear_service_pause(true);
        }
        if let Some(request) = self
            .state
            .runtime
            .queue
            .iter_mut()
            .find(|request| &request.key == key)
        {
            request.priority = PiScanPriority::Foreground;
            request.manual_budget_override_confirmed = true;
            let request_id = request.request_id;
            if let Some(target) = self.state.targets.get_mut(&request_id) {
                target.priority = PiScanPriority::Foreground;
            }
            self.persist()?;
            return Ok(request_id);
        }
        self.retry_terminal_target(key)
    }

    /// Requeue one retained failed/cancelled full identity under a fresh request id.
    fn retry_terminal_target(&mut self, key: &PiScanQueueKey) -> Result<u64, OrchestrationError> {
        let prior_id = self
            .state
            .runtime
            .terminal
            .iter()
            .rev()
            .find(|record| {
                &record.request.key == key
                    && matches!(
                        record.status,
                        PiScanTerminalStatus::Failed
                            | PiScanTerminalStatus::Cancelled
                            | PiScanTerminalStatus::Interrupted
                    )
            })
            .map(|record| record.request.request_id)
            .ok_or_else(|| {
                OrchestrationError::InvalidTarget(
                    "the selected commit is neither queued nor retained for retry; refresh observation"
                        .to_string(),
                )
            })?;
        let mut target = self.state.targets.remove(&prior_id).ok_or_else(|| {
            OrchestrationError::InvalidTarget(
                "the terminal scan no longer has its full frozen retry identity".to_string(),
            )
        })?;
        let request_id = self.allocate_request_id()?;
        target.priority = PiScanPriority::Foreground;
        self.state
            .runtime
            .enqueue(target.queue_request(request_id))
            .map_err(|error| OrchestrationError::InvalidTarget(error.to_string()))?;
        self.state.targets.insert(request_id, target);
        self.persist()?;
        Ok(request_id)
    }

    /// Accept one complete current-HEAD result as the independent observation baseline.
    ///
    /// # Errors
    /// - Returns when the stored result is missing, stale, incomplete, identity-mismatched, or
    ///   when private baseline/result persistence fails.
    pub fn accept_baseline(
        &mut self,
        package_base: &PackageBase,
        commit_oid: &CommitOid,
        scan_id: &str,
        evidence_fingerprint: &str,
        now_unix: u64,
    ) -> Result<(), OrchestrationError> {
        self.ensure_enabled()?;
        if self.config.dry_run {
            return Err(OrchestrationError::Disabled(
                "dry-run cannot accept or persist a Pi scan baseline".to_string(),
            ));
        }
        let (mut document, _) = load_result(
            &self.config.results_root,
            &self.config.result_quarantine_dir,
            package_base.as_str(),
            scan_id,
            now_unix,
        )
        .map_err(|error| OrchestrationError::Persistence(error.to_string()))?;
        let observed_head = if document.observed_head_oid.is_empty() {
            document.commit_oid.as_str()
        } else {
            document.observed_head_oid.as_str()
        };
        let complete = matches!(document.coverage.as_str(), "complete" | "Complete");
        if document.stale
            || !complete
            || document.commit_oid != commit_oid.as_str()
            || observed_head != commit_oid.as_str()
        {
            return Err(OrchestrationError::InvalidTarget(
                "only a complete, current, exact-HEAD result can become the accepted baseline"
                    .to_string(),
            ));
        }
        document.accepted_baseline = true;
        save_result_atomic(&self.config.results_root, &document)
            .map_err(|error| OrchestrationError::Persistence(error.to_string()))?;
        self.baselines.entries.insert(
            package_base.as_str().to_string(),
            AcceptedBaselineEntry {
                package_base: package_base.clone(),
                accepted_commit_oid: commit_oid.clone(),
                accepted_at_unix_ts: now_unix,
                evidence_fingerprint: evidence_fingerprint.to_string(),
                notes: Some("explicit complete current-HEAD Pi scan baseline".to_string()),
            },
        );
        save_versioned_state_atomic(&self.config.baseline_path, &self.baselines)
            .map_err(|error| OrchestrationError::Persistence(error.to_string()))
    }

    /// Re-resolve one validated result's official AUR HEAD before linked continuation.
    ///
    /// # Errors
    /// - Returns setup or fail-closed observation errors.
    pub fn validate_continuation(
        &mut self,
        package_base: &PackageBase,
        observed_head_oid: &CommitOid,
    ) -> Result<bool, OrchestrationError> {
        self.validate_continuation_with_sources(package_base, observed_head_oid, &[])
    }

    /// Re-resolve official AUR HEAD and every mutable advisory source before continuation.
    ///
    /// # Errors
    /// - Returns setup or fail-closed observation errors.
    pub fn validate_continuation_with_sources(
        &mut self,
        package_base: &PackageBase,
        observed_head_oid: &CommitOid,
        mutable_sources: &[MutableSourceIdentity],
    ) -> Result<bool, OrchestrationError> {
        self.ensure_enabled()?;
        self.ensure_setup()?;
        let head_changed = self
            .adapter
            .recheck_continuation(package_base, observed_head_oid)
            .map_err(OrchestrationError::Observation)?;
        let source_changed = self
            .adapter
            .recheck_mutable_sources(mutable_sources)
            .map_err(OrchestrationError::Observation)?;
        Ok(head_changed || source_changed)
    }

    /// Replace typed update candidates for the next observation cycle.
    pub fn set_update_candidates(&mut self, candidates: Vec<UpdateCandidate>) {
        self.adapter.set_update_candidates(candidates);
    }

    /// What: Apply runtime consent and pause controls through the central durable owner.
    ///
    /// Inputs:
    /// - Optional consent replacement, user-pause replacement, service pause request, and
    ///   optional successful service validation.
    ///
    /// Output:
    /// - Persisted policy state.
    ///
    /// Details:
    /// - Clearing the service pause requires an explicit successful validation.
    ///
    /// # Errors
    /// - Returns when persistence fails.
    pub fn update_runtime_policy(
        &mut self,
        consent: Option<PiScanConsentState>,
        user_paused: Option<bool>,
        pause_for_service: bool,
        service_validation: Option<bool>,
    ) -> Result<(), OrchestrationError> {
        let consent_changed = consent.is_some();
        if let Some(consent) = consent {
            self.state.runtime.set_consent(consent);
        }
        if let Some(paused) = user_paused {
            self.state.runtime.set_user_paused(paused);
        }
        if pause_for_service {
            self.state.runtime.pause_for_service();
        }
        if let Some(validation_succeeded) = service_validation {
            self.state.runtime.clear_service_pause(validation_succeeded);
        }
        self.persist()?;
        if consent_changed {
            self.persist_consent()?;
        }
        Ok(())
    }

    /// What: Execute at most one queued target through the adapter.
    ///
    /// Inputs:
    /// - `now_unix`: Start/terminal accounting timestamp.
    /// - `cancelled`: Exact sticky cancellation registration for this active call.
    ///
    /// Output:
    /// - Canonical validated receipt, or `None` when idle/dry-run.
    ///
    /// Details:
    /// - Canonical WS7 persistence completes before WS3 completion is accepted. Any error
    ///   terminalizes the exact active item with full reservation and continues later work.
    ///
    /// # Errors
    /// - Returns explicit pause, cancellation, execution, identity, accounting, or persistence errors.
    pub fn run_next(
        &mut self,
        now_unix: u64,
        cancelled: &AtomicBool,
    ) -> Result<Option<ExecutionReceipt>, OrchestrationError> {
        if self.config.dry_run {
            return Ok(None);
        }
        let shutdown_requested = AtomicBool::new(false);
        self.run_next_registered(now_unix, cancelled, &shutdown_requested, |_| {}, |_| {})
    }

    /// Execute one item while publishing its active correlation and transient phases.
    fn run_next_registered<F, P>(
        &mut self,
        now_unix: u64,
        cancelled: &AtomicBool,
        shutdown_requested: &AtomicBool,
        register: F,
        publish_phase: P,
    ) -> Result<Option<ExecutionReceipt>, OrchestrationError>
    where
        F: FnOnce(&crate::state::pi_scan::PiScanActiveItem),
        P: Fn(PiScanExecutionProgress),
    {
        if self.config.dry_run {
            return Ok(None);
        }
        self.ensure_enabled()?;
        self.ensure_setup()?;
        let has_foreground = self
            .state
            .runtime
            .queue
            .iter()
            .any(|request| request.priority == PiScanPriority::Foreground);
        if !self.config.background_execution && !has_foreground {
            return Err(OrchestrationError::Paused(
                "unattended Pi execution is disabled; enable it explicitly or queue manual foreground work"
                    .to_string(),
            ));
        }
        let active = self
            .state
            .runtime
            .start_next(now_unix, true)
            .map_err(start_block_error)?;
        let Some(active) = active else {
            return Ok(None);
        };
        self.persist()?;
        register(&active);
        let progress = PiScanExecutionPhaseReporter::new(active.correlation_id, &publish_phase);
        progress.report(PiScanExecutionPhase::Preparing);
        let Some(target) = self.state.targets.get(&active.request.request_id).cloned() else {
            self.fail_active(active.correlation_id, now_unix, true)?;
            return Err(OrchestrationError::InvalidTarget(
                "the durable queue item has no matching full frozen identity; scanner paused for recovery"
                    .to_string(),
            ));
        };
        match self
            .adapter
            .execute_with_progress(&target, cancelled, &progress)
        {
            Ok(receipt) => {
                progress.report(PiScanExecutionPhase::ValidatingResult);
                self.accept_execution(active.correlation_id, &target, receipt, now_unix, &progress)
            }
            Err(ExecutionFailure::Cancelled) => {
                self.state
                    .runtime
                    .cancel_active(active.correlation_id, now_unix)
                    .map_err(|error| OrchestrationError::Execution(error.to_string()))?;
                if shutdown_requested.load(Ordering::SeqCst)
                    && let Some(record) = self.state.runtime.terminal.last_mut()
                    && record.correlation_id == active.correlation_id
                {
                    record.status = PiScanTerminalStatus::Interrupted;
                }
                self.persist()?;
                Err(OrchestrationError::Cancelled)
            }
            Err(ExecutionFailure::Service(reason)) => {
                self.fail_active(active.correlation_id, now_unix, true)?;
                Err(OrchestrationError::Execution(reason))
            }
        }
    }

    /// Abort/recover any active item and persist the durability boundary for shutdown.
    ///
    /// # Errors
    /// - Returns recovery-accounting or persistence failures.
    pub fn shutdown(&mut self, now_unix: u64) -> Result<(), OrchestrationError> {
        if self.config.dry_run {
            return Ok(());
        }
        self.state
            .runtime
            .recover_interrupted(now_unix)
            .map_err(|error| OrchestrationError::Persistence(error.to_string()))?;
        self.persist()
    }

    /// Observe packages sequentially, deduplicate split bases, ledger every commit, and queue scans.
    fn observe(
        &mut self,
        now_unix: u64,
        manual: bool,
        selected_package_names: Option<&BTreeSet<String>>,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        self.ensure_enabled()?;
        let setup = if self.config.dry_run {
            let setup = self
                .adapter
                .dry_run_setup()
                .map_err(OrchestrationError::Readiness)?;
            setup.validate()?;
            setup
        } else {
            self.ensure_setup()?.clone()
        };
        let enumerated = if let Some(package_names) = selected_package_names {
            self.adapter.enumerate_selected(package_names)
        } else {
            self.adapter.enumerate_foreign()
        }
        .map_err(OrchestrationError::Observation)?;
        let packages = deduplicate_packages(enumerated);
        let mut discovered = Vec::new();
        for package in packages {
            let cursor = self.state.cursors.get(package.package_base.as_str());
            let observation = self
                .adapter
                .observe_package(&package, cursor)
                .map_err(OrchestrationError::Observation)?;
            if observation.package_base != package.package_base {
                return Err(OrchestrationError::Observation(
                    "the observer returned a different package base; scanner paused without advancing its cursor"
                        .to_string(),
                ));
            }
            if observation.paused_for_rebaseline {
                self.state.runtime.pause_for_service();
                self.persist()?;
                continue;
            }
            let has_baseline = self
                .baselines
                .entries
                .contains_key(package.package_base.as_str());
            for commit in observation.commits {
                if self.ledger_contains(&package.package_base, &commit.oid) {
                    continue;
                }
                let is_current_head = commit.oid == observation.head_oid;
                let baseline_commit = ObservationCommit {
                    oid: commit.oid.clone(),
                    relevance: CommitBuildRelevance::BuildRelevant,
                };
                let target = if has_baseline || is_current_head {
                    self.target_for_commit(
                        &package,
                        &observation.head_oid,
                        if has_baseline {
                            &commit
                        } else {
                            &baseline_commit
                        },
                        &setup,
                        now_unix,
                        manual,
                    )?
                } else {
                    None
                };
                if !self.config.dry_run {
                    self.insert_observed(
                        &package.package_base,
                        &commit,
                        target.as_ref(),
                        now_unix,
                    )?;
                }
                if let Some(target) = target {
                    if self.config.dry_run {
                        self.state
                            .targets
                            .insert(self.state.next_request_id, target.clone());
                    }
                    discovered.push(target);
                }
            }
        }
        if !self.config.dry_run {
            self.state.last_observation_unix = Some(now_unix);
            self.persist()?;
        }
        Ok(discovered)
    }

    /// Ensure feature and explicit setup confirmation gates are active.
    fn ensure_enabled(&self) -> Result<(), OrchestrationError> {
        if !self.config.enabled
            || !(self.config.dry_run
                || self.config.setup_confirmed
                || self.state.setup_consent.disclosure_confirmed)
        {
            return Err(OrchestrationError::Disabled(
                "Pi scanning is disabled until the feature and setup disclosure are explicitly confirmed"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Probe no-model setup once and validate exact model/pricing identity.
    fn ensure_setup(&mut self) -> Result<&SetupSnapshot, OrchestrationError> {
        if self.setup.is_none() {
            let setup = self
                .adapter
                .probe_setup()
                .map_err(OrchestrationError::Readiness)?;
            setup.validate()?;
            self.setup = Some(setup);
        }
        self.setup.as_ref().ok_or_else(|| {
            OrchestrationError::Readiness(
                "Pi setup facts were not retained; re-run scanner setup".to_string(),
            )
        })
    }

    /// Build one exact frozen target only for build-relevant or uncertain commits.
    fn target_for_commit(
        &mut self,
        package: &DiscoveredPackage,
        head_oid: &CommitOid,
        commit: &ObservationCommit,
        setup: &SetupSnapshot,
        now_unix: u64,
        manual: bool,
    ) -> Result<Option<FrozenScanIdentity>, OrchestrationError> {
        if commit.relevance == CommitBuildRelevance::ObservedNoRecipeDelta {
            return Ok(None);
        }
        let request_id = self.allocate_request_id()?;
        let priority = if manual {
            PiScanPriority::Foreground
        } else {
            PiScanPriority::Background
        };
        let package_name = package.installed_names.first().cloned().ok_or_else(|| {
            OrchestrationError::InvalidTarget(
                "an observed package base has no installed package name".to_string(),
            )
        })?;
        let target = FrozenScanIdentity {
            scan_id: format!("scan-{now_unix}-{request_id}"),
            package_name,
            package_base: package.package_base.clone(),
            installed_names: package.installed_names.clone(),
            installed_version: package.installed_version.clone(),
            candidate_version: package.candidate_version.clone(),
            commit_oid: commit.oid.clone(),
            observed_head_oid: head_oid.clone(),
            cycle_id: format!("cycle-{now_unix}"),
            provider: setup.selected_provider.clone(),
            model: setup.selected_model.clone(),
            reservation: setup.reservation,
            priority,
        };
        target.validate()?;
        Ok(Some(target))
    }

    /// Durably insert one ledger entry, optional queue item, and resumable cursor.
    fn insert_observed(
        &mut self,
        package_base: &PackageBase,
        commit: &ObservationCommit,
        target: Option<&FrozenScanIdentity>,
        now_unix: u64,
    ) -> Result<(), OrchestrationError> {
        self.state.ledger.push(OrchestrationLedgerEntry {
            key: PiScanQueueKey {
                package_base: package_base.clone(),
                commit_oid: commit.oid.clone(),
            },
            relevance: commit.relevance,
            observed_at_unix: now_unix,
        });
        self.state
            .cursors
            .insert(package_base.as_str().to_string(), commit.oid.clone());
        if let Some(target) = target {
            let request_id = self.state.next_request_id;
            self.state
                .runtime
                .enqueue(target.queue_request(request_id))
                .map_err(|error| OrchestrationError::InvalidTarget(error.to_string()))?;
            self.state.targets.insert(request_id, target.clone());
        }
        self.persist()
    }

    /// Accept only exact canonical result identity, persist it, then reconcile the queue reservation.
    fn accept_execution(
        &mut self,
        correlation_id: u64,
        target: &FrozenScanIdentity,
        receipt: ExecutionReceipt,
        now_unix: u64,
        progress: &PiScanExecutionPhaseReporter<'_>,
    ) -> Result<Option<ExecutionReceipt>, OrchestrationError> {
        let exact = receipt.result.identity.package_base == target.package_base.as_str()
            && receipt.result.identity.commit_oid == target.commit_oid.as_str()
            && receipt.result.identity.scan_id == target.scan_id;
        if !exact {
            self.fail_active(correlation_id, now_unix, true)?;
            return Err(OrchestrationError::Execution(
                "validated result identity did not match the full frozen target; result was not persisted"
                    .to_string(),
            ));
        }
        progress.report(PiScanExecutionPhase::Finalizing);
        let mut document = StoredScanResult::from_validated_with_staleness(
            &target.scan_id,
            &receipt.result,
            &receipt.provenance,
            &receipt.manifests,
            now_unix,
            false,
            receipt.observed_head_oid.as_str(),
            receipt.stale,
        )
        .map_err(|error| OrchestrationError::Execution(error.to_string()))?;
        document
            .mutable_sources
            .clone_from(&receipt.mutable_sources);
        save_result_atomic(&self.config.results_root, &document)
            .map_err(|error| OrchestrationError::Execution(error.to_string()))?;
        self.state
            .runtime
            .complete(
                correlation_id,
                &PiScanQueueKey {
                    package_base: target.package_base.clone(),
                    commit_oid: target.commit_oid.clone(),
                },
                receipt.usage,
                now_unix,
            )
            .map_err(|error| OrchestrationError::Execution(error.to_string()))?;
        self.persist()?;
        Ok(Some(receipt))
    }

    /// Terminalize one exact active item as failed with full reservation consumption.
    fn fail_active(
        &mut self,
        correlation_id: u64,
        now_unix: u64,
        pause_service: bool,
    ) -> Result<(), OrchestrationError> {
        let _record = self
            .state
            .runtime
            .cancel_active(correlation_id, now_unix)
            .map_err(|error| OrchestrationError::Execution(error.to_string()))?;
        if let Some(last) = self.state.runtime.terminal.last_mut() {
            last.status = PiScanTerminalStatus::Failed;
        }
        if pause_service {
            self.state.runtime.pause_for_service();
        }
        self.persist()
    }

    /// Return whether an exact package-base/commit pair is already durably ledgered.
    fn ledger_contains(&self, package_base: &PackageBase, commit_oid: &CommitOid) -> bool {
        self.state.ledger.iter().any(|entry| {
            &entry.key.package_base == package_base && &entry.key.commit_oid == commit_oid
        })
    }

    /// Allocate one monotonic request id without wrapping.
    fn allocate_request_id(&mut self) -> Result<u64, OrchestrationError> {
        let next = self.state.next_request_id.checked_add(1).ok_or_else(|| {
            OrchestrationError::Persistence(
                "Pi scan request id space is exhausted; preserve state and report this failure"
                    .to_string(),
            )
        })?;
        self.state.next_request_id = next;
        Ok(next)
    }

    /// Persist runtime/setup consent independently unless dry-run is active.
    fn persist_consent(&self) -> Result<(), OrchestrationError> {
        if self.config.dry_run {
            return Ok(());
        }
        save_consent_document(&self.config, &self.state)
    }

    /// Persist the single-owner state unless dry-run is active.
    fn persist(&self) -> Result<(), OrchestrationError> {
        if self.config.dry_run {
            return Ok(());
        }
        save_state(&self.config.state_path, &self.state)
    }
}

/// Exact active cancellation registration shared with UI/channel owners.
struct ActiveCancellation {
    /// Exact active queue item published before acquisition starts.
    item: crate::state::pi_scan::PiScanActiveItem,
    /// Sticky cancellation flag consumed by WS6.
    cancelled: Arc<AtomicBool>,
    /// Distinguishes shutdown interruption from explicit user cancellation.
    shutdown_requested: Arc<AtomicBool>,
}

/// What: Persist queued user pause changes before releasing the active registration.
///
/// Inputs:
/// - `orchestrator`: Locked durable owner immediately after one execution returns.
/// - `active`: Registered active slot whose release permits the next start.
/// - `pending`: FIFO pause requests accepted while execution held the owner lock.
///
/// Output:
/// - Completion is delivered to every queued requester after its persistence attempt.
///
/// Details:
/// - Lock ordering matches [`PiScanSequentialRunner::queue_user_pause_if_active`]. A poisoned
///   pending queue leaves the active slot clear but cannot silently report persistence success.
fn persist_pending_user_pauses<A: OrchestrationAdapter>(
    orchestrator: &mut PiScanOrchestrator<A>,
    active: &Arc<Mutex<Option<ActiveCancellation>>>,
    pending: &Arc<Mutex<VecDeque<PendingUserPause>>>,
) {
    let Ok(mut active_slot) = active.lock() else {
        return;
    };
    if let Ok(mut requests) = pending.lock() {
        while let Some(request) = requests.pop_front() {
            let result =
                orchestrator.update_runtime_policy(None, Some(request.paused), false, None);
            drop(request.completion.send(result));
        }
    }
    *active_slot = None;
}

/// Completion receiver for one pause mutation queued behind active execution.
pub type PiScanQueuedPolicyCompletion =
    tokio::sync::oneshot::Receiver<Result<(), OrchestrationError>>;

/// Correlation and completion pair returned for one queued user-pause request.
pub type PiScanQueuedUserPause = (u64, PiScanQueuedPolicyCompletion);

/// What: Async-runtime facade for one blocking single-owner orchestrator.
///
/// Inputs:
/// - A fully configured [`PiScanOrchestrator`] whose adapter may perform blocking Git/process I/O.
///
/// Output:
/// - Bounded off-UI observation/execution plus exact cancellation and shutdown methods.
///
/// Details:
/// - A mutex serializes ownership; every blocking operation runs through `spawn_blocking`.
/// - Cancellation is registered only after WS3 allocates the active correlation and before
///   acquisition/execution starts.
pub struct PiScanSequentialRunner<A> {
    /// Serialized central owner.
    orchestrator: Arc<Mutex<PiScanOrchestrator<A>>>,
    /// Exact currently active correlation and sticky flag.
    active: Arc<Mutex<Option<ActiveCancellation>>>,
    /// User pause changes queued while one execution owns the orchestrator lock.
    pending_user_pauses: Arc<Mutex<VecDeque<PendingUserPause>>>,
}

/// One user pause mutation waiting for the active execution's durability boundary.
struct PendingUserPause {
    /// Requested durable user-pause value.
    paused: bool,
    /// Completion delivered after persistence under the orchestrator lock.
    completion: tokio::sync::oneshot::Sender<Result<(), OrchestrationError>>,
}

impl<A> Clone for PiScanSequentialRunner<A> {
    fn clone(&self) -> Self {
        Self {
            orchestrator: Arc::clone(&self.orchestrator),
            active: Arc::clone(&self.active),
            pending_user_pauses: Arc::clone(&self.pending_user_pauses),
        }
    }
}

impl<A: OrchestrationAdapter + Send + 'static> PiScanSequentialRunner<A> {
    /// What: Wrap one orchestrator for bounded async-runtime use.
    ///
    /// Inputs:
    /// - `orchestrator`: Single state owner and adapter.
    ///
    /// Output:
    /// - Cloneable runner handle sharing one serialized owner.
    ///
    /// Details:
    /// - Cloning the handle never clones state or the adapter.
    #[must_use]
    pub fn new(orchestrator: PiScanOrchestrator<A>) -> Self {
        Self {
            orchestrator: Arc::new(Mutex::new(orchestrator)),
            active: Arc::new(Mutex::new(None)),
            pending_user_pauses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Return the exact active correlation registered before external execution.
    #[must_use]
    pub fn active_correlation(&self) -> Option<u64> {
        self.active
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|item| item.item.correlation_id))
    }

    /// Return the exact active queue item registered before acquisition starts.
    #[must_use]
    pub fn active_item(&self) -> Option<crate::state::pi_scan::PiScanActiveItem> {
        self.active
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|item| item.item.clone()))
    }

    /// What: Queue a user pause mutation when execution currently owns the orchestrator lock.
    ///
    /// Inputs:
    /// - `paused`: Durable user-pause value requested by the foreground UI action.
    ///
    /// Output:
    /// - A completion receiver when queued, or `None` when no execution is active.
    ///
    /// Details:
    /// - The active slot and pending queue use one lock order so completion cannot race past a
    ///   request. Queued changes persist before the active slot is released for another start.
    ///
    /// # Errors
    /// - Returns when either synchronization boundary is poisoned.
    pub fn queue_user_pause_if_active(
        &self,
        paused: bool,
    ) -> Result<Option<PiScanQueuedUserPause>, OrchestrationError> {
        let active = self.active.lock().map_err(|_| {
            OrchestrationError::Persistence(
                "Pi active-policy lock is poisoned; restart Pacsea to recover".to_string(),
            )
        })?;
        let Some(active_correlation) = active
            .as_ref()
            .map(|registered| registered.item.correlation_id)
        else {
            return Ok(None);
        };
        let mut pending = self.pending_user_pauses.lock().map_err(|_| {
            OrchestrationError::Persistence(
                "Pi queued-policy lock is poisoned; restart Pacsea to recover".to_string(),
            )
        })?;
        let (completion, receiver) = tokio::sync::oneshot::channel();
        pending.push_back(PendingUserPause { paused, completion });
        drop(pending);
        drop(active);
        Ok(Some((active_correlation, receiver)))
    }

    /// What: Cancel only the exact registered active correlation.
    ///
    /// Inputs:
    /// - `correlation_id`: Correlation supplied by the runtime/UI projection.
    ///
    /// Output:
    /// - `true` only when the matching sticky flag was set.
    ///
    /// Details:
    /// - A stale correlation has no effect on current work.
    #[must_use]
    pub fn cancel(&self, correlation_id: u64) -> bool {
        let Ok(active) = self.active.lock() else {
            return false;
        };
        let Some(active) = active.as_ref() else {
            return false;
        };
        if active.item.correlation_id != correlation_id {
            return false;
        }
        active.cancelled.store(true, Ordering::SeqCst);
        true
    }

    /// Run startup observation away from the async UI thread.
    ///
    /// # Errors
    /// - Returns orchestration or blocking-task failures.
    pub async fn startup_observation(
        &self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        self.observe_off_thread(now_unix, ObservationTrigger::Startup)
            .await
    }

    /// Run typed update-candidate observation away from the async UI thread.
    ///
    /// # Errors
    /// - Returns orchestration or blocking-task failures.
    pub async fn update_candidate_observation(
        &self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during update observation"
                            .to_string(),
                    )
                })?
                .update_candidate_observation(now_unix)
        })
        .await
        .map_err(|error| {
            OrchestrationError::Observation(format!("Pi update observation task failed: {error}"))
        })?
    }

    /// Run explicit manual observation away from the async UI thread.
    ///
    /// # Errors
    /// - Returns orchestration or blocking-task failures.
    pub async fn manual_observation(
        &self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        self.observe_off_thread(now_unix, ObservationTrigger::Manual)
            .await
    }

    /// Observe only explicitly selected unresolved package names away from the async UI thread.
    ///
    /// # Errors
    /// - Returns selection, orchestration, or blocking-task failures.
    pub async fn manual_observation_selected(
        &self,
        now_unix: u64,
        package_names: Vec<String>,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        let package_names: BTreeSet<String> = package_names.into_iter().collect();
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Observation(
                        "Pi orchestration owner lock is poisoned; restart Pacsea to recover"
                            .to_string(),
                    )
                })?
                .manual_observation_selected(now_unix, &package_names)
        })
        .await
        .map_err(|error| {
            OrchestrationError::Observation(format!(
                "Pi selected-target observation task failed: {error}; retry observation"
            ))
        })?
    }

    /// Perform acquisition-only dry-run away from the async UI thread.
    ///
    /// # Errors
    /// - Returns target, acquisition, lock, or blocking-task failures.
    pub async fn dry_run_acquisition(
        &self,
        key: PiScanQueueKey,
    ) -> Result<DryRunAcquisitionReceipt, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during dry-run acquisition"
                            .to_string(),
                    )
                })?
                .dry_run_acquisition(&key)
        })
        .await
        .map_err(|error| {
            OrchestrationError::Execution(format!("Pi dry-run acquisition task failed: {error}"))
        })?
    }

    /// Run a due periodic observation away from the async UI thread.
    ///
    /// # Errors
    /// - Returns orchestration or blocking-task failures when due.
    pub async fn periodic_observation(
        &self,
        now_unix: u64,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        self.observe_off_thread(now_unix, ObservationTrigger::Periodic)
            .await
    }

    /// Promote one observed target to foreground priority away from the async UI thread.
    ///
    /// # Errors
    /// - Returns target, setup, persistence, lock, or blocking-task failures.
    pub async fn promote_queued(&self, key: PiScanQueueKey) -> Result<u64, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during queue promotion"
                            .to_string(),
                    )
                })?
                .promote_queued(&key)
        })
        .await
        .map_err(|error| {
            OrchestrationError::Persistence(format!("Pi queue promotion task failed: {error}"))
        })?
    }

    /// Accept one complete current-HEAD result as baseline away from the async UI thread.
    ///
    /// # Errors
    /// - Returns validation, persistence, lock, or blocking-task failures.
    pub async fn accept_baseline(
        &self,
        package_base: PackageBase,
        commit_oid: CommitOid,
        scan_id: String,
        evidence_fingerprint: String,
        now_unix: u64,
    ) -> Result<(), OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during baseline acceptance"
                            .to_string(),
                    )
                })?
                .accept_baseline(
                    &package_base,
                    &commit_oid,
                    &scan_id,
                    &evidence_fingerprint,
                    now_unix,
                )
        })
        .await
        .map_err(|error| {
            OrchestrationError::Persistence(format!("Pi baseline acceptance task failed: {error}"))
        })?
    }

    /// Re-resolve one result identity away from the async UI thread.
    ///
    /// # Errors
    /// - Returns setup, observation, lock, or blocking-task failures.
    pub async fn validate_continuation(
        &self,
        package_base: PackageBase,
        observed_head_oid: CommitOid,
    ) -> Result<bool, OrchestrationError> {
        self.validate_continuation_with_sources(package_base, observed_head_oid, Vec::new())
            .await
    }

    /// Re-resolve official AUR HEAD and mutable source refs away from the async UI thread.
    ///
    /// # Errors
    /// - Returns setup, observation, lock, or blocking-task failures.
    pub async fn validate_continuation_with_sources(
        &self,
        package_base: PackageBase,
        observed_head_oid: CommitOid,
        mutable_sources: Vec<MutableSourceIdentity>,
    ) -> Result<bool, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during continuation recheck"
                            .to_string(),
                    )
                })?
                .validate_continuation_with_sources(
                    &package_base,
                    &observed_head_oid,
                    &mutable_sources,
                )
        })
        .await
        .map_err(|error| {
            OrchestrationError::Observation(format!("Pi continuation recheck task failed: {error}"))
        })?
    }

    /// Replace typed update candidates away from the async UI thread.
    ///
    /// # Errors
    /// - Returns lock or blocking-task failures.
    pub async fn set_update_candidates(
        &self,
        candidates: Vec<UpdateCandidate>,
    ) -> Result<(), OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during update-candidate refresh"
                            .to_string(),
                    )
                })?
                .set_update_candidates(candidates);
            Ok(())
        })
        .await
        .map_err(|error| {
            OrchestrationError::Observation(format!(
                "Pi update-candidate refresh task failed: {error}"
            ))
        })?
    }

    /// Read the durable runtime/target projection away from the async UI thread.
    ///
    /// # Errors
    /// - Returns lock or blocking-task failures.
    pub async fn state_snapshot(&self) -> Result<OrchestrationState, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during runtime restore"
                            .to_string(),
                    )
                })
                .map(|owner| owner.state().clone())
        })
        .await
        .map_err(|error| {
            OrchestrationError::Persistence(format!("Pi runtime restore task failed: {error}"))
        })?
    }

    /// Probe/cache no-model setup facts away from the async UI thread.
    ///
    /// # Errors
    /// - Returns disabled, readiness, lock, or blocking-task failures.
    pub async fn setup_snapshot(&self) -> Result<SetupSnapshot, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during setup verification"
                            .to_string(),
                    )
                })?
                .setup_snapshot()
        })
        .await
        .map_err(|error| {
            OrchestrationError::Readiness(format!("Pi setup verification task failed: {error}"))
        })?
    }

    /// Read durable runtime/setup consent away from the async UI thread.
    ///
    /// # Errors
    /// - Returns lock or blocking-task failures.
    pub async fn consent_snapshot(
        &self,
    ) -> Result<(PiScanConsentState, PiScanSetupConsentState), OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during consent restore"
                            .to_string(),
                    )
                })
                .map(|owner| owner.consent_snapshot())
        })
        .await
        .map_err(|error| {
            OrchestrationError::Persistence(format!("Pi consent restore task failed: {error}"))
        })?
    }

    /// Persist setup confirmations away from the async UI thread.
    ///
    /// # Errors
    /// - Returns persistence, lock, or blocking-task failures.
    pub async fn update_setup_consent(
        &self,
        setup: PiScanSetupConsentState,
    ) -> Result<(), OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during setup-consent update"
                            .to_string(),
                    )
                })?
                .update_setup_consent(setup)
        })
        .await
        .map_err(|error| {
            OrchestrationError::Persistence(format!("Pi setup-consent update task failed: {error}"))
        })?
    }

    /// Apply consent and pause controls away from the async UI thread.
    ///
    /// # Errors
    /// - Returns persistence, lock, or blocking-task failures.
    pub async fn update_runtime_policy(
        &self,
        consent: Option<PiScanConsentState>,
        user_paused: Option<bool>,
        pause_for_service: bool,
        service_validation: Option<bool>,
    ) -> Result<(), OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during policy update".to_string(),
                    )
                })?
                .update_runtime_policy(consent, user_paused, pause_for_service, service_validation)
        })
        .await
        .map_err(|error| {
            OrchestrationError::Persistence(format!("Pi policy update task failed: {error}"))
        })?
    }

    /// What: Execute one item on the blocking pool with exact cancellation registration.
    ///
    /// Inputs:
    /// - `now_unix`: Start and accounting timestamp.
    ///
    /// Output:
    /// - Canonical receipt, idle `None`, or a fail-closed error.
    ///
    /// Details:
    /// - The active registration is removed only when the matching call returns.
    ///
    /// # Errors
    /// - Returns queue, acquisition, execution, cancellation, persistence, or join failures.
    pub async fn run_next(
        &self,
        now_unix: u64,
    ) -> Result<Option<ExecutionReceipt>, OrchestrationError> {
        self.run_next_with_optional_progress(now_unix, None, None)
            .await
    }

    /// What: Execute one item and publish Started from the exact active-registration seam.
    ///
    /// Inputs:
    /// - `now_unix`: Start and accounting timestamp.
    /// - `started_tx`: Typed channel receiving the registered active item.
    ///
    /// Output:
    /// - Canonical receipt, idle `None`, or a fail-closed error.
    ///
    /// Details:
    /// - Registration is sent before adapter execution, so an instantly completing fake or real
    ///   run cannot outrun its Started projection.
    ///
    /// # Errors
    /// - Returns queue, acquisition, execution, cancellation, persistence, or join failures.
    pub async fn run_next_with_started(
        &self,
        now_unix: u64,
        started_tx: tokio::sync::mpsc::UnboundedSender<crate::state::pi_scan::PiScanActiveItem>,
    ) -> Result<Option<ExecutionReceipt>, OrchestrationError> {
        self.run_next_with_optional_progress(now_unix, Some(started_tx), None)
            .await
    }

    /// What: Execute one item with deterministic Started and correlation-owned phase publishers.
    ///
    /// Inputs:
    /// - `now_unix`: Start and accounting timestamp.
    /// - `started_tx`: Typed channel receiving the registered active item.
    /// - `phase_tx`: Typed channel receiving correlation-owned transient execution phases.
    ///
    /// Output:
    /// - Canonical receipt, idle `None`, or a fail-closed error.
    ///
    /// Details:
    /// - Started registration occurs before any phase is reported. Both channels are in-process and
    ///   phase receiver closure never affects scan execution or durable state.
    ///
    /// # Errors
    /// - Returns queue, acquisition, execution, cancellation, persistence, or join failures.
    pub async fn run_next_with_progress(
        &self,
        now_unix: u64,
        started_tx: tokio::sync::mpsc::UnboundedSender<crate::state::pi_scan::PiScanActiveItem>,
        phase_tx: tokio::sync::mpsc::UnboundedSender<PiScanExecutionProgress>,
    ) -> Result<Option<ExecutionReceipt>, OrchestrationError> {
        self.run_next_with_optional_progress(now_unix, Some(started_tx), Some(phase_tx))
            .await
    }

    /// Execute one registered item with optional deterministic progress publishers.
    async fn run_next_with_optional_progress(
        &self,
        now_unix: u64,
        started_tx: Option<
            tokio::sync::mpsc::UnboundedSender<crate::state::pi_scan::PiScanActiveItem>,
        >,
        phase_tx: Option<tokio::sync::mpsc::UnboundedSender<PiScanExecutionProgress>>,
    ) -> Result<Option<ExecutionReceipt>, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        let active = Arc::clone(&self.active);
        let pending_user_pauses = Arc::clone(&self.pending_user_pauses);
        tokio::task::spawn_blocking(move || {
            let cancelled = Arc::new(AtomicBool::new(false));
            let shutdown_requested = Arc::new(AtomicBool::new(false));
            let registration_flag = Arc::clone(&cancelled);
            let registration_shutdown = Arc::clone(&shutdown_requested);
            let mut orchestrator = owner.lock().map_err(|_| {
                OrchestrationError::Execution(
                    "Pi orchestration owner lock is poisoned; restart Pacsea to recover"
                        .to_string(),
                )
            })?;
            let result = orchestrator.run_next_registered(
                now_unix,
                &cancelled,
                &shutdown_requested,
                |item| {
                    if let Ok(mut slot) = active.lock() {
                        *slot = Some(ActiveCancellation {
                            item: item.clone(),
                            cancelled: registration_flag,
                            shutdown_requested: registration_shutdown,
                        });
                        if let Some(sender) = started_tx {
                            drop(sender.send(item.clone()));
                        }
                    }
                },
                |progress| {
                    if let Some(sender) = phase_tx.as_ref() {
                        let _ = sender.send(progress);
                    }
                },
            );
            persist_pending_user_pauses(&mut orchestrator, &active, &pending_user_pauses);
            drop(orchestrator);
            result
        })
        .await
        .map_err(|error| {
            OrchestrationError::Execution(format!(
                "Pi orchestration blocking task failed: {error}; retry the scan"
            ))
        })?
    }

    /// What: Cancel, await reap/terminalization, and persist within ten seconds.
    ///
    /// Inputs:
    /// - `now_unix`: Shutdown recovery timestamp.
    ///
    /// Output:
    /// - `Ok(())` after the persistence boundary.
    ///
    /// Details:
    /// - WS6 owns process abort/reap in response to the sticky flag. This method waits for
    ///   that call to return, then persists recovery state; timeout is explicit.
    ///
    /// # Errors
    /// - Returns timeout, recovery, persistence, lock, or blocking-task failures.
    pub async fn shutdown(&self, now_unix: u64) -> Result<(), OrchestrationError> {
        if let Ok(active) = self.active.lock()
            && let Some(active) = active.as_ref()
        {
            active.shutdown_requested.store(true, Ordering::SeqCst);
            active.cancelled.store(true, Ordering::SeqCst);
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.active_correlation().is_some() {
            if Instant::now() >= deadline {
                return Err(OrchestrationError::Persistence(
                    "Pi scanner shutdown exceeded ten seconds before abort/reap completed; recovery state remains conservative"
                        .to_string(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            owner
                .lock()
                .map_err(|_| {
                    OrchestrationError::Persistence(
                        "Pi orchestration owner lock is poisoned during shutdown".to_string(),
                    )
                })?
                .shutdown(now_unix)
        })
        .await
        .map_err(|error| {
            OrchestrationError::Persistence(format!(
                "Pi orchestration shutdown task failed: {error}"
            ))
        })?
    }

    /// Run one observation trigger through the serialized blocking owner.
    async fn observe_off_thread(
        &self,
        now_unix: u64,
        trigger: ObservationTrigger,
    ) -> Result<Vec<FrozenScanIdentity>, OrchestrationError> {
        let owner = Arc::clone(&self.orchestrator);
        tokio::task::spawn_blocking(move || {
            let mut orchestrator = owner.lock().map_err(|_| {
                OrchestrationError::Observation(
                    "Pi orchestration owner lock is poisoned; restart Pacsea to recover"
                        .to_string(),
                )
            })?;
            match trigger {
                ObservationTrigger::Startup => orchestrator.startup_observation(now_unix),
                ObservationTrigger::Manual => orchestrator.manual_observation(now_unix),
                ObservationTrigger::Periodic => orchestrator.periodic_observation(now_unix),
            }
        })
        .await
        .map_err(|error| {
            OrchestrationError::Observation(format!(
                "Pi observation blocking task failed: {error}; retry observation"
            ))
        })?
    }
}

/// Observation entry point selected by the async facade.
#[derive(Debug, Clone, Copy)]
enum ObservationTrigger {
    /// Startup cycle.
    Startup,
    /// Explicit manual cycle.
    Manual,
    /// Due periodic cycle.
    Periodic,
}

/// Deduplicate package bases while retaining every split-package name in first-seen order.
fn deduplicate_packages(packages: Vec<DiscoveredPackage>) -> Vec<DiscoveredPackage> {
    let mut grouped: Vec<DiscoveredPackage> = Vec::new();
    for package in packages {
        if let Some(existing) = grouped
            .iter_mut()
            .find(|item| item.package_base == package.package_base)
        {
            let mut seen: BTreeSet<String> = existing.installed_names.iter().cloned().collect();
            for name in package.installed_names {
                if seen.insert(name.clone()) {
                    existing.installed_names.push(name);
                }
            }
        } else {
            grouped.push(package);
        }
    }
    grouped
}

/// Convert one WS3 start block into actionable orchestrator state.
fn start_block_error(block: PiScanStartBlock) -> OrchestrationError {
    let reason = match block {
        PiScanStartBlock::RuntimeDisabled => "Pi scanning is disabled".to_string(),
        PiScanStartBlock::PaidExecutionNotConsented => {
            "paid Pi execution is not consented".to_string()
        }
        PiScanStartBlock::Paused(pause) => format!("Pi scanning is paused by {pause:?} policy"),
        PiScanStartBlock::Budget => {
            "the next Pi scan does not fit its exact rolling token/cost reservation".to_string()
        }
        PiScanStartBlock::CorrelationExhausted => {
            "Pi scan correlation id space is exhausted".to_string()
        }
    };
    OrchestrationError::Paused(reason)
}

/// Reset all consent when material provider/model/privacy/pricing configuration changes.
fn reset_consent_for_binding(state: &mut OrchestrationState, binding: &str) {
    state.runtime.set_consent(PiScanConsentState::default());
    state.setup_consent = PiScanSetupConsentState {
        configuration_binding: binding.to_string(),
        ..PiScanSetupConsentState::default()
    };
}

/// Persist the independent versioned consent document with private atomic permissions.
fn save_consent_document(
    config: &OrchestrationConfig,
    state: &OrchestrationState,
) -> Result<(), OrchestrationError> {
    let document = PersistedConsentDocument {
        schema_version: CONSENT_SCHEMA_VERSION,
        configuration_binding: config.consent_binding.clone(),
        runtime: state.runtime.consent,
        setup: state.setup_consent.clone(),
    };
    save_versioned_state_atomic(&config.consent_path, &document)
        .map_err(|error| OrchestrationError::Persistence(error.to_string()))
}

/// Return current Unix seconds, using zero only for a clock before the epoch.
fn current_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// Load missing state as empty while rejecting malformed and newer documents.
fn load_state(
    path: &Path,
    quarantine_dir: &Path,
) -> Result<OrchestrationState, OrchestrationError> {
    let unavailable_marker = quarantine_dir.join("orchestration-unavailable-v1");
    if !path.exists() {
        if unavailable_marker.exists() {
            return Err(OrchestrationError::Persistence(format!(
                "Pi orchestration state remains unavailable after quarantine at {}; restore or explicitly reset scanner state before retrying",
                quarantine_dir.display()
            )));
        }
        return Ok(OrchestrationState::default());
    }
    let bytes = fs::read(path).map_err(|error| {
        OrchestrationError::Persistence(format!(
            "could not read Pi orchestration state at {}: {error}; fix permissions and retry",
            path.display()
        ))
    })?;
    let persisted: PersistedOrchestrationState = match serde_json::from_slice(&bytes) {
        Ok(persisted) => persisted,
        Err(error) => {
            let quarantined = quarantine_state(path, quarantine_dir)?;
            return Err(OrchestrationError::Persistence(format!(
                "Pi orchestration state at {} was corrupt ({error}) and was quarantined to {}; recover or retry explicitly",
                path.display(),
                quarantined.display()
            )));
        }
    };
    if persisted.schema_version != ORCHESTRATION_SCHEMA_VERSION {
        let observed = persisted.schema_version;
        let quarantined = quarantine_state(path, quarantine_dir)?;
        return Err(OrchestrationError::Persistence(format!(
            "Pi orchestration state schema {observed} is unsupported by schema {}; it was quarantined to {}; update Pacsea or recover explicitly",
            ORCHESTRATION_SCHEMA_VERSION,
            quarantined.display()
        )));
    }
    if unavailable_marker.exists() {
        fs::remove_file(&unavailable_marker).map_err(|error| {
            OrchestrationError::Persistence(format!(
                "valid Pi orchestration state was restored, but recovery marker {} could not be cleared: {error}",
                unavailable_marker.display()
            ))
        })?;
    }
    Ok(persisted.state)
}

/// Move one corrupt/newer orchestration document into a private quarantine directory.
fn quarantine_state(path: &Path, quarantine_dir: &Path) -> Result<PathBuf, OrchestrationError> {
    fs::create_dir_all(quarantine_dir).map_err(|error| {
        OrchestrationError::Persistence(format!(
            "could not create Pi orchestration quarantine {}: {error}",
            quarantine_dir.display()
        ))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(quarantine_dir, fs::Permissions::from_mode(0o700)).map_err(
            |error| {
                OrchestrationError::Persistence(format!(
                    "could not secure Pi orchestration quarantine {}: {error}",
                    quarantine_dir.display()
                ))
            },
        )?;
    }
    write_unavailable_marker(quarantine_dir)?;
    for suffix in 0..100u32 {
        let target = quarantine_dir.join(format!("orchestration-{}-{suffix}.json", current_unix()));
        if target.exists() {
            continue;
        }
        fs::rename(path, &target).map_err(|error| {
            OrchestrationError::Persistence(format!(
                "could not quarantine Pi orchestration state {} to {}: {error}; original was left in place",
                path.display(),
                target.display()
            ))
        })?;
        return Ok(target);
    }
    Err(OrchestrationError::Persistence(
        "Pi orchestration quarantine name space was exhausted; original was left in place"
            .to_string(),
    ))
}

/// Persist an unresolved-quarantine marker before moving authoritative state.
fn write_unavailable_marker(quarantine_dir: &Path) -> Result<(), OrchestrationError> {
    let marker = quarantine_dir.join("orchestration-unavailable-v1");
    if marker.exists() {
        return Ok(());
    }
    write_private(&marker, b"schema=1\nstate=unavailable\n")
}

/// Atomically write private orchestration state beside its destination.
fn save_state(path: &Path, state: &OrchestrationState) -> Result<(), OrchestrationError> {
    let parent = path.parent().ok_or_else(|| {
        OrchestrationError::Persistence(
            "Pi orchestration state path has no parent directory".to_string(),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        OrchestrationError::Persistence(format!(
            "could not create Pi orchestration state directory {}: {error}",
            parent.display()
        ))
    })?;
    let bytes = serde_json::to_vec_pretty(&PersistedOrchestrationState {
        schema_version: ORCHESTRATION_SCHEMA_VERSION,
        state: state.clone(),
    })
    .map_err(|error| OrchestrationError::Persistence(error.to_string()))?;
    let temporary = path.with_extension("json.tmp");
    write_private(&temporary, &bytes)?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        OrchestrationError::Persistence(format!(
            "could not atomically persist Pi orchestration state at {}: {error}",
            path.display()
        ))
    })
}

/// Create/truncate one private state temporary file and fully write its bytes.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), OrchestrationError> {
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        OrchestrationError::Persistence(format!(
            "could not create private Pi orchestration state {}: {error}",
            path.display()
        ))
    })?;
    file.write_all(bytes).map_err(|error| {
        OrchestrationError::Persistence(format!(
            "could not write private Pi orchestration state {}: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        OrchestrationError::Persistence(format!(
            "could not sync private Pi orchestration state {}: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::PiScanExecutionPhaseReporter;
    use crate::state::{PiScanExecutionPhase, PiScanExecutionProgress};
    use std::cell::RefCell;

    /// The reporter preserves exact correlation and synchronous phase order.
    #[test]
    fn execution_phase_reporter_binds_correlation_and_order() {
        let updates = RefCell::new(Vec::new());
        let publish = |progress| updates.borrow_mut().push(progress);
        let reporter = PiScanExecutionPhaseReporter::new(73, &publish);
        let phases = [
            PiScanExecutionPhase::Preparing,
            PiScanExecutionPhase::ResolvingMetadata,
            PiScanExecutionPhase::WaitingToRetry,
            PiScanExecutionPhase::AcquiringSources,
            PiScanExecutionPhase::RunningModel,
            PiScanExecutionPhase::RecheckingIdentity,
            PiScanExecutionPhase::ValidatingResult,
            PiScanExecutionPhase::Finalizing,
        ];

        for phase in phases {
            reporter.report(phase);
        }

        let expected = phases
            .into_iter()
            .map(|phase| PiScanExecutionProgress {
                correlation_id: 73,
                phase,
            })
            .collect::<Vec<_>>();
        assert_eq!(*updates.borrow(), expected);
    }
}
