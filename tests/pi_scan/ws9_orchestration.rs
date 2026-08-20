//! Deterministic WS9 end-to-end orchestration tests using only injected fakes.

use std::collections::{BTreeSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use pacsea::logic::pi_scan::baseline::CommitBuildRelevance;
use pacsea::logic::pi_scan::identity::{CommitOid, PackageBase};
use pacsea::logic::pi_scan::manifest::CanonicalManifest;
use pacsea::logic::pi_scan::result::{
    Coverage, ExpectedIdentity, MergedScanResult, ScanProvenance,
};
use pacsea::pi_scan_orchestrator::{
    DiscoveredPackage, DryRunAcquisitionReceipt, ExecutionFailure, ExecutionReceipt,
    FrozenScanIdentity, ObservationCommit, ObservationPackage, OrchestrationAdapter,
    OrchestrationConfig, OrchestrationError, PiScanOrchestrator, PiScanSequentialRunner,
    SetupSnapshot, UpdateCandidate,
};
use pacsea::state::pi_scan::{
    PiScanActualUsage, PiScanPriority, PiScanQueueKey, PiScanReservation,
};

/// Ordered fake adapter proving orchestration ordering and policy gates without external I/O.
#[derive(Default)]
struct FakeAdapter {
    /// Setup response.
    setup: Option<SetupSnapshot>,
    /// Foreign packages returned by discovery.
    packages: Vec<DiscoveredPackage>,
    /// Scripted package observations.
    observations: VecDeque<ObservationPackage>,
    /// Scripted execution results.
    executions: VecDeque<Result<ExecutionReceipt, ExecutionFailure>>,
    /// Ordered operation log.
    log: Vec<String>,
    /// Optional marker used by the off-thread cancellation test.
    execution_started: Option<Arc<AtomicBool>>,
    /// Whether execution waits for sticky cancellation.
    wait_for_cancel: bool,
    /// Whether execution returns an immediate canonical success.
    instant_success: bool,
    /// Scripted linked-continuation HEAD staleness.
    continuation_stale: Option<bool>,
    /// Scripted mutable-source staleness.
    mutable_stale: Option<bool>,
}

impl OrchestrationAdapter for FakeAdapter {
    fn probe_setup(&mut self) -> Result<SetupSnapshot, String> {
        self.log.push("setup".to_string());
        self.setup
            .clone()
            .ok_or_else(|| "Pi or Git is missing; install both and re-run setup".to_string())
    }

    fn dry_run_setup(&mut self) -> Result<SetupSnapshot, String> {
        self.log.push("dry-setup".to_string());
        self.setup
            .clone()
            .ok_or_else(|| "configured dry-run identity is missing".to_string())
    }

    fn enumerate_foreign(&mut self) -> Result<Vec<DiscoveredPackage>, String> {
        self.log.push("enumerate".to_string());
        Ok(self.packages.clone())
    }

    fn set_update_candidates(&mut self, candidates: Vec<UpdateCandidate>) {
        self.log.push("update-candidates".to_string());
        for package in &mut self.packages {
            let candidate = candidates.iter().find(|candidate| {
                package
                    .installed_names
                    .iter()
                    .any(|name| name == &candidate.package_name)
            });
            if let Some(candidate) = candidate {
                package.installed_version = candidate.current_version.clone();
                package.candidate_version = Some(candidate.candidate_version.clone());
            }
        }
    }

    fn observe_package(
        &mut self,
        package: &DiscoveredPackage,
        _cursor: Option<&CommitOid>,
    ) -> Result<ObservationPackage, String> {
        self.log.push(format!("observe:{}", package.package_base));
        self.observations
            .pop_front()
            .ok_or_else(|| "missing fake observation".to_string())
    }

    fn execute(
        &mut self,
        target: &FrozenScanIdentity,
        cancelled: &AtomicBool,
    ) -> Result<ExecutionReceipt, ExecutionFailure> {
        self.log.push(format!("execute:{}", target.commit_oid));
        if let Some(started) = &self.execution_started {
            started.store(true, Ordering::SeqCst);
        }
        if self.wait_for_cancel {
            for _ in 0..200 {
                if cancelled.load(Ordering::SeqCst) {
                    return Err(ExecutionFailure::Cancelled);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            return Err(ExecutionFailure::Service(
                "fake cancellation deadline elapsed".to_string(),
            ));
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err(ExecutionFailure::Cancelled);
        }
        if self.instant_success {
            return Ok(success(target, false));
        }
        self.executions.pop_front().unwrap_or_else(|| {
            Err(ExecutionFailure::Service(
                "missing fake execution".to_string(),
            ))
        })
    }

    fn dry_run_acquisition(
        &mut self,
        target: &FrozenScanIdentity,
    ) -> Result<DryRunAcquisitionReceipt, String> {
        self.log.push(format!("acquire-only:{}", target.commit_oid));
        Ok(DryRunAcquisitionReceipt {
            key: PiScanQueueKey {
                package_base: target.package_base.clone(),
                commit_oid: target.commit_oid.clone(),
            },
            status: "complete".to_string(),
            manifest_count: 2,
            coverage_notes: Vec::new(),
        })
    }

    fn recheck_mutable_sources(
        &mut self,
        sources: &[pacsea::logic::pi_scan::acquisition::MutableSourceIdentity],
    ) -> Result<bool, String> {
        if sources.is_empty() {
            Ok(false)
        } else {
            self.log.push(format!("mutable-recheck:{}", sources.len()));
            self.mutable_stale
                .ok_or_else(|| "missing fake mutable-source recheck".to_string())
        }
    }

    fn recheck_continuation(
        &mut self,
        package_base: &PackageBase,
        observed_head_oid: &CommitOid,
    ) -> Result<bool, String> {
        self.log
            .push(format!("recheck:{package_base}:{observed_head_oid}"));
        self.continuation_stale
            .ok_or_else(|| "missing fake continuation recheck".to_string())
    }
}

/// Build a setup-confirmed exact model and pricing snapshot.
fn setup() -> SetupSnapshot {
    SetupSnapshot {
        pi_version: "0.84.0".to_string(),
        available_models: vec![("provider".to_string(), "model".to_string())],
        selected_provider: "provider".to_string(),
        selected_model: "model".to_string(),
        reservation: PiScanReservation {
            tokens: 10_000,
            cost_microusd: 50,
        },
        route_reservations: vec![(
            "provider".to_string(),
            "model".to_string(),
            PiScanReservation {
                tokens: 10_000,
                cost_microusd: 50,
            },
        )],
        pricing_binding: "test-pricing-v1".to_string(),
        pricing_observed_at_unix_seconds: 1_000,
        maximum_pricing_age_seconds: 900,
        pricing_summary: vec!["provider/model · Pi native metadata · cost=0.05".to_string()],
    }
}

/// Build one discovered installed package group.
fn package(name: &str, base: &str) -> DiscoveredPackage {
    DiscoveredPackage {
        package_base: PackageBase::new(base).expect("base"),
        installed_names: vec![name.to_string()],
        installed_version: "1.0-1".to_string(),
        candidate_version: None,
    }
}

/// Build one full commit OID.
fn oid(value: u64) -> CommitOid {
    CommitOid::new(format!("{value:040x}")).expect("oid")
}

/// Build one successful canonical execution receipt.
fn success(target: &FrozenScanIdentity, stale: bool) -> ExecutionReceipt {
    ExecutionReceipt {
        result: MergedScanResult {
            identity: ExpectedIdentity {
                scan_id: target.scan_id.clone(),
                package_base: target.package_base.as_str().to_string(),
                commit_oid: target.commit_oid.as_str().to_string(),
            },
            coverage: Coverage::Complete,
            limitations: Vec::new(),
            findings: Vec::new(),
        },
        observed_head_oid: target.observed_head_oid.clone(),
        provenance: ScanProvenance {
            pi_version: "0.84.0".to_string(),
            extension_sha256: "a".repeat(64),
            prompt_version: "pacsea-scan-prompt-1".to_string(),
            schema_version: "pacsea-scan-result-1".to_string(),
            tool_contract_version: "pacsea-scan-tools-1".to_string(),
            attempts: Vec::new(),
        },
        manifests: vec![CanonicalManifest::new(Vec::new())],
        usage: PiScanActualUsage {
            tokens: 123,
            cost_microusd: 12,
        },
        stale,
        mutable_sources: Vec::new(),
    }
}

/// Build enabled runtime configuration rooted in one temporary directory.
fn config(root: &Path, dry_run: bool) -> OrchestrationConfig {
    OrchestrationConfig {
        enabled: true,
        setup_confirmed: true,
        background_execution: true,
        initial_consent: pacsea::state::pi_scan::PiScanConsentState {
            background_observation: true,
            paid_execution: true,
        },
        consent_binding: "test-binding-v1".to_string(),
        consent_path: root.join("consent-v1.json"),
        consent_quarantine_dir: root.join("quarantine").join("consent"),
        dry_run,
        state_path: root.join("orchestration-v1.json"),
        results_root: root.join("results-v1"),
        result_quarantine_dir: root.join("quarantine").join("results"),
        quarantine_dir: root.join("quarantine").join("orchestration"),
        baseline_path: root.join("baseline-v1.json"),
        baseline_quarantine_dir: root.join("quarantine").join("baseline"),
        observation_interval_seconds: 900,
        budget_limits: pacsea::state::pi_scan::PiScanBudgetLimits {
            starts_per_hour: 5,
            tokens_per_24h: 500_000,
            cost_microusd_per_24h: 1_000,
        },
    }
}

/// A selected unresolved target must not observe or fail on unrelated foreign packages.
#[test]
fn manual_observation_is_limited_to_selected_package_names() {
    let temp = tempfile::tempdir().expect("temp");
    let pacsea_head = oid(42);
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![
            package("qml-vulkan", "qml-vulkan"),
            package("pacsea-bin", "pacsea-bin"),
        ],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("pacsea-bin").expect("base"),
            head_oid: pacsea_head.clone(),
            commits: vec![ObservationCommit {
                oid: pacsea_head,
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let selected = BTreeSet::from(["pacsea-bin".to_string()]);

    let targets = orchestrator
        .manual_observation_selected(1, &selected)
        .expect("selected observation");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].package_name, "pacsea-bin");
    assert_eq!(targets[0].package_base.as_str(), "pacsea-bin");
}

#[test]
fn linked_continuation_rechecks_exact_head_without_a_model_call() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        continuation_stale: Some(true),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let package_base = PackageBase::new("demo").expect("base");
    let observed_head = oid(1);
    assert!(
        orchestrator
            .validate_continuation(&package_base, &observed_head)
            .expect("recheck")
    );
    assert_eq!(
        orchestrator.adapter().log,
        vec![
            "setup".to_string(),
            format!("recheck:{package_base}:{observed_head}")
        ]
    );
}

#[test]
fn disabled_and_missing_setup_fail_closed_without_discovery() {
    let temp = tempfile::tempdir().expect("temp");
    let mut disabled = config(temp.path(), false);
    disabled.enabled = false;
    let adapter = FakeAdapter::default();
    let mut orchestrator = PiScanOrchestrator::new(disabled, adapter).expect("construct");
    assert!(matches!(
        orchestrator.startup_observation(1),
        Err(OrchestrationError::Disabled(_))
    ));
    assert!(orchestrator.adapter().log.is_empty());

    let adapter = FakeAdapter::default();
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let error = orchestrator
        .startup_observation(1)
        .expect_err("setup fails");
    assert!(error.to_string().contains("install"));
    assert_eq!(orchestrator.adapter().log, vec!["setup"]);
}

#[test]
fn split_packages_and_every_commit_are_queued_oldest_first_once() {
    let temp = tempfile::tempdir().expect("temp");
    let mut adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo"), package("demo-doc", "demo")],
        ..FakeAdapter::default()
    };
    adapter.observations.push_back(ObservationPackage {
        package_base: PackageBase::new("demo").expect("base"),
        head_oid: oid(3),
        commits: vec![
            ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            },
            ObservationCommit {
                oid: oid(2),
                relevance: CommitBuildRelevance::ObservedNoRecipeDelta,
            },
            ObservationCommit {
                oid: oid(3),
                relevance: CommitBuildRelevance::Uncertain,
            },
        ],
        truncated: false,
        paused_for_rebaseline: false,
    });
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let queued = orchestrator.startup_observation(100).expect("observe");
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].commit_oid, oid(3));
    assert_eq!(queued[0].installed_names, vec!["demo", "demo-doc"]);
    assert_eq!(orchestrator.state().ledger.len(), 3);
    assert_eq!(
        orchestrator
            .adapter()
            .log
            .iter()
            .filter(|line| line.starts_with("observe:"))
            .count(),
        1
    );
    assert!(temp.path().join("orchestration-v1.json").is_file());
}

