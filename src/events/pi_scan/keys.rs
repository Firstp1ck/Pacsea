//! Keyboard state transitions for the Pi Scan workspace.

use crate::state::pi_scan_setup::{PiScanSetupHitTarget, PiScanSetupStep};
use crate::state::pi_scan_ui::PiScanNoticeSeverity;
use crate::state::types::AppMode;
use crate::state::{
    AppState, PiScanAvailability, PiScanDryRunPreview, PiScanReadiness, PiScanUiAction, PiScanView,
    Source,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{SystemTime, UNIX_EPOCH};

/// Open Pi Scan with context from the selected Search result.
pub(super) fn open_from_search(app: &mut AppState) {
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
pub(super) fn handle_key(key: KeyEvent, app: &mut AppState) -> bool {
    if app.pi_scan.wizard.is_some() {
        return handle_wizard(key, app);
    }
    if app.pi_scan.budget_dialog.is_some() {
        return handle_budget_dialog(key, app);
    }
    if key.code == KeyCode::Esc {
        app.app_mode = AppMode::Package;
        return true;
    }
    if handle_page_key(key, app) || handle_navigation(key, app) || handle_scroll_key(key, app) {
        return true;
    }
    match app.pi_scan.view {
        PiScanView::Setup => handle_setup(key, app),
        PiScanView::Targets => handle_targets(key, app),
        PiScanView::Progress => handle_progress(key, app),
        PiScanView::Results => handle_results(key, app),
        PiScanView::Details => handle_details(key, app),
        PiScanView::Overview => handle_budget_key(key, app),
    }
}

/// What: Handle one key while the focused budget choice owns keyboard input.
///
/// Inputs:
/// - `key`: Pressed key event.
/// - `app`: Workspace containing the active budget dialog.
///
/// Output:
/// - True for every dialog-owned focus, confirm, and cancel key.
///
/// Details:
/// - Enter dispatches once, Esc closes only before/after submission, and Tab/arrows/h/l move focus.
fn handle_budget_dialog(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Esc => app.pi_scan.cancel_budget_dialog(),
        KeyCode::Enter => app.pi_scan.submit_budget_dialog(),
        KeyCode::Tab
        | KeyCode::BackTab
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Up
        | KeyCode::Down
        | KeyCode::Char('h' | 'j' | 'k' | 'l') => {
            if let Some(dialog) = app.pi_scan.budget_dialog.as_mut() {
                dialog.toggle_selection();
            }
            true
        }
        _ => false,
    }
}

/// What: Open the direct budget choice for an eligible Overview/Progress projection.
///
/// Inputs:
/// - `key`: Candidate plain `b` key.
/// - `app`: Workspace runtime projection.
///
/// Output:
/// - Whether the key opened the choice.
///
/// Details:
/// - Ineligible projections ignore the key and never enter guided setup.
fn handle_budget_key(key: KeyEvent, app: &mut AppState) -> bool {
    matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('b'), KeyModifiers::NONE)
    ) && app.pi_scan.open_budget_dialog_at(pi_scan_unix_now())
}

