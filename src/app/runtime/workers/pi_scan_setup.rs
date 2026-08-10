//! Typed setup-only protocol for the guided Pi Scan initial setup wizard.
//!
//! The setup controller is available while production scanning is disabled and
//! never accepts queue work. Only [`PiScanSetupRequest::ApplySetupCandidate`]
//! may cause durable writes or production runtime replacement; every other
//! request is a read-only, no-model probe or validation.

use crate::state::pi_scan::PiScanConsentState;
use crate::state::pi_scan_setup::PiScanSetupConfirmations;
use crate::theme::PiScanSettings;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// What: Configuration for one setup-only controller instance.
///
/// Inputs:
/// - Session dry-run flag plus the exact durable paths a committed apply owns.
///
/// Output:
/// - Startup policy for [`spawn_pi_scan_setup_controller`].
///
/// Details:
/// - Dry-run controllers must not probe Pi, write configuration/consent, or
///   activate any runtime; they answer with typed dry-run failures.
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
/// - Exactly one correlated [`PiScanSetupEvent`] per request.
///
/// Details:
/// - Requests never contain credentials, prompts, source bodies, or Pi wire
///   records. Stale correlations are ignored by the wizard projection.
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
        /// Candidate independent consent choices.
        consent: PiScanConsentState,
        /// Independent explicit confirmations.
        confirmations: PiScanSetupConfirmations,
    },
    /// Revalidate, activate, and atomically persist the reviewed candidate.
    ApplySetupCandidate {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Complete reviewed candidate settings.
        candidate: PiScanSettings,
        /// Candidate independent consent choices.
        consent: PiScanConsentState,
        /// Independent explicit confirmations.
        confirmations: PiScanSetupConfirmations,
        /// Exact validation binding echoed from `CandidateValidated`.
        validation_binding: String,
    },
}

/// What: Correlated events published by the setup-only controller.
///
/// Inputs:
/// - Produced in response to exactly one [`PiScanSetupRequest`].
///
/// Output:
/// - Wizard-facing verified facts, validation outcome, or typed failure.
///
/// Details:
/// - `Applied` is the only event that may follow durable writes; every failure
///   leaves the previous configuration, runtime, and consent authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanSetupEvent {
    /// Exact no-model capability facts verified for the requested binary.
    CapabilitiesVerified {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Exact verified facts for wizard display and route selection.
        snapshot: Box<crate::pi_scan_orchestrator::SetupSnapshot>,
    },
    /// Candidate validation succeeded without durable changes.
    CandidateValidated {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Binding over the validated candidate and verified facts.
        validation_binding: String,
    },
    /// Reviewed candidate was activated and persisted transactionally.
    Applied {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Exact effective settings now authoritative.
        effective: Box<PiScanSettings>,
        /// Exact setup snapshot rebound immediately before commit.
        snapshot: Box<crate::pi_scan_orchestrator::SetupSnapshot>,
    },
    /// One transaction stage failed; nothing durable changed.
    Failed {
        /// Wizard request correlation.
        correlation_id: u64,
        /// Failing transaction stage.
        stage: PiScanSetupStage,
        /// Actionable retry guidance.
        reason: String,
    },
}

/// Typed channel endpoints owned by the event loop for setup-only requests.
pub struct PiScanSetupChannels {
    /// Correlated request sender.
    pub request_tx: mpsc::UnboundedSender<PiScanSetupRequest>,
    /// Correlated event receiver.
    pub event_rx: mpsc::UnboundedReceiver<PiScanSetupEvent>,
}

/// What: Spawn the setup-only controller used by the wizard while scanning is off.
///
/// Inputs:
/// - `options`: Dry-run flag and durable paths owned by a committed apply.
///
/// Output:
/// - Typed setup channel endpoints.
///
/// Details:
/// - Wave 0 contract stub: every request currently receives a typed
///   [`PiScanSetupEvent::Failed`] naming its stage, so ignored contract tests
///   fail for missing behavior rather than harness mistakes. WS2 replaces the
///   loop body with real probe/validate/apply handling.
#[must_use]
pub fn spawn_pi_scan_setup_controller(
    options: PiScanSetupControllerOptions,
) -> PiScanSetupChannels {
    let (request_tx, mut request_rx) = mpsc::unbounded_channel::<PiScanSetupRequest>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<PiScanSetupEvent>();
    tokio::spawn(async move {
        drop(options);
        while let Some(request) = request_rx.recv().await {
            let (correlation_id, stage) = match &request {
                PiScanSetupRequest::BeginSetupProbe { correlation_id, .. } => {
                    (*correlation_id, PiScanSetupStage::Probe)
                }
                PiScanSetupRequest::ValidateSetupCandidate { correlation_id, .. } => {
                    (*correlation_id, PiScanSetupStage::CandidateValidation)
                }
                PiScanSetupRequest::ApplySetupCandidate { correlation_id, .. } => {
                    (*correlation_id, PiScanSetupStage::Activation)
                }
            };
            let _ = event_tx.send(PiScanSetupEvent::Failed {
                correlation_id,
                stage,
                reason: "the guided setup controller is not implemented yet; this Wave 0 stub \
                         performs no probe, write, or activation"
                    .to_string(),
            });
        }
    });
    PiScanSetupChannels {
        request_tx,
        event_rx,
    }
}