#[test]
fn success_persists_canonical_result_reconciles_usage_and_continues() {
    let temp = tempfile::tempdir().expect("temp");
    let mut adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        ..FakeAdapter::default()
    };
    adapter.observations.push_back(ObservationPackage {
        package_base: PackageBase::new("demo").expect("base"),
        head_oid: oid(2),
        commits: vec![
            ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            },
            ObservationCommit {
                oid: oid(2),
                relevance: CommitBuildRelevance::BuildRelevant,
            },
        ],
        truncated: false,
        paused_for_rebaseline: false,
    });
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let targets = orchestrator.startup_observation(10).expect("observe");
    orchestrator
        .adapter_mut()
        .executions
        .push_back(Ok(success(&targets[0], true)));
    let receipt = orchestrator
        .run_next(11, &AtomicBool::new(false))
        .expect("run")
        .expect("receipt");
    assert!(receipt.stale);
    assert!(
        orchestrator
            .run_next(12, &AtomicBool::new(false))
            .expect("idle")
            .is_none()
    );
    assert_eq!(orchestrator.state().runtime.terminal.len(), 1);
    assert_eq!(
        orchestrator.state().runtime.budget.records[0].effective_tokens(),
        123
    );
    let result_dir = temp.path().join("results-v1/demo");
    assert_eq!(std::fs::read_dir(&result_dir).expect("results").count(), 1);
    let stored = std::fs::read_dir(&result_dir)
        .expect("results")
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .collect::<Vec<_>>();
    assert!(
        stored
            .iter()
            .any(|document| document.contains("\"stale\": true"))
    );
}

