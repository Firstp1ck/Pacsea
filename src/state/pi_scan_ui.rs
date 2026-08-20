//! UI projection for the optional Pi-backed AUR scanner workspace.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

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

/// Transient phase of one active Pi Scan execution.
///
/// This type is intentionally not serializable: durable runtime recovery owns queue and terminal
/// state, while an in-flight phase can only be reported truthfully by the current process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanExecutionPhase {
    /// The orchestrator registered the exact active item and is preparing its frozen target.
    Preparing,
    /// The adapter is resolving current AUR metadata needed for immutable acquisition.
    ResolvingMetadata,
    /// A bounded transient pre-model failure is waiting before its one allowed retry.
    WaitingToRetry,
    /// Immutable recipe and source snapshots are being acquired and integrity-checked.
    AcquiringSources,
    /// Pi is executing the selected model route and validating its typed response.
    RunningModel,
    /// Mutable source references and official AUR HEAD are being rechecked after analysis.
    RecheckingIdentity,
    /// The orchestrator is validating the returned receipt against the frozen target.
    ValidatingResult,
    /// The validated result and exact accounting transition are being persisted.
    Finalizing,
}

/// Correlation-owned transient progress update for one active execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanExecutionProgress {
    /// Exact active runtime correlation owning this update.
    pub correlation_id: u64,
    /// Current observable execution phase.
    pub phase: PiScanExecutionPhase,
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

/// Severity used to style and expire a typed Pi Scan workspace notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanNoticeSeverity {
    /// Informational guidance that expires automatically.
    Info,
    /// Successful outcome that expires automatically.
    Success,
    /// Warning that remains until explicitly replaced or dismissed.
    Warning,
    /// Error that remains until explicitly replaced or dismissed.
    Error,
}

/// One typed workspace notice with a monotonic expiry deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanNotice {
    /// User-facing notice text or localization key awaiting projection.
    pub text: String,
    /// Semantic notice severity.
    pub severity: PiScanNoticeSeverity,
    /// Monotonic expiry for transient severities; persistent notices use `None`.
    pub expires_at: Option<Instant>,
}

impl PiScanNotice {
    /// What: Build a typed notice from one monotonic reference time.
    ///
    /// Inputs:
    /// - `text`: User-facing text or localization key.
    /// - `severity`: Semantic notice severity.
    /// - `now`: Monotonic creation time.
    ///
    /// Output:
    /// - A notice with a six-second deadline for Info/Success or no deadline for Warning/Error.
    ///
    /// Details:
    /// - Wall-clock changes cannot extend or prematurely expire transient notices.
    #[must_use]
    pub fn at(text: impl Into<String>, severity: PiScanNoticeSeverity, now: Instant) -> Self {
        let expires_at = matches!(
            severity,
            PiScanNoticeSeverity::Info | PiScanNoticeSeverity::Success
        )
        .then(|| now + Duration::from_secs(6));
        Self {
            text: text.into(),
            severity,
            expires_at,
        }
    }

    /// Return whether this notice has reached its monotonic deadline.
    #[must_use]
    pub fn is_expired_at(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|deadline| now >= deadline)
    }
}

/// Independent user-action and background-event workspace notice slots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PiScanNoticeSlots {
    /// Foreground notice owned by the latest user-initiated interaction.
    pub foreground: Option<PiScanNotice>,
    /// Background notice that cannot overwrite foreground feedback.
    pub background: Option<PiScanNotice>,
}

impl PiScanNoticeSlots {
    /// Set the foreground slot using the current monotonic time.
    pub fn set_foreground(&mut self, text: impl Into<String>, severity: PiScanNoticeSeverity) {
        self.set_foreground_at(text, severity, Instant::now());
    }

    /// Set the foreground slot using an explicit monotonic reference time.
    pub fn set_foreground_at(
        &mut self,
        text: impl Into<String>,
        severity: PiScanNoticeSeverity,
        now: Instant,
    ) {
        self.foreground = Some(PiScanNotice::at(text, severity, now));
    }

