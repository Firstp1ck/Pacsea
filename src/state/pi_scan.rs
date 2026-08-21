//! Persistent state and deterministic policy for the optional Pi scan runtime.
//!
//! This module deliberately contains no UI state and launches no process. It owns the
//! identity-keyed sequential queue, consent and pause policy, rolling reservations,
//! crash recovery, and private versioned persistence used by the runtime worker.

use crate::logic::pi_scan::identity::{CommitOid, PackageBase};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Runtime-state schema understood by this build.
pub const PI_SCAN_RUNTIME_SCHEMA_VERSION: u32 = 1;
/// Rolling unattended start window in seconds.
pub const START_WINDOW_SECONDS: u64 = 60 * 60;
/// Rolling token and cost window in seconds.
pub const USAGE_WINDOW_SECONDS: u64 = 24 * 60 * 60;
/// Default unattended starts allowed per rolling hour.
pub const DEFAULT_STARTS_PER_HOUR: u32 = 5;
/// Default unattended token allowance per rolling 24 hours.
pub const DEFAULT_TOKEN_CAP_24H: u64 = 500_000;

/// Counter used only to make same-directory atomic temporary names collision-resistant.
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// What: Immutable queue identity for one package recipe commit.
///
/// Inputs:
/// - A validated package base and full immutable Git commit OID.
///
/// Output:
/// - Stable ordering, hashing, and persistence identity.
///
/// Details:
/// - Different commits are never merged. The runtime rejects only an exact duplicate
///   that is already pending or active.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PiScanQueueKey {
    /// Validated AUR package base.
    pub package_base: PackageBase,
    /// Full immutable recipe commit OID.
    pub commit_oid: CommitOid,
}

/// Whether a queued scan is user-foreground or unattended background work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PiScanPriority {
    /// User-requested work that runs next without preempting an active scan.
    Foreground,
    /// Continuously processed unattended work subject to all background budgets.
    Background,
}

/// What: Worst-case reservation attached to a scan before model use.
///
/// Inputs:
/// - Conservative token and micro-USD cost estimates.
///
/// Output:
/// - Values charged while active and charged in full after interruption.
///
/// Details:
/// - Micro-USD integers avoid floating-point drift in persistence and comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanReservation {
    /// Worst-case tokens reserved for the logical scan.
    pub tokens: u64,
    /// Worst-case cost in millionths of one US dollar.
    pub cost_microusd: u64,
}

/// What: One immutable request retained in the sequential queue.
///
/// Inputs:
/// - Request id, queue identity, priority, reservation, and manual override confirmation.
///
/// Output:
/// - Persistable request consumed by the runtime scheduler.
///
/// Details:
/// - `manual_budget_override_confirmed` is recorded for later UI/provenance use. Background
///   work can never use it to bypass unattended limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanJobRequest {
    /// Caller-generated request identifier.
    pub request_id: u64,
    /// Exact package-base and commit identity.
    pub key: PiScanQueueKey,
    /// Foreground or background scheduling class.
    pub priority: PiScanPriority,
    /// Worst-case model reservation.
    pub reservation: PiScanReservation,
    /// Whether the user confirmed a worst-case manual budget override.
    pub manual_budget_override_confirmed: bool,
}

/// Independent user consent switches for observation and paid execution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanConsentState {
    /// Allows read-only background observation and backlog discovery.
    pub background_observation: bool,
    /// Allows a queued item to enter paid model execution.
    pub paid_execution: bool,
}

/// Sticky reasons that stop new work without preempting an active scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PiScanPauseReason {
    /// Explicit user pause; only an explicit user resume clears it.
    User,
    /// Service, auth, readiness, security, or persistence pause; validation clears it.
    Service,
    /// Rolling budget pause; scheduler revalidation may clear it automatically.
    Budget,
}

/// What: Configurable unattended rolling-budget limits.
///
/// Inputs:
/// - Starts/hour, tokens/24h, and cost/24h.
///
/// Output:
/// - Limits used before reserving a background start.
///
/// Details:
/// - Numeric zero is the persisted representation of Unlimited for each independent limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanBudgetLimits {
    /// Maximum unattended starts in the rolling hour.
    pub starts_per_hour: u32,
    /// Maximum unattended tokens in the rolling 24-hour window.
    pub tokens_per_24h: u64,
    /// Maximum unattended cost in micro-USD in the rolling 24-hour window.
    pub cost_microusd_per_24h: u64,
}

impl Default for PiScanBudgetLimits {
    fn default() -> Self {
        Self {
            starts_per_hour: DEFAULT_STARTS_PER_HOUR,
            tokens_per_24h: DEFAULT_TOKEN_CAP_24H,
            cost_microusd_per_24h: 0,
        }
    }
}

/// One independently configurable unattended budget dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PiScanBudgetDimension {
    /// Rolling starts-per-hour limit.
    Starts,
    /// Rolling tokens-per-24-hours limit.
    Tokens,
    /// Rolling micro-USD-per-24-hours limit.
    Cost,
}

/// User-selected policy for currently exceeded unattended limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanBudgetAdjustment {
    /// Multiply each affected finite limit by exactly two with checked arithmetic.
    Double,
    /// Set each affected limit to numeric zero, the persisted Unlimited representation.
    Unlimited,
}

/// Rejected budget adjustment that leaves policy and pauses unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanBudgetAdjustmentError {
    /// Exact doubling overflowed the native type for one affected dimension.
    Overflow(PiScanBudgetDimension),
}

