//! UI projection for the optional Pi-backed AUR scanner workspace.

use std::collections::BTreeSet;

use crate::logic::pi_scan::result::{Coverage, MergedScanResult, Severity};
use crate::state::pi_scan::{PiScanConsentState, PiScanRuntimeState};
use crate::theme::PiScanSettings;

/// Keyboard-selectable Pi Scan workspace page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanView {
    /// First-run disclosures, configuration, and consent.
    Setup,
    /// Combined feature, queue, budget, and readiness summary.
    Overview,
    /// Candidate package-base selection.
    Targets,
    /// Active and queued work.
    Progress,
    /// Validated advisory result list.
    Results,
    /// Selected validated result details.
    Details,
}

impl PiScanView {
    /// Return all workspace pages in keyboard order.
    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::Setup,
            Self::Overview,
            Self::Targets,
            Self::Progress,
            Self::Results,
            Self::Details,
        ]
    }

    /// Return the zero-based page index.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Setup => 0,
            Self::Overview => 1,
            Self::Targets => 2,
            Self::Progress => 3,
            Self::Results => 4,
            Self::Details => 5,
        }
    }
}

/// Availability shown without launching Pi or making a model call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanAvailability {
    /// Runtime setting is off.
    Disabled,
    /// Current platform cannot execute the native runtime.
    Unsupported,
    /// Configured Pi binary was not found.
    MissingBinary,
    /// Pi exists, but central runtime channels remain deliberately default-off.
    RuntimeDisconnected,
    /// Test/integration projection has an attached runtime channel.
    RuntimeConnected,
}

/// Readiness status shown before a manual or background request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanReadiness {
    /// No no-model capability/readiness probe has been attached.
    Unchecked,
    /// Probe warning that requires explicit confirmation before a request.
    Warning(String),
    /// Caller supplied a successful readiness result.
    Confirmed,
}

/// Selectable package-base target shown in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanTarget {
    /// Installed package name that provided context.
    pub package_name: String,
    /// Canonical package base when resolved.
    pub package_base: String,
    /// Frozen recipe commit when resolved.
    pub commit_oid: Option<String>,
    /// Whether this target is selected for a request.
    pub selected: bool,
    /// Short inert status label.
    pub status: PiScanTargetStatus,
}

/// Presentation status for one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanTargetStatus {
    /// No explicit complete current-HEAD baseline exists.
    Unbaselined,
    /// Frozen identity is queued.
    Queued,
    /// Sequential runner is processing the target.
    Running,
    /// Work is paused by policy or user action.
    Paused,
    /// A validated terminal result is available.
    Completed,
    /// Execution ended without a validated result.
    Failed,
    /// Prior active work was recovered after interruption.
    Interrupted,
    /// User cancellation reached terminal state.
    Cancelled,
}

/// Validated result plus mutable identity facts used by acknowledgement policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanDisplayResult {
    /// Strictly validated, deterministically merged result.
    pub validated: MergedScanResult,
    /// Official AUR HEAD frozen when this result target was created.
    pub observed_head_oid: String,
    /// Whether recipe/source identity changed after analysis.
    pub stale: bool,
    /// Mutable Git refs resolved during advisory acquisition.
    pub mutable_sources: Vec<crate::logic::pi_scan::acquisition::MutableSourceIdentity>,
}

impl PiScanDisplayResult {
    /// Return a deterministic result binding for acknowledgement storage.
    #[must_use]
    pub fn binding(&self) -> String {
        let mut value = format!(
            "{}:{}:{}:{}",
            self.validated.identity.scan_id.len(),
            self.validated.identity.scan_id,
            self.validated.identity.commit_oid.len(),
            self.validated.identity.commit_oid
        );
        for finding in &self.validated.findings {
            value.push(':');
            value.push_str(&finding.fingerprint);
        }
        value.push(':');
        value.push_str(&self.observed_head_oid.len().to_string());
        value.push(':');
        value.push_str(&self.observed_head_oid);
        value.push_str(if self.stale { ":stale" } else { ":current" });
        value
    }

    /// Return whether high/critical acknowledgement is required.
    #[must_use]
    pub fn needs_finding_acknowledgement(&self) -> bool {
        self.validated
            .highest_severity()
            .is_some_and(Severity::requires_acknowledgement)
    }

    /// Return the approved completion/result wording.
    #[must_use]
    pub fn completion_wording(&self) -> String {
        self.validated.completion_wording()
    }

