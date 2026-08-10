//! Deterministic controller, persistence, consent, and runtime-transfer coverage.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::logic::pi_scan::pricing::{PricingAccounting, PricingSource, RoutePricing, TokenRates};
use crate::pi_agent::PiVersion;
use crate::pi_agent::session::ModelChoice;
use crate::pi_agent::setup_probe::{
    PiSetupAdvertisedRoute, PiSetupIsolationContract, PiSetupProbeRequest, PiSetupProbeSnapshot,
};
use crate::pi_scan_orchestrator::{OrchestrationAdapter, OrchestrationConfig, PiScanOrchestrator};
use crate::state::pi_scan::{PiScanBudgetLimits, PiScanConsentState, PiScanReservation};
use crate::state::pi_scan_setup::PiScanSetupConfirmations;
use crate::theme::PiScanSettings;

use super::*;

/// Driver behavior injected into one controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DriverFailure {
    /// No injected failure.
    None,
    /// Runtime preparation fails before persistence.
    Prepare,
    /// Settings-save stage fails before persistence begins.
    Save,
    /// Consent stage fails before either durable replacement.
    Consent,
    /// Settings stage fails after consent was replaced and must roll it back.
    SettingsAfterConsent,
    /// Runtime activation fails after durable commit.
    Activation,
}

/// Shared lifecycle evidence retained after the controller consumes fake values.
#[derive(Debug, Default)]
struct DriverEvidence {
    /// Number of metadata probes.
    probes: AtomicUsize,
    /// Number of inert candidates prepared.
    prepared: AtomicUsize,
    /// Number of candidates activated.
    activated: AtomicUsize,
    /// Number of unactivated candidates torn down.
    torn_down: AtomicUsize,
}

/// Fully deterministic probe/runtime driver.
struct FakeDriver {
    /// Stable current Unix time.
    now: u64,
    /// Exact probe facts.
    snapshot: PiSetupProbeSnapshot,
    /// Selected failure point.
    failure: DriverFailure,
    /// Shared lifecycle counters.
    evidence: Arc<DriverEvidence>,
}

impl SetupDriver for FakeDriver {
    fn now_unix_seconds(&self) -> u64 {
        self.now
    }

    fn probe(&mut self, _request: &PiSetupProbeRequest) -> Result<PiSetupProbeSnapshot, String> {
        self.evidence.probes.fetch_add(1, Ordering::SeqCst);
        Ok(self.snapshot.clone())
    }

    fn prepare_runtime(
        &mut self,
        _options: &PiScanSetupControllerOptions,
        _settings: &PiScanSettings,
        _snapshot: &PiSetupProbeSnapshot,
        _models: Vec<ModelChoice>,
        _reservation: PiScanReservation,
    ) -> Result<Box<dyn PreparedRuntime>, String> {
        self.evidence.prepared.fetch_add(1, Ordering::SeqCst);
        if self.failure == DriverFailure::Prepare {
            return Err("injected candidate health-check failure".to_string());
        }
        Ok(Box::new(FakePreparedRuntime {
            activation_fails: self.failure == DriverFailure::Activation,
            evidence: Arc::clone(&self.evidence),
        }))
    }

    fn before_commit(&mut self, _options: &PiScanSetupControllerOptions) -> Result<(), String> {
        if self.failure == DriverFailure::Save {
            Err("injected settings save failure".to_string())
        } else {
            Ok(())
        }
    }

    fn before_consent_commit(
        &mut self,
        _options: &PiScanSetupControllerOptions,
    ) -> Result<(), String> {
        if self.failure == DriverFailure::Consent {
            Err("injected consent persistence failure".to_string())
        } else {
            Ok(())
        }
    }

    fn before_settings_commit(
        &mut self,
        _options: &PiScanSetupControllerOptions,
    ) -> Result<(), String> {
        if self.failure == DriverFailure::SettingsAfterConsent {
            Err("injected settings persistence failure".to_string())
        } else {
            Ok(())
        }
    }
}