#[test]
fn failure_cancel_dry_run_budget_and_recovery_are_fail_closed() {
    let temp = tempfile::tempdir().expect("temp");
    let mut adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        ..FakeAdapter::default()
    };
    let mut dry = PiScanOrchestrator::new(config(temp.path(), true), adapter).expect("construct");
    let preview = dry.manual_observation(1).expect("dry observation");
    assert_eq!(preview.len(), 1);
    assert!(
        dry.run_next(2, &AtomicBool::new(false))
            .expect("dry")
            .is_none()
    );
    assert!(!temp.path().join("orchestration-v1.json").exists());
    assert!(
        !dry.adapter()
            .log
            .iter()
            .any(|line| line.starts_with("execute:"))
    );

    adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        ..FakeAdapter::default()
    };
    let mut live = PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    live.startup_observation(3).expect("observe");
    let cancelled = AtomicBool::new(true);
    let error = live.run_next(4, &cancelled).expect_err("cancel");
    assert!(matches!(error, OrchestrationError::Cancelled));
    assert!(live.state().runtime.active.is_none());
}

#[test]
fn manual_queue_requires_full_frozen_identity_and_exact_pricing() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let target = FrozenScanIdentity {
        scan_id: "manual-1".to_string(),
        package_name: "demo".to_string(),
        package_base: PackageBase::new("demo").expect("base"),
        installed_names: vec!["demo".to_string()],
        installed_version: "1.0-1".to_string(),
        candidate_version: None,
        commit_oid: oid(9),
        observed_head_oid: oid(9),
        cycle_id: "manual".to_string(),
        provider: "provider".to_string(),
        model: "model".to_string(),
        reservation: PiScanReservation {
            tokens: 10_000,
            cost_microusd: 50,
        },
        priority: PiScanPriority::Foreground,
    };
    orchestrator
        .enqueue_frozen(target.clone(), 1)
        .expect("queue");
    let mut invalid = target;
    invalid.model.clear();
    assert!(orchestrator.enqueue_frozen(invalid, 2).is_err());
}