    /// Return canonical text generated only from validated typed data.
    #[must_use]
    pub fn canonical_raw(&self) -> String {
        let findings: Vec<serde_json::Value> = self
            .validated
            .findings
            .iter()
            .map(|finding| {
                serde_json::json!({
                    "fingerprint": finding.fingerprint,
                    "severity": finding.severity.as_str(),
                    "snapshot": finding.snapshot,
                    "path": finding.path,
                    "evidence": finding.evidence,
                    "disagreement": finding.disagreement,
                })
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::json!({
            "scan_id": self.validated.identity.scan_id,
            "package_base": self.validated.identity.package_base,
            "commit_oid": self.validated.identity.commit_oid,
            "observed_head_oid": self.observed_head_oid,
            "coverage": match self.validated.coverage { Coverage::Complete => "complete", Coverage::Incomplete => "incomplete" },
            "stale": self.stale,
            "limitations": self.validated.limitations,
            "findings": findings,
        }))
        .unwrap_or_else(|_| "{\"error\":\"validated result serialization failed\"}".to_string())
    }
}

/// Inert action request for central runtime integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanUiAction {
    /// Probe and display exact Pi/model/pricing facts before consent can be granted.
    ProbeSetup,
    /// Persist explicit runtime/setup consent changes.
    UpdateConsent,
    /// Queue currently selected frozen identities.
    QueueSelected,
    /// Detach from the active progress page.
    Detach,
    /// Reopen active progress.
    Reopen,
    /// Persist a user pause.
    Pause,
    /// Clear a user pause.
    Resume,
    /// Cancel the exact active correlation.
    Cancel(u64),
    /// Retry a selected terminal request manually.
    Retry,
    /// Continue the selected acknowledged result into Pacsea's install/update list.
    ContinueSelected,
    /// Accept the selected complete current-HEAD result as observation baseline.
    AcceptBaseline,
}

/// Dry-run-only preview that never enters the durable queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanDryRunPreview {
    /// Selected package bases.
    pub targets: Vec<String>,
    /// Pi binary and model process that would be used.
    pub process: String,
    /// Explicit preview limitations.
    pub disclosure: String,
}

/// Cohesive Pi Scan workspace projection rooted in the WS3 runtime state.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // Independent consent/confirmation flags are distinct approved user decisions.
pub struct PiScanWorkspaceState {
    /// Cohesive queue/consent/budget projection owned by the runtime contract.
    pub runtime: PiScanRuntimeState,
    /// Effective conservative settings snapshot.
    pub settings: PiScanSettings,
    /// Current page.
    pub view: PiScanView,
    /// Selected row on target/result pages.
    pub selected: usize,
    /// Detail scroll offset.
    pub detail_scroll: u16,
    /// Read-only runtime availability.
    pub availability: PiScanAvailability,
    /// No-model readiness state.
    pub readiness: PiScanReadiness,
    /// Exact Pi version displayed before material consent.
    pub verified_pi_version: String,
    /// Exact Pi-probed selected provider displayed before consent.
    pub verified_provider: String,
    /// Exact Pi-probed selected model displayed before consent.
    pub verified_model: String,
    /// Exact Pi-advertised model identities displayed before consent.
    pub verified_available_models: Vec<String>,
    /// Exact worst-case reservation computed from probed route pricing.
    pub verified_reservation: crate::state::pi_scan::PiScanReservation,
    /// Exact pricing/provenance binding displayed before material consent.
    pub verified_pricing_binding: String,
    /// Exact configured route pricing/provenance descriptions displayed before consent.
    pub verified_pricing_summary: Vec<String>,
    /// Whether exact setup facts were probed in this process and displayed to the user.
    pub setup_facts_verified: bool,
    /// Explicit provider/privacy/cost/coverage disclosure confirmation.
    pub disclosure_confirmed: bool,
    /// Explicit ordered fallback confirmation.
    pub fallback_confirmed: bool,
    /// Explicit warning confirmation for a readiness warning.
    pub readiness_warning_confirmed: bool,
    /// Whether active progress is detached from the visible page.
    pub detached: bool,
    /// Contextual/selectable targets.
    pub targets: Vec<PiScanTarget>,
    /// Validated results only.
    pub results: Vec<PiScanDisplayResult>,
    /// Result bindings acknowledged for high/critical findings.
    pub finding_acknowledgements: BTreeSet<String>,
    /// Result bindings acknowledged for stale identity.
    pub stale_acknowledgements: BTreeSet<String>,
    /// Last inert UI action awaiting central dispatch.
    pub pending_action: Option<PiScanUiAction>,
    /// Dry-run preview, when requested.
    pub dry_run_preview: Option<PiScanDryRunPreview>,
    /// Persistent typed workspace notice.
    pub notice: Option<String>,
    /// Workspace tab hit rectangles.
    pub tab_rects: [Option<(u16, u16, u16, u16)>; 6],
}