    /// Set the background slot using the current monotonic time.
    pub fn set_background(&mut self, text: impl Into<String>, severity: PiScanNoticeSeverity) {
        self.set_background_at(text, severity, Instant::now());
    }

    /// Set the background slot using an explicit monotonic reference time.
    pub fn set_background_at(
        &mut self,
        text: impl Into<String>,
        severity: PiScanNoticeSeverity,
        now: Instant,
    ) {
        self.background = Some(PiScanNotice::at(text, severity, now));
    }

    /// Remove transient notices whose monotonic deadlines have elapsed.
    pub fn expire_at(&mut self, now: Instant) {
        if self
            .foreground
            .as_ref()
            .is_some_and(|notice| notice.is_expired_at(now))
        {
            self.foreground = None;
        }
        if self
            .background
            .as_ref()
            .is_some_and(|notice| notice.is_expired_at(now))
        {
            self.background = None;
        }
    }

    /// Return foreground notice text for compatibility rendering.
    #[must_use]
    pub fn foreground_text(&self) -> Option<&str> {
        self.foreground.as_ref().map(|notice| notice.text.as_str())
    }
}

/// Exact user intent retained while selected package identities are resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanQueueIntentSnapshot {
    /// Originally selected package names, sorted and deduplicated.
    pub package_names: Vec<String>,
    /// Token reservation context shown when the action was requested.
    pub reservation_tokens: u64,
    /// Exact decimal cost-cap text shown when the action was requested.
    pub reservation_cost_cap: String,
}

/// Correlation-owned state for one setup Apply transaction that outlives its wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanSetupTransaction {
    /// Exact setup-controller correlation that owns transfer or rollback completion.
    pub correlation_id: u64,
    /// Current user abandonment state.
    pub abandonment: PiScanSetupAbandonment,
}

/// Two-Escape abandonment progression for a setup Apply transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanSetupAbandonment {
    /// Apply is proceeding and no abandonment warning has been shown.
    Active,
    /// First Escape warned while retaining the wizard and transaction.
    Warned,
    /// Second Escape closed the wizard and requested explicit rollback.
    AbandonRequested,
}

/// Independent scroll offsets for every Pi Scan workspace view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PiScanViewScrollState {
    /// Setup-page line offset.
    pub setup: u16,
    /// Overview-page line offset.
    pub overview: u16,
    /// Target-list item offset.
    pub targets: usize,
    /// Progress-page line offset.
    pub progress: u16,
    /// Result-list item offset.
    pub results: usize,
    /// Details-page line offset.
    pub details: u16,
}

/// One rendered list-row rectangle retained for mouse hit testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanListHitRect {
    /// Zero-based item index represented by the row.
    pub index: usize,
    /// Left coordinate.
    pub x: u16,
    /// Top coordinate.
    pub y: u16,
    /// Rectangle width.
    pub width: u16,
    /// Rectangle height.
    pub height: u16,
}

impl PiScanListHitRect {
    /// Return whether one terminal coordinate is inside this half-open rectangle.
    #[must_use]
    pub const fn contains(self, column: u16, row: u16) -> bool {
        column >= self.x
            && column < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }
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
    /// Correlation-owned in-process phase for the current active execution.
    pub active_progress: Option<PiScanExecutionProgress>,
    /// Effective conservative settings snapshot.
    pub settings: PiScanSettings,
    /// Current page.
    pub view: PiScanView,
    /// Legacy renderer projection synchronized from the active independent selection.
    pub selected: usize,
    /// Independently selected target row.
    pub selected_target: usize,
    /// Independently selected result row, preserved when Details opens.
    pub selected_result: usize,
    /// Session-only result indices whose package details are expanded.
    pub expanded_results: BTreeSet<usize>,
    /// Legacy detail-scroll projection synchronized with `view_scroll.details`.
    pub detail_scroll: u16,
    /// Independent per-view line and item scroll offsets.
    pub view_scroll: PiScanViewScrollState,
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
    /// Independent paid background-execution confirmation.
    pub background_paid_execution_confirmed: bool,
    /// Explicit warning confirmation for a readiness warning.
    pub readiness_warning_confirmed: bool,
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
    /// Exact queue intent retained across identity observation.
    pub pending_queue_intent: Option<PiScanQueueIntentSnapshot>,
    /// Number of validated results inserted since Results was last entered.
    pub unseen_result_count: usize,
    /// Session-only raw-output visibility override.
    pub show_raw_output: bool,
    /// Independent typed foreground/background notice slots.
    pub notices: PiScanNoticeSlots,
    /// Correlation-owned setup transaction retained after wizard abandonment.
    pub setup_transaction: Option<PiScanSetupTransaction>,
    /// Workspace tab hit rectangles.
    pub tab_rects: [Option<(u16, u16, u16, u16)>; 6],
    /// Rendered target row hit rectangles.
    pub target_row_rects: Vec<PiScanListHitRect>,
    /// Rendered result row hit rectangles.
    pub result_row_rects: Vec<PiScanListHitRect>,
    /// Last setup-controller correlation retained across wizard sessions.
    pub last_setup_correlation: u64,
    /// Active guided setup wizard session; `None` outside the wizard.
    pub wizard: Option<crate::state::pi_scan_setup::PiScanSetupWizardState>,
}

