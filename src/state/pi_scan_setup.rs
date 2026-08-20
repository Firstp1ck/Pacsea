//! Draft state for the guided Pi Scan initial setup wizard.
//!
//! The wizard edits a candidate configuration in isolation: nothing in this
//! module mutates `AppState.settings`, disk, runtime, or durable consent.
//! Only the final setup-controller apply transaction may cause writes.

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

    /// Return the step immediately before this one, when navigation is allowed.
    #[must_use]
    pub const fn previous(self) -> Option<Self> {
        match self {
            Self::Welcome => None,
            Self::PiReadiness => Some(Self::Welcome),
            Self::Route => Some(Self::PiReadiness),
            Self::PricingPrivacy => Some(Self::Route),
            Self::OptionalBehavior => Some(Self::PricingPrivacy),
            Self::Review => Some(Self::OptionalBehavior),
            Self::Activate => Some(Self::Review),
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

/// Inert wizard action consumed by the central setup-controller integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanSetupDraftAction {
    /// Run a no-model Pi version/capability/model/pricing probe.
    Probe {
        /// Correlation allocated by this wizard session.
        correlation_id: u64,
        /// User-selected Pi executable name or absolute path.
        binary: String,
    },
    /// Validate the complete draft without writes or runtime replacement.
    Validate {
        /// Correlation allocated by this wizard session.
        correlation_id: u64,
    },
    /// Revalidate and transactionally apply the reviewed draft.
    Apply {
        /// Correlation allocated by this wizard session.
        correlation_id: u64,
        /// Fresh controller validation binding displayed on Review.
        validation_binding: String,
    },
}

/// Mouse-selectable wizard control recorded by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanSetupHitTarget {
    /// One focused body control, addressed by its page-local index.
    Control(usize),
    /// Navigate to the preceding wizard page.
    Back,
    /// Validate the current page and continue.
    Next,
    /// Drop the isolated draft without any side effect.
    Cancel,
    /// Retry a failed probe, validation, or apply operation.
    Retry,
    /// Apply the reviewed candidate transactionally.
    Apply,
}

/// One screen rectangle associated with a wizard mouse target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanSetupHitRect {
    /// Semantic target activated by the rectangle.
    pub target: PiScanSetupHitTarget,
    /// Left coordinate.
    pub x: u16,
    /// Top coordinate.
    pub y: u16,
    /// Rectangle width.
    pub width: u16,
    /// Rectangle height.
    pub height: u16,
}

impl PiScanSetupHitRect {
    /// Return whether one terminal coordinate falls inside this rectangle.
    #[must_use]
    pub const fn contains(self, column: u16, row: u16) -> bool {
        column >= self.x
            && column < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }
}

/// What: Exact Pi capability facts verified for the wizard without a model call.
///
/// Inputs:
/// - Copied from the setup controller's correlated capability event.
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
    /// Exact reservation for every advertised provider/model route.
    pub route_reservations: Vec<(String, String, PiScanReservation)>,
    /// Exact worst-case token/micro-USD reservation for the initial reviewed route.
    pub reservation: PiScanReservation,
    /// SHA-256 binding over exact Pi-reported pricing/provenance.
    pub pricing_binding: String,
    /// Unix timestamp of the exact pricing observation.
    pub pricing_observed_at_unix_seconds: u64,
    /// Maximum accepted age of that pricing observation.
    pub maximum_pricing_age_seconds: u64,
    /// Human-readable exact pricing/provenance summary lines.
    pub pricing_summary: Vec<String>,
}