/// Fake queue-inert runtime activated only through the transfer seam.
struct FakePreparedRuntime {
    /// Whether activation must fail.
    activation_fails: bool,
    /// Shared lifecycle evidence.
    evidence: Arc<DriverEvidence>,
}

impl PreparedRuntime for FakePreparedRuntime {
    fn activate(self: Box<Self>) -> Result<PiScanRuntimeChannels, String> {
        self.evidence.activated.fetch_add(1, Ordering::SeqCst);
        if self.activation_fails {
            Err("injected runtime activation failure".to_string())
        } else {
            Ok(crate::app::runtime::workers::pi_scan::spawn_default_off_pi_scan_worker())
        }
    }

    fn teardown(self: Box<Self>) -> Result<(), String> {
        self.evidence.torn_down.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Minimal adapter used only to prove setup consent schema compatibility.
struct InertAdapter;

impl OrchestrationAdapter for InertAdapter {
    fn probe_setup(&mut self) -> Result<SetupSnapshot, String> {
        Err("not called by consent-load test".to_string())
    }

    fn enumerate_foreign(
        &mut self,
    ) -> Result<Vec<crate::pi_scan_orchestrator::DiscoveredPackage>, String> {
        Ok(Vec::new())
    }

    fn observe_package(
        &mut self,
        _package: &crate::pi_scan_orchestrator::DiscoveredPackage,
        _cursor: Option<&crate::logic::pi_scan::identity::CommitOid>,
    ) -> Result<crate::pi_scan_orchestrator::ObservationPackage, String> {
        Err("not called by consent-load test".to_string())
    }

    fn execute(
        &mut self,
        _target: &crate::pi_scan_orchestrator::FrozenScanIdentity,
        _cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<
        crate::pi_scan_orchestrator::ExecutionReceipt,
        crate::pi_scan_orchestrator::ExecutionFailure,
    > {
        Err(crate::pi_scan_orchestrator::ExecutionFailure::Service(
            "not called by consent-load test".to_string(),
        ))
    }
}

/// Create a unique private temporary root.
fn temp_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pacsea_ws2b_{tag}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ));
    std::fs::create_dir_all(&root).expect("test root");
    root
}

/// Build exact setup paths rooted under one test directory.
fn options(root: &Path, dry_run: bool) -> PiScanSetupControllerOptions {
    PiScanSetupControllerOptions {
        dry_run,
        settings_path: root.join("settings.conf"),
        consent_path: root.join("pi_scan/consent-v1.json"),
        state_path: root.join("pi_scan/backlog-v1.json"),
        quarantine_dir: root.join("pi_scan/quarantine"),
    }
}

/// Build one complete exact priced probe snapshot.
fn snapshot(now: u64) -> PiSetupProbeSnapshot {
    PiSetupProbeSnapshot {
        executable: PathBuf::from("/opt/pi/bin/pi"),
        pi_version: PiVersion {
            major: 0,
            minor: 84,
            patch: 0,
        },
        isolation: PiSetupIsolationContract {
            tool_contract_version: crate::pi_agent::TOOL_CONTRACT_VERSION.to_string(),
            extension_sha256: crate::pi_agent::process::EMBEDDED_EXTENSION_SHA256.to_string(),
            active_tools: vec!["pacsea_read_snapshot".to_string()],
            argv: vec!["--mode".to_string(), "rpc".to_string()],
        },
        routes: vec![
            route("provider-a", "model-a", 10),
            route("provider-b", "model-b", 20),
        ],
        pricing_observed_at_unix_seconds: now,
        maximum_pricing_age: SETUP_PROBE_MAXIMUM_PRICING_AGE,
        pricing_binding: format!("pricing-{now}"),
    }
}

/// Build one exact route with deterministic metered pricing.
fn route(provider: &str, model: &str, rate: u64) -> PiSetupAdvertisedRoute {
    PiSetupAdvertisedRoute {
        provider: provider.to_string(),
        model: model.to_string(),
        pricing: RoutePricing {
            provider: provider.to_string(),
            model: model.to_string(),
            rates: TokenRates {
                input_microusd_per_million: rate,
                output_microusd_per_million: rate,
            },
            source: PricingSource::PiModelCost,
            accounting: PricingAccounting::Metered,
        },
        pricing_provenance: "pi-rpc:get_available_models/Model.cost".to_string(),
        reservation: PiScanReservation {
            tokens: SETUP_PROBE_RESERVATION_TOKENS,
            cost_microusd: rate,
        },
    }
}

