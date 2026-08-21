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
    AppState, Focus, PackageItem, PiScanAvailability, PiScanDisplayResult, PiScanExecutionPhase,
    PiScanExecutionProgress, PiScanUiAction, PiScanView, PkgbuildCheckRequest, Source,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};
use tokio::sync::mpsc;

/// Load one shipped locale into application state for render assertions.
fn load_locale(app: &mut AppState, locale: &str) {
    let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
    app.translations = pacsea::i18n::load_locale_file(locale, &locales).expect("requested locale");
    app.translations_fallback =
        pacsea::i18n::load_locale_file("en-US", &locales).expect("English fallback locale");
}

/// Load the shipped English locale into an application state for render assertions.
fn load_english(app: &mut AppState) {
    load_locale(app, "en-US");
}

/// Render one full application frame and return its final terminal buffer.
fn render_buffer(app: &mut AppState, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| pacsea::ui::ui(frame, app))
        .expect("test render");
    terminal.backend().buffer().clone()
}

/// Render one full application frame and return its visible terminal text.
fn render_text(app: &mut AppState, width: u16, height: u16) -> String {
    render_buffer(app, width, height)
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>()
}

/// Locate the first terminal cell containing one rendered text fragment.
fn position_for_text(buffer: &Buffer, needle: &str) -> Option<(u16, u16)> {
    for y in buffer.area.y..buffer.area.bottom() {
        let mut line = String::new();
        let mut cell_starts = Vec::new();
        for x in buffer.area.x..buffer.area.right() {
            cell_starts.push((line.len(), x));
            line.push_str(buffer[(x, y)].symbol());
        }
        if let Some(byte_index) = line.find(needle) {
            let x = cell_starts
                .iter()
                .rev()
                .find_map(|(start, x)| (*start <= byte_index).then_some(*x))?;
            return Some((x, y));
        }
    }
    None
}

/// Return the foreground color of the first cell in one rendered text fragment.
fn foreground_for_text(buffer: &Buffer, needle: &str) -> Color {
    for y in buffer.area.y..buffer.area.bottom() {
        let mut line = String::new();
        let mut cell_starts = Vec::new();
        for x in buffer.area.x..buffer.area.right() {
            cell_starts.push((line.len(), x));
            line.push_str(buffer[(x, y)].symbol());
        }
        if let Some(byte_index) = line.find(needle) {
            let x = cell_starts
                .iter()
                .rev()
                .find_map(|(start, x)| (*start <= byte_index).then_some(*x))
                .expect("rendered cell position");
            return buffer[(x, y)].fg;
        }
    }
    panic!("rendered text fragment not found: {needle:?}");
}

/// Render and click one visible wizard label, scrolling until the label enters the viewport.
fn click_wizard_label(
    step: pacsea::state::PiScanSetupStep,
    width: u16,
    label: &str,
    expected_index: usize,
) -> (AppState, u16) {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.begin_setup_wizard(true);
    let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
    wizard.step = step;
    wizard.focus = (expected_index + 1) % wizard.focus_count();
    if step == pacsea::state::PiScanSetupStep::Route {
        wizard.candidate.provider = "provider-one".to_string();
        wizard.candidate.model = "model-one".to_string();
        wizard.verified = Some(pacsea::state::PiScanSetupVerifiedFacts {
            routes: vec![
                ("provider-one".to_string(), "model-one".to_string()),
                ("provider-two".to_string(), "model-two".to_string()),
            ],
            ..pacsea::state::PiScanSetupVerifiedFacts::default()
        });
    }
    for scroll in 0..=30 {
        app.pi_scan.wizard.as_mut().expect("wizard").body_scroll = scroll;
        let buffer = render_buffer(&mut app, width, 24);
        let Some((column, row)) = position_for_text(&buffer, label) else {
            continue;
        };
        assert!(pacsea::events::pi_scan::handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            },
            &mut app,
        ));
        assert_eq!(
            app.pi_scan.wizard.as_ref().expect("wizard").focus,
            expected_index,
            "clicking {label:?} at width {width} and scroll {scroll} selected the wrong control"
        );
        return (app, scroll);
    }
    panic!("wizard label {label:?} was not visible at width {width}");
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
            evidence: format!("evidence {name} line {index}"),
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

