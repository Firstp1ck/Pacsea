//! Keyboard state transitions for the Pi Scan workspace.

use crate::state::types::AppMode;
use crate::state::{
    AppState, PiScanAvailability, PiScanDryRunPreview, PiScanReadiness, PiScanUiAction, PiScanView,
    Source,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Open Pi Scan with context from the selected Search result.
pub fn open_from_search(app: &mut AppState) {
    let context = app
        .results
        .get(app.selected)
        .map(|item| (item.name.clone(), matches!(item.source, Source::Aur)));
    let (name, is_aur) = context.as_ref().map_or((None, false), |(name, is_aur)| {
        (Some(name.as_str()), *is_aur)
    });
    app.pi_scan.open_context(name, is_aur);
    app.app_mode = AppMode::PiScan;
}

/// Handle one pressed key while the Pi Scan workspace is active.
pub fn handle_key(key: KeyEvent, app: &mut AppState) -> bool {
    if key.code == KeyCode::Esc {
        app.app_mode = AppMode::Package;
        return true;
    }
    if handle_page_key(key, app) || handle_navigation(key, app) {
        return true;
    }
    match app.pi_scan.view {
        PiScanView::Setup => handle_setup(key, app),
        PiScanView::Targets => handle_targets(key, app),
        PiScanView::Progress => handle_progress(key, app),
        PiScanView::Results => handle_results(key, app),
        PiScanView::Details => handle_details(key, app),
        PiScanView::Overview => false,
    }
}

/// Select pages by number or cycle them with Tab/BackTab.
fn handle_page_key(key: KeyEvent, app: &mut AppState) -> bool {
    let current = app.pi_scan.view.index();
    let next = match key.code {
        KeyCode::Char('1'..='6') if key.modifiers.is_empty() => usize::from(match key.code {
            KeyCode::Char(ch) => ch as u8 - b'1',
            _ => 0,
        }),
        KeyCode::Tab => (current + 1) % 6,
        KeyCode::BackTab => current.checked_sub(1).unwrap_or(5),
        _ => return false,
    };
    app.pi_scan.view = PiScanView::all()[next];
    app.pi_scan.selected = 0;
    true
}

/// Move target/result selection and details scroll.
fn handle_navigation(key: KeyEvent, app: &mut AppState) -> bool {
    let delta = match key.code {
        KeyCode::Up | KeyCode::Char('k') => -1isize,
        KeyCode::Down | KeyCode::Char('j') => 1,
        _ => return false,
    };
    let len = match app.pi_scan.view {
        PiScanView::Targets => app.pi_scan.targets.len(),
        PiScanView::Results | PiScanView::Details => app.pi_scan.results.len(),
        _ => 0,
    };
    if len > 0 {
        app.pi_scan.selected = app
            .pi_scan
            .selected
            .saturating_add_signed(delta)
            .min(len - 1);
    }
    true
}

/// Apply disclosure and independent consent toggles.
fn handle_setup(key: KeyEvent, app: &mut AppState) -> bool {
    if matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('v'), KeyModifiers::NONE)
    ) {
        app.pi_scan.pending_action = Some(PiScanUiAction::ProbeSetup);
        app.pi_scan.notice = Some(
            "Verifying exact Pi version, route pricing, and provenance before consent…".to_string(),
        );
        return true;
    }
    let is_consent_key = matches!(
        (key.code, key.modifiers),
        (
            KeyCode::Char('c' | 'o' | 'p' | 'f' | 'w'),
            KeyModifiers::NONE
        )
    );
    if is_consent_key && !app.pi_scan.setup_facts_verified {
        app.pi_scan.pending_action = Some(PiScanUiAction::ProbeSetup);
        app.pi_scan.notice = Some(
            "Review the verified Pi version and exact pricing facts, then press the consent key again"
                .to_string(),
        );
        return true;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            app.pi_scan.disclosure_confirmed = !app.pi_scan.disclosure_confirmed;
        }
        (KeyCode::Char('o'), KeyModifiers::NONE) => {
            app.pi_scan.runtime.consent.background_observation =
                !app.pi_scan.runtime.consent.background_observation;
        }
        (KeyCode::Char('p'), KeyModifiers::NONE) => {
            app.pi_scan.runtime.consent.paid_execution =
                !app.pi_scan.runtime.consent.paid_execution;
        }
        (KeyCode::Char('f'), KeyModifiers::NONE) => {
            app.pi_scan.fallback_confirmed = !app.pi_scan.fallback_confirmed;
        }
        (KeyCode::Char('w'), KeyModifiers::NONE) => {
            app.pi_scan.readiness_warning_confirmed = !app.pi_scan.readiness_warning_confirmed;
        }
        _ => return false,
    }
    app.pi_scan.pending_action = Some(PiScanUiAction::UpdateConsent);
    app.pi_scan.notice = Some(crate::i18n::t(app, "app.pi_scan.notices.session_only"));
    true
}