impl Default for PiScanWorkspaceState {
    fn default() -> Self {
        Self {
            runtime: PiScanRuntimeState::default(),
            settings: PiScanSettings::default(),
            view: PiScanView::Setup,
            selected: 0,
            detail_scroll: 0,
            availability: PiScanAvailability::Disabled,
            readiness: PiScanReadiness::Unchecked,
            verified_pi_version: String::new(),
            verified_provider: String::new(),
            verified_model: String::new(),
            verified_available_models: Vec::new(),
            verified_reservation: crate::state::pi_scan::PiScanReservation {
                tokens: 0,
                cost_microusd: 0,
            },
            verified_pricing_binding: String::new(),
            verified_pricing_summary: Vec::new(),
            setup_facts_verified: false,
            disclosure_confirmed: false,
            fallback_confirmed: false,
            readiness_warning_confirmed: false,
            detached: false,
            targets: Vec::new(),
            results: Vec::new(),
            finding_acknowledgements: BTreeSet::new(),
            stale_acknowledgements: BTreeSet::new(),
            pending_action: None,
            dry_run_preview: None,
            notice: None,
            tab_rects: [None; 6],
        }
    }
}

impl PiScanWorkspaceState {
    /// Apply settings and derive truthful default-off availability without probing Pi.
    pub fn apply_settings(&mut self, settings: PiScanSettings, pi_binary_found: bool) {
        self.settings = settings;
        self.availability = if !cfg!(target_os = "linux") {
            PiScanAvailability::Unsupported
        } else if !self.settings.enabled {
            PiScanAvailability::Disabled
        } else if !pi_binary_found {
            PiScanAvailability::MissingBinary
        } else {
            PiScanAvailability::RuntimeDisconnected
        };
        if !self.settings.enabled {
            self.view = PiScanView::Setup;
        }
    }

    /// Add or select package context when Shift+A opens the workspace.
    pub fn open_context(&mut self, package_name: Option<&str>, is_aur: bool) {
        if let Some(name) = package_name.filter(|_| is_aur) {
            if !self
                .targets
                .iter()
                .any(|target| target.package_name == name)
            {
                self.targets.push(PiScanTarget {
                    package_name: name.to_string(),
                    package_base: name.to_string(),
                    commit_oid: None,
                    selected: true,
                    status: PiScanTargetStatus::Unbaselined,
                });
            }
            self.view = if self.settings.enabled {
                PiScanView::Targets
            } else {
                PiScanView::Setup
            };
        } else {
            self.view = if self.settings.enabled {
                PiScanView::Overview
            } else {
                PiScanView::Setup
            };
        }
        self.selected = 0;
    }

    /// Replace the independently consented observation and paid-execution switches.
    pub const fn set_consent(&mut self, observation: bool, paid_execution: bool) {
        self.runtime.consent = PiScanConsentState {
            background_observation: observation,
            paid_execution,
        };
    }

    /// Return the selected validated result.
    #[must_use]
    pub fn selected_result(&self) -> Option<&PiScanDisplayResult> {
        self.results.get(self.selected)
    }

    /// Record high/critical acknowledgement for only the selected result binding.
    pub fn acknowledge_selected_findings(&mut self) {
        if let Some(binding) = self.selected_result().map(PiScanDisplayResult::binding) {
            self.finding_acknowledgements.insert(binding);
        }
    }

    /// Record stale-identity acknowledgement for only the selected result binding.
    pub fn acknowledge_selected_stale(&mut self) {
        if let Some(binding) = self.selected_result().map(PiScanDisplayResult::binding) {
            self.stale_acknowledgements.insert(binding);
        }
    }

    /// Return whether selected-result continuation is currently allowed.
    #[must_use]
    pub fn selected_result_acknowledged(&self) -> bool {
        let Some(result) = self.selected_result() else {
            return false;
        };
        let binding = result.binding();
        (!result.needs_finding_acknowledgement()
            || self.finding_acknowledgements.contains(&binding))
            && (!result.stale || self.stale_acknowledgements.contains(&binding))
    }
}