/// Verify Shift+A opens Pi Scan globally with the selected AUR context.
#[test]
fn shift_a_from_install_insert_mode_opens_contextual_pi_scan() {
    let mut app = AppState {
        focus: Focus::Install,
        search_normal_mode: false,
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

/// Raw details start collapsed and `t` changes only session state, not persisted settings.
#[test]
fn details_raw_output_starts_collapsed_and_toggle_is_session_only() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.settings.show_raw_output = true;
    app.pi_scan.results.push(display_result("raw-demo", 1));
    app.pi_scan.set_view(PiScanView::Details);
    assert!(app.pi_scan.settings.show_raw_output);

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut app,
    ));
    assert!(app.pi_scan.is_result_expanded(0));
    let hidden = render_text(&mut app, 120, 30);
    assert!(hidden.contains("Technical details"), "{hidden:?}");
    assert!(hidden.contains("hidden · t to show"), "{hidden:?}");
    assert!(!hidden.contains("\"scan_id\""), "{hidden:?}");

    assert!(pacsea::events::pi_scan::handle_key(
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        &mut app,
    ));

    assert!(app.pi_scan.show_raw_output);
    assert!(app.pi_scan.settings.show_raw_output);
    let shown = render_text(&mut app, 120, 40);
    assert!(shown.contains("visible · t to hide"), "{shown:?}");
    assert!(shown.contains("\"scan_id\""), "{shown:?}");
}

/// Details keep every package header visible while hiding collapsed package content.
#[test]
fn details_render_package_headers_and_collapsed_content() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.results.push(display_result("first", 1));
    app.pi_scan.results.push(display_result("second", 1));
    app.pi_scan.set_view(PiScanView::Details);

    let collapsed = render_text(&mut app, 120, 24);
    assert!(collapsed.contains("▸ first  [selected]"), "{collapsed:?}");
    assert!(collapsed.contains("▸ second"), "{collapsed:?}");
    assert!(
        !collapsed.contains("evidence first line 0"),
        "{collapsed:?}"
    );
    assert!(
        !collapsed.contains("evidence second line 0"),
        "{collapsed:?}"
    );

    assert!(app.pi_scan.toggle_result_expansion(1));
    let second_expanded = render_text(&mut app, 120, 24);
    assert!(second_expanded.contains("▸ first  [selected]"));
    assert!(second_expanded.contains("▾ second"));
    assert!(!second_expanded.contains("evidence first line 0"));
    assert!(second_expanded.contains("evidence second line 0"));
}