impl fmt::Display for PiScanBudgetAdjustmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Overflow(dimension) = self;
        write!(
            formatter,
            "Pi scan {dimension:?} budget cannot be doubled without overflow; choose Unlimited or enter a smaller finite value"
        )
    }
}

impl std::error::Error for PiScanBudgetAdjustmentError {}

/// What: Authoritative result of applying one budget adjustment.
///
/// Inputs:
/// - Previous/current limits and scheduler-classified affected/remaining dimensions.
///
/// Output:
/// - Projection-safe evidence of the exact mutation and derived pause state.
///
/// Details:
/// - User and service pauses are not represented as budget success and remain independently sticky.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiScanBudgetAdjustmentResult {
    /// Limits before the adjustment.
    pub previous_limits: PiScanBudgetLimits,
    /// Limits after the adjustment.
    pub current_limits: PiScanBudgetLimits,
    /// Dimensions exceeded when Apply was authoritatively recomputed.
    pub affected: BTreeSet<PiScanBudgetDimension>,
    /// Dimensions still exceeded after applying and revalidating the selected policy.
    pub remaining_exceeded: BTreeSet<PiScanBudgetDimension>,
    /// Whether the scheduler-derived Budget pause remains.
    pub budget_paused: bool,
}

/// Accounting source for one started scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PiScanAccountingClass {
    /// Unattended usage counted against rolling limits.
    Background,
    /// Manual usage retained separately and not charged to unattended caps.
    Foreground,
}

/// What: One rolling reservation/accounting record.
///
/// Inputs:
/// - Start time, class, reservation, and optional reconciled usage.
///
/// Output:
/// - Conservative effective usage for rolling checks.
///
/// Details:
/// - `None` consumption means the reservation remains fully charged. Interrupted and
///   cancelled work explicitly records full consumption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanBudgetRecord {
    /// Runtime correlation id for the start.
    pub correlation_id: u64,
    /// Start timestamp in Unix seconds.
    pub started_at_unix: u64,
    /// Background or foreground accounting class.
    pub class: PiScanAccountingClass,
    /// Worst-case reservation.
    pub reserved: PiScanReservation,
    /// Reconciled token use, or `None` while the full reservation applies.
    pub consumed_tokens: Option<u64>,
    /// Reconciled micro-USD use, or `None` while the full reservation applies.
    pub consumed_cost_microusd: Option<u64>,
}

impl PiScanBudgetRecord {
    /// Return the conservatively charged token count.
    #[must_use]
    pub fn effective_tokens(&self) -> u64 {
        self.consumed_tokens.unwrap_or(self.reserved.tokens)
    }

    /// Return the conservatively charged micro-USD cost.
    #[must_use]
    pub fn effective_cost_microusd(&self) -> u64 {
        self.consumed_cost_microusd
            .unwrap_or(self.reserved.cost_microusd)
    }

    /// Consume the full reservation after cancellation, interruption, or unknown usage.
    pub const fn consume_full_reservation(&mut self) {
        self.consumed_tokens = Some(self.reserved.tokens);
        self.consumed_cost_microusd = Some(self.reserved.cost_microusd);
    }
}

/// Durable rolling budget state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanBudgetState {
    /// Started reservations retained for the rolling 24-hour window.
    pub records: Vec<PiScanBudgetRecord>,
}

/// One active sequential queue item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanActiveItem {
    /// Monotonic runtime correlation id.
    pub correlation_id: u64,
    /// Exact queued request.
    pub request: PiScanJobRequest,
    /// Unix timestamp when execution was dispatched.
    pub started_at_unix: u64,
    /// Sticky suppression set by cancellation before any late completion is considered.
    pub cancellation_suppressed: bool,
}

/// Terminal status retained for recovery and UI integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PiScanTerminalStatus {
    /// Strictly correlated completion was accepted.
    Completed,
    /// User cancellation completed or was forced.
    Cancelled,
    /// Active work was recovered after an unclean stop or shutdown deadline.
    Interrupted,
    /// Execution failed without a validated result.
    Failed,
}

/// Terminal queue history record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanTerminalRecord {
    /// Exact request identity and reservation.
    pub request: PiScanJobRequest,
    /// Correlation id of the terminal attempt.
    pub correlation_id: u64,
    /// Terminal status.
    pub status: PiScanTerminalStatus,
    /// Unix timestamp of the transition.
    pub finished_at_unix: u64,
}

/// What: Durable runtime state independent from top-level `AppState`.
///
/// Inputs:
/// - Queue, active item, terminal history, consent, pauses, budgets, and correlation counter.
///
/// Output:
/// - Cohesive state root for WS3 and later WS4 projection.
///
/// Details:
/// - No top-level UI state changes are required. WS4 can hold this type or a read-only
///   projection while requests continue through typed runtime channels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanRuntimeState {
    /// Oldest-first pending work without commit coalescing.
    pub queue: VecDeque<PiScanJobRequest>,
    /// At most one active item.
    pub active: Option<PiScanActiveItem>,
    /// Bounded-by-retention terminal history; WS4/WS5 owns retention presentation.
    pub terminal: Vec<PiScanTerminalRecord>,
    /// Independent observation and paid-execution consent.
    pub consent: PiScanConsentState,
    /// Sticky pause reasons.
    pub pause_reasons: BTreeSet<PiScanPauseReason>,
    /// Configurable rolling limits.
    pub budget_limits: PiScanBudgetLimits,
    /// Rolling reservation/accounting history.
    pub budget: PiScanBudgetState,
    /// Last allocated runtime correlation id.
    pub next_correlation_id: u64,
    /// Marks an unclean or durability-uncertain shutdown for recovery UI.
    pub recovery_marker: bool,
}

