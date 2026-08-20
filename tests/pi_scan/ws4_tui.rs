//! Deterministic WS4 settings, keyflow, state, and narrow-render coverage.

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use pacsea::logic::pi_scan::identity::{CommitOid, PackageBase};
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

/// Load the shipped English locale into an application state for render assertions.
fn load_english(app: &mut AppState) {
    let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
    let translations = pacsea::i18n::load_locale_file("en-US", &locales).expect("English locale");
    app.translations.clone_from(&translations);
    app.translations_fallback = translations;
}

/// Render one full application frame and return its visible terminal text.
fn render_text(app: &mut AppState, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| pacsea::ui::ui(frame, app))
        .expect("test render");
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>()
}

/// Build one deterministic validated display result for list and details regressions.
fn display_result(name: &str, finding_count: usize) -> PiScanDisplayResult {
    let commit_oid = format!("{name:0<40}");
    let findings = (0..finding_count)
        .map(|index| MergedFinding {
            fingerprint: format!("fingerprint-{name}-{index}"),
            severity: Severity::Medium,
            snapshot: "recipe".to_string(),
            path: format!("path/{index}"),
            evidence: format!("evidence line {index}"),
            assessments: Vec::new(),
            disagreement: false,
        })
        .collect();
    PiScanDisplayResult {
        validated: MergedScanResult {
            identity: ExpectedIdentity {
                scan_id: format!("scan-{name}"),
                package_base: name.to_string(),
                commit_oid: commit_oid.clone(),
            },
            coverage: Coverage::Complete,
            limitations: Vec::new(),
            findings,
        },
        observed_head_oid: commit_oid,
        stale: false,
        mutable_sources: Vec::new(),
    }
}

/// Build one deterministic foreground request for active/progress render assertions.
fn scan_request() -> pacsea::state::pi_scan::PiScanJobRequest {
    pacsea::state::pi_scan::PiScanJobRequest {
        request_id: 7,
        key: pacsea::state::pi_scan::PiScanQueueKey {
            package_base: PackageBase::new("demo").expect("package base"),
            commit_oid: CommitOid::new("a".repeat(40)).expect("commit oid"),
        },
        priority: pacsea::state::pi_scan::PiScanPriority::Foreground,
        reservation: pacsea::state::pi_scan::PiScanReservation {
            tokens: 12_345,
            cost_microusd: 125_000,
        },
        manual_budget_override_confirmed: true,
    }
}

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

/// Pi Scan `BackTab` must navigate backward without mutating hidden Package sort state.
#[test]
fn pi_scan_backtab_navigates_without_hidden_package_mutation() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan.view = PiScanView::Targets;
    let original_sort = app.sort_mode;
    let channels = event_channels();

    let exited = pacsea::events::handle_event(
        &Event::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
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
    assert_eq!(app.pi_scan.view, PiScanView::Overview);
    assert_eq!(app.sort_mode, original_sort);
}

/// Printable help chords must edit the focused wizard text field before opening Help.
#[test]
fn wizard_text_field_question_mark_wins_over_global_help() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan.begin_setup_wizard(true);
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = pacsea::state::PiScanSetupStep::PiReadiness;
    wizard.focus = 0;
    let original_binary = wizard.candidate.binary.clone();
    let channels = event_channels();

    pacsea::events::handle_event(
        &Event::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT)),
        &mut app,
        &channels.0,
        &channels.1,
        &channels.2,
        &channels.3,
        &channels.4,
        &channels.5,
        &channels.6,
    );

    assert!(matches!(app.modal, pacsea::state::Modal::None));
    assert_eq!(
        app.pi_scan
            .wizard
            .as_ref()
            .expect("wizard")
            .candidate
            .binary,
        format!("{original_binary}?")
    );
}

