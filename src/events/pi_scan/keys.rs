//! Keyboard state transitions for the Pi Scan workspace.

use crate::state::pi_scan_setup::{PiScanSetupApplyStatus, PiScanSetupHitTarget, PiScanSetupStep};
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
    if !app.pi_scan.setup_complete() {
        app.pi_scan.begin_setup_wizard(true);
    }
    app.app_mode = AppMode::PiScan;
}

/// Handle one pressed key while the Pi Scan workspace is active.
pub fn handle_key(key: KeyEvent, app: &mut AppState) -> bool {
    if app.pi_scan.wizard.is_some() {
        return handle_wizard(key, app);
    }
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

/// Handle one key inside the isolated keyboard-first setup wizard.
fn handle_wizard(key: KeyEvent, app: &mut AppState) -> bool {
    if key.code == KeyCode::Esc {
        app.pi_scan.cancel_setup_wizard();
        return true;
    }
    let dry_run = app.dry_run;
    let Some(wizard) = app.pi_scan.wizard.as_mut() else {
        return false;
    };
    match key.code {
        KeyCode::Tab => wizard.move_focus(true),
        KeyCode::BackTab => wizard.move_focus(false),
        KeyCode::PageUp => wizard.scroll_body(false),
        KeyCode::PageDown => wizard.scroll_body(true),
        KeyCode::Backspace if wizard.edit_text(None, true) => {}
        KeyCode::Char(character)
            if key.modifiers.is_empty() && wizard.edit_text(Some(character), false) => {}
        KeyCode::Left | KeyCode::Char('h') => wizard.adjust_focused(false),
        KeyCode::Right | KeyCode::Char('l') => wizard.adjust_focused(true),
        KeyCode::Char(' ') => wizard.toggle_focused(),
        KeyCode::Enter => activate_focused(app, dry_run),
        KeyCode::Char('b') => wizard.back(),
        KeyCode::Char('n') => wizard.next(dry_run),
        KeyCode::Char('a') if wizard.step == PiScanSetupStep::Review => {
            wizard.request_apply(dry_run);
        }
        KeyCode::Char('r') => wizard.retry(dry_run),
        KeyCode::Char('q') => app.pi_scan.cancel_setup_wizard(),
        _ => return false,
    }
    true
}

/// Activate the focused body control or the page's primary Enter action.
fn activate_focused(app: &mut AppState, dry_run: bool) {
    let Some(wizard) = app.pi_scan.wizard.as_mut() else {
        return;
    };
    match (wizard.step, wizard.focus) {
        (PiScanSetupStep::PiReadiness, 1) => wizard.request_probe(dry_run),
        (PiScanSetupStep::Route, _) | (PiScanSetupStep::OptionalBehavior, 3..=6) => {
            wizard.adjust_focused(true);
        }
        (PiScanSetupStep::PricingPrivacy, _) | (PiScanSetupStep::OptionalBehavior, 0..=2) => {
            wizard.toggle_focused();
        }
        (PiScanSetupStep::Review, _) => wizard.request_apply(dry_run),
        (PiScanSetupStep::Activate, _)
            if matches!(wizard.apply_status, PiScanSetupApplyStatus::Failed(_)) =>
        {
            wizard.retry(dry_run);
        }
        (PiScanSetupStep::Activate, _)
            if matches!(wizard.apply_status, PiScanSetupApplyStatus::Complete) =>
        {
            app.pi_scan.cancel_setup_wizard();
        }
        (PiScanSetupStep::PiReadiness, 0) if wizard.verified.is_none() => {
            wizard.request_probe(dry_run);
        }
        (PiScanSetupStep::PiReadiness, 0) | (PiScanSetupStep::OptionalBehavior, 7) => {
            wizard.next(dry_run);
        }
        _ => wizard.next(dry_run),
    }
}

/// Activate one semantic mouse target through the same state transitions as keys.
pub(super) fn activate_wizard_target(target: PiScanSetupHitTarget, app: &mut AppState) {
    let dry_run = app.dry_run;
    match target {
        PiScanSetupHitTarget::Cancel => app.pi_scan.cancel_setup_wizard(),
        PiScanSetupHitTarget::Back => {
            if let Some(wizard) = app.pi_scan.wizard.as_mut() {
                wizard.back();
            }
        }
        PiScanSetupHitTarget::Next => {
            if let Some(wizard) = app.pi_scan.wizard.as_mut() {
                wizard.next(dry_run);
            }
        }
        PiScanSetupHitTarget::Retry => {
            if let Some(wizard) = app.pi_scan.wizard.as_mut() {
                wizard.retry(dry_run);
            }
        }
        PiScanSetupHitTarget::Apply => {
            if let Some(wizard) = app.pi_scan.wizard.as_mut() {
                wizard.request_apply(dry_run);
            }
        }
        PiScanSetupHitTarget::Control(index) => {
            if let Some(wizard) = app.pi_scan.wizard.as_mut() {
                wizard.focus = index.min(wizard.focus_count().saturating_sub(1));
            }
            activate_focused(app, dry_run);
        }
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
        (KeyCode::Char('r'), KeyModifiers::NONE)
    ) {
        app.pi_scan.begin_setup_wizard(false);
        return true;
    }
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
            KeyCode::Char('c' | 'o' | 'p' | 'b' | 'f' | 'w'),
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
        (KeyCode::Char('b'), KeyModifiers::NONE) => {
            app.pi_scan.background_paid_execution_confirmed =
                !app.pi_scan.background_paid_execution_confirmed;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::pi_scan_setup::{PiScanSetupDraftAction, PiScanSetupStep};

    /// Escape must cancel only the isolated draft and preserve Pi Scan mode.
    #[test]
    fn wizard_escape_drops_draft_without_runtime_action() {
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        let original = app.pi_scan.settings.clone();
        app.pi_scan.begin_setup_wizard(true);
        app.pi_scan
            .wizard
            .as_mut()
            .expect("wizard")
            .candidate
            .provider = "draft".to_string();
        assert!(handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut app,
        ));
        assert!(app.pi_scan.wizard.is_none());
        assert_eq!(app.pi_scan.settings, original);
        assert!(app.pi_scan.pending_action.is_none());
        assert_eq!(app.app_mode, AppMode::PiScan);
    }

    /// Dry-run Enter on Verify must show guidance without queuing a probe.
    #[test]
    fn wizard_dry_run_verify_queues_no_probe() {
        let mut app = AppState {
            dry_run: true,
            ..AppState::default()
        };
        app.pi_scan.begin_setup_wizard(true);
        let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
        wizard.step = PiScanSetupStep::PiReadiness;
        wizard.focus = 1;
        assert!(handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        ));
        let wizard = app.pi_scan.wizard.as_ref().expect("wizard");
        assert!(wizard.pending_action.is_none());
        assert!(!wizard.validation_issues.is_empty());
    }

    /// Readiness Enter must queue only the correlated inert probe action.
    #[test]
    fn wizard_verify_key_queues_correlated_probe() {
        let mut app = AppState::default();
        app.pi_scan.begin_setup_wizard(true);
        let wizard = app.pi_scan.wizard.as_mut().expect("wizard");
        wizard.step = PiScanSetupStep::PiReadiness;
        wizard.focus = 1;
        assert!(handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        ));
        assert!(matches!(
            app.pi_scan.wizard.as_ref().expect("wizard").pending_action,
            Some(PiScanSetupDraftAction::Probe {
                correlation_id: 1,
                ..
            })
        ));
        assert!(app.pi_scan.pending_action.is_none());
    }
}