/// Queue, scheduling, correlation, or accounting error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanStateError {
    /// The same package-base and commit is already pending or active.
    DuplicateIdentity(PiScanQueueKey),
    /// A completion or cancellation did not match the active correlation and identity.
    StaleResponse,
    /// No active item exists for the requested transition.
    NoActiveItem,
    /// Reconciled usage exceeded the reserved maximum.
    UsageExceedsReservation,
}

impl fmt::Display for PiScanStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity(key) => write!(
                formatter,
                "Pi scan for {} at {} is already pending or active; wait for it to finish before retrying",
                key.package_base, key.commit_oid
            ),
            Self::StaleResponse => write!(
                formatter,
                "stale Pi scan response did not match the active package, commit, and correlation id; it was ignored"
            ),
            Self::NoActiveItem => write!(formatter, "no Pi scan is currently active"),
            Self::UsageExceedsReservation => write!(
                formatter,
                "reported Pi usage exceeds its confirmed reservation; keep the full reservation and review provider accounting"
            ),
        }
    }
}

impl std::error::Error for PiScanStateError {}

/// Reason a pending item cannot start yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiScanStartBlock {
    /// Runtime feature gate is disabled or unsupported.
    RuntimeDisabled,
    /// Paid execution has not been consented independently from observation.
    PaidExecutionNotConsented,
    /// A sticky user or service pause is present.
    Paused(PiScanPauseReason),
    /// A rolling unattended limit cannot fit the next reservation.
    Budget,
    /// Monotonic runtime correlation id space was exhausted.
    CorrelationExhausted,
}

/// Correlated actual usage reported by a completed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiScanActualUsage {
    /// Actual or conservatively estimated tokens.
    pub tokens: u64,
    /// Actual or conservatively estimated cost in micro-USD.
    pub cost_microusd: u64,
}

impl PiScanRuntimeState {
    /// What: Append a unique queue item without commit coalescing.
    ///
    /// Inputs:
    /// - `request`: Frozen package-base and commit request.
    ///
    /// Output:
    /// - Queue length after insertion.
    ///
    /// Details:
    /// - Different commits always remain distinct and preserve insertion order.
    ///
    /// # Errors
    /// - Returns [`PiScanStateError::DuplicateIdentity`] for an exact pending/active key.
    pub fn enqueue(&mut self, request: PiScanJobRequest) -> Result<usize, PiScanStateError> {
        let duplicate_pending = self.queue.iter().any(|item| item.key == request.key);
        let duplicate_active = self
            .active
            .as_ref()
            .is_some_and(|item| item.request.key == request.key);
        if duplicate_pending || duplicate_active {
            return Err(PiScanStateError::DuplicateIdentity(request.key));
        }
        self.queue.push_back(request);
        Ok(self.queue.len())
    }

    /// Set independent observation and paid-execution consent.
    pub const fn set_consent(&mut self, consent: PiScanConsentState) {
        self.consent = consent;
    }

    /// Apply or clear the sticky user pause.
    pub fn set_user_paused(&mut self, paused: bool) {
        set_pause_reason(&mut self.pause_reasons, PiScanPauseReason::User, paused);
    }

    /// Apply a service/security/readiness pause.
    pub fn pause_for_service(&mut self) {
        self.pause_reasons.insert(PiScanPauseReason::Service);
    }

    /// Clear the service pause only after the caller reports a successful validation.
    pub fn clear_service_pause(&mut self, validation_succeeded: bool) {
        if validation_succeeded {
            self.pause_reasons.remove(&PiScanPauseReason::Service);
        }
    }

    /// What: Classify every finite budget exceeded by the next queued background reservation.
    ///
    /// Inputs:
    /// - `now_unix`: Deterministic Unix time used for rolling windows.
    ///
    /// Output:
    /// - Scheduler-owned set of exceeded budget dimensions.
    ///
    /// Details:
    /// - Numeric zero means Unlimited and is never classified as exceeded. Unknown active usage
    ///   remains conservatively charged at its reservation, and arithmetic overflow exceeds a
    ///   finite limit rather than wrapping.
    #[must_use]
    pub fn exceeded_budget_limits(&self, now_unix: u64) -> BTreeSet<PiScanBudgetDimension> {
        self.queue
            .iter()
            .find(|item| item.priority == PiScanPriority::Background)
            .map_or_else(BTreeSet::new, |item| {
                self.exceeded_for_reservation(item.reservation, now_unix)
            })
    }