/// A reload that closes guided setup must report the localized warning.
#[test]
fn settings_reload_closes_wizard_with_localized_warning() {
    let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        translations: pacsea::i18n::load_locale_file("de-DE", &locales).expect("German locale"),
        translations_fallback: pacsea::i18n::load_locale_file("en-US", &locales)
            .expect("English locale"),
        ..AppState::default()
    };
    app.pi_scan.begin_setup_wizard(false);
    let mut settings = pacsea::theme::Settings::default();
    settings.pi_scan = app.pi_scan.settings.clone();
    settings.pi_scan.binary = "different-pi".to_string();

    pacsea::app::apply_settings_to_app_state(&mut app, &settings);

    assert!(app.pi_scan.wizard.is_none());
    assert_eq!(
        app.pi_scan.notices.foreground_text(),
        Some(
            "Die Pi-Scan-Einstellungen wurden beim Neuladen geändert; die geführte Einrichtung wurde geschlossen, damit Sie die neuen Werte prüfen können."
        )
    );
}

/// Settings reload must preserve Pi Scan mode and live runtime-connected truth.
#[test]
fn settings_reload_preserves_pi_scan_mode_and_runtime_truth() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan.availability = PiScanAvailability::RuntimeConnected;
    app.pi_scan.begin_setup_wizard(false);
    let mut settings = pacsea::theme::Settings::default();
    settings.pi_scan = app.pi_scan.settings.clone();

    pacsea::app::apply_settings_to_app_state(&mut app, &settings);

    assert_eq!(app.app_mode, AppMode::PiScan);
    assert_eq!(
        app.pi_scan.availability,
        PiScanAvailability::RuntimeConnected
    );
    assert!(app.pi_scan.wizard.is_some());
}

/// The config editor must open Pi Scan setup through the configured chord.
#[test]
fn config_editor_uses_configured_pi_scan_setup_chord() {
    let mut app = AppState {
        app_mode: AppMode::ConfigEditor,
        ..AppState::default()
    };
    app.keymap.config_editor_pi_scan_setup = vec![pacsea::theme::KeyChord {
        code: KeyCode::Char('p'),
        mods: KeyModifiers::ALT,
    }];
    app.config_editor_state.selected_file = pacsea::theme::ConfigFile::Settings;
    app.config_editor_state.view = pacsea::state::ConfigEditorView::KeyList;
    app.config_editor_state.query = "pi_scan_binary".to_string();
    app.config_editor_state.clamp_key_cursor();
    let channels = event_channels();

    pacsea::events::handle_event(
        &Event::Key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT)),
        &mut app,
        &channels.0,
        &channels.1,
        &channels.2,
        &channels.3,
        &channels.4,
        &channels.5,
        &channels.6,
    );

    assert_eq!(app.app_mode, AppMode::PiScan);
    assert!(app.pi_scan.wizard.is_some());
}

/// Package-only global chords must not mutate hidden panes while Pi Scan owns input.
#[test]
fn pi_scan_blocks_package_only_global_chords() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan.view = PiScanView::Overview;
    let channels = event_channels();

    for (code, modifiers) in [
        (KeyCode::Char('x'), KeyModifiers::CONTROL),
        (KeyCode::Char('t'), KeyModifiers::CONTROL),
        (KeyCode::Char('k'), KeyModifiers::CONTROL),
        (KeyCode::Char('d'), KeyModifiers::CONTROL),
    ] {
        pacsea::events::handle_event(
            &Event::Key(KeyEvent::new(code, modifiers)),
            &mut app,
            &channels.0,
            &channels.1,
            &channels.2,
            &channels.3,
            &channels.4,
            &channels.5,
            &channels.6,
        );
    }

    assert!(!app.pkgb_visible);
    assert!(!app.comments_visible);
    assert_eq!(app.pi_scan.view, PiScanView::Overview);
}

/// Help remains global when no wizard text field owns the printable chord.
#[test]
fn wizard_non_text_focus_question_mark_opens_help() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan.begin_setup_wizard(true);
    let channels = event_channels();

    pacsea::events::handle_event(
        &Event::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
        &mut app,
        &channels.0,
        &channels.1,
        &channels.2,
        &channels.3,
        &channels.4,
        &channels.5,
        &channels.6,
    );

    assert!(matches!(app.modal, pacsea::state::Modal::Help));
}