/// Build a valid normalized candidate selecting the first route.
fn candidate() -> PiScanSettings {
    PiScanSettings {
        enabled: true,
        binary: "/opt/pi/bin/pi".to_string(),
        provider: "provider-a".to_string(),
        model: "model-a".to_string(),
        ..PiScanSettings::default()
    }
}

/// Build required independent confirmations with conservative optional behavior.
fn confirmations() -> PiScanSetupConfirmations {
    PiScanSetupConfirmations {
        disclosure_confirmed: true,
        foreground_paid_confirmed: true,
        fallback_confirmed: false,
        readiness_warning_confirmed: true,
    }
}

/// Spawn one deterministic controller and return shared evidence.
fn spawn_fake(
    root: &Path,
    dry_run: bool,
    failure: DriverFailure,
) -> (PiScanSetupChannels, Arc<DriverEvidence>) {
    let evidence = Arc::new(DriverEvidence::default());
    let driver = FakeDriver {
        now: 1_000,
        snapshot: snapshot(1_000),
        failure,
        evidence: Arc::clone(&evidence),
    };
    (
        spawn_pi_scan_setup_controller_with_driver(options(root, dry_run), Box::new(driver)),
        evidence,
    )
}

/// Receive one bounded setup event.
async fn recv_event(channels: &mut PiScanSetupChannels) -> PiScanSetupEvent {
    tokio::time::timeout(Duration::from_secs(3), channels.event_rx.recv())
        .await
        .expect("bounded event")
        .expect("event channel")
}

/// Probe and validate, returning the exact binding.
async fn probe_and_validate(
    channels: &mut PiScanSetupChannels,
    selected: PiScanSettings,
) -> String {
    channels
        .request_tx
        .send(PiScanSetupRequest::BeginSetupProbe {
            correlation_id: 1,
            binary: selected.binary.clone(),
        })
        .expect("probe send");
    assert!(matches!(
        recv_event(channels).await,
        PiScanSetupEvent::CapabilitiesVerified { .. }
    ));
    channels
        .request_tx
        .send(PiScanSetupRequest::ValidateSetupCandidate {
            correlation_id: 2,
            candidate: selected,
            consent: PiScanConsentState::default(),
            confirmations: confirmations(),
        })
        .expect("validate send");
    match recv_event(channels).await {
        PiScanSetupEvent::CandidateValidated {
            validation_binding, ..
        } => validation_binding,
        event => panic!("expected validation, got {event:?}"),
    }
}

/// Send one final apply with the standard candidate.
fn send_apply(channels: &PiScanSetupChannels, binding: String) {
    channels
        .request_tx
        .send(PiScanSetupRequest::ApplySetupCandidate {
            correlation_id: 3,
            candidate: candidate(),
            consent: PiScanConsentState::default(),
            confirmations: confirmations(),
            validation_binding: binding,
        })
        .expect("apply send");
}

/// Validation must bind exact material without touching either durable file.
#[tokio::test]
async fn validation_is_write_free_and_unadvertised_routes_fail_closed() {
    let root = temp_root("validation");
    let prior = "unrelated = keep\npi_scan_enabled = false\n";
    std::fs::write(root.join("settings.conf"), prior).expect("seed settings");
    let (mut channels, _) = spawn_fake(&root, false, DriverFailure::None);
    let validation_binding = probe_and_validate(&mut channels, candidate()).await;
    assert!(!validation_binding.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("settings.conf")).expect("settings"),
        prior
    );
    assert!(!root.join("pi_scan/consent-v1.json").exists());

    let mut invalid = candidate();
    invalid.model = "not-advertised".to_string();
    channels
        .request_tx
        .send(PiScanSetupRequest::ValidateSetupCandidate {
            correlation_id: 4,
            candidate: invalid,
            consent: PiScanConsentState::default(),
            confirmations: confirmations(),
        })
        .expect("invalid validation send");
    match recv_event(&mut channels).await {
        PiScanSetupEvent::Failed { stage, reason, .. } => {
            assert_eq!(stage, PiScanSetupStage::CandidateValidation);
            assert!(reason.contains("not in the exact verified advertised snapshot"));
        }
        event => panic!("expected route failure, got {event:?}"),
    }
}

