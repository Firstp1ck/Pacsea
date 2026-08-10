//! Wave 0 contracts and lifecycle spike for the guided Pi Scan setup wizard.
//!
//! Ignored tests are behavior contracts for WS1/WS2/integration work: they
//! compile against the Wave 0 contract surface and fail for missing behavior,
//! not harness mistakes. Run them explicitly with `--ignored`. The lifecycle
//! spike is not ignored and proves bounded restart-free worker replacement.

use pacsea::app::{
    PiScanProgressMessage, PiScanRequestMessage, PiScanSetupControllerOptions, PiScanSetupEvent,
    PiScanSetupRequest, PiScanSetupStage, PiScanShutdownMessage, spawn_default_off_pi_scan_worker,
    spawn_pi_scan_setup_controller,
};
use pacsea::state::pi_scan::PiScanConsentState;
use pacsea::state::pi_scan_setup::PiScanSetupConfirmations;
use pacsea::state::{AppState, PackageItem, Source};
use pacsea::theme::PiScanSettings;
use std::time::Duration;

/// Bounded await for one correlated setup event on a fresh controller.
async fn recv_event(
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<PiScanSetupEvent>,
) -> PiScanSetupEvent {
    tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
        .await
        .expect("setup controller must answer within the bounded deadline")
        .expect("setup controller channel must stay open while requests are pending")
}

/// Controller options rooted in a private temporary directory.
fn controller_options(root: &std::path::Path, dry_run: bool) -> PiScanSetupControllerOptions {
    PiScanSetupControllerOptions {
        dry_run,
        settings_path: root.join("settings.conf"),
        consent_path: root.join("pi_scan").join("consent-v1.json"),
        state_path: root.join("pi_scan").join("backlog-v1.json"),
        quarantine_dir: root.join("pi_scan").join("quarantine"),
    }
}

/// Unique per-test temporary root that never touches the user configuration.
fn temp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pacsea_setup_wizard_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_nanos())
    ));
    std::fs::create_dir_all(&root).expect("temporary contract root must be creatable");
    root
}

/// Contract: pressing the Pi Scan shortcut with `pi_scan_enabled = false` must
/// open the guided wizard instead of the raw Setup information dump.
#[test]
fn wizard_opens_while_scanning_is_disabled() {
    let mut app = AppState::default();
    assert!(!app.pi_scan.settings.enabled);
    app.results.push(PackageItem {
        name: "demo-bin".to_string(),
        version: "1".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    });
    pacsea::events::pi_scan::open_from_search(&mut app);
    let wizard = app
        .pi_scan
        .wizard
        .as_ref()
        .expect("incomplete setup must route Shift+A into the guided wizard");
    assert!(wizard.first_run);
    assert_eq!(
        wizard.step,
        pacsea::state::PiScanSetupStep::Welcome,
        "first-run entry must start at the Welcome step"
    );
}

/// Contract: a verified existing setup bypasses the wizard and opens the
/// normal workspace path unchanged.
#[test]
fn verified_setup_bypasses_wizard() {
    let mut app = AppState::default();
    app.pi_scan.settings.enabled = true;
    app.pi_scan.settings.provider = "provider".to_string();
    app.pi_scan.settings.model = "model".to_string();
    app.pi_scan.disclosure_confirmed = true;
    app.pi_scan.runtime.consent.paid_execution = true;
    app.pi_scan.setup_facts_verified = true;
    pacsea::events::pi_scan::open_from_search(&mut app);
    assert!(
        app.pi_scan.wizard.is_none(),
        "a verified setup must open the normal workspace, not the wizard"
    );
}