/// Typed notice slots expire transient messages without dropping persistent errors.
#[test]
fn typed_notice_slots_expire_monotonically_and_remain_independent() {
    let mut slots = pacsea::state::pi_scan_ui::PiScanNoticeSlots::default();
    let now = std::time::Instant::now();
    slots.set_foreground_at(
        "queued",
        pacsea::state::pi_scan_ui::PiScanNoticeSeverity::Info,
        now,
    );
    slots.set_background_at(
        "background failed",
        pacsea::state::pi_scan_ui::PiScanNoticeSeverity::Error,
        now,
    );

    slots.expire_at(now + std::time::Duration::from_secs(7));

    assert!(slots.foreground.is_none());
    assert_eq!(
        slots.background.as_ref().map(|notice| notice.text.as_str()),
        Some("background failed")
    );
}

/// Independent target/result selection, queue intent, and no-list navigation stay isolated.
#[test]
fn workspace_state_foundations_preserve_independent_intent_and_navigation() {
    let mut app = AppState::default();
    app.pi_scan.targets.extend([
        pacsea::state::PiScanTarget {
            package_name: "zeta-bin".to_string(),
            package_base: "zeta".to_string(),
            commit_oid: None,
            selected: true,
            status: pacsea::state::PiScanTargetStatus::Unbaselined,
        },
        pacsea::state::PiScanTarget {
            package_name: "alpha-bin".to_string(),
            package_base: "alpha".to_string(),
            commit_oid: None,
            selected: true,
            status: pacsea::state::PiScanTargetStatus::Unbaselined,
        },
    ]);
    app.pi_scan.settings.background_token_cap_24h = 42;
    app.pi_scan.settings.background_cost_cap_24h = "1.25".to_string();
    app.pi_scan.snapshot_queue_intent();
    let intent = app.pi_scan.pending_queue_intent.as_ref().expect("intent");
    assert_eq!(intent.package_names, ["alpha-bin", "zeta-bin"]);
    assert_eq!(intent.reservation_tokens, 42);
    assert_eq!(intent.reservation_cost_cap, "1.25");

    app.pi_scan.selected_target = 1;
    app.pi_scan.selected_result = 4;
    app.pi_scan.set_view(PiScanView::Targets);
    assert_eq!(app.pi_scan.selected, 1);
    app.pi_scan.set_view(PiScanView::Results);
    assert_eq!(app.pi_scan.selected_result, 4);
    app.pi_scan.clamp_selection();
    assert_eq!(app.pi_scan.selected_result, 0);

    app.pi_scan.set_view(PiScanView::Overview);
    assert!(!pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        &mut app,
    ));

    app.pi_scan.record_result_inserted();
    assert_eq!(app.pi_scan.unseen_result_count, 1);
    app.pi_scan.set_view(PiScanView::Results);
    assert_eq!(app.pi_scan.unseen_result_count, 0);

    app.pi_scan
        .set_target_row_rects(vec![pacsea::state::pi_scan_ui::PiScanListHitRect {
            index: 1,
            x: 4,
            y: 8,
            width: 10,
            height: 1,
        }]);
    assert_eq!(app.pi_scan.target_hit_test(5, 8), Some(1));
    assert_eq!(app.pi_scan.target_hit_test(14, 8), None);
}

/// Cancelling with no active scan must still produce visible typed feedback.
#[test]
fn cancel_without_active_scan_sets_notice() {
    let mut app = AppState::default();
    load_english(&mut app);
    app.pi_scan.view = PiScanView::Progress;

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &mut app,
    ));

    assert!(
        app.pi_scan
            .notices
            .foreground_text()
            .is_some_and(|notice| notice.contains("No active"))
    );
}

