//! Draft state for the guided Pi Scan initial setup wizard.
//!
//! The wizard edits a candidate configuration in isolation: nothing in this
//! module mutates `AppState.settings`, disk, runtime, or durable consent.
//! Only the final `ApplySetupCandidate` runtime transaction may cause writes.

use crate::state::pi_scan::{PiScanConsentState, PiScanReservation};
use crate::theme::PiScanSettings;

/// One wizard page shown to the user, in fixed keyboard order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanSetupStep {
    /// Advisory scope, data flow, and prerequisites.
    Welcome,
    /// Pi binary selection and exact capability verification.
    PiReadiness,
    /// Exact provider/model route and thinking selection.
    Route,
    /// Exact pricing provenance and privacy disclosure.
    PricingPrivacy,
    /// Observation, paid background execution, fallback, budgets, retention, proxy.
    OptionalBehavior,
    /// Full effective-value review before Apply.
    Review,
    /// Transactional validation, activation, and persistence outcome.
    Activate,
}

impl PiScanSetupStep {
    /// Return all wizard steps in presentation order.
    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::Welcome,
            Self::PiReadiness,
            Self::Route,
            Self::PricingPrivacy,
            Self::OptionalBehavior,
            Self::Review,
            Self::Activate,
        ]
    }

    /// Return the zero-based step index used by the progress indicator.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::PiReadiness => 1,
            Self::Route => 2,
            Self::PricingPrivacy => 3,
            Self::OptionalBehavior => 4,
            Self::Review => 5,
            Self::Activate => 6,
        }
    }
}

/// Transactional apply progress projected from the setup controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanSetupApplyStatus {
    /// No apply is in flight.
    Idle,
    /// Candidate revalidation is running.
    Validating,
    /// Candidate production runtime is being constructed and health-checked.
    Activating,
    /// Settings and consent are being persisted atomically.
    Persisting,
    /// Apply committed; production runtime is authoritative.
    Complete,
    /// Apply failed; previous configuration/runtime/consent remain authoritative.
    Failed(String),
}

/// Independent wizard confirmations that never collapse into one checkbox.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "privacy, foreground payment, fallback, and readiness acceptance are independent approved decisions"
)]
pub struct PiScanSetupConfirmations {
    /// Provider/privacy/cost/coverage disclosure confirmation.
    pub disclosure_confirmed: bool,
    /// Explicit foreground paid model execution confirmation.
    pub foreground_paid_confirmed: bool,
    /// Explicit ordered-fallback confirmation.
    pub fallback_confirmed: bool,
    /// Explicit readiness-warning acceptance.
    pub readiness_warning_confirmed: bool,
}

/// What: Exact Pi capability facts verified for the wizard without a model call.
///
/// Inputs:
/// - Copied from the setup controller's correlated `CapabilitiesVerified` event.
///
/// Output:
/// - Plain display/selection data bound to the reviewed candidate.
///
/// Details:
/// - Contains no credential, prompt, source content, or raw Pi output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanSetupVerifiedFacts {
    /// Exact verified Pi version.
    pub pi_version: String,
    /// Exact advertised provider/model routes.
    pub routes: Vec<(String, String)>,
    /// Exact worst-case token/micro-USD reservation for the reviewed route.
    pub reservation: PiScanReservation,
    /// SHA-256 binding over exact Pi-reported pricing/provenance.
    pub pricing_binding: String,
    /// Human-readable exact pricing/provenance summary lines.
    pub pricing_summary: Vec<String>,
}

impl Default for PiScanSetupVerifiedFacts {
    fn default() -> Self {
        Self {
            pi_version: String::new(),
            routes: Vec::new(),
            reservation: PiScanReservation {
                tokens: 0,
                cost_microusd: 0,
            },
            pricing_binding: String::new(),
            pricing_summary: Vec::new(),
        }
    }
}

/// What: Complete draft state for one guided setup wizard session.
///
/// Inputs:
/// - Immutable original settings/consent snapshots taken when the wizard opened.
///
/// Output:
/// - Candidate settings, confirmations, and verified facts reviewed before Apply.
///
/// Details:
/// - The draft excludes credentials, raw Pi output, prompts, source content, and
///   provider responses. Cancel drops this state without any write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanSetupWizardState {
    /// Current wizard page.
    pub step: PiScanSetupStep,
    /// Focused control index on the current page.
    pub focus: usize,
    /// Immutable settings snapshot restored verbatim on Cancel.
    pub original_settings: PiScanSettings,
    /// Immutable consent snapshot restored verbatim on Cancel.
    pub original_consent: PiScanConsentState,
    /// Mutable candidate settings edited by the wizard.
    pub candidate: PiScanSettings,
    /// Candidate independent observation/paid-execution consent choices.
    pub candidate_consent: PiScanConsentState,
    /// Exact verified Pi facts, present only after a successful probe.
    pub verified: Option<PiScanSetupVerifiedFacts>,
    /// Independent explicit confirmations.
    pub confirmations: PiScanSetupConfirmations,
    /// Current inline validation issues for the focused step.
    pub validation_issues: Vec<String>,
    /// Correlation id of the in-flight setup request, when any.
    pub in_flight_correlation: Option<u64>,
    /// Last correlation id allocated by this wizard session.
    pub last_correlation: u64,
    /// Validation binding echoed by the controller and required by Apply.
    pub validation_binding: String,
    /// Transactional apply progress.
    pub apply_status: PiScanSetupApplyStatus,
    /// Whether this session is first-run setup rather than explicit reconfiguration.
    pub first_run: bool,
    /// Persistent wizard notice line.
    pub notice: Option<String>,
}