/// Stale request correlations, validation bindings, and external config drift must not apply.
#[tokio::test]
async fn stale_correlation_binding_and_settings_fingerprint_fail_closed() {
    let root = temp_root("stale");
    let (mut channels, evidence) = spawn_fake(&root, false, DriverFailure::None);
    let _binding = probe_and_validate(&mut channels, candidate()).await;
    channels
        .request_tx
        .send(PiScanSetupRequest::ValidateSetupCandidate {
            correlation_id: 1,
            candidate: candidate(),
            consent: PiScanConsentState::default(),
            confirmations: confirmations(),
        })
        .expect("stale send");
    assert!(matches!(
        recv_event(&mut channels).await,
        PiScanSetupEvent::Failed {
            stage: PiScanSetupStage::CandidateValidation,
            ..
        }
    ));
    send_apply(&channels, "wrong-binding".to_string());
    assert!(matches!(
        recv_event(&mut channels).await,
        PiScanSetupEvent::Failed {
            stage: PiScanSetupStage::CandidateValidation,
            ..
        }
    ));
    assert_eq!(evidence.prepared.load(Ordering::SeqCst), 0);

    let root = temp_root("drift");
    let (mut channels, evidence) = spawn_fake(&root, false, DriverFailure::None);
    let binding = probe_and_validate(&mut channels, candidate()).await;
    std::fs::write(root.join("settings.conf"), "external = edit\n").expect("external edit");
    send_apply(&channels, binding);
    match recv_event(&mut channels).await {
        PiScanSetupEvent::Failed { stage, reason, .. } => {
            assert_eq!(stage, PiScanSetupStage::CandidateValidation);
            assert!(reason.contains("changed after validation"));
        }
        event => panic!("expected drift failure, got {event:?}"),
    }
    assert_eq!(evidence.probes.load(Ordering::SeqCst), 1);
    assert!(!root.join("pi_scan/consent-v1.json").exists());
}

/// Dry-run must dispatch no probe, runtime preparation, process, or write.
#[tokio::test]
async fn dry_run_launches_no_probe_or_runtime_and_writes_nothing() {
    let root = temp_root("dry-run");
    let (mut channels, evidence) = spawn_fake(&root, true, DriverFailure::None);
    channels
        .request_tx
        .send(PiScanSetupRequest::BeginSetupProbe {
            correlation_id: 1,
            binary: "/opt/pi/bin/pi".to_string(),
        })
        .expect("dry-run send");
    match recv_event(&mut channels).await {
        PiScanSetupEvent::Failed { reason, .. } => assert!(reason.contains("dry-run")),
        event => panic!("expected dry-run failure, got {event:?}"),
    }
    assert_eq!(evidence.probes.load(Ordering::SeqCst), 0);
    assert_eq!(evidence.prepared.load(Ordering::SeqCst), 0);
    assert!(!root.join("settings.conf").exists());
    assert!(!root.join("pi_scan/consent-v1.json").exists());
}

/// The settings transaction must preserve unrelated data and write all Pi keys in one result.
#[test]
fn atomic_settings_patch_changes_exactly_pi_scan_keys() {
    let root = temp_root("atomic-settings");
    let path = root.join("settings.conf");
    let prior = "# retained comment\nunrelated_key = exact value\npi_scan_model = old\n";
    std::fs::write(&path, prior).expect("seed settings");
    let fingerprint = snapshot_config_file(&path).expect("snapshot").fingerprint;
    patch_pi_scan_settings_atomic(&path, &fingerprint, &candidate()).expect("atomic patch");
    let saved = std::fs::read_to_string(&path).expect("saved settings");
    assert!(saved.contains("# retained comment\nunrelated_key = exact value\n"));
    assert!(saved.contains("pi_scan_enabled = true\n"));
    assert!(saved.contains("pi_scan_provider = provider-a\n"));
    assert_eq!(saved.matches("pi_scan_model = ").count(), 1);
    assert_eq!(saved.matches("unrelated_key = ").count(), 1);
}