/// Session raw-output toggling changes workspace state without rewriting settings.
#[test]
fn details_raw_output_toggle_is_session_only() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.results.push(display_result("raw-demo", 1));
    app.pi_scan.set_view(PiScanView::Details);
    assert!(!app.pi_scan.settings.show_raw_output);
    let hidden = render_text(&mut app, 100, 24);
    assert!(!hidden.contains("Canonical validated-data view"));

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        &mut app,
    ));

    assert!(app.pi_scan.show_raw_output);
    assert!(!app.pi_scan.settings.show_raw_output);
    let shown = render_text(&mut app, 100, 30);
    assert!(shown.contains("Canonical validated-data view"), "{shown:?}");
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

/// Material Pi Scan reload changes close only the wizard and explain the reset.
#[test]
fn material_pi_scan_reload_closes_wizard_with_typed_notice() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.availability = PiScanAvailability::RuntimeConnected;
    app.pi_scan.begin_setup_wizard(false);
    let mut settings = pacsea::theme::Settings::default();
    settings.pi_scan = app.pi_scan.settings.clone();
    settings.pi_scan.provider = "changed-provider".to_string();

    pacsea::app::apply_settings_to_app_state(&mut app, &settings);

    assert_eq!(app.app_mode, AppMode::PiScan);
    assert_eq!(
        app.pi_scan.availability,
        PiScanAvailability::RuntimeConnected
    );
    assert!(app.pi_scan.wizard.is_none());
    let notice = app
        .pi_scan
        .notices
        .foreground
        .as_ref()
        .expect("reload notice");
    assert_eq!(
        notice.severity,
        pacsea::state::pi_scan_ui::PiScanNoticeSeverity::Warning
    );
    assert!(notice.text.contains("settings changed"));
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
            "app.pi_scan.setup.pricing_binding",
            "app.pi_scan.wizard.pricing.selected_route",
            "app.pi_scan.wizard.pricing.worst_case",
            "app.pi_scan.wizard.pricing.tokens",
            "app.pi_scan.wizard.pricing.provenance",
            "app.pi_scan.wizard.pricing.provenance_value",
            "app.pi_scan.wizard.in_flight",
            "app.pi_scan.wizard.failure.controller_unavailable",
            "app.pi_scan.wizard.failure_timeout.probe",
            "app.pi_scan.wizard.failure_timeout.validation",
            "app.pi_scan.wizard.failure_timeout.activation",
            "app.pi_scan.wizard.failure_timeout.persistence",
            "app.pi_scan.targets.dry_run_disclosure",
            "app.pi_scan.progress.running_for",
            "app.pi_scan.progress.reservation",
            "app.pi_scan.details.ack_keys",
            "app.pi_scan.footer.keys.targets",
            "app.pi_scan.footer.keys.progress",
            "app.pi_scan.footer.keys.results",
            "app.pi_scan.footer.keys.details",
            "app.pi_scan.top_bar.running",
            "app.pi_scan.top_bar.new_results",
            "app.pi_scan.notices.runtime_disconnected",
            "app.pi_scan.notices.non_aur_entry",
            "app.pi_scan.notices.settings_changed_reload",
            "app.pi_scan.notices.select_result_continue",
            "app.pi_scan.notices.select_result_baseline",
            "app.pi_scan.notices.resolving_queue_intent",
            "app.pi_scan.notices.queue_intent_unresolved",
            "app.pi_scan.notices.queue_intent_submitted",
            "app.pi_scan.notices.validated_complete",
            "app.pi_scan.notices.cancelled",
            "app.pi_scan.notices.baseline_persisted",
            "app.pi_scan.notices.baseline_binding_changed",
            "app.pi_scan.notices.continuation_complete",
            "app.pi_scan.notices.runtime_rejected",
            "app.pi_scan.notices.dry_run_acquired",
            "app.pi_scan.notices.setup_complete",
            "app.pi_scan.notices.setup_failed",
            "app.pi_scan.notices.setup_rollback_complete",
            "app.pi_scan.notices.setup_rollback_failed",
            "app.pi_scan.notices.setup_secondary_outcome",
            "app.pi_scan.notices.policy.pause.requesting",
            "app.pi_scan.notices.policy.pause.queued",
            "app.pi_scan.notices.policy.pause.persisted",
            "app.pi_scan.notices.policy.pause.failed",
            "app.pi_scan.notices.policy.resume.requesting",
            "app.pi_scan.notices.policy.resume.queued",
            "app.pi_scan.notices.policy.resume.persisted",
            "app.pi_scan.notices.policy.resume.failed",
            "app.modals.help.sections.pi_scan",
            "app.modals.help.pi_scan_lines",
            "app.modals.help.key_labels.pi_scan_setup",
        ] {
            assert!(translations.contains_key(key), "{locale} missing {key}");
        }
    }
}