/// Contract: Cancel restores the original projection and performs no writes.
#[test]
fn wizard_cancel_restores_original_state_without_writes() {
    let mut app = AppState::default();
    let original = app.pi_scan.settings.clone();
    app.pi_scan.wizard = Some(pacsea::state::PiScanSetupWizardState::open(
        original.clone(),
        PiScanConsentState::default(),
        true,
    ));
    if let Some(wizard) = app.pi_scan.wizard.as_mut() {
        wizard.candidate.enabled = true;
        wizard.candidate.provider = "changed".to_string();
    }
    let handled = pacsea::events::pi_scan::handle_key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
        &mut app,
    );
    assert!(handled);
    assert!(
        app.pi_scan.wizard.is_none(),
        "Cancel must close the wizard and drop the draft"
    );
    assert_eq!(
        app.pi_scan.settings, original,
        "Cancel must leave the effective settings untouched"
    );
    assert!(
        app.pi_scan.pending_action.is_none(),
        "Cancel must not queue any runtime action"
    );
}

/// Contract: the probe verifies Pi and enumerates routes without a model call.
#[tokio::test]
#[ignore = "Wave 0 contract: WS2 no-model capability probe"]
async fn setup_probe_verifies_capabilities_without_model_call() {
    let root = temp_root("probe");
    let mut channels = spawn_pi_scan_setup_controller(controller_options(&root, false));
    channels
        .request_tx
        .send(PiScanSetupRequest::BeginSetupProbe {
            correlation_id: 1,
            binary: "pi".to_string(),
        })
        .expect("controller must accept probe requests");
    match recv_event(&mut channels.event_rx).await {
        PiScanSetupEvent::CapabilitiesVerified {
            correlation_id,
            snapshot,
        } => {
            assert_eq!(correlation_id, 1);
            assert!(!snapshot.pi_version.trim().is_empty());
            assert!(!snapshot.available_models.is_empty());
            assert!(!snapshot.pricing_binding.trim().is_empty());
        }
        other => panic!("expected CapabilitiesVerified, got {other:?}"),
    }
}

/// Live acceptance: Apply must activate production and preserve newly reviewed consent in-process.
#[tokio::test]
#[ignore = "requires installed Pi >=0.84.0 with configured priced routes; metadata only"]
async fn live_setup_apply_activates_and_restores_consent_without_restart() {
    let root = temp_root("live-activation");
    let mut channels = spawn_pi_scan_setup_controller(controller_options(&root, false));
    channels
        .request_tx
        .send(PiScanSetupRequest::BeginSetupProbe {
            correlation_id: 1,
            binary: "pi".to_string(),
        })
        .expect("probe send");
    let snapshot = match recv_event(&mut channels.event_rx).await {
        PiScanSetupEvent::CapabilitiesVerified { snapshot, .. } => snapshot,
        event => panic!("expected live capabilities, got {event:?}"),
    };
    let candidate = PiScanSettings {
        enabled: true,
        binary: "pi".to_string(),
        provider: snapshot.selected_provider.clone(),
        model: snapshot.selected_model.clone(),
        ..PiScanSettings::default()
    };
    let confirmations = PiScanSetupConfirmations {
        disclosure_confirmed: true,
        foreground_paid_confirmed: true,
        fallback_confirmed: false,
        readiness_warning_confirmed: true,
    };
    channels
        .request_tx
        .send(PiScanSetupRequest::ValidateSetupCandidate {
            correlation_id: 2,
            candidate: candidate.clone(),
            consent: PiScanConsentState::default(),
            confirmations,
        })
        .expect("validation send");
    let validation_binding = match recv_event(&mut channels.event_rx).await {
        PiScanSetupEvent::CandidateValidated {
            validation_binding, ..
        } => validation_binding,
        event => panic!("expected live validation, got {event:?}"),
    };
    channels
        .request_tx
        .send(PiScanSetupRequest::ApplySetupCandidate {
            correlation_id: 3,
            candidate,
            consent: PiScanConsentState::default(),
            confirmations,
            validation_binding,
        })
        .expect("apply send");
    assert!(matches!(
        recv_event(&mut channels.event_rx).await,
        PiScanSetupEvent::Applied { .. }
    ));
    let transfer = tokio::time::timeout(Duration::from_secs(30), channels.transfer_rx.recv())
        .await
        .expect("bounded transfer")
        .expect("runtime transfer");
    let activated = transfer.activate().expect("live candidate activation");
    let expected_pricing = activated.snapshot().pricing_binding.clone();
    let mut runtime = activated.commit().expect("accept live runtime");
    let restored = loop {
        let progress = tokio::time::timeout(Duration::from_secs(30), runtime.progress_rx.recv())
            .await
            .expect("bounded production startup")
            .expect("production progress");
        if let PiScanProgressMessage::RestoredConsent { consent, setup } = progress {
            break (consent, setup);
        }
    };
    assert!(restored.0.paid_execution);
    assert!(restored.1.disclosure_confirmed);
    assert_eq!(restored.1.confirmed_pricing_binding, expected_pricing);

    let (acknowledge, receiver) = std::sync::mpsc::sync_channel(1);
    runtime
        .shutdown_tx
        .send(PiScanShutdownMessage { acknowledge })
        .expect("shutdown send");
    let acknowledgement =
        tokio::task::spawn_blocking(move || receiver.recv_timeout(Duration::from_secs(10)))
            .await
            .expect("shutdown wait task")
            .expect("shutdown acknowledgement");
    assert!(acknowledgement.persisted);
}

