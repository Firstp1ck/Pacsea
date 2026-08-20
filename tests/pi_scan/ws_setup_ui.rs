//! WS1 guided Pi Scan setup-wizard state, key, render, and mouse contracts.
//!
//! This owned test module requires registration in both shared Pi Scan test
//! harnesses by the integration owner.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pacsea::state::pi_scan::PiScanConsentState;
use pacsea::state::pi_scan_setup::{
    PiScanSetupDraftAction, PiScanSetupHitRect, PiScanSetupHitTarget, PiScanSetupStep,
    PiScanSetupVerifiedFacts,
};
use pacsea::state::types::AppMode;
use pacsea::state::{AppState, PiScanSetupWizardState};
use ratatui::{Terminal, backend::TestBackend};

/// Load the shipped English locale for user-facing wizard render assertions.
fn load_english(app: &mut AppState) {
    let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
    let translations = pacsea::i18n::load_locale_file("en-US", &locales).expect("English locale");
    app.translations.clone_from(&translations);
    app.translations_fallback = translations;
}

/// Build deterministic exact advertised route and pricing facts.
fn verified_facts() -> PiScanSetupVerifiedFacts {
    PiScanSetupVerifiedFacts {
        pi_version: "0.84.0".to_string(),
        routes: vec![("provider".to_string(), "model".to_string())],
        route_reservations: vec![(
            "provider".to_string(),
            "model".to_string(),
            pacsea::state::pi_scan::PiScanReservation {
                tokens: 10_000,
                cost_microusd: 125,
            },
        )],
        reservation: pacsea::state::pi_scan::PiScanReservation {
            tokens: 10_000,
            cost_microusd: 125,
        },
        pricing_binding: "pricing-binding".to_string(),
        pricing_observed_at_unix_seconds: 1_000,
        maximum_pricing_age_seconds: 900,
        pricing_summary: vec!["provider/model · exact Pi metadata".to_string()],
    }
}

/// Cancel must drop only the draft and leave effective state/action untouched.
#[test]
fn escape_cancel_is_side_effect_free() {
    let mut app = AppState::default();
    let original_settings = app.pi_scan.settings.clone();
    let original_consent = app.pi_scan.runtime.consent;
    app.pi_scan.begin_setup_wizard(true);
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.candidate.enabled = true;
    wizard.candidate.provider = "draft-only".to_string();
    wizard.candidate_consent.background_observation = true;

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.wizard.is_none());
    assert_eq!(app.pi_scan.settings, original_settings);
    assert_eq!(app.pi_scan.runtime.consent, original_consent);
    assert!(app.pi_scan.pending_action.is_none());
}

/// Exact route, confirmations, conservative options, validation, and Apply stay ordered.
#[test]
fn keyboard_flow_reaches_apply_with_independent_defaults() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(true);
    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &mut app,
    ));
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.request_probe(false);
    let probe = wizard.last_correlation;
    assert!(wizard.accept_verified_facts(probe, verified_facts()));

    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &mut app,
    );
    assert_eq!(
        app.pi_scan.wizard.as_ref().expect("wizard").step,
        PiScanSetupStep::Route
    );
    // Route focus Enter cycles the sole exact route; `n` advances.
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &mut app,
    );
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
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
    wizard.request_apply(false);
    assert!(matches!(
        wizard.pending_action,
        Some(PiScanSetupDraftAction::Apply { .. })
    ));
}

/// Enter activates only focused controls while `n` alone advances wizard pages.
#[test]
fn enter_activates_focused_control_and_n_alone_advances() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(true);

    assert!(!pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut app,
    ));
    assert_eq!(
        app.pi_scan.wizard.as_ref().expect("wizard").step,
        PiScanSetupStep::Welcome
    );
    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &mut app,
    ));
    assert_eq!(
        app.pi_scan.wizard.as_ref().expect("wizard").step,
        PiScanSetupStep::PiReadiness
    );

    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = PiScanSetupStep::PricingPrivacy;
    wizard.focus = 0;
    assert!(!wizard.confirmations.disclosure_confirmed);
    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut app,
    ));
    assert!(
        app.pi_scan
            .wizard
            .as_ref()
            .expect("wizard")
            .confirmations
            .disclosure_confirmed
    );
}