/// Toggle targets or request an inert queue/dry-run preview.
fn handle_targets(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char(' ') => {
            if let Some(target) = app.pi_scan.targets.get_mut(app.pi_scan.selected) {
                target.selected = !target.selected;
            }
        }
        KeyCode::Enter => request_queue(app),
        _ => return false,
    }
    true
}

/// Validate setup gates before queueing or previewing.
fn request_queue(app: &mut AppState) {
    let targets: Vec<String> = app
        .pi_scan
        .targets
        .iter()
        .filter(|target| target.selected)
        .map(|target| target.package_base.clone())
        .collect();
    if targets.is_empty() {
        app.pi_scan.notice = Some(crate::i18n::t(app, "app.pi_scan.notices.select_target"));
        return;
    }
    if app.dry_run {
        app.pi_scan.dry_run_preview = Some(PiScanDryRunPreview {
            targets,
            process: format!(
                "{} --mode rpc --no-session --offline … {}",
                app.pi_scan.settings.binary, app.pi_scan.settings.model
            ),
            disclosure: crate::i18n::t(app, "app.pi_scan.targets.dry_run_disclosure"),
        });
        app.pi_scan.notice = Some(crate::i18n::t(app, "app.pi_scan.notices.preview_only"));
        app.pi_scan.pending_action = Some(PiScanUiAction::QueueSelected);
        return;
    }
    if !app.pi_scan.settings.enabled
        || !app.pi_scan.disclosure_confirmed
        || !app.pi_scan.runtime.consent.paid_execution
    {
        app.pi_scan.notice = Some(crate::i18n::t(app, "app.pi_scan.notices.setup_required"));
        return;
    }
    let warning_unconfirmed = matches!(app.pi_scan.readiness, PiScanReadiness::Warning(_))
        && !app.pi_scan.readiness_warning_confirmed;
    let fallback_unconfirmed =
        !app.pi_scan.settings.fallback_models.trim().is_empty() && !app.pi_scan.fallback_confirmed;
    if warning_unconfirmed || fallback_unconfirmed {
        app.pi_scan.notice = Some(crate::i18n::t(app, "app.pi_scan.notices.confirm_required"));
        return;
    }
    if !matches!(
        app.pi_scan.availability,
        PiScanAvailability::RuntimeConnected
    ) {
        app.pi_scan.notice = Some(crate::i18n::t(
            app,
            "app.pi_scan.notices.runtime_disconnected",
        ));
        return;
    }
    app.pi_scan.pending_action = Some(PiScanUiAction::QueueSelected);
    app.pi_scan.view = PiScanView::Progress;
}

/// Set detach/reopen/pause/cancel/retry affordance state.
fn handle_progress(key: KeyEvent, app: &mut AppState) -> bool {
    let action = match key.code {
        KeyCode::Char('d') => {
            app.pi_scan.detached = true;
            PiScanUiAction::Detach
        }
        KeyCode::Char('o') => {
            app.pi_scan.detached = false;
            PiScanUiAction::Reopen
        }
        KeyCode::Char('p') => PiScanUiAction::Pause,
        KeyCode::Char('u') => PiScanUiAction::Resume,
        KeyCode::Char('x') => {
            let Some(id) = app
                .pi_scan
                .runtime
                .active
                .as_ref()
                .map(|active| active.correlation_id)
            else {
                return true;
            };
            PiScanUiAction::Cancel(id)
        }
        KeyCode::Char('r') => PiScanUiAction::Retry,
        _ => return false,
    };
    app.pi_scan.pending_action = Some(action);
    true
}

/// Open the selected validated result.
fn handle_results(key: KeyEvent, app: &mut AppState) -> bool {
    if key.code == KeyCode::Enter && app.pi_scan.selected_result().is_some() {
        app.pi_scan.view = PiScanView::Details;
        return true;
    }
    false
}

/// Apply separate result-bound finding and stale acknowledgements.
fn handle_details(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char('a') => app.pi_scan.acknowledge_selected_findings(),
        KeyCode::Char('s') => app.pi_scan.acknowledge_selected_stale(),
        KeyCode::Char('c') if app.pi_scan.selected_result_acknowledged() => {
            app.pi_scan.pending_action = Some(PiScanUiAction::ContinueSelected);
        }
        KeyCode::Char('b') if app.pi_scan.selected_result_acknowledged() => {
            app.pi_scan.pending_action = Some(PiScanUiAction::AcceptBaseline);
        }
        KeyCode::Char('c' | 'b') => {
            app.pi_scan.notice = Some(crate::i18n::t(app, "app.pi_scan.notices.confirm_required"));
        }
        _ => return false,
    }
    true
}