/// Expanded Details use readable sections and keep exact scanner messages hidden by default.
#[test]
fn single_details_section_is_package_labeled_and_readable() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    let mut result = display_result("single", 1);
    result.validated.coverage = Coverage::Incomplete;
    result.validated.limitations = vec![
        "Recipe and source snapshot tools were unavailable, so recipe files could not be inspected."
            .to_string(),
        "The recipe tree contains an incomplete archive entry: pax_global_header.".to_string(),
        "The source snapshot is empty and has documented incomplete archive coverage."
            .to_string(),
        "recipe tree is incomplete: archive special or unknown entry `pax_global_header`"
            .to_string(),
        "source `package.xml` is incomplete: package.xml is unsupported: malformed URL: relative URL without a base"
            .to_string(),
        "source `platform-37.0_r02.zip` is incomplete: at least one strong checksum matched; archive entry-count limit exceeded"
            .to_string(),
        "source package.xml is malformed due to a relative URL without a base.".to_string(),
        "source platform-37.0_r02.zip exceeded the archive entry-count limit.".to_string(),
    ];
    app.pi_scan.results.push(result);
    app.pi_scan.set_view(PiScanView::Details);
    assert!(app.pi_scan.toggle_result_expansion(0));

    let rendered = render_text(&mut app, 160, 45);
    for expected in [
        "▾ single  [selected]",
        "Review summary",
        "1 finding needs review",
        "Some files could not be checked",
        "What could not be checked (5)",
        "Some package files could not be inspected",
        "The package archive contains an unsupported entry: `pax_global_header`.",
        "No source files were available to inspect.",
        "`package.xml` could not be inspected because its download address is incomplete.",
        "`platform-37.0_r02.zip` was only partially checked",
        "Findings (1)",
        "Location: recipe / path/0",
        "Evidence: evidence single line 0",
        "Technical details",
        "hidden · t to show",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}: {rendered:?}"
        );
    }
    assert!(
        !rendered.contains("relative URL without a base"),
        "{rendered:?}"
    );
    assert!(
        !rendered.contains("archive entry-count limit exceeded"),
        "{rendered:?}"
    );
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
            "app.pi_scan.setup.validation_issue.executable",
            "app.pi_scan.setup.validation_issue.at_least",
            "app.pi_scan.setup.validation_issue.between",
            "app.pi_scan.setup.validation_issue.cannot_exceed",
            "app.pi_scan.setup.validation_issue.nonnegative_decimal",
            "app.pi_scan.setup.validation_issue.https_proxy",
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
            "app.pi_scan.progress.current_step",
            "app.pi_scan.progress.working",
            "app.pi_scan.progress.phase.preparing",
            "app.pi_scan.progress.phase.resolving_metadata",
            "app.pi_scan.progress.phase.waiting_to_retry",
            "app.pi_scan.progress.phase.acquiring_sources",
            "app.pi_scan.progress.phase.running_model",
            "app.pi_scan.progress.phase.rechecking_identity",
            "app.pi_scan.progress.phase.validating_result",
            "app.pi_scan.progress.phase.finalizing",
            "app.pi_scan.results.completion.complete_no_findings",
            "app.pi_scan.results.completion.incomplete_no_findings",
            "app.pi_scan.results.completion.one_finding",
            "app.pi_scan.results.completion.many_findings",
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
    assert!(app.pi_scan.toggle_result_expansion(0));
    assert!(app.pi_scan.toggle_result_expansion(1));

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
    app.pi_scan.active_progress = Some(PiScanExecutionProgress {
        correlation_id: 7,
        phase: PiScanExecutionPhase::RunningModel,
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
    assert!(progress.contains("Current step"), "{progress:?}");
    assert!(
        progress.contains("Pi is analyzing the package and validating its response"),
        "{progress:?}"
    );
    assert!(progress.contains("12,345 tokens"));
    assert!(progress.contains("$0.125 USD"));

    app.pi_scan.set_view(PiScanView::Overview);
    let overview = render_text(&mut app, 120, 24);
    assert!(
        overview.contains("Tokens used / limit: 1,234"),
        "{overview:?}"
    );
    assert!(overview.contains("$0.05 USD"));
}

/// Queued paused work must use a static marker and explain every sticky pause reason.
#[test]
fn queued_progress_renders_static_pause_reasons_and_position() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.runtime.queue.push_back(scan_request());
    app.pi_scan
        .runtime
        .pause_reasons
        .insert(pacsea::state::pi_scan::PiScanPauseReason::User);
    app.pi_scan
        .runtime
        .pause_reasons
        .insert(pacsea::state::pi_scan::PiScanPauseReason::Budget);
    app.pi_scan.set_view(PiScanView::Progress);

    let progress = render_text(&mut app, 120, 24);

    assert!(
        progress.contains("Waiting for the next scan to start"),
        "{progress:?}"
    );
    assert!(progress.contains("Paused: user pause, budget pause"));
    assert!(progress.contains("1 queued"));
    assert!(progress.contains("1. demo — foreground · commit: aaaaaaaaaaaa"));
    assert!(progress.contains('⏸'), "{progress:?}");
    for frame in ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"] {
        assert!(
            !progress.contains(frame),
            "paused UI contained {frame}: {progress:?}"
        );
    }
}