/// Help must document the complete Pi Scan workspace, wizard, and configured setup chord.
#[test]
fn pi_scan_help_renders_workspace_wizard_and_configured_chord() {
    let mut app = AppState {
        modal: pacsea::state::Modal::Help,
        ..AppState::default()
    };
    load_english(&mut app);
    app.keymap.config_editor_pi_scan_setup = vec![pacsea::theme::KeyChord {
        code: KeyCode::Char('g'),
        mods: KeyModifiers::CONTROL,
    }];

    let rendered = render_text(&mut app, 120, 60);

    assert!(rendered.contains("Pi Scan workspace"), "{rendered:?}");
    assert!(rendered.contains("Progress: p pause · u resume · x cancel · r retry"));
    assert!(rendered.contains("Wizard:"));
    assert!(rendered.contains("Ctrl+G"));
    assert!(!rendered.contains("Detach"));
    assert!(!rendered.contains("Reopen"));
}

/// Progress footer must advertise exactly the four production actions and no removed controls.
#[test]
fn progress_footer_advertises_exact_p_u_x_r_actions() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.set_view(PiScanView::Progress);

    let rendered = render_text(&mut app, 120, 24);

    assert!(rendered.contains("p Pause · u Resume · x Cancel · r Retry"));
    assert!(!rendered.contains("detach"));
    assert!(!rendered.contains("reopen"));
    assert!(app.updates_button_rect.is_none());
    assert!(app.config_button_rect.is_none());
    assert!(app.panels_button_rect.is_none());
    assert!(app.options_button_rect.is_none());
}

/// Opening Pi Scan without an AUR package must explain how to add a target.
#[test]
fn non_aur_entry_sets_actionable_notice() {
    let mut app = AppState::default();
    load_english(&mut app);
    app.pi_scan.settings.enabled = true;
    app.pi_scan.setup_facts_verified = true;
    app.pi_scan.disclosure_confirmed = true;
    app.pi_scan.runtime.consent.paid_execution = true;
    app.results.push(PackageItem {
        name: "core-package".to_string(),
        version: "1".to_string(),
        description: String::new(),
        source: Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
    });

    pacsea::events::pi_scan::open_from_search(&mut app);

    let notice = app.pi_scan.notices.foreground_text().expect("entry notice");
    assert!(notice.contains("analyzes AUR packages"));
    assert!(notice.contains("Shift+A"));
}

/// Package mode shows Pi Scan activity only when the feature is enabled.
#[test]
fn package_top_bar_appends_enabled_pi_scan_running_and_unseen_status() {
    let mut app = AppState::default();
    load_english(&mut app);
    app.pi_scan.settings.enabled = true;
    app.pi_scan.unseen_result_count = 2;

    let rendered = render_text(&mut app, 120, 24);
    assert!(rendered.contains("Pi Scan: 2 new results"));

    app.pi_scan.runtime.active = Some(pacsea::state::pi_scan::PiScanActiveItem {
        correlation_id: 7,
        request: scan_request(),
        started_at_unix: 1,
        cancellation_suppressed: false,
    });
    let rendered = render_text(&mut app, 120, 24);
    assert!(rendered.contains("Pi Scan: running"));

    app.pi_scan.settings.enabled = false;
    let rendered = render_text(&mut app, 120, 24);
    assert!(!rendered.contains("Pi Scan:"));
}

