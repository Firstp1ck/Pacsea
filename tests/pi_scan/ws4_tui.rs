//! Deterministic WS4 settings, keyflow, state, and narrow-render coverage.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use pacsea::logic::pi_scan::result::{
    Coverage, ExpectedIdentity, MergedFinding, MergedScanResult, Severity,
};
use pacsea::state::types::AppMode;
use pacsea::state::{
    AppState, PackageItem, PiScanAvailability, PiScanDisplayResult, PiScanUiAction, PiScanView,
    PkgbuildCheckRequest, Source,
};
use ratatui::{Terminal, backend::TestBackend};
use tokio::sync::mpsc;

/// Channel tuple used by the public event dispatcher test.
type EventChannels = (
    mpsc::UnboundedSender<pacsea::state::QueryInput>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<PackageItem>,
    mpsc::UnboundedSender<String>,
    mpsc::UnboundedSender<PkgbuildCheckRequest>,
);

/// Build all channels required by the public event dispatcher.
fn event_channels() -> EventChannels {
    let (query, _) = mpsc::unbounded_channel();
    let (details, _) = mpsc::unbounded_channel();
    let (preview, _) = mpsc::unbounded_channel();
    let (add, _) = mpsc::unbounded_channel();
    let (pkgbuild, _) = mpsc::unbounded_channel();
    let (comments, _) = mpsc::unbounded_channel();
    let (checks, _) = mpsc::unbounded_channel();
    (query, details, preview, add, pkgbuild, comments, checks)
}

/// Verify conservative runtime defaults and actionable upper-bound validation.
#[test]
fn pi_scan_settings_are_conservative_and_report_raised_limits() {
    let mut settings = pacsea::theme::PiScanSettings::default();
    assert!(!settings.enabled);
    assert!(!settings.background_enabled);
    assert_eq!(settings.binary, "pi");
    assert_eq!(settings.thinking, "medium");
    assert_eq!(settings.observation_interval_seconds, 900);
    assert_eq!(settings.background_cost_cap_24h, "0.00");
    assert!(!settings.show_raw_output);
    assert!(settings.validation_issues().is_empty());

    settings.head_query_timeout_seconds = 16;
    settings.background_token_cap_24h = 500_001;
    assert_eq!(settings.validation_issues().len(), 2);
}

/// Verify Shift+A opens Pi Scan only from Search normal mode with AUR context.
#[test]
fn shift_a_from_search_normal_mode_opens_contextual_pi_scan() {
    let mut app = AppState {
        search_normal_mode: true,
        ..AppState::default()
    };
    app.results.push(PackageItem {
        name: "demo-bin".to_string(),
        version: "1".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    });
    let channels = event_channels();
    let event = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT));
    let exited = pacsea::events::handle_event(
        &event,
        &mut app,
        &channels.0,
        &channels.1,
        &channels.2,
        &channels.3,
        &channels.4,
        &channels.5,
        &channels.6,
    );
    assert!(!exited);
    assert_eq!(app.app_mode, AppMode::PiScan);
    assert_eq!(app.pi_scan.view, PiScanView::Setup);
    assert_eq!(app.pi_scan.targets[0].package_name, "demo-bin");
}

/// Verify dry-run queue action requests bounded acquisition without local queue mutation.
#[test]
fn dry_run_target_action_creates_preview_without_queue_mutation() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    app.pi_scan.settings.enabled = true;
    app.pi_scan.open_context(Some("demo"), true);
    app.pi_scan.view = PiScanView::Targets;
    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.dry_run_preview.is_some());
    assert!(app.pi_scan.runtime.queue.is_empty());
    assert_eq!(
        app.pi_scan.pending_action,
        Some(pacsea::state::PiScanUiAction::QueueSelected)
    );
}

/// Verify material consent keys first request exact setup facts and require a second press.
#[test]
fn setup_consent_requires_verified_pi_and_pricing_facts() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan.view = PiScanView::Setup;

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        &mut app,
    ));
    assert!(!app.pi_scan.runtime.consent.paid_execution);
    assert_eq!(app.pi_scan.pending_action, Some(PiScanUiAction::ProbeSetup));

    app.pi_scan.setup_facts_verified = true;
    app.pi_scan.pending_action = None;
    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.runtime.consent.paid_execution);
    assert_eq!(
        app.pi_scan.pending_action,
        Some(PiScanUiAction::UpdateConsent)
    );
}