    /// What: Adjust every and only currently exceeded budget and revalidate its derived pause.
    ///
    /// Inputs:
    /// - `adjustment`: Exact checked Double or affected-only Unlimited policy.
    /// - `now_unix`: Deterministic rolling-window timestamp.
    ///
    /// Output:
    /// - Exact previous/current limits, affected set, residual exceeded set, and pause state.
    ///
    /// Details:
    /// - The exceeded set is recomputed at Apply time. Overflow rejects before any mutation;
    ///   user/service pauses and consent are never changed.
    ///
    /// # Errors
    /// - Returns [`PiScanBudgetAdjustmentError::Overflow`] without mutation when Double cannot
    ///   be represented exactly.
    pub fn adjust_exceeded_budgets(
        &mut self,
        adjustment: PiScanBudgetAdjustment,
        now_unix: u64,
    ) -> Result<PiScanBudgetAdjustmentResult, PiScanBudgetAdjustmentError> {
        let previous_limits = self.budget_limits;
        let affected = self.exceeded_budget_limits(now_unix);
        if affected.is_empty() {
            return Ok(PiScanBudgetAdjustmentResult {
                previous_limits,
                current_limits: previous_limits,
                affected,
                remaining_exceeded: BTreeSet::new(),
                budget_paused: false,
            });
        }
        self.prune_budget(now_unix);
        let current_limits = adjusted_limits(previous_limits, &affected, adjustment)?;
        self.budget_limits = current_limits;
        let budget_paused = self.revalidate_budget_pause(now_unix);
        let remaining_exceeded = self.exceeded_budget_limits(now_unix);
        Ok(PiScanBudgetAdjustmentResult {
            previous_limits,
            current_limits,
            affected,
            remaining_exceeded,
            budget_paused,
        })
    }

    /// What: Revalidate and automatically clear or set the rolling budget pause.
    ///
    /// Inputs:
    /// - `now_unix`: Deterministic Unix time used for rolling windows.
    ///
    /// Output:
    /// - Whether a budget pause remains.
    ///
    /// Details:
    /// - User and service pauses are untouched. Foreground work does not clear a budget
    ///   pause that still blocks the next background item.
    pub fn revalidate_budget_pause(&mut self, now_unix: u64) -> bool {
        self.prune_budget(now_unix);
        let next_background = self
            .queue
            .iter()
            .find(|item| item.priority == PiScanPriority::Background);
        let blocked = next_background
            .is_some_and(|item| !self.background_budget_fits(item.reservation, now_unix));
        set_pause_reason(&mut self.pause_reasons, PiScanPauseReason::Budget, blocked);
        blocked
    }

    /// What: Start the next eligible item while preserving active work.
    ///
    /// Inputs:
    /// - `now_unix`: Deterministic Unix start timestamp.
    /// - `runtime_enabled`: Default-off and Arch/Linux gate result.
    ///
    /// Output:
    /// - Newly active item, `None` if idle, or an explicit blocking reason.
    ///
    /// Details:
    /// - An existing active item is never preempted. The oldest foreground item is selected
    ///   ahead of background work; ordering within each priority remains stable.
    ///
    /// # Errors
    /// - Returns the exact policy, budget, platform, or correlation reason that blocks a start.
    pub fn start_next(
        &mut self,
        now_unix: u64,
        runtime_enabled: bool,
    ) -> Result<Option<PiScanActiveItem>, PiScanStartBlock> {
        if self.active.is_some() || self.queue.is_empty() {
            return Ok(None);
        }
        if !runtime_enabled {
            return Err(PiScanStartBlock::RuntimeDisabled);
        }
        if !self.consent.paid_execution {
            return Err(PiScanStartBlock::PaidExecutionNotConsented);
        }
        if let Some(reason) = sticky_non_budget_pause(&self.pause_reasons) {
            return Err(PiScanStartBlock::Paused(reason));
        }
        self.revalidate_budget_pause(now_unix);
        let index = foreground_or_oldest_index(&self.queue);
        let Some(candidate) = self.queue.get(index) else {
            return Ok(None);
        };
        let background_blocked = candidate.priority == PiScanPriority::Background
            && self.pause_reasons.contains(&PiScanPauseReason::Budget);
        let unconfirmed_manual_bypass = candidate.priority == PiScanPriority::Foreground
            && !candidate.manual_budget_override_confirmed
            && !self.background_budget_fits(candidate.reservation, now_unix);
        if background_blocked || unconfirmed_manual_bypass {
            return Err(PiScanStartBlock::Budget);
        }
        let correlation_id = self
            .next_correlation_id
            .checked_add(1)
            .ok_or(PiScanStartBlock::CorrelationExhausted)?;
        self.next_correlation_id = correlation_id;
        let request = self
            .queue
            .remove(index)
            .ok_or(PiScanStartBlock::RuntimeDisabled)?;
        let active = PiScanActiveItem {
            correlation_id,
            started_at_unix: now_unix,
            cancellation_suppressed: false,
            request,
        };
        self.reserve(&active);
        self.active = Some(active.clone());
        Ok(Some(active))
    }

    /// What: Accept only a completion matching active correlation and exact scan identity.
    ///
    /// Inputs:
    /// - `correlation_id`: Runtime attempt correlation.
    /// - `key`: Package-base and commit returned by execution.
    /// - `usage`: Reconciled actual usage.
    /// - `finished_at_unix`: Deterministic terminal timestamp.
    ///
    /// Output:
    /// - Terminal completion record.
    ///
    /// Details:
    /// - A stale or post-cancel response leaves active and terminal state unchanged.
    ///
    /// # Errors
    /// - Returns an identity, correlation, reservation, or accounting error without accepting the response.
    pub fn complete(
        &mut self,
        correlation_id: u64,
        key: &PiScanQueueKey,
        usage: PiScanActualUsage,
        finished_at_unix: u64,
    ) -> Result<PiScanTerminalRecord, PiScanStateError> {
        if self.response_is_stale(correlation_id, key) {
            return Err(PiScanStateError::StaleResponse);
        }
        let active = self.matching_active(correlation_id, key)?;
        if usage.tokens > active.request.reservation.tokens
            || usage.cost_microusd > active.request.reservation.cost_microusd
        {
            return Err(PiScanStateError::UsageExceedsReservation);
        }
        self.reconcile_budget(correlation_id, usage)?;
        let active = self.active.take().ok_or(PiScanStateError::NoActiveItem)?;
        Ok(self.push_terminal(active, PiScanTerminalStatus::Completed, finished_at_unix))
    }