/// Details keys and wheel must reach long content without changing the selected result.
#[test]
fn long_details_scrolls_by_keys_and_wheel_while_preserving_selection() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan.results.push(display_result("first", 100));
    app.pi_scan.results.push(display_result("second", 100));
    app.pi_scan.selected_result = 1;
    app.pi_scan.set_view(PiScanView::Details);

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        &mut app,
    ));
    assert!(pacsea::events::pi_scan::handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        },
        &mut app,
    ));

    assert!(app.pi_scan.view_scroll.details >= 4);
    assert_eq!(app.pi_scan.selected_result, 1);

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        &mut app,
    ));
    load_english(&mut app);
    let _ = render_text(&mut app, 100, 20);
    assert!(app.pi_scan.view_scroll.details > 0);
    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        &mut app,
    ));
    assert_eq!(app.pi_scan.view_scroll.details, 0);
    assert_eq!(app.pi_scan.selected_result, 1);
}

/// Long target navigation must keep the selected row inside the rendered viewport.
#[test]
fn long_targets_navigation_keeps_selection_visible() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    app.pi_scan
        .targets
        .extend((0..40).map(|index| pacsea::state::PiScanTarget {
            package_name: format!("package-{index}"),
            package_base: format!("base-{index}"),
            commit_oid: Some(format!("{index:040}")),
            selected: true,
            status: pacsea::state::PiScanTargetStatus::Queued,
        }));
    app.pi_scan.set_view(PiScanView::Targets);
    for _ in 0..30 {
        pacsea::events::pi_scan::handle_key(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            &mut app,
        );
    }
    load_english(&mut app);

    let rendered = render_text(&mut app, 90, 16);

    assert!(rendered.contains("package-30"), "{rendered:?}");
    assert!(app.pi_scan.view_scroll.targets > 0);
}

/// Clicking the second rendered Results row selects it, and entering Results clears unseen state.
#[test]
fn second_results_row_click_selects_it_and_render_does_not_clear_unseen() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.results.push(display_result("first", 0));
    app.pi_scan.results.push(display_result("second", 0));
    app.pi_scan.view = PiScanView::Results;
    app.pi_scan.unseen_result_count = 3;
    let _ = render_text(&mut app, 100, 24);
    assert_eq!(app.pi_scan.unseen_result_count, 3);
    let second = app
        .pi_scan
        .result_row_rects
        .get(1)
        .copied()
        .expect("second row");

    assert!(pacsea::events::pi_scan::handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: second.x,
            row: second.y,
            modifiers: KeyModifiers::NONE,
        },
        &mut app,
    ));
    assert_eq!(app.pi_scan.selected_result, 1);

    app.pi_scan.set_view(PiScanView::Overview);
    app.pi_scan.unseen_result_count = 2;
    app.pi_scan.set_view(PiScanView::Results);
    assert_eq!(app.pi_scan.unseen_result_count, 0);
}

/// Active progress and Overview budget accounting must render truthful elapsed/reservation usage.
#[test]
fn active_progress_and_overview_render_elapsed_reservation_and_consumed_usage() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    let request = scan_request();
    app.pi_scan.runtime.active = Some(pacsea::state::pi_scan::PiScanActiveItem {
        correlation_id: 7,
        request: request.clone(),
        started_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs()
            .saturating_sub(65),
        cancellation_suppressed: false,
    });
    app.pi_scan
        .runtime
        .budget
        .records
        .push(pacsea::state::pi_scan::PiScanBudgetRecord {
            correlation_id: 7,
            started_at_unix: 1,
            class: pacsea::state::pi_scan::PiScanAccountingClass::Background,
            reserved: request.reservation,
            consumed_tokens: Some(1_234),
            consumed_cost_microusd: Some(50_000),
        });
    app.pi_scan.set_view(PiScanView::Progress);
    let progress = render_text(&mut app, 120, 24);
    assert!(progress.contains("01:05"), "{progress:?}");
    assert!(progress.contains("12,345 tokens"));
    assert!(progress.contains("$0.125 USD"));

    app.pi_scan.set_view(PiScanView::Overview);
    let overview = render_text(&mut app, 120, 24);
    assert!(overview.contains("tokens 1,234"), "{overview:?}");
    assert!(overview.contains("$0.05 USD"));
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