impl Default for PiScanWorkspaceState {
    fn default() -> Self {
        Self {
            runtime: PiScanRuntimeState::default(),
            active_progress: None,
            settings: PiScanSettings::default(),
            view: PiScanView::Setup,
            selected: 0,
            selected_target: 0,
            selected_result: 0,
            expanded_results: BTreeSet::new(),
            detail_scroll: 0,
            view_scroll: PiScanViewScrollState::default(),
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
            background_paid_execution_confirmed: false,
            readiness_warning_confirmed: false,
            targets: Vec::new(),
            results: Vec::new(),
            finding_acknowledgements: BTreeSet::new(),
            stale_acknowledgements: BTreeSet::new(),
            pending_action: None,
            dry_run_preview: None,
            pending_queue_intent: None,
            unseen_result_count: 0,
            show_raw_output: false,
            notices: PiScanNoticeSlots::default(),
            setup_transaction: None,
            tab_rects: [None; 6],
            target_row_rects: Vec::new(),
            result_row_rects: Vec::new(),
            last_setup_correlation: 0,
            wizard: None,
        }
    }
}

impl PiScanWorkspaceState {
    /// What: Start an isolated guided setup session from the effective projection.
    ///
    /// Inputs:
    /// - `first_run`: Whether setup is incomplete rather than an explicit rerun.
    ///
    /// Output:
    /// - A Welcome-step draft attached to the existing Setup page.
    ///
    /// Details:
    /// - Effective settings, runtime, and consent remain untouched until central
    ///   integration dispatches a successful final Apply transaction.
    pub fn begin_setup_wizard(&mut self, first_run: bool) {
        if self.setup_transaction.is_some() {
            self.notices.set_foreground(
                "A previous Pi Scan setup Apply is still resolving; wait for its rollback or completion",
                PiScanNoticeSeverity::Warning,
            );
            return;
        }
        let mut wizard = crate::state::pi_scan_setup::PiScanSetupWizardState::open(
            self.settings.clone(),
            PiScanConsentState {
                background_observation: self.runtime.consent.background_observation,
                paid_execution: self.background_paid_execution_confirmed,
            },
            first_run,
        );
        wizard.last_correlation = self.last_setup_correlation;
        self.wizard = Some(wizard);
        self.set_view(PiScanView::Setup);
        self.pending_action = None;
    }

    /// Drop a non-applying wizard draft and expose first-run restart guidance.
    pub fn cancel_setup_wizard(&mut self) {
        let first_run = self.wizard.as_ref().is_some_and(|wizard| wizard.first_run);
        self.wizard = None;
        self.pending_action = None;
        if first_run {
            self.notices.set_foreground(
                "Guided setup cancelled — press r to restart it, Esc to leave",
                PiScanNoticeSeverity::Info,
            );
        }
    }