/// `BackTab` must reverse wizard focus instead of mutating hidden Package sorting.
#[test]
fn wizard_backtab_reverses_focus() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(true);
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = PiScanSetupStep::PricingPrivacy;
    wizard.focus = 0;

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
        &mut app,
    ));

    assert_eq!(app.pi_scan.wizard.as_ref().expect("wizard").focus, 2);
}

/// Apply abandonment must use first-Escape warning and second-Escape ownership retention.
#[test]
fn apply_abandonment_requires_two_escape_presses() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(false);
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = PiScanSetupStep::Review;
    wizard.validation_binding = "binding".to_string();

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        &mut app,
    ));
    let correlation = app
        .pi_scan
        .wizard
        .as_ref()
        .expect("wizard")
        .in_flight_correlation
        .expect("apply correlation");
    assert!(app.pi_scan.setup_transaction_matches(correlation));

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.wizard.is_some());
    assert_eq!(
        app.pi_scan
            .setup_transaction
            .expect("transaction")
            .abandonment,
        pacsea::state::pi_scan_ui::PiScanSetupAbandonment::Warned
    );

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.wizard.is_none());
    assert_eq!(
        app.pi_scan
            .setup_transaction
            .expect("transaction")
            .abandonment,
        pacsea::state::pi_scan_ui::PiScanSetupAbandonment::AbandonRequested
    );
}

/// A first-run cancel must explain how to restart guided setup and how to leave.
#[test]
fn first_run_cancel_sets_restart_and_leave_guidance() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(true);

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &mut app,
    ));

    let notice = app
        .pi_scan
        .notices
        .foreground
        .as_ref()
        .expect("cancel guidance");
    assert!(notice.text.contains("press r"));
    assert!(notice.text.contains("Esc to leave"));
}

/// Wizard-only `q` is unbound; Escape is the sole cancel key.
#[test]
fn q_does_not_cancel_the_wizard() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(true);

    assert!(!pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.wizard.is_some());
}

/// Dry-run keyboard actions must not queue a Pi probe or Apply.
#[test]
fn dry_run_keys_queue_no_probe_or_apply() {
    let mut app = AppState {
        dry_run: true,
        ..AppState::default()
    };
    app.pi_scan.begin_setup_wizard(true);
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = PiScanSetupStep::PiReadiness;
    wizard.focus = 1;
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut app,
    );
    assert!(
        app.pi_scan
            .wizard
            .as_ref()
            .expect("wizard")
            .pending_action
            .is_none()
    );

    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = PiScanSetupStep::Review;
    wizard.validation_binding = "binding".to_string();
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        &mut app,
    );
    assert!(
        app.pi_scan
            .wizard
            .as_ref()
            .expect("wizard")
            .pending_action
            .is_none()
    );
}

/// Each wizard step must render at narrow dimensions and retain progress text.
#[test]
fn seven_steps_render_narrow_without_panic() {
    for step in PiScanSetupStep::all() {
        let backend = TestBackend::new(36, 12);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        app.pi_scan.wizard = Some(PiScanSetupWizardState::open(
            app.pi_scan.settings.clone(),
            PiScanConsentState::default(),
            true,
        ));
        app.pi_scan.wizard.as_mut().expect("wizard").step = step;
        terminal
            .draw(|frame| pacsea::ui::ui(frame, &mut app))
            .expect("narrow render");
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(text.contains('◆'));
    }
}