#[tokio::test]
async fn fast_run_reports_started_from_the_active_registration_seam() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        instant_success: true,
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    orchestrator.startup_observation(1).expect("observe");
    let runner = PiScanSequentialRunner::new(orchestrator);
    let (started_tx, mut started_rx) = tokio::sync::mpsc::unbounded_channel();

    let receipt = runner
        .run_next_with_started(2, started_tx)
        .await
        .expect("fast run")
        .expect("receipt");
    let started = started_rx.try_recv().expect("deterministic Started");

    assert_eq!(started.request.key.package_base.as_str(), "demo");
    assert_eq!(
        started.request.key.commit_oid.as_str(),
        receipt.result.identity.commit_oid
    );
}

#[tokio::test]
async fn active_pause_queues_then_persists_before_the_next_start() {
    let temp = tempfile::tempdir().expect("temp");
    let started = Arc::new(AtomicBool::new(false));
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo"), package("other", "other")],
        observations: VecDeque::from(vec![
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(1),
                commits: vec![ObservationCommit {
                    oid: oid(1),
                    relevance: CommitBuildRelevance::BuildRelevant,
                }],
                truncated: false,
                paused_for_rebaseline: false,
            },
            ObservationPackage {
                package_base: PackageBase::new("other").expect("base"),
                head_oid: oid(2),
                commits: vec![ObservationCommit {
                    oid: oid(2),
                    relevance: CommitBuildRelevance::BuildRelevant,
                }],
                truncated: false,
                paused_for_rebaseline: false,
            },
        ]),
        execution_started: Some(Arc::clone(&started)),
        wait_for_cancel: true,
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    orchestrator.startup_observation(1).expect("observe");
    let runner = PiScanSequentialRunner::new(orchestrator);
    let execution_runner = runner.clone();
    let execution = tokio::spawn(async move { execution_runner.run_next(2).await });
    let correlation = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if started.load(Ordering::SeqCst)
                && let Some(correlation) = runner.active_correlation()
            {
                return correlation;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("active registration");

    let (queued_correlation, persisted) = runner
        .queue_user_pause_if_active(true)
        .expect("queue pause")
        .expect("active pause must queue");
    assert_eq!(queued_correlation, correlation);
    assert!(runner.cancel(correlation));
    assert!(matches!(
        execution.await.expect("join"),
        Err(OrchestrationError::Cancelled)
    ));
    persisted
        .await
        .expect("policy completion")
        .expect("policy persisted");

    let state = runner.state_snapshot().await.expect("state");
    assert!(
        state
            .runtime
            .pause_reasons
            .contains(&pacsea::state::pi_scan::PiScanPauseReason::User)
    );
    assert_eq!(state.runtime.queue.len(), 1);
    assert!(matches!(
        runner.run_next(3).await,
        Err(OrchestrationError::Paused(reason)) if reason.contains("User")
    ));
}