    /// What: Process Escape while a wizard may own an in-flight Apply transaction.
    ///
    /// Inputs:
    /// - Current wizard status and correlation-owned workspace transaction.
    ///
    /// Output:
    /// - First Apply Escape warns and keeps the wizard; second records abandonment and closes it.
    ///
    /// Details:
    /// - Transaction ownership remains in the workspace so a later transfer can be rolled back
    ///   explicitly by central projection rather than silently through `Drop`.
    pub fn cancel_or_abandon_setup_wizard(&mut self) {
        let applying = self.wizard.as_ref().is_some_and(|wizard| {
            matches!(
                wizard.apply_status,
                crate::state::pi_scan_setup::PiScanSetupApplyStatus::Validating
                    | crate::state::pi_scan_setup::PiScanSetupApplyStatus::Activating
                    | crate::state::pi_scan_setup::PiScanSetupApplyStatus::Persisting
            )
        });
        if !applying {
            self.cancel_setup_wizard();
            return;
        }
        self.ensure_setup_transaction_from_wizard();
        let Some(transaction) = self.setup_transaction.as_mut() else {
            return;
        };
        if transaction.abandonment == PiScanSetupAbandonment::Active {
            transaction.abandonment = PiScanSetupAbandonment::Warned;
            self.notices.set_foreground(
                "Apply in progress — press Esc again to abandon and roll back",
                PiScanNoticeSeverity::Warning,
            );
            return;
        }
        transaction.abandonment = PiScanSetupAbandonment::AbandonRequested;
        self.wizard = None;
        self.pending_action = None;
        self.notices.set_foreground(
            "Pi Scan setup abandonment requested; waiting for explicit rollback",
            PiScanNoticeSeverity::Warning,
        );
    }

    /// Register the correlation allocated by the wizard's latest Apply request.
    pub fn register_setup_apply(&mut self) {
        self.ensure_setup_transaction_from_wizard();
    }

    /// Return whether a workspace-owned setup transaction matches a correlation.
    #[must_use]
    pub fn setup_transaction_matches(&self, correlation_id: u64) -> bool {
        self.setup_transaction
            .is_some_and(|transaction| transaction.correlation_id == correlation_id)
    }

    /// Clear one matching terminal setup transaction after completion or explicit rollback.
    pub fn finish_setup_transaction(&mut self, correlation_id: u64) -> bool {
        if !self.setup_transaction_matches(correlation_id) {
            return false;
        }
        self.setup_transaction = None;
        true
    }

    /// Recover workspace transaction ownership from the active wizard correlation.
    fn ensure_setup_transaction_from_wizard(&mut self) {
        if self.setup_transaction.is_some() {
            return;
        }
        let correlation_id = self
            .wizard
            .as_ref()
            .and_then(|wizard| wizard.in_flight_correlation);
        if let Some(correlation_id) = correlation_id {
            self.setup_transaction = Some(PiScanSetupTransaction {
                correlation_id,
                abandonment: PiScanSetupAbandonment::Active,
            });
        }
    }

    /// Return whether material setup facts and foreground consent are currently verified.
    #[must_use]
    pub const fn setup_complete(&self) -> bool {
        self.settings.enabled
            && self.setup_facts_verified
            && self.disclosure_confirmed
            && self.runtime.consent.paid_execution
    }

    /// Apply settings and preserve a live runtime-connected availability projection.
    pub fn apply_settings(&mut self, settings: PiScanSettings, pi_binary_found: bool) -> bool {
        let runtime_connected = self.availability == PiScanAvailability::RuntimeConnected;
        let settings_changed = self.settings != settings;
        let material_changed = self.setup_facts_verified && settings_changed;
        self.settings = settings;
        if material_changed {
            self.setup_facts_verified = false;
            self.disclosure_confirmed = false;
            self.fallback_confirmed = false;
            self.background_paid_execution_confirmed = false;
            self.readiness_warning_confirmed = false;
            self.runtime.consent = PiScanConsentState::default();
            self.readiness = PiScanReadiness::Unchecked;
        }
        self.availability = if runtime_connected {
            PiScanAvailability::RuntimeConnected
        } else if !cfg!(target_os = "linux") {
            PiScanAvailability::Unsupported
        } else if !self.settings.enabled {
            PiScanAvailability::Disabled
        } else if !pi_binary_found {
            PiScanAvailability::MissingBinary
        } else {
            PiScanAvailability::RuntimeDisconnected
        };
        if !self.settings.enabled && !runtime_connected {
            self.set_view(PiScanView::Setup);
        }
        settings_changed
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
        self.selected_target = 0;
        self.selected_result = 0;
        self.selected = 0;
    }