    /// What: Suppress late results, consume the full reservation, and cancel the active item.
    ///
    /// Inputs:
    /// - `correlation_id`: Exact active correlation.
    /// - `finished_at_unix`: Deterministic cancellation timestamp.
    ///
    /// Output:
    /// - Terminal cancellation record.
    ///
    /// Details:
    /// - The worker performs correlated RPC abort and process-group reap before reporting the
    ///   cancellation outcome; this state transition makes response suppression sticky.
    ///
    /// # Errors
    /// - Returns an error when there is no matching active correlation or accounting cannot be finalized.
    pub fn cancel_active(
        &mut self,
        correlation_id: u64,
        finished_at_unix: u64,
    ) -> Result<PiScanTerminalRecord, PiScanStateError> {
        let Some(active) = self.active.as_mut() else {
            return Err(PiScanStateError::NoActiveItem);
        };
        if active.correlation_id != correlation_id {
            return Err(PiScanStateError::StaleResponse);
        }
        active.cancellation_suppressed = true;
        self.consume_full_budget(correlation_id)?;
        let active = self.active.take().ok_or(PiScanStateError::NoActiveItem)?;
        Ok(self.push_terminal(active, PiScanTerminalStatus::Cancelled, finished_at_unix))
    }

    /// What: Recover an active item as interrupted with full reservation consumption.
    ///
    /// Inputs:
    /// - `recovered_at_unix`: Deterministic recovery timestamp.
    ///
    /// Output:
    /// - Interrupted terminal record when an active item existed.
    ///
    /// Details:
    /// - Interrupted work is never automatically requeued; manual retry must enqueue it.
    ///
    /// # Errors
    /// - Returns an accounting error when the active reservation cannot be consumed safely.
    pub fn recover_interrupted(
        &mut self,
        recovered_at_unix: u64,
    ) -> Result<Option<PiScanTerminalRecord>, PiScanStateError> {
        let Some(active) = self.active.take() else {
            return Ok(None);
        };
        self.consume_full_budget(active.correlation_id)?;
        self.recovery_marker = true;
        Ok(Some(self.push_terminal(
            active,
            PiScanTerminalStatus::Interrupted,
            recovered_at_unix,
        )))
    }

    /// Return whether a response should be suppressed as stale or cancelled.
    #[must_use]
    pub fn response_is_stale(&self, correlation_id: u64, key: &PiScanQueueKey) -> bool {
        self.active.as_ref().is_none_or(|active| {
            active.correlation_id != correlation_id
                || active.request.key != *key
                || active.cancellation_suppressed
        })
    }

    /// Remove accounting records outside the rolling 24-hour window.
    pub fn prune_budget(&mut self, now_unix: u64) {
        self.budget.records.retain(|record| {
            now_unix.saturating_sub(record.started_at_unix) < USAGE_WINDOW_SECONDS
        });
    }

    /// Validate and borrow the exact active item.
    fn matching_active(
        &self,
        correlation_id: u64,
        key: &PiScanQueueKey,
    ) -> Result<&PiScanActiveItem, PiScanStateError> {
        let Some(active) = self.active.as_ref() else {
            return Err(PiScanStateError::NoActiveItem);
        };
        if active.correlation_id != correlation_id || active.request.key != *key {
            return Err(PiScanStateError::StaleResponse);
        }
        Ok(active)
    }

    /// Return whether an unattended reservation fits every finite rolling limit.
    fn background_budget_fits(&self, reservation: PiScanReservation, now_unix: u64) -> bool {
        self.exceeded_for_reservation(reservation, now_unix)
            .is_empty()
    }

    /// Classify finite limits exceeded by one reservation under conservative rolling usage.
    fn exceeded_for_reservation(
        &self,
        reservation: PiScanReservation,
        now_unix: u64,
    ) -> BTreeSet<PiScanBudgetDimension> {
        let usage = self.background_usage(now_unix);
        let mut exceeded = BTreeSet::new();
        if finite_starts_exceeded(
            usage.starts,
            usage.starts_overflowed,
            self.budget_limits.starts_per_hour,
        ) {
            exceeded.insert(PiScanBudgetDimension::Starts);
        }
        if finite_usage_exceeded(
            usage.tokens,
            usage.tokens_overflowed,
            reservation.tokens,
            self.budget_limits.tokens_per_24h,
        ) {
            exceeded.insert(PiScanBudgetDimension::Tokens);
        }
        if finite_usage_exceeded(
            usage.cost_microusd,
            usage.cost_overflowed,
            reservation.cost_microusd,
            self.budget_limits.cost_microusd_per_24h,
        ) {
            exceeded.insert(PiScanBudgetDimension::Cost);
        }
        exceeded
    }