#[tokio::test]
async fn off_thread_runner_registers_exact_cancel_and_shuts_down_bounded() {
    let temp = tempfile::tempdir().expect("temp");
    let started = Arc::new(AtomicBool::new(false));
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        execution_started: Some(Arc::clone(&started)),
        wait_for_cancel: true,
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    orchestrator.startup_observation(1).expect("observe");
    let runner = PiScanSequentialRunner::new(orchestrator);
    let execution_runner = runner.clone();
    let execution = tokio::spawn(async move { execution_runner.run_next(2).await });

    let correlation = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if started.load(Ordering::SeqCst)
                && let Some(correlation) = runner.active_correlation()
            {
                return correlation;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("registration deadline");
    assert!(
        !runner.cancel(correlation + 1),
        "stale cancellation is inert"
    );
    assert!(runner.cancel(correlation), "exact cancellation is accepted");
    assert!(matches!(
        execution.await.expect("join"),
        Err(OrchestrationError::Cancelled)
    ));
    tokio::time::timeout(Duration::from_secs(2), runner.shutdown(3))
        .await
        .expect("shutdown deadline")
        .expect("shutdown");
}

#[test]
fn failed_execution_terminalizes_and_the_next_item_continues() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo"), package("other", "other")],
        observations: VecDeque::from(vec![
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(1),
                commits: vec![ObservationCommit {
                    oid: oid(1),
                    relevance: CommitBuildRelevance::BuildRelevant,
                }],
                truncated: false,
                paused_for_rebaseline: false,
            },
            ObservationPackage {
                package_base: PackageBase::new("other").expect("base"),
                head_oid: oid(2),
                commits: vec![ObservationCommit {
                    oid: oid(2),
                    relevance: CommitBuildRelevance::BuildRelevant,
                }],
                truncated: false,
                paused_for_rebaseline: false,
            },
        ]),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let targets = orchestrator.startup_observation(1).expect("observe");
    orchestrator
        .adapter_mut()
        .executions
        .push_back(Err(ExecutionFailure::Service(
            "provider failed".to_string(),
        )));
    orchestrator
        .adapter_mut()
        .executions
        .push_back(Ok(success(&targets[1], false)));
    assert!(matches!(
        orchestrator.run_next(2, &AtomicBool::new(false)),
        Err(OrchestrationError::Execution(reason)) if reason == "provider failed"
    ));
    assert_eq!(
        orchestrator.state().runtime.terminal[0].status,
        pacsea::state::pi_scan::PiScanTerminalStatus::Failed
    );
    assert!(matches!(
        orchestrator.run_next(3, &AtomicBool::new(false)),
        Err(OrchestrationError::Paused(reason)) if reason.contains("Service")
    ));
    let second_key = PiScanQueueKey {
        package_base: targets[1].package_base.clone(),
        commit_oid: targets[1].commit_oid.clone(),
    };
    orchestrator
        .promote_queued(&second_key)
        .expect("manual promotion clears a successfully revalidated service pause");
    assert!(
        orchestrator
            .run_next(4, &AtomicBool::new(false))
            .expect("explicit second")
            .is_some()
    );
}