/// Save and consent failures must tear down candidates and preserve prior files.
#[tokio::test]
async fn save_and_consent_failures_preserve_files_and_teardown_candidate() {
    for (tag, failure) in [
        ("save-failure", DriverFailure::Save),
        ("consent-failure", DriverFailure::Consent),
        (
            "settings-after-consent-failure",
            DriverFailure::SettingsAfterConsent,
        ),
    ] {
        let root = temp_root(tag);
        let prior_settings = "unrelated = before\npi_scan_enabled = false\n";
        let prior_consent = "{\"prior\":true}\n";
        std::fs::write(root.join("settings.conf"), prior_settings).expect("seed settings");
        std::fs::create_dir_all(root.join("pi_scan")).expect("consent parent");
        std::fs::write(root.join("pi_scan/consent-v1.json"), prior_consent).expect("seed consent");
        let (mut channels, evidence) = spawn_fake(&root, false, failure);
        let binding = probe_and_validate(&mut channels, candidate()).await;
        send_apply(&channels, binding);
        assert!(matches!(
            recv_event(&mut channels).await,
            PiScanSetupEvent::Failed {
                stage: PiScanSetupStage::Persistence,
                ..
            }
        ));
        assert_eq!(
            std::fs::read_to_string(root.join("settings.conf")).expect("settings"),
            prior_settings
        );
        assert_eq!(
            std::fs::read_to_string(root.join("pi_scan/consent-v1.json")).expect("consent"),
            prior_consent
        );
        assert_eq!(evidence.torn_down.load(Ordering::SeqCst), 1);
    }
}

/// Activation failure must restore exact files and report rollback status explicitly.
#[tokio::test]
async fn activation_failure_rolls_back_committed_files() {
    let root = temp_root("activation-failure");
    let prior_settings = "unrelated = before\npi_scan_enabled = false\n";
    let prior_consent = "{\"prior\":true}\n";
    std::fs::write(root.join("settings.conf"), prior_settings).expect("seed settings");
    std::fs::create_dir_all(root.join("pi_scan")).expect("consent parent");
    std::fs::write(root.join("pi_scan/consent-v1.json"), prior_consent).expect("seed consent");
    let (mut channels, evidence) = spawn_fake(&root, false, DriverFailure::Activation);
    let binding = probe_and_validate(&mut channels, candidate()).await;
    send_apply(&channels, binding);
    assert!(matches!(
        recv_event(&mut channels).await,
        PiScanSetupEvent::Applied { .. }
    ));
    let transfer = channels.transfer_rx.recv().await.expect("transfer");
    let Err(error) = transfer.activate() else {
        panic!("activation must fail");
    };
    assert_eq!(error.reason, "injected runtime activation failure");
    assert!(error.rollback_failure.is_none());
    assert_eq!(
        std::fs::read_to_string(root.join("settings.conf")).expect("settings"),
        prior_settings
    );
    assert_eq!(
        std::fs::read_to_string(root.join("pi_scan/consent-v1.json")).expect("consent"),
        prior_consent
    );
    assert_eq!(evidence.activated.load(Ordering::SeqCst), 1);
}