impl Default for PiScanSetupVerifiedFacts {
    fn default() -> Self {
        Self {
            pi_version: String::new(),
            routes: Vec::new(),
            route_reservations: Vec::new(),
            reservation: PiScanReservation {
                tokens: 0,
                cost_microusd: 0,
            },
            pricing_binding: String::new(),
            pricing_observed_at_unix_seconds: 0,
            maximum_pricing_age_seconds: 0,
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
    /// Vertical body scroll offset for narrow terminals and long review/error content.
    pub body_scroll: u16,
    /// Immutable settings snapshot retained for Cancel verification.
    pub original_settings: PiScanSettings,
    /// Immutable consent snapshot retained for Cancel verification.
    pub original_consent: PiScanConsentState,
    /// Mutable candidate settings edited by the wizard.
    pub candidate: PiScanSettings,
    /// Candidate independent observation/paid-background consent choices.
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
    /// Inert action awaiting central setup-controller dispatch.
    pub pending_action: Option<PiScanSetupDraftAction>,
    /// Mouse hit rectangles refreshed on each render.
    pub hit_rects: Vec<PiScanSetupHitRect>,
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
    /// - Nothing outside the isolated draft is changed. Repository defaults keep
    ///   initial observation, background paid execution, fallback, and cost off.
    #[must_use]
    pub fn open(settings: PiScanSettings, consent: PiScanConsentState, first_run: bool) -> Self {
        Self {
            step: PiScanSetupStep::Welcome,
            focus: 0,
            body_scroll: 0,
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
            pending_action: None,
            hit_rects: Vec::new(),
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

    /// Return whether keyboard focus currently owns an editable text field.
    #[must_use]
    pub const fn focuses_text_field(&self) -> bool {
        matches!(
            (self.step, self.focus),
            (PiScanSetupStep::PiReadiness, 0) | (PiScanSetupStep::OptionalBehavior, 7)
        )
    }

    /// Return the number of page-local controls available to keyboard focus.
    #[must_use]
    pub const fn focus_count(&self) -> usize {
        match self.step {
            PiScanSetupStep::Welcome | PiScanSetupStep::Review | PiScanSetupStep::Activate => 0,
            PiScanSetupStep::PiReadiness | PiScanSetupStep::Route => 2,
            PiScanSetupStep::PricingPrivacy => 3,
            PiScanSetupStep::OptionalBehavior => 8,
        }
    }

    /// Move the body viewport by a bounded number of rendered lines.
    pub const fn scroll_body(&mut self, down: bool) {
        self.body_scroll = if down {
            self.body_scroll.saturating_add(3)
        } else {
            self.body_scroll.saturating_sub(3)
        };
    }

    /// Move focus by one control, wrapping within the current page.
    pub fn move_focus(&mut self, forward: bool) {
        let count = self.focus_count();
        if count == 0 {
            self.focus = 0;
        } else if forward {
            self.focus = (self.focus + 1) % count;
        } else {
            self.focus = self.focus.checked_sub(1).unwrap_or(count - 1);
        }
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

    /// Return the exact worst-case reservation for the current primary and fallback routes.
    #[must_use]
    pub fn reviewed_reservation(&self) -> PiScanReservation {
        let Some(facts) = &self.verified else {
            return PiScanReservation {
                tokens: 0,
                cost_microusd: 0,
            };
        };
        let mut selected = vec![(
            self.candidate.provider.as_str(),
            self.candidate.model.as_str(),
        )];
        selected.extend(
            self.candidate
                .fallback_models
                .split(',')
                .filter_map(|fallback| fallback.trim().split_once('/')),
        );
        facts
            .route_reservations
            .iter()
            .filter(|(provider, model, _)| {
                selected.iter().any(|(selected_provider, selected_model)| {
                    provider == selected_provider && model == selected_model
                })
            })
            .fold(
                PiScanReservation {
                    tokens: 0,
                    cost_microusd: 0,
                },
                |reservation, (_, _, route)| PiScanReservation {
                    tokens: reservation.tokens.max(route.tokens),
                    cost_microusd: reservation.cost_microusd.max(route.cost_microusd),
                },
            )
    }

    /// Queue a no-model capability probe, or record dry-run guidance without a request.
    pub fn request_probe(&mut self, dry_run: bool) {
        self.validation_issues.clear();
        if dry_run {
            self.validation_issues
                .push("app.pi_scan.wizard.validation.dry_run_probe".to_string());
            self.notice = Some("app.pi_scan.wizard.notices.dry_run_review".to_string());
            return;
        }
        if self.candidate.binary.trim().is_empty() {
            self.validation_issues
                .push("app.pi_scan.wizard.validation.binary_required".to_string());
            return;
        }
        let binary = self.candidate.binary.trim().to_string();
        let correlation_id = self.next_correlation();
        self.pending_action = Some(PiScanSetupDraftAction::Probe {
            correlation_id,
            binary,
        });
        self.notice = Some("app.pi_scan.wizard.notices.verifying".to_string());
    }

    /// What: Accept exact verified facts from a matching probe response.
    ///
    /// Inputs:
    /// - `correlation_id`: Controller response correlation.
    /// - `facts`: Exact version, routes, reservation, and pricing provenance.
    ///
    /// Output:
    /// - `true` when accepted; `false` for stale responses.
    ///
    /// Details:
    /// - Empty or duplicate route snapshots remain visible to validation and do
    ///   not auto-select an arbitrary route.
    pub fn accept_verified_facts(
        &mut self,
        correlation_id: u64,
        mut facts: PiScanSetupVerifiedFacts,
    ) -> bool {
        if !self.accepts_correlation(correlation_id) {
            return false;
        }
        facts.routes.sort();
        facts.routes.dedup();
        self.in_flight_correlation = None;
        self.pending_action = None;
        self.validation_issues.clear();
        if facts.routes.is_empty() {
            self.validation_issues
                .push("app.pi_scan.wizard.validation.no_routes".to_string());
            return true;
        }
        let route_changed = !facts.routes.iter().any(|(provider, model)| {
            provider == self.candidate.provider.trim() && model == self.candidate.model.trim()
        });
        if route_changed {
            let (provider, model) = facts.routes[0].clone();
            self.candidate.provider = provider;
            self.candidate.model = model;
            self.notice = Some(format!(
                "Previous route is no longer advertised; selected {}/{} — review before continuing",
                self.candidate.provider, self.candidate.model
            ));
        }
        self.verified = Some(facts);
        if !route_changed {
            self.notice = Some("app.pi_scan.wizard.notices.readiness_verified".to_string());
        }
        true
    }

    /// Select the next or previous exact advertised provider/model route.
    pub fn cycle_route(&mut self, forward: bool) {
        let Some(facts) = &self.verified else {
            return;
        };
        let count = facts.routes.len();
        if count == 0 {
            return;
        }
        let current = facts
            .routes
            .iter()
            .position(|(provider, model)| {
                provider == &self.candidate.provider && model == &self.candidate.model
            })
            .unwrap_or(0);
        let index = if forward {
            (current + 1) % count
        } else {
            current.checked_sub(1).unwrap_or(count - 1)
        };
        let (provider, model) = &facts.routes[index];
        self.candidate.provider.clone_from(provider);
        self.candidate.model.clone_from(model);
        self.invalidate_review();
    }

    /// Cycle through supported conservative thinking-level choices.
    pub fn cycle_thinking(&mut self, forward: bool) {
        const LEVELS: [&str; 4] = ["off", "low", "medium", "high"];
        let current = LEVELS
            .iter()
            .position(|level| *level == self.candidate.thinking)
            .unwrap_or(2);
        let index = if forward {
            (current + 1) % LEVELS.len()
        } else {
            current.checked_sub(1).unwrap_or(LEVELS.len() - 1)
        };
        self.candidate.thinking = LEVELS[index].to_string();
        self.invalidate_review();
    }

    /// Toggle the focused independent confirmation or optional behavior.
    pub fn toggle_focused(&mut self) {
        match (self.step, self.focus) {
            (PiScanSetupStep::PricingPrivacy, 0) => {
                self.confirmations.disclosure_confirmed = !self.confirmations.disclosure_confirmed;
            }
            (PiScanSetupStep::PricingPrivacy, 1) => {
                self.confirmations.foreground_paid_confirmed =
                    !self.confirmations.foreground_paid_confirmed;
            }
            (PiScanSetupStep::PricingPrivacy, 2) => {
                self.confirmations.readiness_warning_confirmed =
                    !self.confirmations.readiness_warning_confirmed;
            }
            (PiScanSetupStep::OptionalBehavior, 0) => {
                self.candidate_consent.background_observation =
                    !self.candidate_consent.background_observation;
            }
            (PiScanSetupStep::OptionalBehavior, 1) => {
                let enabled = !self.candidate.background_enabled;
                self.candidate.background_enabled = enabled;
                self.candidate_consent.paid_execution = enabled;
            }
            (PiScanSetupStep::OptionalBehavior, 2) => self.toggle_fallback(),
            _ => return,
        }
        self.invalidate_review();
    }

    /// Adjust the focused route, thinking, or bounded numeric optional setting.
    pub fn adjust_focused(&mut self, increase: bool) {
        match (self.step, self.focus) {
            (PiScanSetupStep::Route, 0) => self.cycle_route(increase),
            (PiScanSetupStep::Route, 1) => self.cycle_thinking(increase),
            (PiScanSetupStep::OptionalBehavior, 3) => {
                self.candidate.background_starts_per_hour =
                    adjust_u32(self.candidate.background_starts_per_hour, increase, 0, 5, 1);
            }
            (PiScanSetupStep::OptionalBehavior, 4) => {
                self.candidate.background_token_cap_24h = adjust_u64(
                    self.candidate.background_token_cap_24h,
                    increase,
                    0,
                    500_000,
                    10_000,
                );
            }
            (PiScanSetupStep::OptionalBehavior, 5) => {
                self.candidate.background_cost_cap_24h =
                    adjust_cost(&self.candidate.background_cost_cap_24h, increase);
            }
            (PiScanSetupStep::OptionalBehavior, 6) => {
                self.candidate.result_retention_days =
                    adjust_u32(self.candidate.result_retention_days, increase, 1, 365, 1);
            }
            _ => return,
        }
        self.invalidate_review();
    }

    /// Edit the focused binary or credential-free proxy text field.
    pub fn edit_text(&mut self, character: Option<char>, backspace: bool) -> bool {
        let edits_binary = matches!((self.step, self.focus), (PiScanSetupStep::PiReadiness, 0));
        let field = match (self.step, self.focus) {
            (PiScanSetupStep::PiReadiness, 0) => &mut self.candidate.binary,
            (PiScanSetupStep::OptionalBehavior, 7) => &mut self.candidate.https_proxy,
            _ => return false,
        };
        if backspace {
            field.pop();
        } else if let Some(character) = character.filter(|value| !value.is_control()) {
            field.push(character);
        }
        if edits_binary {
            self.verified = None;
        }
        self.invalidate_review();
        true
    }

    /// Validate and advance from the current page without applying changes.
    pub fn next(&mut self, dry_run: bool) {
        self.validation_issues = self.current_step_issues();
        if !self.validation_issues.is_empty() {
            return;
        }
        match self.step {
            PiScanSetupStep::Welcome => self.set_step(PiScanSetupStep::PiReadiness),
            PiScanSetupStep::PiReadiness => self.set_step(PiScanSetupStep::Route),
            PiScanSetupStep::Route => self.set_step(PiScanSetupStep::PricingPrivacy),
            PiScanSetupStep::PricingPrivacy => self.set_step(PiScanSetupStep::OptionalBehavior),
            PiScanSetupStep::OptionalBehavior => self.request_validation(dry_run),
            PiScanSetupStep::Review | PiScanSetupStep::Activate => {}
        }
    }

    /// Navigate backward without changing the original settings/runtime/consent projection.
    pub fn back(&mut self) {
        let Some(previous) = self.step.previous() else {
            return;
        };
        self.pending_action = None;
        self.in_flight_correlation = None;
        self.validation_issues.clear();
        self.invalidate_review();
        self.set_step(previous);
    }

    /// Accept a matching controller validation and expose the final Review page.
    pub fn accept_validation(&mut self, correlation_id: u64, validation_binding: String) -> bool {
        if !self.accepts_correlation(correlation_id) {
            return false;
        }
        self.in_flight_correlation = None;
        self.pending_action = None;
        self.validation_binding = validation_binding;
        self.validation_issues.clear();
        self.notice = Some("app.pi_scan.wizard.notices.validation_write_free".to_string());
        self.set_step(PiScanSetupStep::Review);
        true
    }

    /// Queue final transactional Apply, or show dry-run guidance without an action.
    pub fn request_apply(&mut self, dry_run: bool) {
        self.validation_issues.clear();
        if dry_run {
            self.validation_issues
                .push("app.pi_scan.wizard.validation.dry_run_apply".to_string());
            return;
        }
        if self.step != PiScanSetupStep::Review || self.validation_binding.trim().is_empty() {
            self.validation_issues
                .push("app.pi_scan.wizard.validation.validate_before_apply".to_string());
            return;
        }
        self.candidate.enabled = true;
        let validation_binding = self.validation_binding.clone();
        let correlation_id = self.next_correlation();
        self.pending_action = Some(PiScanSetupDraftAction::Apply {
            correlation_id,
            validation_binding,
        });
        self.apply_status = PiScanSetupApplyStatus::Validating;
        self.set_step(PiScanSetupStep::Activate);
    }

    /// Accept one matching apply-progress stage and ignore stale updates.
    pub fn accept_apply_status(
        &mut self,
        correlation_id: u64,
        status: PiScanSetupApplyStatus,
    ) -> bool {
        if !self.accepts_correlation(correlation_id) {
            return false;
        }
        let terminal = matches!(
            status,
            PiScanSetupApplyStatus::Complete | PiScanSetupApplyStatus::Failed(_)
        );
        self.apply_status = status;
        if terminal {
            self.in_flight_correlation = None;
            self.pending_action = None;
        }
        true
    }

    /// Accept one matching controller failure and keep the draft retryable.
    pub fn accept_failure(
        &mut self,
        correlation_id: u64,
        apply_failure: bool,
        reason: String,
    ) -> bool {
        if !self.accepts_correlation(correlation_id) {
            return false;
        }
        self.in_flight_correlation = None;
        self.pending_action = None;
        self.notice = Some(reason.clone());
        if apply_failure {
            self.apply_status = PiScanSetupApplyStatus::Failed(reason);
            self.set_step(PiScanSetupStep::Activate);
        } else {
            self.validation_issues = vec![reason];
        }
        true
    }

    /// Retry the operation appropriate to the current recoverable failure.
    pub fn retry(&mut self, dry_run: bool) {
        if !matches!(self.apply_status, PiScanSetupApplyStatus::Failed(_)) {
            self.request_probe(dry_run);
            return;
        }
        self.apply_status = PiScanSetupApplyStatus::Idle;
        self.set_step(PiScanSetupStep::Review);
        self.request_apply(dry_run);
    }

    /// Return actionable validation issues for the current step.
    #[must_use]
    pub fn current_step_issues(&self) -> Vec<String> {
        match self.step {
            PiScanSetupStep::Welcome | PiScanSetupStep::Review | PiScanSetupStep::Activate => {
                Vec::new()
            }
            PiScanSetupStep::PiReadiness => self.readiness_issues(),
            PiScanSetupStep::Route => self.route_issues(),
            PiScanSetupStep::PricingPrivacy => self.confirmation_issues(),
            PiScanSetupStep::OptionalBehavior => self.optional_issues(),
        }
    }

    /// Replace renderer-owned mouse rectangles after one frame.
    pub fn set_hit_rects(&mut self, hit_rects: Vec<PiScanSetupHitRect>) {
        self.hit_rects = hit_rects;
    }

    /// Resolve one mouse coordinate to its current wizard target.
    #[must_use]
    pub fn hit_test(&self, column: u16, row: u16) -> Option<PiScanSetupHitTarget> {
        self.hit_rects
            .iter()
            .find(|rect| rect.contains(column, row))
            .map(|rect| rect.target)
    }

    /// Toggle a conservative single exact fallback route.
    fn toggle_fallback(&mut self) {
        if !self.candidate.fallback_models.trim().is_empty() {
            self.candidate.fallback_models.clear();
            self.confirmations.fallback_confirmed = false;
            return;
        }
        let Some(route) = self.verified.as_ref().and_then(|facts| {
            facts.routes.iter().find(|(provider, model)| {
                provider != &self.candidate.provider || model != &self.candidate.model
            })
        }) else {
            return;
        };
        self.candidate.fallback_models = format!("{}/{}", route.0, route.1);
        self.confirmations.fallback_confirmed = true;
    }

    /// Queue inert candidate validation before exposing Review.
    fn request_validation(&mut self, dry_run: bool) {
        if dry_run {
            self.validation_issues
                .push("app.pi_scan.wizard.validation.dry_run_validation".to_string());
            return;
        }
        self.candidate.enabled = true;
        let correlation_id = self.next_correlation();
        self.pending_action = Some(PiScanSetupDraftAction::Validate { correlation_id });
        self.notice = Some("app.pi_scan.wizard.notices.validating".to_string());
    }

    /// Return readiness-page issues.
    fn readiness_issues(&self) -> Vec<String> {
        if self.candidate.binary.trim().is_empty() {
            return vec!["app.pi_scan.wizard.validation.binary_required".to_string()];
        }
        if self.verified.is_none() {
            return vec!["app.pi_scan.wizard.validation.verify_first".to_string()];
        }
        Vec::new()
    }

    /// Return route-page issues.
    fn route_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if !self.candidate_route_advertised() {
            issues.push("app.pi_scan.wizard.validation.route_required".to_string());
        }
        if !matches!(
            self.candidate.thinking.as_str(),
            "off" | "low" | "medium" | "high"
        ) {
            issues.push("app.pi_scan.wizard.validation.thinking_required".to_string());
        }
        issues
    }

    /// Return pricing/privacy-page issues.
    fn confirmation_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if !self.confirmations.disclosure_confirmed {
            issues.push("app.pi_scan.wizard.validation.disclosure_required".to_string());
        }
        if !self.confirmations.foreground_paid_confirmed {
            issues.push("app.pi_scan.wizard.validation.foreground_required".to_string());
        }
        issues
    }

    /// Return optional-settings issues without mutating or silently clamping values.
    fn optional_issues(&self) -> Vec<String> {
        let mut issues = self.candidate.validation_issues();
        if !self.candidate.fallback_models.trim().is_empty()
            && !self.confirmations.fallback_confirmed
        {
            issues.push("app.pi_scan.wizard.validation.fallback_required".to_string());
        }
        if self.candidate.background_enabled && !self.candidate_consent.paid_execution {
            issues.push("app.pi_scan.wizard.validation.background_paid_required".to_string());
        }
        issues
    }

    /// Change page and reset page-local focus.
    fn set_step(&mut self, step: PiScanSetupStep) {
        self.step = step;
        self.focus = 0;
        self.body_scroll = 0;
        self.hit_rects.clear();
    }

    /// Clear reviewed bindings after any draft mutation.
    fn invalidate_review(&mut self) {
        self.validation_binding.clear();
        self.validation_issues.clear();
    }
}

/// Adjust one bounded unsigned 32-bit setting without overflow.
fn adjust_u32(value: u32, increase: bool, minimum: u32, maximum: u32, step: u32) -> u32 {
    if increase {
        value.saturating_add(step).min(maximum)
    } else {
        value.saturating_sub(step).max(minimum)
    }
}

/// Adjust one bounded unsigned 64-bit setting without overflow.
fn adjust_u64(value: u64, increase: bool, minimum: u64, maximum: u64, step: u64) -> u64 {
    if increase {
        value.saturating_add(step).min(maximum)
    } else {
        value.saturating_sub(step).max(minimum)
    }
}

/// Adjust the optional background dollar cap in conservative one-dollar steps.
fn adjust_cost(value: &str, increase: bool) -> String {
    let whole = value
        .split_once('.')
        .map_or(value, |(whole, _)| whole)
        .parse::<u64>()
        .unwrap_or(0);
    let adjusted = if increase {
        whole.saturating_add(1).min(10_000)
    } else {
        whole.saturating_sub(1)
    };
    format!("{adjusted}.00")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build exact verified facts used by deterministic state tests.
    fn facts() -> PiScanSetupVerifiedFacts {
        PiScanSetupVerifiedFacts {
            pi_version: "0.84.0".to_string(),
            routes: vec![
                ("provider-a".to_string(), "model-a".to_string()),
                ("provider-b".to_string(), "model-b".to_string()),
            ],
            route_reservations: vec![
                (
                    "provider-a".to_string(),
                    "model-a".to_string(),
                    PiScanReservation {
                        tokens: 10_000,
                        cost_microusd: 50,
                    },
                ),
                (
                    "provider-b".to_string(),
                    "model-b".to_string(),
                    PiScanReservation {
                        tokens: 10_000,
                        cost_microusd: 500,
                    },
                ),
            ],
            reservation: PiScanReservation {
                tokens: 10_000,
                cost_microusd: 50,
            },
            pricing_binding: "pricing".to_string(),
            pricing_observed_at_unix_seconds: 1_000,
            maximum_pricing_age_seconds: 900,
            pricing_summary: vec!["provider-a/model-a exact Pi metadata".to_string()],
        }
    }

    /// Opening the wizard must copy, not mutate, the original snapshots.
    #[test]
    fn open_preserves_original_snapshots_and_conservative_defaults() {
        let settings = PiScanSettings::default();
        let consent = PiScanConsentState::default();
        let wizard = PiScanSetupWizardState::open(settings.clone(), consent, true);
        assert_eq!(wizard.original_settings, settings);
        assert_eq!(wizard.candidate, settings);
        assert_eq!(wizard.original_consent, consent);
        assert!(!wizard.candidate.background_enabled);
        assert!(!wizard.candidate_consent.background_observation);
        assert!(wizard.candidate.fallback_models.is_empty());
        assert_eq!(wizard.candidate.thinking, "medium");
        assert_eq!(wizard.candidate.background_cost_cap_24h, "0.00");
    }

    /// Correlation acceptance must reject stale probe, validation, and apply responses.
    #[test]
    fn stale_correlations_cannot_advance_or_complete_the_wizard() {
        let mut wizard = PiScanSetupWizardState::open(
            PiScanSettings::default(),
            PiScanConsentState::default(),
            true,
        );
        wizard.request_probe(false);
        let first = wizard.last_correlation;
        wizard.request_probe(false);
        let second = wizard.last_correlation;
        assert!(!wizard.accept_verified_facts(first, facts()));
        assert!(wizard.verified.is_none());
        assert!(wizard.accept_verified_facts(second, facts()));
        assert!(wizard.verified.is_some());
        assert!(!wizard.accept_validation(first, "stale".to_string()));
        assert_ne!(wizard.step, PiScanSetupStep::Review);
    }

    /// The state machine must require exact routes and separate confirmations.
    #[test]
    fn seven_step_flow_requires_exact_route_and_independent_confirmations() {
        let mut wizard = PiScanSetupWizardState::open(
            PiScanSettings::default(),
            PiScanConsentState::default(),
            true,
        );
        wizard.next(false);
        wizard.request_probe(false);
        let correlation = wizard.last_correlation;
        assert!(wizard.accept_verified_facts(correlation, facts()));
        wizard.next(false);
        assert_eq!(wizard.step, PiScanSetupStep::Route);
        assert!(wizard.candidate_route_advertised());
        wizard.next(false);
        assert_eq!(wizard.step, PiScanSetupStep::PricingPrivacy);
        wizard.toggle_focused();
        wizard.focus = 1;
        wizard.toggle_focused();
        wizard.next(false);
        assert_eq!(wizard.step, PiScanSetupStep::OptionalBehavior);
        assert!(!wizard.candidate_consent.background_observation);
        assert!(!wizard.candidate_consent.paid_execution);
        assert!(wizard.candidate.fallback_models.is_empty());
        wizard.next(false);
        assert!(matches!(
            wizard.pending_action,
            Some(PiScanSetupDraftAction::Validate { .. })
        ));
        let validation = wizard.last_correlation;
        assert!(wizard.accept_validation(validation, "binding".to_string()));
        assert_eq!(wizard.step, PiScanSetupStep::Review);
        wizard.request_apply(false);
        assert_eq!(wizard.step, PiScanSetupStep::Activate);
        assert!(matches!(
            wizard.pending_action,
            Some(PiScanSetupDraftAction::Apply { .. })
        ));
    }

    /// Re-probing must call attention to an automatically replaced route.
    #[test]
    fn unavailable_candidate_route_sets_review_notice() {
        let mut wizard = PiScanSetupWizardState::open(
            PiScanSettings::default(),
            PiScanConsentState::default(),
            false,
        );
        wizard.candidate.provider = "removed-provider".to_string();
        wizard.candidate.model = "removed-model".to_string();
        wizard.request_probe(false);
        let correlation = wizard.last_correlation;

        assert!(wizard.accept_verified_facts(correlation, facts()));

        assert_eq!(wizard.candidate.provider, "provider-a");
        assert_eq!(wizard.candidate.model, "model-a");
        assert!(
            wizard
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("no longer advertised"))
        );
    }

    /// Displayed reservation must follow the current primary and ordered fallback choices.
    #[test]
    fn reviewed_reservation_tracks_route_and_fallback_changes() {
        let mut wizard = PiScanSetupWizardState::open(
            PiScanSettings::default(),
            PiScanConsentState::default(),
            true,
        );
        wizard.verified = Some(facts());
        wizard.candidate.provider = "provider-a".to_string();
        wizard.candidate.model = "model-a".to_string();
        assert_eq!(wizard.reviewed_reservation().cost_microusd, 50);

        wizard.cycle_route(true);
        assert_eq!(wizard.reviewed_reservation().cost_microusd, 500);

        wizard.candidate.provider = "provider-a".to_string();
        wizard.candidate.model = "model-a".to_string();
        wizard.candidate.fallback_models = "provider-b/model-b".to_string();
        assert_eq!(wizard.reviewed_reservation().cost_microusd, 500);
    }

    /// Dry-run must never queue a probe, validation, or Apply action.
    #[test]
    fn dry_run_queues_no_probe_validation_or_apply() {
        let mut wizard = PiScanSetupWizardState::open(
            PiScanSettings::default(),
            PiScanConsentState::default(),
            true,
        );
        wizard.request_probe(true);
        assert!(wizard.pending_action.is_none());
        wizard.step = PiScanSetupStep::OptionalBehavior;
        wizard.verified = Some(facts());
        wizard.candidate.provider = "provider-a".to_string();
        wizard.candidate.model = "model-a".to_string();
        wizard.next(true);
        assert!(wizard.pending_action.is_none());
        wizard.step = PiScanSetupStep::Review;
        wizard.validation_binding = "binding".to_string();
        wizard.request_apply(true);
        assert!(wizard.pending_action.is_none());
    }

    /// Mouse rectangles must use half-open bounds and preserve semantic targets.
    #[test]
    fn hit_testing_is_deterministic() {
        let mut wizard = PiScanSetupWizardState::open(
            PiScanSettings::default(),
            PiScanConsentState::default(),
            true,
        );
        wizard.set_hit_rects(vec![PiScanSetupHitRect {
            target: PiScanSetupHitTarget::Next,
            x: 4,
            y: 8,
            width: 6,
            height: 1,
        }]);
        assert_eq!(wizard.hit_test(4, 8), Some(PiScanSetupHitTarget::Next));
        assert_eq!(wizard.hit_test(9, 8), Some(PiScanSetupHitTarget::Next));
        assert_eq!(wizard.hit_test(10, 8), None);
    }
}