#[test]
fn failed_or_cancelled_identity_can_be_retried_without_reobservation() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let target = orchestrator
        .startup_observation(1)
        .expect("observe")
        .remove(0);
    orchestrator
        .adapter_mut()
        .executions
        .push_back(Err(ExecutionFailure::Service(
            "provider failed".to_string(),
        )));
    assert!(orchestrator.run_next(2, &AtomicBool::new(false)).is_err());

    let key = PiScanQueueKey {
        package_base: target.package_base.clone(),
        commit_oid: target.commit_oid.clone(),
    };
    orchestrator.promote_queued(&key).expect("retry requeues");
    orchestrator
        .adapter_mut()
        .executions
        .push_back(Ok(success(&target, false)));
    assert!(
        orchestrator
            .run_next(3, &AtomicBool::new(false))
            .expect("retry executes")
            .is_some()
    );
}

#[test]
fn exact_background_budget_blocks_before_external_execution() {
    let temp = tempfile::tempdir().expect("temp");
    let mut bounded = config(temp.path(), false);
    bounded.budget_limits.cost_microusd_per_24h = 0;
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        ..FakeAdapter::default()
    };
    let mut orchestrator = PiScanOrchestrator::new(bounded, adapter).expect("construct");
    orchestrator.startup_observation(1).expect("observe");
    assert!(matches!(
        orchestrator.run_next(2, &AtomicBool::new(false)),
        Err(OrchestrationError::Paused(reason)) if reason.contains("reservation")
    ));
    assert!(
        !orchestrator
            .adapter()
            .log
            .iter()
            .any(|entry| entry.starts_with("execute:"))
    );
}

#[test]
fn persisted_full_identity_queue_recovers_and_executes() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        ..FakeAdapter::default()
    };
    let target = {
        let mut first =
            PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
        first.startup_observation(1).expect("observe")[0].clone()
    };
    let mut recovered = PiScanOrchestrator::new(
        config(temp.path(), false),
        FakeAdapter {
            setup: Some(setup()),
            executions: VecDeque::from(vec![Ok(success(&target, false))]),
            ..FakeAdapter::default()
        },
    )
    .expect("recover");
    assert_eq!(recovered.state().runtime.queue.len(), 1);
    assert_eq!(recovered.state().targets.len(), 1);
    assert!(
        recovered
            .run_next(2, &AtomicBool::new(false))
            .expect("execute")
            .is_some()
    );
}

#[test]
fn quarantined_orchestration_remains_unavailable_on_second_restart() {
    let temp = tempfile::tempdir().expect("temp");
    let state_path = temp.path().join("orchestration-v1.json");
    std::fs::write(&state_path, b"{not-json").expect("corrupt state");
    let Err(first) = PiScanOrchestrator::new(
        config(temp.path(), false),
        FakeAdapter {
            setup: Some(setup()),
            ..FakeAdapter::default()
        },
    ) else {
        panic!("first load must quarantine corrupt state");
    };
    assert!(first.to_string().contains("quarantined"));
    assert!(!state_path.exists());
    assert!(
        temp.path()
            .join("quarantine/orchestration/orchestration-unavailable-v1")
            .is_file()
    );

    let Err(second) = PiScanOrchestrator::new(
        config(temp.path(), false),
        FakeAdapter {
            setup: Some(setup()),
            ..FakeAdapter::default()
        },
    ) else {
        panic!("second load must remain unavailable");
    };
    assert!(second.to_string().contains("remains unavailable"));
}