    /// Compute conservative rolling background usage without discarding aggregate overflow truth.
    fn background_usage(&self, now_unix: u64) -> RollingBackgroundUsage {
        let mut usage = RollingBackgroundUsage::default();
        for record in self
            .budget
            .records
            .iter()
            .filter(|record| record.class == PiScanAccountingClass::Background)
        {
            let age = now_unix.saturating_sub(record.started_at_unix);
            if age < START_WINDOW_SECONDS {
                let (starts, overflowed) = usage.starts.overflowing_add(1);
                usage.starts = starts;
                usage.starts_overflowed |= overflowed;
            }
            if age < USAGE_WINDOW_SECONDS {
                let (tokens, tokens_overflowed) =
                    usage.tokens.overflowing_add(record.effective_tokens());
                usage.tokens = tokens;
                usage.tokens_overflowed |= tokens_overflowed;
                let (cost, cost_overflowed) = usage
                    .cost_microusd
                    .overflowing_add(record.effective_cost_microusd());
                usage.cost_microusd = cost;
                usage.cost_overflowed |= cost_overflowed;
            }
        }
        usage
    }

    /// Record a new active reservation.
    fn reserve(&mut self, active: &PiScanActiveItem) {
        let class = match active.request.priority {
            PiScanPriority::Foreground => PiScanAccountingClass::Foreground,
            PiScanPriority::Background => PiScanAccountingClass::Background,
        };
        self.budget.records.push(PiScanBudgetRecord {
            correlation_id: active.correlation_id,
            started_at_unix: active.started_at_unix,
            class,
            reserved: active.request.reservation,
            consumed_tokens: None,
            consumed_cost_microusd: None,
        });
    }

    /// Reconcile one exact reservation with bounded actual usage.
    fn reconcile_budget(
        &mut self,
        correlation_id: u64,
        usage: PiScanActualUsage,
    ) -> Result<(), PiScanStateError> {
        let Some(record) = self
            .budget
            .records
            .iter_mut()
            .find(|record| record.correlation_id == correlation_id)
        else {
            return Err(PiScanStateError::NoActiveItem);
        };
        record.consumed_tokens = Some(usage.tokens);
        record.consumed_cost_microusd = Some(usage.cost_microusd);
        Ok(())
    }

    /// Charge one exact reservation in full.
    fn consume_full_budget(&mut self, correlation_id: u64) -> Result<(), PiScanStateError> {
        let Some(record) = self
            .budget
            .records
            .iter_mut()
            .find(|record| record.correlation_id == correlation_id)
        else {
            return Err(PiScanStateError::NoActiveItem);
        };
        record.consume_full_reservation();
        Ok(())
    }

    /// Append one terminal history record.
    fn push_terminal(
        &mut self,
        active: PiScanActiveItem,
        status: PiScanTerminalStatus,
        finished_at_unix: u64,
    ) -> PiScanTerminalRecord {
        let record = PiScanTerminalRecord {
            request: active.request,
            correlation_id: active.correlation_id,
            status,
            finished_at_unix,
        };
        self.terminal.push(record.clone());
        record
    }
}

/// What: Versioned persistence envelope for the cohesive runtime state.
///
/// Inputs:
/// - Supported schema version and runtime state.
///
/// Output:
/// - Atomic private on-disk representation.
///
/// Details:
/// - Newer versions are quarantined and never treated as empty state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiScanPersistedState {
    /// Persistence schema version.
    pub schema_version: u32,
    /// Cohesive runtime state.
    pub state: PiScanRuntimeState,
}

impl Default for PiScanPersistedState {
    fn default() -> Self {
        Self {
            schema_version: PI_SCAN_RUNTIME_SCHEMA_VERSION,
            state: PiScanRuntimeState::default(),
        }
    }
}

/// Versioned persistence and quarantine failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiScanPersistenceError {
    /// State file I/O failed.
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying failure text.
        message: String,
    },
    /// State was malformed and moved to quarantine.
    Corrupt {
        /// Original state path.
        path: PathBuf,
        /// Quarantine destination.
        quarantined_to: PathBuf,
        /// Parse failure text.
        message: String,
    },
    /// State came from a newer unsupported schema and moved to quarantine.
    UnsupportedNewer {
        /// Original state path.
        path: PathBuf,
        /// Observed schema version.
        observed: u32,
        /// Quarantine destination.
        quarantined_to: PathBuf,
    },
    /// Quarantine failed and the original was intentionally left untouched.
    QuarantineFailed {
        /// Original state path.
        path: PathBuf,
        /// Underlying failure text.
        message: String,
    },
}

impl fmt::Display for PiScanPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(
                formatter,
                "could not access Pi scan state {}: {message}; check the Pacsea config directory permissions",
                path.display()
            ),
            Self::Corrupt {
                path,
                quarantined_to,
                message,
            } => write!(
                formatter,
                "Pi scan state {} is corrupt ({message}) and was quarantined to {}; retry recovery or reset only this scanner state",
                path.display(),
                quarantined_to.display()
            ),
            Self::UnsupportedNewer {
                path,
                observed,
                quarantined_to,
            } => write!(
                formatter,
                "Pi scan state {} uses unsupported schema {observed} and was quarantined to {}; upgrade Pacsea or restore a supported backup",
                path.display(),
                quarantined_to.display()
            ),
            Self::QuarantineFailed { path, message } => write!(
                formatter,
                "Pi scan state {} could not be quarantined ({message}); the original was left untouched and scanning remains unavailable",
                path.display()
            ),
        }
    }
}

impl std::error::Error for PiScanPersistenceError {}