/// Contract: candidate validation before a verified advertised-route snapshot fails closed.
#[tokio::test]
async fn candidate_validation_rejects_unverified_route() {
    let root = temp_root("route");
    let mut channels = spawn_pi_scan_setup_controller(controller_options(&root, false));
    let candidate = PiScanSettings {
        enabled: true,
        provider: "never-advertised".to_string(),
        model: "missing-model".to_string(),
        ..PiScanSettings::default()
    };
    channels
        .request_tx
        .send(PiScanSetupRequest::ValidateSetupCandidate {
            correlation_id: 7,
            candidate,
            consent: PiScanConsentState::default(),
            confirmations: PiScanSetupConfirmations {
                disclosure_confirmed: true,
                foreground_paid_confirmed: true,
                fallback_confirmed: false,
                readiness_warning_confirmed: true,
            },
        })
        .expect("controller must accept validation requests");
    match recv_event(&mut channels.event_rx).await {
        PiScanSetupEvent::Failed {
            correlation_id,
            stage,
            reason,
        } => {
            assert_eq!(correlation_id, 7);
            assert_eq!(stage, PiScanSetupStage::CandidateValidation);
            assert!(
                reason.to_lowercase().contains("advertis")
                    || reason.to_lowercase().contains("route")
                    || reason.to_lowercase().contains("probe"),
                "rejection must explain the unadvertised route: {reason}"
            );
        }
        other => panic!("expected typed CandidateValidation failure, got {other:?}"),
    }
}

/// Contract: candidate validation makes no durable change.
#[tokio::test]
async fn candidate_validation_writes_nothing() {
    let root = temp_root("validate_inert");
    let options = controller_options(&root, false);
    let settings_path = options.settings_path.clone();
    let consent_path = options.consent_path.clone();
    let mut channels = spawn_pi_scan_setup_controller(options);
    channels
        .request_tx
        .send(PiScanSetupRequest::ValidateSetupCandidate {
            correlation_id: 2,
            candidate: PiScanSettings::default(),
            consent: PiScanConsentState::default(),
            confirmations: PiScanSetupConfirmations::default(),
        })
        .expect("controller must accept validation requests");
    let _ = recv_event(&mut channels.event_rx).await;
    assert!(
        !settings_path.exists(),
        "validation must not create or patch settings.conf"
    );
    assert!(
        !consent_path.exists(),
        "validation must not persist any consent document"
    );
}