#[test]
fn material_configuration_change_invalidates_all_persisted_consent() {
    let temp = tempfile::tempdir().expect("temp");
    let mut first = PiScanOrchestrator::new(
        config(temp.path(), false),
        FakeAdapter {
            setup: Some(setup()),
            ..FakeAdapter::default()
        },
    )
    .expect("construct");
    first
        .update_runtime_policy(
            Some(pacsea::state::pi_scan::PiScanConsentState {
                background_observation: true,
                paid_execution: true,
            }),
            None,
            false,
            None,
        )
        .expect("runtime consent");
    first
        .update_setup_consent(pacsea::pi_scan_orchestrator::PiScanSetupConsentState {
            configuration_binding: String::new(),
            disclosure_confirmed: true,
            fallback_confirmed: true,
            background_paid_execution: false,
            readiness_warning_confirmed: true,
            confirmed_pi_version: "0.84.0".to_string(),
            confirmed_pricing_binding: "test-pricing-v1".to_string(),
        })
        .expect("setup consent");
    assert!(temp.path().join("consent-v1.json").is_file());
    drop(first);

    let mut changed = config(temp.path(), false);
    changed.consent_binding = "test-binding-v2".to_string();
    let recovered = PiScanOrchestrator::new(
        changed,
        FakeAdapter {
            setup: Some(setup()),
            ..FakeAdapter::default()
        },
    )
    .expect("recover under changed material config");
    let (runtime, setup) = recovered.consent_snapshot();
    assert!(!runtime.background_observation);
    assert!(!runtime.paid_execution);
    assert!(!setup.disclosure_confirmed);
    assert!(!setup.fallback_confirmed);
    assert!(!setup.readiness_warning_confirmed);
    assert_eq!(setup.configuration_binding, "test-binding-v2");
}

#[test]
fn continuation_rechecks_mutable_source_identity_and_invalidates_result() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        continuation_stale: Some(false),
        mutable_stale: Some(true),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let package_base = PackageBase::new("demo").expect("base");
    let observed_head = oid(1);
    let mutable_sources = vec![pacsea::logic::pi_scan::acquisition::MutableSourceIdentity {
        declaration: "git+https://example.invalid/demo.git#branch=main".to_string(),
        repository_url: "https://example.invalid/demo.git".to_string(),
        reference: "refs/heads/main".to_string(),
        resolved_oid: oid(2),
    }];

    assert!(
        orchestrator
            .validate_continuation_with_sources(&package_base, &observed_head, &mutable_sources,)
            .expect("recheck")
    );
    assert!(
        orchestrator
            .adapter()
            .log
            .contains(&"mutable-recheck:1".to_string())
    );
}

#[test]
fn periodic_observation_respects_the_fifteen_minute_floor() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(1),
                commits: Vec::new(),
                truncated: false,
                paused_for_rebaseline: false,
            },
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(1),
                commits: Vec::new(),
                truncated: false,
                paused_for_rebaseline: false,
            },
        ]),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    orchestrator.startup_observation(100).expect("startup");
    assert!(
        orchestrator
            .periodic_observation(999)
            .expect("not due")
            .is_empty()
    );
    assert_eq!(
        orchestrator
            .adapter()
            .log
            .iter()
            .filter(|entry| entry.starts_with("observe:"))
            .count(),
        1
    );
    orchestrator.periodic_observation(1_000).expect("due");
    assert_eq!(
        orchestrator
            .adapter()
            .log
            .iter()
            .filter(|entry| entry.starts_with("observe:"))
            .count(),
        2
    );
}

#[test]
fn acquisition_only_dry_run_skips_pi_execution_and_persistence() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![ObservationPackage {
            package_base: PackageBase::new("demo").expect("base"),
            head_oid: oid(1),
            commits: vec![ObservationCommit {
                oid: oid(1),
                relevance: CommitBuildRelevance::BuildRelevant,
            }],
            truncated: false,
            paused_for_rebaseline: false,
        }]),
        ..FakeAdapter::default()
    };
    let mut dry_run_config = config(temp.path(), true);
    dry_run_config.setup_confirmed = false;
    dry_run_config.initial_consent = pacsea::state::pi_scan::PiScanConsentState::default();
    let mut orchestrator = PiScanOrchestrator::new(dry_run_config, adapter).expect("construct");
    let target = orchestrator.manual_observation(1).expect("observe")[0].clone();
    let receipt = orchestrator
        .dry_run_acquisition(&PiScanQueueKey {
            package_base: target.package_base,
            commit_oid: target.commit_oid,
        })
        .expect("acquire only");

    assert_eq!(receipt.status, "complete");
    assert_eq!(receipt.manifest_count, 2);
    assert_eq!(
        orchestrator.adapter().log,
        vec![
            "dry-setup".to_string(),
            "enumerate".to_string(),
            "observe:demo".to_string(),
            format!("acquire-only:{}", oid(1)),
        ]
    );
    assert!(!temp.path().join("orchestration-v1.json").exists());
    assert!(!temp.path().join("results-v1").exists());
}