/// Mixed terminal outcomes must render summary counts and remain safe at narrow widths.
#[test]
fn progress_summary_renders_mixed_counts_and_narrow_active_layout() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    let request = scan_request();
    app.pi_scan.runtime.active = Some(pacsea::state::pi_scan::PiScanActiveItem {
        correlation_id: 9,
        request: request.clone(),
        started_at_unix: 1,
        cancellation_suppressed: false,
    });
    app.pi_scan.runtime.queue.push_back(request.clone());
    for (correlation_id, status) in [
        (7, pacsea::state::pi_scan::PiScanTerminalStatus::Completed),
        (8, pacsea::state::pi_scan::PiScanTerminalStatus::Failed),
    ] {
        app.pi_scan
            .runtime
            .terminal
            .push(pacsea::state::pi_scan::PiScanTerminalRecord {
                request: request.clone(),
                correlation_id,
                status,
                finished_at_unix: 2,
            });
    }
    app.pi_scan.set_view(PiScanView::Progress);

    let progress = render_text(&mut app, 120, 24);
    for expected in ["2/4", "1 running", "1 queued", "1 completed", "1 failed"] {
        assert!(
            progress.contains(expected),
            "missing {expected}: {progress:?}"
        );
    }

    app.pi_scan.view_scroll.progress = u16::MAX;
    let narrow = render_text(&mut app, 20, 10);
    assert!(!narrow.trim().is_empty());
    assert!(app.pi_scan.view_scroll.progress < u16::MAX);
}

/// Targets, Progress, and Results share hierarchy, short identities, and semantic styles.
#[test]
fn readability_tabs_render_hierarchy_short_identities_and_semantic_styles() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    let exact_target_oid = "0123456789abcdef0123456789abcdef01234567";
    for (name, status) in [
        (
            "selected-package",
            pacsea::state::PiScanTargetStatus::Queued,
        ),
        (
            "completed-package",
            pacsea::state::PiScanTargetStatus::Completed,
        ),
        ("failed-package", pacsea::state::PiScanTargetStatus::Failed),
    ] {
        app.pi_scan.targets.push(pacsea::state::PiScanTarget {
            package_name: name.to_string(),
            package_base: name.trim_end_matches("-package").to_string(),
            commit_oid: Some(exact_target_oid.to_string()),
            selected: true,
            status,
        });
    }
    app.pi_scan.set_view(PiScanView::Targets);
    let target_buffer = render_buffer(&mut app, 160, 30);
    let target_text = target_buffer
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    let th = pacsea::theme::theme();
    assert!(target_text.contains("Scan targets"), "{target_text:?}");
    assert!(target_text.contains("commit: 0123456789ab"));
    assert!(!target_text.contains(exact_target_oid));
    assert_eq!(
        foreground_for_text(&target_buffer, "selected-package"),
        th.sapphire
    );
    assert_eq!(
        foreground_for_text(&target_buffer, "queued · base"),
        th.yellow
    );
    assert_eq!(
        foreground_for_text(&target_buffer, "completed · base"),
        th.green
    );
    assert_eq!(foreground_for_text(&target_buffer, "failed · base"), th.red);

    let request = scan_request();
    app.pi_scan.runtime.active = Some(pacsea::state::pi_scan::PiScanActiveItem {
        correlation_id: 7,
        request: request.clone(),
        started_at_unix: 1,
        cancellation_suppressed: false,
    });
    app.pi_scan.runtime.queue.push_back(request);
    app.pi_scan.set_view(PiScanView::Progress);
    let progress = render_text(&mut app, 160, 30);
    for heading in ["Session", "Current work", "Queue"] {
        assert!(
            progress.contains(heading),
            "missing {heading:?}: {progress:?}"
        );
    }
    assert!(progress.contains("commit: aaaaaaaaaaaa"), "{progress:?}");
    assert!(!progress.contains("aaaaaaaaaaaaa"), "{progress:?}");

    let mut current = display_result("current-result", 0);
    current.validated.identity.commit_oid = "abcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string();
    current.observed_head_oid = current.validated.identity.commit_oid.clone();
    let mut stale = display_result("stale-result", 1);
    stale.validated.coverage = Coverage::Incomplete;
    stale.validated.findings[0].severity = Severity::High;
    stale.stale = true;
    app.pi_scan.results.extend([current, stale]);
    app.pi_scan.set_view(PiScanView::Results);
    let result_buffer = render_buffer(&mut app, 160, 30);
    let result_text = result_buffer
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(result_text.contains("Validated results"), "{result_text:?}");
    assert!(
        result_text.contains("commit: abcdefabcdef"),
        "{result_text:?}"
    );
    assert!(
        result_text.contains("[CURRENT IDENTITY]"),
        "{result_text:?}"
    );
    assert!(result_text.contains("[STALE IDENTITY]"), "{result_text:?}");
    assert!(result_text.contains("[high]"), "{result_text:?}");
    assert_eq!(
        foreground_for_text(&result_buffer, "Complete — no findings"),
        th.green
    );
    assert_eq!(foreground_for_text(&result_buffer, "incomplete"), th.yellow);
    assert_eq!(
        foreground_for_text(&result_buffer, "[STALE IDENTITY]"),
        th.red
    );
    assert_eq!(foreground_for_text(&result_buffer, "high"), th.red);
}