impl PiScanSetupWizardState {
    /// What: Open a wizard session from the current effective settings/consent.
    ///
    /// Inputs:
    /// - `settings`: Effective Pi Scan settings snapshot.
    /// - `consent`: Effective runtime consent snapshot.
    /// - `first_run`: Whether setup is incomplete rather than explicitly rerun.
    ///
    /// Output:
    /// - Draft state whose candidate starts equal to the original snapshot.
    ///
    /// Details:
    /// - Conservative defaults are preserved: the wizard never pre-enables
    ///   scanning, observation, paid background execution, or fallback.
    #[must_use]
    pub fn open(settings: PiScanSettings, consent: PiScanConsentState, first_run: bool) -> Self {
        Self {
            step: PiScanSetupStep::Welcome,
            focus: 0,
            candidate: settings.clone(),
            candidate_consent: consent,
            original_settings: settings,
            original_consent: consent,
            verified: None,
            confirmations: PiScanSetupConfirmations::default(),
            validation_issues: Vec::new(),
            in_flight_correlation: None,
            last_correlation: 0,
            validation_binding: String::new(),
            apply_status: PiScanSetupApplyStatus::Idle,
            first_run,
            notice: None,
        }
    }

    /// Allocate the next request correlation id for this wizard session.
    pub const fn next_correlation(&mut self) -> u64 {
        self.last_correlation += 1;
        self.in_flight_correlation = Some(self.last_correlation);
        self.last_correlation
    }

    /// Return whether a correlated controller response matches the in-flight request.
    #[must_use]
    pub fn accepts_correlation(&self, correlation_id: u64) -> bool {
        self.in_flight_correlation == Some(correlation_id)
    }

    /// Return whether the candidate's selected route exists in the verified snapshot.
    #[must_use]
    pub fn candidate_route_advertised(&self) -> bool {
        self.verified.as_ref().is_some_and(|facts| {
            facts.routes.iter().any(|(provider, model)| {
                provider == self.candidate.provider.trim() && model == self.candidate.model.trim()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening the wizard must copy, not mutate, the original snapshots.
    #[test]
    fn open_preserves_original_snapshots_and_conservative_defaults() {
        let settings = PiScanSettings {
            provider: "provider".to_string(),
            ..PiScanSettings::default()
        };
        let consent = PiScanConsentState::default();
        let wizard = PiScanSetupWizardState::open(settings.clone(), consent, true);
        assert_eq!(wizard.original_settings, settings);
        assert_eq!(wizard.candidate, settings);
        assert_eq!(wizard.original_consent, consent);
        assert_eq!(wizard.step, PiScanSetupStep::Welcome);
        assert_eq!(wizard.apply_status, PiScanSetupApplyStatus::Idle);
        assert!(!wizard.confirmations.disclosure_confirmed);
        assert!(wizard.first_run);
        assert!(wizard.verified.is_none());
    }

    /// Correlation acceptance must reject stale and unknown responses.
    #[test]
    fn stale_correlations_are_rejected() {
        let wizard_settings = PiScanSettings::default();
        let mut wizard =
            PiScanSetupWizardState::open(wizard_settings, PiScanConsentState::default(), true);
        let first = wizard.next_correlation();
        let second = wizard.next_correlation();
        assert!(!wizard.accepts_correlation(first));
        assert!(wizard.accepts_correlation(second));
        assert!(!wizard.accepts_correlation(second + 1));
    }

    /// Route validation must fail closed until a verified snapshot advertises the route.
    #[test]
    fn candidate_route_requires_verified_snapshot() {
        let mut wizard = PiScanSetupWizardState::open(
            PiScanSettings::default(),
            PiScanConsentState::default(),
            true,
        );
        wizard.candidate.provider = "openrouter".to_string();
        wizard.candidate.model = "model-a".to_string();
        assert!(!wizard.candidate_route_advertised());
        wizard.verified = Some(PiScanSetupVerifiedFacts {
            routes: vec![("openrouter".to_string(), "model-a".to_string())],
            ..PiScanSetupVerifiedFacts::default()
        });
        assert!(wizard.candidate_route_advertised());
        wizard.candidate.model = "model-b".to_string();
        assert!(!wizard.candidate_route_advertised());
    }
}
