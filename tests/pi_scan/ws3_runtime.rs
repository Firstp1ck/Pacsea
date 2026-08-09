//! Deterministic WS3 queue, budget, persistence, cancellation, dry-run, and shutdown tests.

use super::*;
use crate::logic::pi_scan::identity::{CommitOid, PackageBase};
use crate::state::pi_scan::{
    PI_SCAN_RUNTIME_SCHEMA_VERSION, PiScanActualUsage, PiScanBudgetLimits, PiScanConsentState,
    PiScanJobRequest, PiScanPauseReason, PiScanPersistedState, PiScanPersistenceError,
    PiScanPriority, PiScanQueueKey, PiScanReservation, PiScanRuntimeState, PiScanStartBlock,
    PiScanTerminalStatus, load_pi_scan_state, save_pi_scan_state_atomic,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, timeout};

/// Build one deterministic immutable queue key.
fn key(package_base: &str, index: u64) -> PiScanQueueKey {
    PiScanQueueKey {
        package_base: PackageBase::new(package_base).expect("valid package base"),
        commit_oid: CommitOid::new(format!("{index:040x}")).expect("valid full OID"),
    }
}

/// Build one deterministic queue request.
fn request(
    request_id: u64,
    package_base: &str,
    priority: PiScanPriority,
    tokens: u64,
    cost_microusd: u64,
) -> PiScanJobRequest {
    PiScanJobRequest {
        request_id,
        key: key(package_base, request_id),
        priority,
        reservation: PiScanReservation {
            tokens,
            cost_microusd,
        },
        manual_budget_override_confirmed: priority == PiScanPriority::Foreground,
    }
}

/// Enable paid execution while leaving observation independently disabled.
fn enable_paid_execution(state: &mut PiScanRuntimeState) {
    state.set_consent(PiScanConsentState {
        background_observation: false,
        paid_execution: true,
    });
}

/// Build enabled runtime options rooted in a temporary directory.
fn enabled_options(root: &std::path::Path, dry_run: bool) -> PiScanRuntimeOptions {
    PiScanRuntimeOptions {
        enabled: true,
        dry_run,
        state_path: root.join("pi_scan").join("backlog-v1.json"),
        quarantine_dir: root.join("pi_scan").join("quarantine"),
        production: None,
    }
}

/// Receive progress until one active dispatch appears.
async fn receive_started(
    channels: &mut PiScanRuntimeChannels,
) -> crate::state::pi_scan::PiScanActiveItem {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Some(PiScanProgressMessage::Started(active)) = channels.progress_rx.recv().await
            {
                return active;
            }
        }
    })
    .await
    .expect("started progress deadline")
}

/// Receive progress until process registration is acknowledged.
async fn receive_session_registration(channels: &mut PiScanRuntimeChannels, correlation_id: u64) {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Some(PiScanProgressMessage::SessionRegistered {
                correlation_id: observed,
            }) = channels.progress_rx.recv().await
                && observed == correlation_id
            {
                return;
            }
        }
    })
    .await
    .expect("session registration deadline");
}

/// What: Verify commit ordering, exact duplicate rejection, foreground priority, and no preemption.
///
/// Inputs:
/// - Two background commits, then one foreground commit while the first is active.
///
/// Output:
/// - First active remains active; foreground runs next; distinct commits are not coalesced.
///
/// Details:
/// - Ordering stays stable within priority and duplicate identity fails explicitly.
#[test]
fn queue_preserves_commits_and_foreground_does_not_preempt() {
    let mut state = PiScanRuntimeState::default();
    enable_paid_execution(&mut state);
    state.budget_limits.cost_microusd_per_24h = 1_000;
    let first = request(1, "demo", PiScanPriority::Background, 10, 0);
    let second = request(2, "demo", PiScanPriority::Background, 10, 0);
    state.enqueue(first.clone()).expect("first queued");
    state.enqueue(second.clone()).expect("second commit queued");
    assert!(state.enqueue(second).is_err(), "exact duplicate must fail");

    let active = state
        .start_next(100, true)
        .expect("eligible")
        .expect("first active");
    assert_eq!(active.request.key, first.key);
    let foreground = request(3, "manual", PiScanPriority::Foreground, 10, 0);
    state
        .enqueue(foreground.clone())
        .expect("foreground queued");
    assert!(
        state
            .start_next(101, true)
            .expect("active preserved")
            .is_none(),
        "manual work must not preempt"
    );
    state
        .complete(
            active.correlation_id,
            &active.request.key,
            PiScanActualUsage {
                tokens: 1,
                cost_microusd: 0,
            },
            102,
        )
        .expect("first completion");
    let next = state
        .start_next(103, true)
        .expect("eligible")
        .expect("foreground next");
    assert_eq!(next.request.key, foreground.key);
    assert_eq!(
        state.queue.front().map(|item| &item.key),
        Some(&first_key(2))
    );
}