/// What: Load versioned runtime state and recover an interrupted active item.
///
/// Inputs:
/// - `path`: Durable runtime state file.
/// - `quarantine_dir`: Private destination for corrupt or newer state.
/// - `now_unix`: Deterministic recovery timestamp.
///
/// Output:
/// - Default state when missing, or supported recovered state.
///
/// Details:
/// - Corrupt/newer state is moved without replacement. Quarantine failure leaves the
///   original untouched and returns an unavailable-state error.
///
/// # Errors
/// - Returns an actionable persistence error for I/O, corruption, newer schema, or
///   interrupted-reservation recovery inconsistency.
pub fn load_pi_scan_state(
    path: &Path,
    quarantine_dir: &Path,
    now_unix: u64,
) -> Result<PiScanPersistedState, PiScanPersistenceError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PiScanPersistedState::default());
        }
        Err(error) => return Err(io_error(path, &error)),
    };
    let header: SchemaHeader = match serde_json::from_slice(&bytes) {
        Ok(header) => header,
        Err(error) => {
            let destination = quarantine_state(path, quarantine_dir, &bytes, "runtime")?;
            return Err(PiScanPersistenceError::Corrupt {
                path: path.to_path_buf(),
                quarantined_to: destination,
                message: error.to_string(),
            });
        }
    };
    if header.schema_version != PI_SCAN_RUNTIME_SCHEMA_VERSION {
        let destination = quarantine_state(path, quarantine_dir, &bytes, "runtime-version")?;
        if header.schema_version > PI_SCAN_RUNTIME_SCHEMA_VERSION {
            return Err(PiScanPersistenceError::UnsupportedNewer {
                path: path.to_path_buf(),
                observed: header.schema_version,
                quarantined_to: destination,
            });
        }
        return Err(PiScanPersistenceError::Corrupt {
            path: path.to_path_buf(),
            quarantined_to: destination,
            message: format!(
                "schema {} has no approved migration to schema {}",
                header.schema_version, PI_SCAN_RUNTIME_SCHEMA_VERSION
            ),
        });
    }
    let mut persisted: PiScanPersistedState = match serde_json::from_slice(&bytes) {
        Ok(state) => state,
        Err(error) => {
            let destination = quarantine_state(path, quarantine_dir, &bytes, "runtime")?;
            return Err(PiScanPersistenceError::Corrupt {
                path: path.to_path_buf(),
                quarantined_to: destination,
                message: error.to_string(),
            });
        }
    };
    persisted
        .state
        .recover_interrupted(now_unix)
        .map_err(|error| PiScanPersistenceError::Io {
            path: path.to_path_buf(),
            message: format!("interrupted reservation recovery failed: {error}"),
        })?;
    Ok(persisted)
}

/// What: Atomically persist private versioned runtime state.
///
/// Inputs:
/// - `path`: Final state file.
/// - `state`: Supported persistence envelope.
///
/// Output:
/// - Fully synced atomic replacement at mode 0600 on Unix.
///
/// Details:
/// - A unique same-directory `create_new` temporary file prevents symlink following and
///   concurrent clobbering; rename and parent-directory sync form the durability boundary.
///
/// # Errors
/// - Returns an actionable I/O error without reinterpreting the prior durable state.
pub fn save_pi_scan_state_atomic(
    path: &Path,
    state: &PiScanPersistedState,
) -> Result<(), PiScanPersistenceError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    create_private_dir(parent)?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|error| PiScanPersistenceError::Io {
        path: path.to_path_buf(),
        message: format!("could not serialize runtime state: {error}"),
    })?;
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        unique_time_nonce(),
        sequence
    ));
    let write_result = write_private_new(&temporary, &bytes);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(io_error(path, &error));
    }
    sync_directory(parent)?;
    Ok(())
}

/// Header parsed before a full versioned state document.
#[derive(Deserialize)]
struct SchemaHeader {
    /// Observed schema version.
    schema_version: u32,
}

/// Conservative rolling totals plus sticky arithmetic-overflow evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RollingBackgroundUsage {
    /// Starts in the rolling hour.
    starts: u32,
    /// Whether the mathematical start count exceeded `u32`.
    starts_overflowed: bool,
    /// Tokens in the rolling 24-hour window when representable.
    tokens: u64,
    /// Whether the mathematical token total exceeded `u64`.
    tokens_overflowed: bool,
    /// Micro-USD in the rolling 24-hour window when representable.
    cost_microusd: u64,
    /// Whether the mathematical cost total exceeded `u64`.
    cost_overflowed: bool,
}

/// Return whether a finite starts limit is exceeded by the next start.
const fn finite_starts_exceeded(current: u32, overflowed: bool, limit: u32) -> bool {
    limit != 0 && (overflowed || current >= limit)
}

/// Return whether a finite usage limit is exceeded by a conservative reservation.
fn finite_usage_exceeded(current: u64, overflowed: bool, reservation: u64, limit: u64) -> bool {
    limit != 0
        && (overflowed
            || current
                .checked_add(reservation)
                .is_none_or(|total| total > limit))
}