/// Wrapped wizard labels activate only their own page-local controls at common widths.
#[test]
fn wrapped_wizard_labels_activate_their_own_controls() {
    let mut narrow_pricing_scroll = 0;
    for width in [80, 48] {
        let (readiness_binary, _) = click_wizard_label(
            pacsea::state::PiScanSetupStep::PiReadiness,
            width,
            "Pi executable: pi_",
            0,
        );
        assert!(
            readiness_binary
                .pi_scan
                .wizard
                .as_ref()
                .expect("wizard")
                .pending_action
                .is_none()
        );
        let (readiness_verify, _) = click_wizard_label(
            pacsea::state::PiScanSetupStep::PiReadiness,
            width,
            "[Enter] Verify without",
            1,
        );
        assert!(matches!(
            readiness_verify
                .pi_scan
                .wizard
                .as_ref()
                .expect("wizard")
                .pending_action,
            Some(pacsea::state::pi_scan_setup::PiScanSetupDraftAction::Probe { .. })
        ));

        let (route_primary, _) = click_wizard_label(
            pacsea::state::PiScanSetupStep::Route,
            width,
            "← Primary route:",
            0,
        );
        let route_wizard = route_primary.pi_scan.wizard.as_ref().expect("wizard");
        assert_eq!(route_wizard.candidate.provider, "provider-two");
        assert_eq!(route_wizard.candidate.thinking, "medium");
        let (route_thinking, _) = click_wizard_label(
            pacsea::state::PiScanSetupStep::Route,
            width,
            "← Thinking:",
            1,
        );
        let thinking_wizard = route_thinking.pi_scan.wizard.as_ref().expect("wizard");
        assert_eq!(thinking_wizard.candidate.provider, "provider-one");
        assert_eq!(thinking_wizard.candidate.thinking, "high");

        for (label, expected_index) in [
            ("[Space] I reviewed privacy", 0),
            ("[Space] I separately allow", 1),
            ("[Space] I separately accept", 2),
        ] {
            let (pricing, scroll) = click_wizard_label(
                pacsea::state::PiScanSetupStep::PricingPrivacy,
                width,
                label,
                expected_index,
            );
            if width == 48 {
                narrow_pricing_scroll = narrow_pricing_scroll.max(scroll);
            }
            let confirmations = &pricing
                .pi_scan
                .wizard
                .as_ref()
                .expect("wizard")
                .confirmations;
            assert_eq!(
                [
                    confirmations.disclosure_confirmed,
                    confirmations.foreground_paid_confirmed,
                    confirmations.readiness_warning_confirmed,
                ],
                std::array::from_fn(|index| index == expected_index),
                "clicking {label:?} changed the wrong confirmation"
            );
        }
    }
    assert!(
        narrow_pricing_scroll > 0,
        "narrow Pricing regression did not exercise body scrolling"
    );
}

/// German Results rows localize zero, one, and multiple-finding completion wording.
#[test]
fn german_results_localize_completion_wording_by_finding_count() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_locale(&mut app, "de-DE");
    app.pi_scan.results.extend([
        display_result("keine-funde", 0),
        display_result("ein-fund", 1),
        display_result("mehrere-funde", 2),
    ]);
    app.pi_scan.set_view(PiScanView::Results);

    let rendered = render_text(&mut app, 180, 30);

    for expected in [
        "Vollständig — keine Funde im analysierten Umfang",
        "1 Fund im analysierten Umfang",
        "2 Funde im analysierten Umfang",
    ] {
        assert!(
            rendered.contains(expected),
            "missing localized completion {expected:?}: {rendered:?}"
        );
    }
    assert!(!rendered.contains("finding(s) in analyzed scope"));
}