    /// What: Change workspace view while preserving independent list selections.
    ///
    /// Inputs:
    /// - `view`: Destination workspace page.
    ///
    /// Output:
    /// - Updates the active view and legacy renderer selection projection.
    ///
    /// Details:
    /// - Entering Results clears the unseen count; entering Details resets only details line scroll.
    pub fn set_view(&mut self, view: PiScanView) {
        let entering_details = self.view != PiScanView::Details && view == PiScanView::Details;
        self.view = view;
        match view {
            PiScanView::Targets => self.selected = self.selected_target,
            PiScanView::Results => {
                self.selected = self.selected_result;
                self.unseen_result_count = 0;
            }
            PiScanView::Details => {
                self.selected = self.selected_result;
                self.view_scroll.details = 0;
                self.detail_scroll = 0;
                if entering_details {
                    self.reset_result_expansion();
                }
            }
            PiScanView::Setup | PiScanView::Overview | PiScanView::Progress => self.selected = 0,
        }
    }

    /// Clamp independent selections and item scroll offsets after asynchronous mutation.
    pub fn clamp_selection(&mut self) {
        self.selected_target = clamp_index(self.selected_target, self.targets.len());
        self.selected_result = clamp_index(self.selected_result, self.results.len());
        self.expanded_results
            .retain(|index| *index < self.results.len());
        self.view_scroll.targets = clamp_index(self.view_scroll.targets, self.targets.len());
        self.view_scroll.results = clamp_index(self.view_scroll.results, self.results.len());
        self.selected = match self.view {
            PiScanView::Targets => self.selected_target,
            PiScanView::Results | PiScanView::Details => self.selected_result,
            PiScanView::Setup | PiScanView::Overview | PiScanView::Progress => 0,
        };
    }

    /// Record one validated result insertion without mutating state during rendering.
    pub fn record_result_inserted(&mut self) {
        if self.view != PiScanView::Results {
            self.unseen_result_count = self.unseen_result_count.saturating_add(1);
        }
        self.clamp_selection();
    }

    /// Snapshot exact package-name and reservation intent for later identity resolution.
    pub fn snapshot_queue_intent(&mut self) {
        let mut package_names: Vec<String> = self
            .targets
            .iter()
            .filter(|target| target.selected)
            .map(|target| target.package_name.clone())
            .collect();
        package_names.sort();
        package_names.dedup();
        self.pending_queue_intent =
            (!package_names.is_empty()).then(|| PiScanQueueIntentSnapshot {
                package_names,
                reservation_tokens: self.settings.background_token_cap_24h,
                reservation_cost_cap: self.settings.background_cost_cap_24h.clone(),
            });
    }

    /// Toggle session-only raw output visibility without rewriting settings.
    pub const fn toggle_raw_output(&mut self) {
        self.show_raw_output = !self.show_raw_output;
    }

    /// Return whether one valid result package is expanded in Details.
    #[must_use]
    pub fn is_result_expanded(&self, index: usize) -> bool {
        index < self.results.len() && self.expanded_results.contains(&index)
    }

    /// Toggle one valid result package and return its resulting expansion state.
    pub fn toggle_result_expansion(&mut self, index: usize) -> bool {
        if index >= self.results.len() {
            return false;
        }
        if !self.expanded_results.insert(index) {
            self.expanded_results.remove(&index);
        }
        self.is_result_expanded(index)
    }

    /// Clear all session-only package expansion state.
    pub fn reset_result_expansion(&mut self) {
        self.expanded_results.clear();
    }