/// Verify high/critical and stale acknowledgements are separate and exact-result-bound.
#[test]
fn acknowledgements_are_separate_and_bound_to_validated_result() {
    let result = MergedScanResult {
        identity: ExpectedIdentity {
            scan_id: "scan-1".to_string(),
            package_base: "demo".to_string(),
            commit_oid: "0123456789012345678901234567890123456789".to_string(),
        },
        coverage: Coverage::Incomplete,
        limitations: vec!["mutable source remained".to_string()],
        findings: vec![MergedFinding {
            fingerprint: "fingerprint-1".to_string(),
            severity: Severity::High,
            snapshot: "recipe".to_string(),
            path: "PKGBUILD".to_string(),
            evidence: "curl example".to_string(),
            assessments: Vec::new(),
            disagreement: false,
        }],
    };
    let mut app = AppState::default();
    app.pi_scan.results.push(PiScanDisplayResult {
        observed_head_oid: result.identity.commit_oid.clone(),
        validated: result,
        stale: true,
        mutable_sources: Vec::new(),
    });
    app.pi_scan.view = PiScanView::Details;
    assert!(!app.pi_scan.selected_result_acknowledged());
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        &mut app,
    );
    assert!(app.pi_scan.pending_action.is_none());
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        &mut app,
    );
    assert!(!app.pi_scan.selected_result_acknowledged());
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        &mut app,
    );
    assert!(app.pi_scan.selected_result_acknowledged());
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        &mut app,
    );
    assert_eq!(
        app.pi_scan.pending_action,
        Some(PiScanUiAction::ContinueSelected)
    );
}

/// Verify every shipped locale carries the Pi Scan workspace keys directly.
#[test]
fn all_locales_include_pi_scan_workspace_translations() {
    let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
    for locale in ["en-US", "de-DE", "hu-HU"] {
        let translations = pacsea::i18n::load_locale_file(locale, &locales)
            .unwrap_or_else(|error| panic!("{locale} locale failed: {error}"));
        for key in [
            "app.pi_scan.title",
            "app.pi_scan.tabs.setup",
            "app.pi_scan.setup.privacy_cost",
            "app.pi_scan.wizard.pricing.selected_route",
            "app.pi_scan.wizard.pricing.worst_case",
            "app.pi_scan.wizard.pricing.tokens",
            "app.pi_scan.wizard.pricing.provenance",
            "app.pi_scan.wizard.pricing.provenance_value",
            "app.pi_scan.targets.dry_run_disclosure",
            "app.pi_scan.details.ack_keys",
            "app.pi_scan.notices.runtime_disconnected",
        ] {
            assert!(translations.contains_key(key), "{locale} missing {key}");
        }
    }
}

/// Verify approved no-findings wording and narrow terminal rendering remain deterministic.
#[test]
fn exact_completion_wording_and_narrow_rendering() {
    let validated = MergedScanResult {
        identity: ExpectedIdentity {
            scan_id: "scan-2".to_string(),
            package_base: "demo".to_string(),
            commit_oid: "abcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string(),
        },
        coverage: Coverage::Complete,
        limitations: Vec::new(),
        findings: Vec::new(),
    };
    assert_eq!(
        validated.completion_wording(),
        "Complete — no findings in analyzed scope"
    );

    for (width, height) in [(36, 10), (20, 6)] {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        app.pi_scan.availability = PiScanAvailability::MissingBinary;
        app.pi_scan.results.push(PiScanDisplayResult {
            observed_head_oid: validated.identity.commit_oid.clone(),
            validated: validated.clone(),
            stale: false,
            mutable_sources: Vec::new(),
        });
        terminal
            .draw(|frame| pacsea::ui::ui(frame, &mut app))
            .expect("narrow Pi Scan render");
        assert_eq!(terminal.backend().buffer().area.width, width);
        assert_eq!(terminal.backend().buffer().area.height, height);
    }
}