/// WS2 list pages remain renderable and keep bounded scroll state at 20x10.
#[test]
fn readability_tabs_render_at_twenty_by_ten() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan.targets.push(pacsea::state::PiScanTarget {
        package_name: "narrow-target".to_string(),
        package_base: "narrow".to_string(),
        commit_oid: Some("a".repeat(40)),
        selected: true,
        status: pacsea::state::PiScanTargetStatus::Paused,
    });
    app.pi_scan.runtime.queue.push_back(scan_request());
    app.pi_scan.results.push(display_result("narrow-result", 1));
    app.pi_scan.view_scroll.progress = u16::MAX;

    for view in [
        PiScanView::Targets,
        PiScanView::Progress,
        PiScanView::Results,
    ] {
        app.pi_scan.set_view(view);
        let buffer = render_buffer(&mut app, 20, 10);
        assert_eq!(buffer.area.width, 20);
        assert_eq!(buffer.area.height, 10);
        assert!(buffer.content.iter().any(|cell| cell.symbol() != " "));
        match view {
            PiScanView::Targets => {
                let rect = app.pi_scan.target_row_rects[0];
                assert_eq!(buffer[(rect.x, rect.y)].symbol(), "[");
            }
            PiScanView::Results => {
                let rect = app.pi_scan.result_row_rects[0];
                let row = (rect.x..rect.x.saturating_add(rect.width))
                    .map(|x| buffer[(x, rect.y)].symbol())
                    .collect::<String>();
                assert!(row.contains("narrow-result"), "{row:?}");
            }
            _ => {}
        }
    }
    assert!(app.pi_scan.view_scroll.progress < u16::MAX);
}

/// Target and result hit rectangles stay on exact one-row visual seams.
#[test]
fn target_and_result_hit_rectangles_match_visual_rows() {
    let mut app = AppState {
        app_mode: AppMode::PiScan,
        ..AppState::default()
    };
    load_english(&mut app);
    app.pi_scan
        .targets
        .extend((0..3).map(|index| pacsea::state::PiScanTarget {
            package_name: format!("target-{index}"),
            package_base: format!("base-{index}"),
            commit_oid: Some(format!("{index:040}")),
            selected: index != 1,
            status: pacsea::state::PiScanTargetStatus::Queued,
        }));
    app.pi_scan.set_view(PiScanView::Targets);
    let target_buffer = render_buffer(&mut app, 100, 24);
    let target_rects = app.pi_scan.target_row_rects.clone();
    assert_eq!(target_rects.len(), 3);
    for (index, rect) in target_rects.iter().copied().enumerate() {
        assert_eq!(rect.index, index);
        assert_eq!(rect.height, 1);
        assert_eq!(target_buffer[(rect.x, rect.y)].symbol(), "[");
        assert_eq!(app.pi_scan.target_hit_test(rect.x, rect.y), Some(index));
        assert_eq!(
            app.pi_scan
                .target_hit_test(rect.x + rect.width.saturating_sub(1), rect.y),
            Some(index)
        );
        assert_eq!(
            app.pi_scan.target_hit_test(rect.x + rect.width, rect.y),
            None
        );
        if index > 0 {
            assert_eq!(rect.y, target_rects[index - 1].y + 1);
        }
    }

    app.pi_scan.results.extend([
        display_result("first-result", 0),
        display_result("second-result", 1),
        display_result("third-result", 0),
    ]);
    app.pi_scan.set_view(PiScanView::Results);
    let result_buffer = render_buffer(&mut app, 100, 24);
    let result_rects = app.pi_scan.result_row_rects.clone();
    assert_eq!(result_rects.len(), 3);
    for (index, rect) in result_rects.iter().copied().enumerate() {
        assert_eq!(rect.index, index);
        assert_eq!(rect.height, 1);
        let row_text = (rect.x..rect.x.saturating_add(rect.width))
            .map(|x| result_buffer[(x, rect.y)].symbol())
            .collect::<String>();
        assert!(row_text.contains(&format!("{}-result", ["first", "second", "third"][index])));
        assert_eq!(app.pi_scan.result_hit_test(rect.x, rect.y), Some(index));
        assert_eq!(
            app.pi_scan
                .result_hit_test(rect.x + rect.width.saturating_sub(1), rect.y),
            Some(index)
        );
        assert_eq!(
            app.pi_scan.result_hit_test(rect.x + rect.width, rect.y),
            None
        );
        if index > 0 {
            assert_eq!(rect.y, result_rects[index - 1].y + 1);
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