/// What: Read Unix time for rolling budget eligibility at an interactive key press.
///
/// Inputs:
/// - Current system clock.
///
/// Output:
/// - Unix seconds, or zero if the clock predates the epoch.
///
/// Details:
/// - Runtime Apply still recomputes authoritatively using its request timestamp.
fn pi_scan_unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// Handle one key inside the isolated keyboard-first setup wizard.
fn handle_wizard(key: KeyEvent, app: &mut AppState) -> bool {
    if key.code == KeyCode::Esc {
        app.pi_scan.cancel_or_abandon_setup_wizard();
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
        KeyCode::Char('n') => wizard.next(dry_run),
        KeyCode::Char(character)
            if (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                && wizard.edit_text(Some(character), false) => {}
        KeyCode::Left | KeyCode::Char('h') => wizard.adjust_focused(false),
        KeyCode::Right | KeyCode::Char('l') => wizard.adjust_focused(true),
        KeyCode::Char(' ') => wizard.toggle_focused(),
        KeyCode::Enter => return activate_focused(app, dry_run),
        KeyCode::Char('b') => wizard.back(),
        KeyCode::Char('a') if wizard.step == PiScanSetupStep::Review => {
            return request_wizard_apply(app, dry_run);
        }
        KeyCode::Char('r') => wizard.retry(dry_run),
        _ => return false,
    }
    true
}

/// Activate only the currently focused wizard body control.
fn activate_focused(app: &mut AppState, dry_run: bool) -> bool {
    let Some(wizard) = app.pi_scan.wizard.as_mut() else {
        return false;
    };
    match (wizard.step, wizard.focus) {
        (PiScanSetupStep::PiReadiness, 1) => wizard.request_probe(dry_run),
        (PiScanSetupStep::Route, _) | (PiScanSetupStep::OptionalBehavior, 3..=6) => {
            wizard.adjust_focused(true);
        }
        (PiScanSetupStep::PricingPrivacy, _) | (PiScanSetupStep::OptionalBehavior, 0..=2) => {
            wizard.toggle_focused();
        }
        _ => return false,
    }
    true
}

/// Request final Apply through its explicit wizard action and retain correlation ownership.
fn request_wizard_apply(app: &mut AppState, dry_run: bool) -> bool {
    let Some(wizard) = app.pi_scan.wizard.as_mut() else {
        return false;
    };
    wizard.request_apply(dry_run);
    app.pi_scan.register_setup_apply();
    true
}

/// Activate one semantic mouse target through the same state transitions as keys.
pub(super) fn activate_wizard_target(target: PiScanSetupHitTarget, app: &mut AppState) {
    let dry_run = app.dry_run;
    match target {
        PiScanSetupHitTarget::Cancel => app.pi_scan.cancel_or_abandon_setup_wizard(),
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
            request_wizard_apply(app, dry_run);
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
    app.pi_scan.set_view(PiScanView::all()[next]);
    true
}

/// Move independent target/result selection or the Details line viewport.
fn handle_navigation(key: KeyEvent, app: &mut AppState) -> bool {
    let delta = match key.code {
        KeyCode::Up | KeyCode::Char('k') => -1isize,
        KeyCode::Down | KeyCode::Char('j') => 1,
        _ => return false,
    };
    match app.pi_scan.view {
        PiScanView::Targets if !app.pi_scan.targets.is_empty() => {
            app.pi_scan.selected_target = app
                .pi_scan
                .selected_target
                .saturating_add_signed(delta)
                .min(app.pi_scan.targets.len() - 1);
            app.pi_scan.selected = app.pi_scan.selected_target;
            keep_selected_visible(
                app.pi_scan.selected_target,
                &mut app.pi_scan.view_scroll.targets,
            );
            true
        }
        PiScanView::Results if !app.pi_scan.results.is_empty() => {
            app.pi_scan.selected_result = app
                .pi_scan
                .selected_result
                .saturating_add_signed(delta)
                .min(app.pi_scan.results.len() - 1);
            app.pi_scan.selected = app.pi_scan.selected_result;
            keep_selected_visible(
                app.pi_scan.selected_result,
                &mut app.pi_scan.view_scroll.results,
            );
            true
        }
        PiScanView::Details if app.pi_scan.results.len() > 1 => {
            let next = app
                .pi_scan
                .selected_result
                .saturating_add_signed(delta)
                .min(app.pi_scan.results.len() - 1);
            app.pi_scan.selected_result = next;
            app.pi_scan.selected = next;
            app.pi_scan.view_scroll.details = 0;
            app.pi_scan.detail_scroll = 0;
            true
        }
        PiScanView::Details => {
            app.pi_scan.view_scroll.details = app
                .pi_scan
                .view_scroll
                .details
                .saturating_add_signed(if delta < 0 { -1i16 } else { 1i16 });
            app.pi_scan.detail_scroll = app.pi_scan.view_scroll.details;
            true
        }
        PiScanView::Setup
        | PiScanView::Overview
        | PiScanView::Progress
        | PiScanView::Targets
        | PiScanView::Results => false,
    }
}

/// Keep one selected item within the conservative minimum list viewport.
const fn keep_selected_visible(selected: usize, offset: &mut usize) {
    const MIN_VISIBLE_ROWS: usize = 6;
    if selected < *offset {
        *offset = selected;
    } else if selected >= offset.saturating_add(MIN_VISIBLE_ROWS) {
        *offset = selected.saturating_add(1).saturating_sub(MIN_VISIBLE_ROWS);
    }
}

/// Scroll the current page or list by a page, or jump to its beginning/end.
fn handle_scroll_key(key: KeyEvent, app: &mut AppState) -> bool {
    let command = match key.code {
        KeyCode::PageUp => ScrollCommand::PageUp,
        KeyCode::PageDown => ScrollCommand::PageDown,
        KeyCode::Char('g') if key.modifiers.is_empty() => ScrollCommand::Top,
        KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => ScrollCommand::Bottom,
        _ => return false,
    };
    apply_scroll_command(app, command);
    true
}

/// One semantic workspace scroll command shared by page-key handling.
#[derive(Clone, Copy)]
enum ScrollCommand {
    /// Move toward the beginning by one page.
    PageUp,
    /// Move toward the end by one page.
    PageDown,
    /// Jump to the beginning.
    Top,
    /// Jump to the end.
    Bottom,
}

/// Scroll the current workspace view by one page from mouse-wheel input.
pub(super) fn scroll_current(app: &mut AppState, down: bool) {
    apply_scroll_command(
        app,
        if down {
            ScrollCommand::PageDown
        } else {
            ScrollCommand::PageUp
        },
    );
}

/// Apply one scroll command to the current independent view offset and selection.
fn apply_scroll_command(app: &mut AppState, command: ScrollCommand) {
    match app.pi_scan.view {
        PiScanView::Targets => scroll_items(
            command,
            &mut app.pi_scan.selected_target,
            &mut app.pi_scan.view_scroll.targets,
            app.pi_scan.targets.len(),
        ),
        PiScanView::Results => scroll_items(
            command,
            &mut app.pi_scan.selected_result,
            &mut app.pi_scan.view_scroll.results,
            app.pi_scan.results.len(),
        ),
        PiScanView::Setup => scroll_lines(command, &mut app.pi_scan.view_scroll.setup),
        PiScanView::Overview => scroll_lines(command, &mut app.pi_scan.view_scroll.overview),
        PiScanView::Progress => scroll_lines(command, &mut app.pi_scan.view_scroll.progress),
        PiScanView::Details => {
            scroll_lines(command, &mut app.pi_scan.view_scroll.details);
            app.pi_scan.detail_scroll = app.pi_scan.view_scroll.details;
        }
    }
    app.pi_scan.selected = match app.pi_scan.view {
        PiScanView::Targets => app.pi_scan.selected_target,
        PiScanView::Results | PiScanView::Details => app.pi_scan.selected_result,
        PiScanView::Setup | PiScanView::Overview | PiScanView::Progress => 0,
    };
}

/// Move list selection and offset together for page and boundary navigation.
fn scroll_items(command: ScrollCommand, selected: &mut usize, offset: &mut usize, len: usize) {
    const PAGE: usize = 6;
    if len == 0 {
        *selected = 0;
        *offset = 0;
        return;
    }
    match command {
        ScrollCommand::PageUp => {
            *selected = selected.saturating_sub(PAGE);
            *offset = offset.saturating_sub(PAGE);
        }
        ScrollCommand::PageDown => {
            *selected = selected.saturating_add(PAGE).min(len - 1);
            *offset = offset.saturating_add(PAGE).min(len - 1);
        }
        ScrollCommand::Top => {
            *selected = 0;
            *offset = 0;
        }
        ScrollCommand::Bottom => {
            *selected = len - 1;
            *offset = len.saturating_sub(PAGE);
        }
    }
}

/// Move a line-based viewport with renderer-side content clamping.
const fn scroll_lines(command: ScrollCommand, offset: &mut u16) {
    const PAGE: u16 = 6;
    *offset = match command {
        ScrollCommand::PageUp => offset.saturating_sub(PAGE),
        ScrollCommand::PageDown => offset.saturating_add(PAGE),
        ScrollCommand::Top => 0,
        ScrollCommand::Bottom => u16::MAX,
    };
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
        app.pi_scan.set_foreground_notice(
            "Verifying exact Pi version, route pricing, and provenance before consent…",
            PiScanNoticeSeverity::Info,
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
        app.pi_scan.set_foreground_notice(
            "Review the verified Pi version and exact pricing facts, then press the consent key again",
            PiScanNoticeSeverity::Info,
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
    let notice = crate::i18n::t(app, "app.pi_scan.notices.session_only");
    app.pi_scan
        .set_foreground_notice(notice, PiScanNoticeSeverity::Info);
    true
}

/// Toggle targets or request an inert queue/dry-run preview.
fn handle_targets(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char(' ') => {
            if let Some(target) = app.pi_scan.targets.get_mut(app.pi_scan.selected_target) {
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
        let notice = crate::i18n::t(app, "app.pi_scan.notices.select_target");
        app.pi_scan
            .set_foreground_notice(notice, PiScanNoticeSeverity::Warning);
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
        let notice = crate::i18n::t(app, "app.pi_scan.notices.preview_only");
        app.pi_scan
            .set_foreground_notice(notice, PiScanNoticeSeverity::Info);
        app.pi_scan.pending_action = Some(PiScanUiAction::QueueSelected);
        return;
    }
    if !app.pi_scan.settings.enabled
        || !app.pi_scan.disclosure_confirmed
        || !app.pi_scan.runtime.consent.paid_execution
    {
        let notice = crate::i18n::t(app, "app.pi_scan.notices.setup_required");
        app.pi_scan
            .set_foreground_notice(notice, PiScanNoticeSeverity::Warning);
        return;
    }
    let warning_unconfirmed = matches!(app.pi_scan.readiness, PiScanReadiness::Warning(_))
        && !app.pi_scan.readiness_warning_confirmed;
    let fallback_unconfirmed =
        !app.pi_scan.settings.fallback_models.trim().is_empty() && !app.pi_scan.fallback_confirmed;
    if warning_unconfirmed || fallback_unconfirmed {
        let notice = crate::i18n::t(app, "app.pi_scan.notices.confirm_required");
        app.pi_scan
            .set_foreground_notice(notice, PiScanNoticeSeverity::Warning);
        return;
    }
    if !matches!(
        app.pi_scan.availability,
        PiScanAvailability::RuntimeConnected
    ) {
        let notice = crate::i18n::t(app, "app.pi_scan.notices.runtime_disconnected");
        app.pi_scan
            .set_foreground_notice(notice, PiScanNoticeSeverity::Error);
        return;
    }
    app.pi_scan.snapshot_queue_intent();
    app.pi_scan.pending_action = Some(PiScanUiAction::QueueSelected);
    app.pi_scan.set_view(PiScanView::Progress);
}

/// Set pause/cancel/retry affordance state.
fn handle_progress(key: KeyEvent, app: &mut AppState) -> bool {
    if handle_budget_key(key, app) {
        return true;
    }
    let action = match key.code {
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
                app.pi_scan.set_foreground_notice(
                    "No active Pi scan to cancel",
                    PiScanNoticeSeverity::Info,
                );
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
        let selected = app.pi_scan.selected_result;
        app.pi_scan.set_view(PiScanView::Details);
        app.pi_scan.toggle_result_expansion(selected);
        return true;
    }
    false
}

/// Apply separate result-bound finding and stale acknowledgements.
fn handle_details(key: KeyEvent, app: &mut AppState) -> bool {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            let selected = app.pi_scan.selected_result;
            app.pi_scan.toggle_result_expansion(selected);
        }
        KeyCode::Char('a') => app.pi_scan.acknowledge_selected_findings(),
        KeyCode::Char('s') => app.pi_scan.acknowledge_selected_stale(),
        KeyCode::Char('c') if app.pi_scan.selected_result_acknowledged() => {
            app.pi_scan.pending_action = Some(PiScanUiAction::ContinueSelected);
        }
        KeyCode::Char('b') if app.pi_scan.selected_result_acknowledged() => {
            app.pi_scan.pending_action = Some(PiScanUiAction::AcceptBaseline);
        }
        KeyCode::Char('t') => app.pi_scan.toggle_raw_output(),
        KeyCode::Char('c' | 'b') => {
            let notice = crate::i18n::t(app, "app.pi_scan.notices.confirm_required");
            app.pi_scan
                .set_foreground_notice(notice, PiScanNoticeSeverity::Warning);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::pi_scan::result::{Coverage, ExpectedIdentity, MergedScanResult};
    use crate::state::PiScanDisplayResult;
    use crate::state::pi_scan::{
        PiScanJobRequest, PiScanPauseReason, PiScanPriority, PiScanQueueKey, PiScanReservation,
    };
    use crate::state::pi_scan_setup::{PiScanSetupDraftAction, PiScanSetupStep};

    /// Build queued background work for direct budget-adjustment keyboard tests.
    fn budget_blocked_request() -> PiScanJobRequest {
        PiScanJobRequest {
            request_id: 99,
            key: PiScanQueueKey {
                package_base: crate::logic::pi_scan::identity::PackageBase::new("budget-demo")
                    .expect("package base"),
                commit_oid: crate::logic::pi_scan::identity::CommitOid::new("a".repeat(40))
                    .expect("commit oid"),
            },
            priority: PiScanPriority::Background,
            reservation: PiScanReservation {
                tokens: 501,
                cost_microusd: 1,
            },
            manual_budget_override_confirmed: false,
        }
    }

    /// Build a minimal validated result for keyboard interaction tests.
    fn display_result(package: &str) -> PiScanDisplayResult {
        PiScanDisplayResult {
            validated: MergedScanResult {
                identity: ExpectedIdentity {
                    scan_id: format!("scan-{package}"),
                    package_base: package.to_string(),
                    commit_oid: "commit".to_string(),
                },
                coverage: Coverage::Complete,
                limitations: Vec::new(),
                findings: Vec::new(),
            },
            observed_head_oid: "head".to_string(),
            stale: false,
            mutable_sources: Vec::new(),
        }
    }

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

    /// Plain b opens the direct budget choice only for queued budget-blocked work.
    #[test]
    fn budget_key_opens_direct_choice_without_guided_setup() {
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        app.pi_scan.set_view(PiScanView::Overview);
        app.pi_scan
            .runtime
            .queue
            .push_back(budget_blocked_request());
        app.pi_scan.runtime.budget_limits.tokens_per_24h = 500;
        app.pi_scan
            .runtime
            .pause_reasons
            .insert(PiScanPauseReason::Budget);

        assert!(handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &mut app,
        ));
        assert!(app.pi_scan.wizard.is_none());
        assert!(app.pi_scan.pending_action.is_none());
    }

    /// Budget choice focus, cancellation, submission, and pending keys are deterministic.
    #[test]
    fn budget_dialog_handles_focus_enter_escape_and_pending_state() {
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        app.pi_scan.set_view(PiScanView::Progress);
        app.pi_scan
            .runtime
            .queue
            .push_back(budget_blocked_request());
        app.pi_scan.runtime.budget_limits.tokens_per_24h = 500;
        app.pi_scan
            .runtime
            .pause_reasons
            .insert(PiScanPauseReason::Budget);

        assert!(handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &mut app,
        ));
        let dialog = app.pi_scan.budget_dialog.as_ref().expect("dialog");
        assert_eq!(
            dialog.selection,
            crate::state::pi_scan::PiScanBudgetAdjustment::Double
        );
        assert!(handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut app,
        ));
        assert_eq!(
            app.pi_scan
                .budget_dialog
                .as_ref()
                .expect("dialog")
                .selection,
            crate::state::pi_scan::PiScanBudgetAdjustment::Unlimited
        );
        assert!(handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut app,
        ));
        assert!(app.pi_scan.budget_dialog.is_none());

        assert!(handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &mut app,
        ));
        assert!(handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        ));
        assert_eq!(
            app.pi_scan.pending_action,
            Some(PiScanUiAction::AdjustBudgets(
                crate::state::pi_scan::PiScanBudgetAdjustment::Double
            ))
        );
        assert_eq!(
            app.pi_scan.budget_dialog.as_ref().expect("dialog").status,
            crate::state::pi_scan_ui::PiScanBudgetDialogStatus::Submitting
        );
        assert!(handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut app,
        ));
        assert!(app.pi_scan.budget_dialog.is_some());
        assert!(handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        ));
        assert_eq!(
            app.pi_scan.pending_action,
            Some(PiScanUiAction::AdjustBudgets(
                crate::state::pi_scan::PiScanBudgetAdjustment::Double
            ))
        );
    }

    /// Ineligible b is ignored and Progress r remains the independent Retry action.
    #[test]
    fn budget_key_requires_budget_block_and_progress_retry_is_unchanged() {
        let mut app = AppState::default();
        app.pi_scan.set_view(PiScanView::Overview);
        assert!(!handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &mut app,
        ));
        assert!(app.pi_scan.budget_dialog.is_none());

        app.pi_scan.set_view(PiScanView::Progress);
        assert!(handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            &mut app,
        ));
        assert_eq!(app.pi_scan.pending_action, Some(PiScanUiAction::Retry));
        assert!(app.pi_scan.budget_dialog.is_none());
    }

    /// Enter on a result opens Details with that selected package expanded.
    #[test]
    fn results_enter_expands_selected_package_in_details() {
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        app.pi_scan.results = vec![display_result("alpha"), display_result("beta")];
        app.pi_scan.set_view(PiScanView::Results);
        app.pi_scan.selected_result = 1;
        app.pi_scan.selected = 1;

        assert!(handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        ));
        assert_eq!(app.pi_scan.view, PiScanView::Details);
        assert_eq!(app.pi_scan.selected_result, 1);
        assert!(app.pi_scan.is_result_expanded(1));
        assert!(!app.pi_scan.is_result_expanded(0));
    }

    /// Details navigation selects package headers while page keys retain line scrolling.
    #[test]
    fn details_navigation_selects_packages_without_losing_scroll() {
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        app.pi_scan.results = vec![display_result("alpha"), display_result("beta")];
        app.pi_scan.set_view(PiScanView::Details);
        app.pi_scan.view_scroll.details = 4;
        app.pi_scan.detail_scroll = 4;

        assert!(handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
        ));
        assert_eq!(app.pi_scan.selected_result, 1);
        assert_eq!(app.pi_scan.view_scroll.details, 0);
        assert!(handle_key(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &mut app,
        ));
        assert_eq!(app.pi_scan.selected_result, 1);
        assert_eq!(app.pi_scan.view_scroll.details, 6);
    }

    /// Enter and Space toggle only the selected package's expanded content.
    #[test]
    fn details_keys_toggle_selected_package_expansion() {
        let mut app = AppState {
            app_mode: AppMode::PiScan,
            ..AppState::default()
        };
        app.pi_scan.results = vec![display_result("alpha"), display_result("beta")];
        app.pi_scan.set_view(PiScanView::Details);

        assert!(handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        ));
        assert!(app.pi_scan.is_result_expanded(0));
        assert!(handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
        ));
        assert!(handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut app,
        ));
        assert!(app.pi_scan.is_result_expanded(0));
        assert!(app.pi_scan.is_result_expanded(1));
        assert!(handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut app,
        ));
        assert!(!app.pi_scan.is_result_expanded(1));
    }
}