#[test]
fn accepted_current_head_baseline_enables_later_history_scans() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(2),
                commits: vec![
                    ObservationCommit {
                        oid: oid(1),
                        relevance: CommitBuildRelevance::BuildRelevant,
                    },
                    ObservationCommit {
                        oid: oid(2),
                        relevance: CommitBuildRelevance::BuildRelevant,
                    },
                ],
                truncated: false,
                paused_for_rebaseline: false,
            },
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(4),
                commits: vec![
                    ObservationCommit {
                        oid: oid(3),
                        relevance: CommitBuildRelevance::BuildRelevant,
                    },
                    ObservationCommit {
                        oid: oid(4),
                        relevance: CommitBuildRelevance::BuildRelevant,
                    },
                ],
                truncated: false,
                paused_for_rebaseline: false,
            },
        ]),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    let baseline_target = orchestrator.startup_observation(1).expect("observe")[0].clone();
    orchestrator
        .adapter_mut()
        .executions
        .push_back(Ok(success(&baseline_target, false)));
    orchestrator
        .run_next(2, &AtomicBool::new(false))
        .expect("run")
        .expect("receipt");
    orchestrator
        .accept_baseline(
            &baseline_target.package_base,
            &baseline_target.commit_oid,
            &baseline_target.scan_id,
            "validated-binding",
            3,
        )
        .expect("accept baseline");

    let later = orchestrator.manual_observation(4).expect("later history");
    assert_eq!(
        later
            .iter()
            .map(|target| target.commit_oid.clone())
            .collect::<Vec<_>>(),
        vec![oid(3), oid(4)]
    );
    assert!(temp.path().join("baseline-v1.json").is_file());
}

#[test]
fn typed_update_candidate_is_frozen_into_current_head_target() {
    let temp = tempfile::tempdir().expect("temp");
    let adapter = FakeAdapter {
        setup: Some(setup()),
        packages: vec![package("demo", "demo")],
        observations: VecDeque::from(vec![
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(5),
                commits: Vec::new(),
                truncated: false,
                paused_for_rebaseline: false,
            },
            ObservationPackage {
                package_base: PackageBase::new("demo").expect("base"),
                head_oid: oid(5),
                commits: Vec::new(),
                truncated: false,
                paused_for_rebaseline: false,
            },
        ]),
        ..FakeAdapter::default()
    };
    let mut orchestrator =
        PiScanOrchestrator::new(config(temp.path(), false), adapter).expect("construct");
    orchestrator.set_update_candidates(vec![UpdateCandidate {
        package_name: "demo".to_string(),
        current_version: "1.0-1".to_string(),
        candidate_version: "2.0-1".to_string(),
    }]);
    let targets = orchestrator
        .update_candidate_observation(10)
        .expect("update observation");

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].commit_oid, oid(5));
    assert_eq!(targets[0].candidate_version.as_deref(), Some("2.0-1"));
    assert_eq!(targets[0].cycle_id, "cycle-10");
    orchestrator
        .adapter_mut()
        .executions
        .push_back(Ok(success(&targets[0], false)));
    assert!(
        orchestrator
            .run_next(11, &AtomicBool::new(false))
            .expect("complete first update scan")
            .is_some()
    );
    let repeated = orchestrator
        .update_candidate_observation(12)
        .expect("repeat update observation");
    assert!(repeated.is_empty());
    assert!(orchestrator.state().runtime.queue.is_empty());
}

#[test]
fn persisted_explicit_consent_is_not_flattened_on_restart() {
    let temp = tempfile::tempdir().expect("temp");
    let mut first = PiScanOrchestrator::new(
        config(temp.path(), false),
        FakeAdapter {
            setup: Some(setup()),
            ..FakeAdapter::default()
        },
    )
    .expect("construct");
    first
        .update_runtime_policy(
            Some(pacsea::state::pi_scan::PiScanConsentState {
                background_observation: false,
                paid_execution: false,
            }),
            None,
            false,
            None,
        )
        .expect("persist consent");
    drop(first);

    let recovered = PiScanOrchestrator::new(
        config(temp.path(), false),
        FakeAdapter {
            setup: Some(setup()),
            ..FakeAdapter::default()
        },
    )
    .expect("recover");
    assert!(!recovered.state().runtime.consent.background_observation);
    assert!(!recovered.state().runtime.consent.paid_execution);
}