/// Return the deterministic key used by a request id under the `demo` package base.
fn first_key(request_id: u64) -> PiScanQueueKey {
    key("demo", request_id)
}

/// What: Verify stale identity/correlation responses cannot complete the active item.
///
/// Inputs:
/// - Active request plus mismatched commit and correlation responses.
///
/// Output:
/// - Both fail as stale and active state remains unchanged.
///
/// Details:
/// - Exact package-base, commit OID, and runtime correlation all participate.
#[test]
fn stale_identity_and_correlation_are_rejected() {
    let mut state = PiScanRuntimeState::default();
    enable_paid_execution(&mut state);
    let job = request(10, "stale-demo", PiScanPriority::Foreground, 100, 0);
    state.enqueue(job).expect("queued");
    let active = state
        .start_next(10, true)
        .expect("eligible")
        .expect("active");
    let usage = PiScanActualUsage {
        tokens: 1,
        cost_microusd: 0,
    };
    assert_eq!(
        state.complete(active.correlation_id, &key("stale-demo", 11), usage, 11),
        Err(crate::state::pi_scan::PiScanStateError::StaleResponse)
    );
    assert_eq!(
        state.complete(active.correlation_id + 1, &active.request.key, usage, 11),
        Err(crate::state::pi_scan::PiScanStateError::StaleResponse)
    );
    assert_eq!(
        state.active.as_ref().map(|item| item.correlation_id),
        Some(active.correlation_id)
    );
}

/// What: Verify five starts/hour, 500k tokens/24h, rolling expiry, and configurable cost caps.
///
/// Inputs:
/// - Five 100k-token background starts and a separate micro-USD budget scenario.
///
/// Output:
/// - Sixth start blocks, 24-hour expiry resumes, and exact configured cost is enforced.
///
/// Details:
/// - Reservations are charged before dispatch and reconciled only on correlated completion.
#[test]
fn rolling_background_budgets_reserve_account_and_resume() {
    let mut state = PiScanRuntimeState::default();
    enable_paid_execution(&mut state);
    for index in 1..=5 {
        let job = request(index, "budget-demo", PiScanPriority::Background, 100_000, 0);
        state.enqueue(job).expect("queued");
        let active = state
            .start_next(1_000 + index, true)
            .expect("within cap")
            .expect("started");
        state
            .complete(
                active.correlation_id,
                &active.request.key,
                PiScanActualUsage {
                    tokens: 100_000,
                    cost_microusd: 0,
                },
                1_000 + index,
            )
            .expect("accounted");
    }
    state
        .enqueue(request(6, "budget-demo", PiScanPriority::Background, 1, 0))
        .expect("sixth queued");
    assert_eq!(state.start_next(1_100, true), Err(PiScanStartBlock::Budget));
    assert!(state.pause_reasons.contains(&PiScanPauseReason::Budget));
    assert!(
        state
            .start_next(1_100 + crate::state::pi_scan::USAGE_WINDOW_SECONDS, true)
            .expect("window expired")
            .is_some()
    );

    let mut cost_state = PiScanRuntimeState {
        budget_limits: PiScanBudgetLimits {
            starts_per_hour: 100,
            tokens_per_24h: 1_000_000,
            cost_microusd_per_24h: 100,
        },
        ..PiScanRuntimeState::default()
    };
    enable_paid_execution(&mut cost_state);
    for (index, cost) in [(1, 60), (2, 40)] {
        let job = request(index, "cost-demo", PiScanPriority::Background, 1, cost);
        cost_state.enqueue(job).expect("cost queued");
        let active = cost_state
            .start_next(index, true)
            .expect("cost fits")
            .expect("cost started");
        cost_state
            .complete(
                active.correlation_id,
                &active.request.key,
                PiScanActualUsage {
                    tokens: 1,
                    cost_microusd: cost,
                },
                index,
            )
            .expect("cost accounted");
    }
    cost_state
        .enqueue(request(3, "cost-demo", PiScanPriority::Background, 1, 1))
        .expect("third queued");
    assert_eq!(
        cost_state.start_next(3, true),
        Err(PiScanStartBlock::Budget)
    );
}