/// Pricing must summarize only the selected route with human-readable units.
#[test]
fn pricing_page_omits_catalog_dump_and_formats_the_reservation() {
    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    let mut facts = verified_facts();
    facts
        .routes
        .push(("other-provider".to_string(), "other-model".to_string()));
    facts.route_reservations.push((
        "other-provider".to_string(),
        "other-model".to_string(),
        pacsea::state::pi_scan::PiScanReservation {
            tokens: 10_000,
            cost_microusd: 999_999,
        },
    ));
    facts.pricing_summary = vec![
        "provider/model · input=100 output=250 micro-USD/million · Metered · pi-rpc:model-cost"
            .to_string(),
        "other-provider/other-model · input=999999 output=999999 micro-USD/million · Metered · pi-rpc:model-cost"
            .to_string(),
    ];
    let mut wizard = PiScanSetupWizardState::open(
        app.pi_scan.settings.clone(),
        PiScanConsentState::default(),
        true,
    );
    wizard.step = PiScanSetupStep::PricingPrivacy;
    wizard.candidate.provider = "provider".to_string();
    wizard.candidate.model = "model".to_string();
    wizard.verified = Some(facts);
    app.pi_scan.wizard = Some(wizard);

    terminal
        .draw(|frame| pacsea::ui::ui(frame, &mut app))
        .expect("pricing render");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();

    assert!(rendered.contains("provider/model"));
    assert!(
        rendered.contains("10,000 "),
        "pricing page omitted readable token count: {rendered:?}"
    );
    assert!(rendered.contains("$0.000125 USD"));
    assert!(!rendered.contains("other-provider/other-model"));
    assert!(!rendered.contains("micro-USD"));
}

/// In-flight probe and retryable Apply failure state must be visible in the rendered wizard.
#[test]
fn wizard_renders_in_flight_and_retryable_failure_guidance() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.begin_setup_wizard(false);
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = PiScanSetupStep::PiReadiness;
    wizard.request_probe(false);
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| pacsea::ui::ui(frame, &mut app))
        .expect("in-flight render");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("working #1"), "{rendered:?}");

    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = PiScanSetupStep::Activate;
    wizard.in_flight_correlation = None;
    wizard.apply_status = pacsea::state::pi_scan_setup::PiScanSetupApplyStatus::Failed(
        "app.pi_scan.wizard.failure_timeout.activation".to_string(),
    );
    terminal
        .draw(|frame| pacsea::ui::ui(frame, &mut app))
        .expect("failure render");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("Runtime activation timed out"));
    assert!(rendered.contains("Correct the reported issue"));
}

/// Mouse hit targets must use the same Next transition as keyboard input.
#[test]
fn mouse_next_uses_shared_transition() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(true);
    app.pi_scan
        .wizard
        .as_mut()
        .expect("wizard")
        .set_hit_rects(vec![PiScanSetupHitRect {
            target: PiScanSetupHitTarget::Next,
            x: 10,
            y: 5,
            width: 6,
            height: 1,
        }]);
    assert!(pacsea::events::pi_scan::handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 5,
            modifiers: KeyModifiers::NONE,
        },
        &mut app,
    ));
    assert_eq!(
        app.pi_scan.wizard.as_ref().expect("wizard").step,
        PiScanSetupStep::PiReadiness
    );
}

/// The advanced page must expose an explicit guided-setup rerun action.
#[test]
fn advanced_page_rerun_key_opens_wizard() {
    let mut app = AppState::default();
    app.pi_scan.view = pacsea::state::PiScanView::Setup;

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.wizard.is_some());
}