/// Contract: apply without a fresh validation binding fails closed before
/// activation or persistence.
#[tokio::test]
async fn apply_requires_fresh_validation_binding() {
    let root = temp_root("binding");
    let options = controller_options(&root, false);
    let settings_path = options.settings_path.clone();
    let consent_path = options.consent_path.clone();
    let mut channels = spawn_pi_scan_setup_controller(options);
    channels
        .request_tx
        .send(PiScanSetupRequest::ApplySetupCandidate {
            correlation_id: 3,
            candidate: PiScanSettings::default(),
            consent: PiScanConsentState::default(),
            confirmations: PiScanSetupConfirmations::default(),
            validation_binding: "stale-or-missing".to_string(),
        })
        .expect("controller must accept apply requests");
    match recv_event(&mut channels.event_rx).await {
        PiScanSetupEvent::Failed { stage, .. } => {
            assert!(
                matches!(
                    stage,
                    PiScanSetupStage::CandidateValidation | PiScanSetupStage::Probe
                ),
                "apply with a stale binding must fail before activation, got {stage:?}"
            );
        }
        other => panic!("expected typed failure, got {other:?}"),
    }
    assert!(!settings_path.exists());
    assert!(!consent_path.exists());
}

/// Contract: dry-run setup answers with typed dry-run failures and never
/// probes Pi or touches durable paths.
#[tokio::test]
async fn dry_run_setup_is_inert() {
    let root = temp_root("dry_run");
    let options = controller_options(&root, true);
    let settings_path = options.settings_path.clone();
    let consent_path = options.consent_path.clone();
    let mut channels = spawn_pi_scan_setup_controller(options);
    channels
        .request_tx
        .send(PiScanSetupRequest::BeginSetupProbe {
            correlation_id: 4,
            binary: "pi".to_string(),
        })
        .expect("controller must accept probe requests");
    match recv_event(&mut channels.event_rx).await {
        PiScanSetupEvent::Failed { reason, .. } => {
            assert!(
                reason.to_lowercase().contains("dry-run")
                    || reason.to_lowercase().contains("dry run"),
                "dry-run rejection must name dry-run mode: {reason}"
            );
        }
        other => panic!("dry-run must not verify capabilities, got {other:?}"),
    }
    assert!(!settings_path.exists());
    assert!(!consent_path.exists());
}

/// Contract: wizard locale keys exist in every shipped locale.
#[test]
fn wizard_locale_keys_exist_in_all_locales() {
    let required_marker = "wizard:";
    for locale in ["en-US", "de-DE", "hu-HU"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config/locales")
            .join(format!("{locale}.yml"));
        let body = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("locale file {} must be readable: {error}", path.display())
        });
        assert!(
            body.contains(required_marker),
            "{locale} must define the pi_scan wizard key namespace"
        );
        let wizard = body
            .split_once("      wizard:")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split_once("      overview:").map(|(wizard, _)| wizard))
            .expect("wizard locale section");
        for key in [
            "dry_run_probe:",
            "binary_required:",
            "failure_stage:",
            "validation_write_free:",
            "pi_version:",
            "compiled:",
        ] {
            assert!(wizard.contains(key), "{locale} must define {key}");
        }
        assert!(
            !wizard.contains("TODO: translate"),
            "{locale} wizard messages must be genuinely localized"
        );
    }
}

/// Lifecycle spike (not ignored): the channel owner can replace one Pi scan
/// worker with another without restart, and the old worker acknowledges a
/// bounded shutdown at its durability boundary.
#[tokio::test]
async fn lifecycle_spike_worker_swap_is_bounded_and_restart_free() {
    let old_channels = spawn_default_off_pi_scan_worker();
    let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
    old_channels
        .shutdown_tx
        .send(PiScanShutdownMessage {
            acknowledge: ack_tx,
        })
        .expect("old worker must accept a shutdown request");
    let ack = tokio::task::spawn_blocking(move || {
        ack_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("old worker must acknowledge shutdown within its bounded deadline")
    })
    .await
    .expect("shutdown wait task must join");
    assert!(
        ack.persisted,
        "default-off shutdown must reach its durability boundary"
    );
    assert!(!ack.active_interrupted);

    let replacement = spawn_default_off_pi_scan_worker();
    replacement
        .request_tx
        .send(PiScanRequestMessage::RevalidateBudgets { now_unix: 0 })
        .expect("replacement worker must accept requests after the swap");
}