/// Build an all-or-nothing adjusted limit set for scheduler-classified dimensions.
fn adjusted_limits(
    previous: PiScanBudgetLimits,
    affected: &BTreeSet<PiScanBudgetDimension>,
    adjustment: PiScanBudgetAdjustment,
) -> Result<PiScanBudgetLimits, PiScanBudgetAdjustmentError> {
    let mut adjusted = previous;
    for dimension in affected {
        match (dimension, adjustment) {
            (PiScanBudgetDimension::Starts, PiScanBudgetAdjustment::Double) => {
                adjusted.starts_per_hour = previous.starts_per_hour.checked_mul(2).ok_or(
                    PiScanBudgetAdjustmentError::Overflow(PiScanBudgetDimension::Starts),
                )?;
            }
            (PiScanBudgetDimension::Tokens, PiScanBudgetAdjustment::Double) => {
                adjusted.tokens_per_24h = previous.tokens_per_24h.checked_mul(2).ok_or(
                    PiScanBudgetAdjustmentError::Overflow(PiScanBudgetDimension::Tokens),
                )?;
            }
            (PiScanBudgetDimension::Cost, PiScanBudgetAdjustment::Double) => {
                adjusted.cost_microusd_per_24h =
                    previous.cost_microusd_per_24h.checked_mul(2).ok_or(
                        PiScanBudgetAdjustmentError::Overflow(PiScanBudgetDimension::Cost),
                    )?;
            }
            (PiScanBudgetDimension::Starts, PiScanBudgetAdjustment::Unlimited) => {
                adjusted.starts_per_hour = 0;
            }
            (PiScanBudgetDimension::Tokens, PiScanBudgetAdjustment::Unlimited) => {
                adjusted.tokens_per_24h = 0;
            }
            (PiScanBudgetDimension::Cost, PiScanBudgetAdjustment::Unlimited) => {
                adjusted.cost_microusd_per_24h = 0;
            }
        }
    }
    Ok(adjusted)
}

/// Set or clear one pause reason without affecting the others.
fn set_pause_reason(
    reasons: &mut BTreeSet<PiScanPauseReason>,
    reason: PiScanPauseReason,
    active: bool,
) {
    if active {
        reasons.insert(reason);
    } else {
        reasons.remove(&reason);
    }
}

/// Return the highest-priority non-budget sticky pause.
fn sticky_non_budget_pause(reasons: &BTreeSet<PiScanPauseReason>) -> Option<PiScanPauseReason> {
    [PiScanPauseReason::User, PiScanPauseReason::Service]
        .into_iter()
        .find(|reason| reasons.contains(reason))
}

/// Return the oldest foreground index, otherwise the oldest queue index.
fn foreground_or_oldest_index(queue: &VecDeque<PiScanJobRequest>) -> usize {
    queue
        .iter()
        .position(|request| request.priority == PiScanPriority::Foreground)
        .unwrap_or(0)
}

/// Create a directory and enforce private Unix permissions.
fn create_private_dir(path: &Path) -> Result<(), PiScanPersistenceError> {
    fs::create_dir_all(path).map_err(|error| io_error(path, &error))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(path, &error))?;
    }
    Ok(())
}

/// Write a new private file and sync its contents.
fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), PiScanPersistenceError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| io_error(path, &error))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| io_error(path, &error))
}

/// Render digest bytes as lowercase hexadecimal without intermediate allocations.
fn format_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

/// Move corrupt/newer state into private quarantine without replacing any artifact.
fn quarantine_state(
    source: &Path,
    quarantine_dir: &Path,
    bytes: &[u8],
    prefix: &str,
) -> Result<PathBuf, PiScanPersistenceError> {
    use sha2::Digest as _;
    create_private_dir(quarantine_dir).map_err(|error| {
        PiScanPersistenceError::QuarantineFailed {
            path: source.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    let digest = sha2::Sha256::digest(bytes);
    let hash = format_hex(&digest);
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let destination = quarantine_dir.join(format!(
        "{prefix}-{}-{}-{sequence}-{hash}.json",
        std::process::id(),
        unique_time_nonce()
    ));
    if destination.exists() {
        return Err(PiScanPersistenceError::QuarantineFailed {
            path: source.to_path_buf(),
            message: format!(
                "quarantine destination {} already exists",
                destination.display()
            ),
        });
    }
    fs::rename(source, &destination).map_err(|error| PiScanPersistenceError::QuarantineFailed {
        path: source.to_path_buf(),
        message: error.to_string(),
    })?;
    if let Err(sync_error) = sync_directory(quarantine_dir) {
        return match fs::rename(&destination, source) {
            Ok(()) => Err(PiScanPersistenceError::QuarantineFailed {
                path: source.to_path_buf(),
                message: format!(
                    "quarantine directory sync failed and the original was restored: {sync_error}"
                ),
            }),
            Err(rollback_error) => Err(PiScanPersistenceError::QuarantineFailed {
                path: source.to_path_buf(),
                message: format!(
                    "quarantine directory sync failed ({sync_error}) and rollback failed ({rollback_error}); inspect {} before recovery",
                    destination.display()
                ),
            }),
        };
    }
    Ok(destination)
}

/// Return a high-resolution nonce for collision-resistant private artifact names.
fn unique_time_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

/// Sync a directory after an atomic rename on Unix.
fn sync_directory(path: &Path) -> Result<(), PiScanPersistenceError> {
    #[cfg(unix)]
    {
        let directory = fs::File::open(path).map_err(|error| io_error(path, &error))?;
        directory
            .sync_all()
            .map_err(|error| io_error(path, &error))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Convert an I/O failure into an actionable persistence error.
fn io_error(path: &Path, error: &std::io::Error) -> PiScanPersistenceError {
    PiScanPersistenceError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