/// What: Verify independent consents and sticky user/service/budget pause clearing rules.
///
/// Inputs:
/// - Observation-only consent, paid consent, user pause, and failed/successful service validation.
///
/// Output:
/// - Paid start remains gated independently and only owning transitions clear sticky reasons.
///
/// Details:
/// - Budget revalidation never clears user or service pauses.
#[test]
fn consent_and_pause_resume_ownership_are_independent() {
    let mut state = PiScanRuntimeState::default();
    state.set_consent(PiScanConsentState {
        background_observation: true,
        paid_execution: false,
    });
    state
        .enqueue(request(1, "pause-demo", PiScanPriority::Background, 1, 0))
        .expect("queued through observation");
    assert_eq!(
        state.start_next(1, true),
        Err(PiScanStartBlock::PaidExecutionNotConsented)
    );
    enable_paid_execution(&mut state);
    state.set_user_paused(true);
    state.pause_for_service();
    assert_eq!(
        state.start_next(2, true),
        Err(PiScanStartBlock::Paused(PiScanPauseReason::User))
    );
    state.clear_service_pause(false);
    state.set_user_paused(false);
    assert_eq!(
        state.start_next(3, true),
        Err(PiScanStartBlock::Paused(PiScanPauseReason::Service))
    );
    state.clear_service_pause(true);
    assert!(state.start_next(4, true).expect("resumed").is_some());
}

/// What: Verify cancellation suppression and interrupted recovery consume full reservations.
///
/// Inputs:
/// - One cancelled active item and one persisted active item recovered after restart.
///
/// Output:
/// - Late completion is stale; both reservations remain fully consumed.
///
/// Details:
/// - Interrupted work becomes terminal and is not automatically retried.
#[test]
fn cancellation_and_recovery_suppress_late_results_and_consume_full() {
    let mut state = PiScanRuntimeState::default();
    enable_paid_execution(&mut state);
    state
        .enqueue(request(
            1,
            "cancel-demo",
            PiScanPriority::Foreground,
            500,
            25,
        ))
        .expect("queued");
    let active = state
        .start_next(1, true)
        .expect("eligible")
        .expect("active");
    let cancelled = state
        .cancel_active(active.correlation_id, 2)
        .expect("cancelled");
    assert_eq!(cancelled.status, PiScanTerminalStatus::Cancelled);
    assert_eq!(state.budget.records[0].effective_tokens(), 500);
    assert_eq!(state.budget.records[0].effective_cost_microusd(), 25);
    assert_eq!(
        state.complete(
            active.correlation_id,
            &active.request.key,
            PiScanActualUsage {
                tokens: 1,
                cost_microusd: 1,
            },
            3,
        ),
        Err(crate::state::pi_scan::PiScanStateError::StaleResponse)
    );

    state
        .enqueue(request(
            2,
            "cancel-demo",
            PiScanPriority::Foreground,
            700,
            30,
        ))
        .expect("retry explicitly queued");
    let interrupted_active = state
        .start_next(4, true)
        .expect("eligible")
        .expect("active");
    let recovered = state
        .recover_interrupted(5)
        .expect("recovery")
        .expect("interrupted record");
    assert_eq!(recovered.status, PiScanTerminalStatus::Interrupted);
    let budget = state
        .budget
        .records
        .iter()
        .find(|record| record.correlation_id == interrupted_active.correlation_id)
        .expect("recovery budget");
    assert_eq!(budget.effective_tokens(), 700);
    assert!(state.active.is_none());
    assert!(state.recovery_marker);
}