/// Material settings edits must revoke prior setup facts and every durable consent projection.
#[test]
fn material_settings_edit_invalidates_setup_consent() {
    let mut app = AppState::default();
    app.pi_scan.setup_facts_verified = true;
    app.pi_scan.disclosure_confirmed = true;
    app.pi_scan.fallback_confirmed = true;
    app.pi_scan.background_paid_execution_confirmed = true;
    app.pi_scan.readiness_warning_confirmed = true;
    app.pi_scan.runtime.consent = PiScanConsentState {
        background_observation: true,
        paid_execution: true,
    };
    let mut changed = app.pi_scan.settings.clone();
    changed.provider = "changed-provider".to_string();

    app.pi_scan.apply_settings(changed, true);

    assert!(!app.pi_scan.setup_facts_verified);
    assert!(!app.pi_scan.disclosure_confirmed);
    assert!(!app.pi_scan.fallback_confirmed);
    assert!(!app.pi_scan.background_paid_execution_confirmed);
    assert!(!app.pi_scan.readiness_warning_confirmed);
    assert_eq!(app.pi_scan.runtime.consent, PiScanConsentState::default());
}

/// PageUp/PageDown must expose long review and error content on narrow terminals.
#[test]
fn wizard_body_scroll_is_keyboard_reachable_and_bounded() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(false);

    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        &mut app,
    );
    assert_eq!(app.pi_scan.wizard.as_ref().expect("wizard").body_scroll, 3);
    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        &mut app,
    );
    assert_eq!(app.pi_scan.wizard.as_ref().expect("wizard").body_scroll, 0);
}

/// Mouse wheel must scroll the wizard body through the same bounded state as PageDown/PageUp.
#[test]
fn wizard_mouse_wheel_scrolls_body() {
    let mut app = AppState::default();
    app.pi_scan.begin_setup_wizard(false);

    assert!(pacsea::events::pi_scan::handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 8,
            modifiers: KeyModifiers::NONE,
        },
        &mut app,
    ));
    assert_eq!(app.pi_scan.wizard.as_ref().expect("wizard").body_scroll, 3);

    assert!(pacsea::events::pi_scan::handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 8,
            modifiers: KeyModifiers::NONE,
        },
        &mut app,
    ));
    assert_eq!(app.pi_scan.wizard.as_ref().expect("wizard").body_scroll, 0);
}

/// Reopened wizard sessions must continue the process-wide setup correlation sequence.
#[test]
fn rerun_wizard_correlations_remain_monotonic() {
    let mut app = AppState::default();
    app.pi_scan.last_setup_correlation = 7;
    app.pi_scan.begin_setup_wizard(false);

    app.pi_scan
        .wizard
        .as_mut()
        .expect("wizard")
        .request_probe(false);

    assert_eq!(
        app.pi_scan
            .wizard
            .as_ref()
            .expect("wizard")
            .last_correlation,
        8
    );
}

/// Advanced setup keys must preserve every foreground/background payment combination.
#[test]
fn advanced_setup_payment_keys_are_independent() {
    let mut app = AppState::default();
    app.pi_scan.view = pacsea::state::PiScanView::Setup;
    app.pi_scan.setup_facts_verified = true;

    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        &mut app,
    );
    assert!(app.pi_scan.runtime.consent.paid_execution);
    assert!(!app.pi_scan.background_paid_execution_confirmed);

    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        &mut app,
    );
    assert!(app.pi_scan.runtime.consent.paid_execution);
    assert!(app.pi_scan.background_paid_execution_confirmed);

    pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        &mut app,
    );
    assert!(!app.pi_scan.runtime.consent.paid_execution);
    assert!(app.pi_scan.background_paid_execution_confirmed);
}

/// Foreground payment and paid background execution must remain separate wizard decisions.
#[test]
fn wizard_preserves_independent_foreground_and_background_payment() {
    let mut app = AppState::default();
    app.pi_scan.runtime.consent.paid_execution = true;
    app.pi_scan.background_paid_execution_confirmed = false;

    app.pi_scan.begin_setup_wizard(false);

    let wizard = app.pi_scan.wizard.as_ref().expect("wizard");
    assert!(!wizard.candidate_consent.paid_execution);
    assert!(!wizard.confirmations.foreground_paid_confirmed);
}