    /// Replace target-row hit rectangles after one render.
    pub fn set_target_row_rects(&mut self, rects: Vec<PiScanListHitRect>) {
        self.target_row_rects = rects;
    }

    /// Replace result-row hit rectangles after one render.
    pub fn set_result_row_rects(&mut self, rects: Vec<PiScanListHitRect>) {
        self.result_row_rects = rects;
    }

    /// Resolve one target-list mouse coordinate to its item index.
    #[must_use]
    pub fn target_hit_test(&self, column: u16, row: u16) -> Option<usize> {
        hit_test_rows(&self.target_row_rects, column, row)
    }

    /// Resolve one result-list mouse coordinate to its item index.
    #[must_use]
    pub fn result_hit_test(&self, column: u16, row: u16) -> Option<usize> {
        hit_test_rows(&self.result_row_rects, column, row)
    }

    /// Replace the foreground typed notice.
    pub fn set_foreground_notice(
        &mut self,
        text: impl Into<String>,
        severity: PiScanNoticeSeverity,
    ) {
        self.notices.set_foreground(text, severity);
    }

    /// Replace the background typed notice without overwriting foreground feedback.
    pub fn set_background_notice(
        &mut self,
        text: impl Into<String>,
        severity: PiScanNoticeSeverity,
    ) {
        self.notices.set_background(text, severity);
    }

    /// Expire transient foreground/background notices at one monotonic instant.
    pub fn expire_notices_at(&mut self, now: Instant) {
        self.notices.expire_at(now);
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
        self.results.get(self.selected_result)
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

/// Clamp an index to a possibly empty collection.
fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { index.min(len - 1) }
}

/// Resolve a terminal coordinate against one set of half-open list-row rectangles.
fn hit_test_rows(rects: &[PiScanListHitRect], column: u16, row: u16) -> Option<usize> {
    rects
        .iter()
        .find(|rect| rect.contains(column, row))
        .map(|rect| rect.index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::pi_scan::result::{ExpectedIdentity, MergedScanResult};

    /// Build a minimal validated result for expansion-state tests.
    fn display_result(package: &str) -> PiScanDisplayResult {
        PiScanDisplayResult {
            validated: MergedScanResult {
                identity: ExpectedIdentity {
                    scan_id: format!("scan-{package}"),
                    package_base: package.to_string(),
                    commit_oid: "commit".to_string(),
                },
                coverage: Coverage::Complete,
                limitations: Vec::new(),
                findings: Vec::new(),
            },
            observed_head_oid: "head".to_string(),
            stale: false,
            mutable_sources: Vec::new(),
        }
    }

    /// Expansion toggles only valid result indices and reports the resulting state.
    #[test]
    fn expansion_toggle_is_safe_and_deterministic() {
        let mut state = PiScanWorkspaceState::default();
        state.results.push(display_result("alpha"));

        assert!(!state.is_result_expanded(1));
        assert!(!state.toggle_result_expansion(1));
        assert!(state.toggle_result_expansion(0));
        assert!(state.is_result_expanded(0));
        assert!(!state.toggle_result_expansion(0));
        assert!(!state.is_result_expanded(0));
    }

    /// Clamping drops expansion entries that no longer identify a result.
    #[test]
    fn clamp_selection_removes_stale_expansion_indices() {
        let mut state = PiScanWorkspaceState::default();
        state.results.push(display_result("alpha"));
        state.results.push(display_result("beta"));
        state.expanded_results.extend([0, 1, 7]);

        state.results.truncate(1);
        state.clamp_selection();

        assert_eq!(state.expanded_results, BTreeSet::from([0]));
    }

    /// Entering Details starts a fresh session-only expansion projection.
    #[test]
    fn entering_details_resets_expansion_state() {
        let mut state = PiScanWorkspaceState::default();
        state.results.push(display_result("alpha"));
        state.expanded_results.insert(0);

        state.set_view(PiScanView::Details);

        assert!(state.expanded_results.is_empty());
        assert_eq!(state.view_scroll.details, 0);
        assert_eq!(state.detail_scroll, 0);
    }
}