/// What: Verify atomic versioned persistence, interrupted recovery, and fail-closed quarantine.
///
/// Inputs:
/// - Persisted active state followed by malformed JSON.
///
/// Output:
/// - Active becomes interrupted on load; malformed state moves to quarantine and errors.
///
/// Details:
/// - Missing state alone returns a supported empty version, while corruption never does.
#[test]
fn persistence_is_versioned_atomic_and_quarantine_aware() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("pi_scan").join("backlog-v1.json");
    let quarantine = temp.path().join("pi_scan").join("quarantine");
    let mut state = PiScanRuntimeState::default();
    enable_paid_execution(&mut state);
    state
        .enqueue(request(
            1,
            "persist-demo",
            PiScanPriority::Foreground,
            99,
            0,
        ))
        .expect("queued");
    state
        .start_next(10, true)
        .expect("eligible")
        .expect("active");
    save_pi_scan_state_atomic(
        &path,
        &PiScanPersistedState {
            schema_version: PI_SCAN_RUNTIME_SCHEMA_VERSION,
            state,
        },
    )
    .expect("atomic save");
    let loaded = load_pi_scan_state(&path, &quarantine, 20).expect("supported load");
    assert!(loaded.state.active.is_none());
    assert_eq!(
        loaded.state.terminal.last().map(|item| item.status),
        Some(PiScanTerminalStatus::Interrupted)
    );
    assert_eq!(loaded.state.budget.records[0].effective_tokens(), 99);

    std::fs::write(&path, b"{not-json").expect("corrupt fixture");
    let error = load_pi_scan_state(&path, &quarantine, 30).expect_err("corrupt must fail");
    assert!(matches!(error, PiScanPersistenceError::Corrupt { .. }));
    assert!(
        !path.exists(),
        "original must move only after quarantine succeeds"
    );
    assert_eq!(
        std::fs::read_dir(&quarantine).expect("quarantine").count(),
        1
    );

    std::fs::write(&path, br#"{"schema_version":2,"state":{}}"#).expect("newer fixture");
    assert!(matches!(
        load_pi_scan_state(&path, &quarantine, 31),
        Err(PiScanPersistenceError::UnsupportedNewer { observed: 2, .. })
    ));
    std::fs::write(&path, br#"{"schema_version":0,"state":{}}"#).expect("older fixture");
    assert!(matches!(
        load_pi_scan_state(&path, &quarantine, 32),
        Err(PiScanPersistenceError::Corrupt { .. })
    ));
    assert_eq!(
        std::fs::read_dir(&quarantine).expect("quarantine").count(),
        3
    );
}

/// What: Verify dry-run emits a preview with zero durable mutation and bounded shutdown ack.
///
/// Inputs:
/// - Enabled dry-run worker and a pre-existing durable sentinel file.
///
/// Output:
/// - Preview is emitted, sentinel bytes are unchanged, and shutdown acknowledges without Pi.
///
/// Details:
/// - No consent, queue, budget, result, pricing, or process state is loaded or persisted.
#[tokio::test]
async fn dry_run_never_mutates_durable_state_or_launches_execution() {
    let temp = tempfile::tempdir().expect("temp dir");
    let options = enabled_options(temp.path(), true);
    std::fs::create_dir_all(options.state_path.parent().expect("parent")).expect("state parent");
    std::fs::write(&options.state_path, b"durable-sentinel").expect("sentinel");
    let mut channels = spawn_pi_scan_worker(options.clone()).expect("dry worker");
    let job = request(1, "dry-demo", PiScanPriority::Foreground, 1_000, 10);
    channels
        .request_tx
        .send(PiScanRequestMessage::Enqueue(job.clone()))
        .expect("preview request");
    let progress = timeout(Duration::from_secs(2), channels.progress_rx.recv())
        .await
        .expect("preview deadline")
        .expect("preview");
    assert_eq!(progress, PiScanProgressMessage::DryRunPreview(job));
    let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
    channels
        .shutdown_tx
        .send(PiScanShutdownMessage {
            acknowledge: ack_tx,
        })
        .expect("shutdown");
    let ack = tokio::task::spawn_blocking(move || ack_rx.recv_timeout(Duration::from_secs(2)))
        .await
        .expect("join")
        .expect("ack");
    assert!(ack.persisted);
    assert!(!ack.active_interrupted);
    assert_eq!(
        std::fs::read(&options.state_path).expect("sentinel read"),
        b"durable-sentinel"
    );
    assert!(!options.quarantine_dir.exists());
}

/// Recorder proving the worker calls the registered correlated abort/reap target.
struct RecordingAbortTarget {
    /// Bound correlation id.
    correlation_id: u64,
    /// Shared invocation marker.
    called: Arc<AtomicBool>,
}

impl PiScanAbortTarget for RecordingAbortTarget {
    fn correlation_id(&self) -> u64 {
        self.correlation_id
    }

    fn abort_and_reap(&mut self) -> Result<(), String> {
        self.called.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// What: Verify cancellation routes through the correlated abort target and suppresses late completion.
///
/// Inputs:
/// - Enabled worker, active item, registered recorder, cancellation, and late completion.
///
/// Output:
/// - Recorder is called, cancellation is terminal, and late result is rejected stale.
///
/// Details:
/// - The production target uses WS2 RPC abort and process-group reap behind the same trait.
#[tokio::test]
async fn cancellation_routes_abort_and_suppresses_late_completion() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut channels = spawn_pi_scan_worker(enabled_options(temp.path(), false)).expect("worker");
    channels
        .request_tx
        .send(PiScanRequestMessage::SetConsent(PiScanConsentState {
            background_observation: false,
            paid_execution: true,
        }))
        .expect("consent");
    channels
        .request_tx
        .send(PiScanRequestMessage::Enqueue(request(
            1,
            "worker-cancel",
            PiScanPriority::Foreground,
            100,
            0,
        )))
        .expect("enqueue");
    let active = receive_started(&mut channels).await;
    let called = Arc::new(AtomicBool::new(false));
    channels
        .session_tx
        .send(PiScanSessionRegistration {
            correlation_id: active.correlation_id,
            target: Box::new(RecordingAbortTarget {
                correlation_id: active.correlation_id,
                called: Arc::clone(&called),
            }),
        })
        .expect("register");
    receive_session_registration(&mut channels, active.correlation_id).await;
    channels
        .cancel_tx
        .send(PiScanCancelMessage {
            correlation_id: active.correlation_id,
            requested_at_unix: 50,
        })
        .expect("cancel");
    let cancelled = timeout(Duration::from_secs(2), async {
        loop {
            if let Some(PiScanResultMessage::Cancelled { record, .. }) =
                channels.result_rx.recv().await
            {
                return record;
            }
        }
    })
    .await
    .expect("cancel deadline");
    assert!(called.load(Ordering::SeqCst));
    assert_eq!(cancelled.status, PiScanTerminalStatus::Cancelled);

    channels
        .request_tx
        .send(PiScanRequestMessage::Complete {
            correlation_id: active.correlation_id,
            key: active.request.key,
            usage: PiScanActualUsage {
                tokens: 1,
                cost_microusd: 0,
            },
            finished_at_unix: 51,
        })
        .expect("late result");
    let rejected = timeout(Duration::from_secs(2), async {
        loop {
            if let Some(PiScanResultMessage::Rejected { reason }) = channels.result_rx.recv().await
            {
                return reason;
            }
        }
    })
    .await
    .expect("stale rejection deadline");
    assert!(rejected.contains("stale Pi scan response"));
}

/// What: Verify bounded shutdown interrupts active work, persists full reservation, and acknowledges.
///
/// Inputs:
/// - Enabled worker with one active item and no launched Pi process.
///
/// Output:
/// - Ack arrives within two seconds and persisted recovery state is interrupted/full-charged.
///
/// Details:
/// - Absence of a Pi target is valid before deferred acquisition launches the child.
#[tokio::test]
async fn shutdown_acknowledges_after_interrupted_state_is_durable() {
    let temp = tempfile::tempdir().expect("temp dir");
    let options = enabled_options(temp.path(), false);
    let mut channels = spawn_pi_scan_worker(options.clone()).expect("worker");
    channels
        .request_tx
        .send(PiScanRequestMessage::SetConsent(PiScanConsentState {
            background_observation: false,
            paid_execution: true,
        }))
        .expect("consent");
    channels
        .request_tx
        .send(PiScanRequestMessage::Enqueue(request(
            1,
            "shutdown-demo",
            PiScanPriority::Foreground,
            321,
            0,
        )))
        .expect("enqueue");
    let _active = receive_started(&mut channels).await;
    let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
    channels
        .shutdown_tx
        .send(PiScanShutdownMessage {
            acknowledge: ack_tx,
        })
        .expect("shutdown");
    let ack = tokio::task::spawn_blocking(move || ack_rx.recv_timeout(Duration::from_secs(2)))
        .await
        .expect("join")
        .expect("ack deadline");
    assert!(ack.persisted);
    assert!(ack.active_interrupted);
    let loaded = load_pi_scan_state(&options.state_path, &options.quarantine_dir, 100)
        .expect("persisted shutdown state");
    assert_eq!(
        loaded.state.terminal.last().map(|item| item.status),
        Some(PiScanTerminalStatus::Interrupted)
    );
    assert_eq!(loaded.state.budget.records[0].effective_tokens(), 321);
}