/// Dropping an unaccepted transfer must tear down its candidate and roll back durable files.
#[tokio::test]
async fn unaccepted_transfer_tears_down_and_restores_previous_files() {
    let root = temp_root("transfer-drop");
    let prior = "pi_scan_enabled = false\n";
    std::fs::write(root.join("settings.conf"), prior).expect("seed settings");
    let (mut channels, evidence) = spawn_fake(&root, false, DriverFailure::None);
    let binding = probe_and_validate(&mut channels, candidate()).await;
    send_apply(&channels, binding);
    assert!(matches!(
        recv_event(&mut channels).await,
        PiScanSetupEvent::Applied { .. }
    ));
    let transfer = channels.transfer_rx.recv().await.expect("transfer");
    drop(transfer);
    assert_eq!(
        std::fs::read_to_string(root.join("settings.conf")).expect("settings"),
        prior
    );
    assert!(!root.join("pi_scan/consent-v1.json").exists());
    assert_eq!(evidence.torn_down.load(Ordering::SeqCst), 1);
}

/// Successful activation exposes queue-inert channels and leaves compatible material consent.
#[tokio::test]
async fn successful_transfer_commits_runtime_and_compatible_consent() {
    let root = temp_root("success");
    let (mut channels, evidence) = spawn_fake(&root, false, DriverFailure::None);
    let selected = candidate();
    let binding = probe_and_validate(&mut channels, selected.clone()).await;
    send_apply(&channels, binding);
    assert!(matches!(
        recv_event(&mut channels).await,
        PiScanSetupEvent::Applied { .. }
    ));
    let transfer = channels.transfer_rx.recv().await.expect("transfer");
    assert_eq!(transfer.correlation_id(), 3);
    let activated = transfer.activate().expect("activation");
    assert_eq!(activated.effective(), &selected);
    let runtime_channels = activated.commit().expect("channel commit");
    assert_eq!(evidence.activated.load(Ordering::SeqCst), 1);

    let consent_bytes = std::fs::read(root.join("pi_scan/consent-v1.json")).expect("consent");
    let document: serde_json::Value = serde_json::from_slice(&consent_bytes).expect("consent JSON");
    let binding = document["configuration_binding"]
        .as_str()
        .expect("material binding")
        .to_string();
    let expected_binding = crate::pi_scan_production::production_consent_binding(
        &production_runtime_settings(
            &selected,
            vec![ModelChoice {
                provider: selected.provider.clone(),
                model: selected.model.clone(),
            }],
            PiScanReservation {
                tokens: SETUP_PROBE_RESERVATION_TOKENS,
                cost_microusd: 10,
            },
        )
        .expect("production settings"),
    );
    assert_eq!(binding, expected_binding);
    let orchestrator = PiScanOrchestrator::new(
        OrchestrationConfig {
            enabled: true,
            setup_confirmed: false,
            background_execution: false,
            initial_consent: PiScanConsentState::default(),
            consent_binding: binding.clone(),
            consent_path: root.join("pi_scan/consent-v1.json"),
            consent_quarantine_dir: root.join("pi_scan/quarantine/consent"),
            dry_run: false,
            state_path: root.join("pi_scan/orchestration-v1.json"),
            results_root: root.join("pi_scan/results-v1"),
            result_quarantine_dir: root.join("pi_scan/quarantine/results"),
            quarantine_dir: root.join("pi_scan/quarantine/orchestration"),
            baseline_path: root.join("pi_scan/baseline-v1.json"),
            baseline_quarantine_dir: root.join("pi_scan/quarantine/baseline"),
            observation_interval_seconds: 900,
            budget_limits: PiScanBudgetLimits::default(),
        },
        InertAdapter,
    )
    .expect("production-compatible consent load");
    let (runtime_consent, setup_consent) = orchestrator.consent_snapshot();
    assert!(runtime_consent.paid_execution);
    assert!(!runtime_consent.background_observation);
    assert_eq!(setup_consent.configuration_binding, binding);
    assert!(!setup_consent.background_paid_execution);
    assert_eq!(setup_consent.confirmed_pi_version, "0.84.0");
    assert!(!setup_consent.confirmed_pricing_binding.is_empty());

    let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
    runtime_channels
        .shutdown_tx
        .send(PiScanShutdownMessage {
            acknowledge: ack_tx,
        })
        .expect("shutdown send");
    let ack = tokio::task::spawn_blocking(move || ack_rx.recv_timeout(Duration::from_secs(3)))
        .await
        .expect("wait join")
        .expect("shutdown ack");
    assert!(ack.persisted);
}
