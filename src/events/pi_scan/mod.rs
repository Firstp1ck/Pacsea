//! Keyboard and mouse handling for the native Pi Scan workspace.

mod keys;

use crate::state::pi_scan_ui::PiScanNoticeSeverity;
use crate::state::{AppState, PiScanView, Source, types::AppMode};
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};

/// Handle one workspace key and localize legacy state-owned notice text at the event boundary.
pub fn handle_key(key: KeyEvent, app: &mut AppState) -> bool {
    let handled = keys::handle_key(key, app);
    localize_foreground_notice(app);
    handled
}

/// Localize legacy notice text without moving state mutation into rendering.
fn localize_foreground_notice(app: &mut AppState) {
    let key = match app
        .pi_scan
        .notices
        .foreground
        .as_ref()
        .map(|notice| notice.text.as_str())
    {
        Some("Verifying exact Pi version, route pricing, and provenance before consent…") => {
            Some("app.pi_scan.notices.verifying_setup")
        }
        Some(
            "Review the verified Pi version and exact pricing facts, then press the consent key again",
        ) => Some("app.pi_scan.notices.review_setup_facts"),
        Some("No active Pi scan to cancel") => Some("app.pi_scan.notices.no_active_cancel"),
        _ => None,
    };
    if let Some(key) = key {
        let text = crate::i18n::t(app, key);
        if let Some(notice) = app.pi_scan.notices.foreground.as_mut() {
            notice.text = text;
        }
    }
}

/// Open Pi Scan from Search and explain when the current context is not an AUR package.
pub fn open_from_search(app: &mut AppState) {
    let has_aur_context = app
        .results
        .get(app.selected)
        .is_some_and(|item| matches!(item.source, Source::Aur));
    keys::open_from_search(app);
    if !has_aur_context {
        let notice = crate::i18n::t(app, "app.pi_scan.notices.non_aur_entry");
        app.pi_scan
            .set_foreground_notice(notice, PiScanNoticeSeverity::Info);
    }
}

/// What: Open the Pi Scan workspace without attaching stale package context.
///
/// Inputs:
/// - `app`: Mutable application state.
///
/// Output:
/// - Switches to Pi Scan Overview or guided Setup when prerequisites are incomplete.
///
/// Details:
/// - Used by mode-independent navigation such as the Package and News Options menus.
pub fn open_workspace(app: &mut AppState) {
    app.pi_scan.open_context(None, false);
    if !app.pi_scan.setup_complete() {
        app.pi_scan.begin_setup_wizard(true);
    }
    app.app_mode = AppMode::PiScan;
}

/// Handle workspace tabs, list rows, and wheel scrolling.
pub fn handle_mouse(event: MouseEvent, app: &mut AppState) -> bool {
    if let Some(wizard) = app.pi_scan.wizard.as_mut() {
        match event.kind {
            MouseEventKind::ScrollUp => wizard.scroll_body(false),
            MouseEventKind::ScrollDown => wizard.scroll_body(true),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(target) = wizard.hit_test(event.column, event.row) {
                    keys::activate_wizard_target(target, app);
                }
            }
            _ => {}
        }
        // The modal-like wizard owns all mouse input so workspace controls cannot bypass it.
        return true;
    }
    match event.kind {
        MouseEventKind::ScrollUp => {
            keys::scroll_current(app, false);
            return true;
        }
        MouseEventKind::ScrollDown => {
            keys::scroll_current(app, true);
            return true;
        }
        MouseEventKind::Down(MouseButton::Left) => {}
        _ => return false,
    }
    if activate_tab(event.column, event.row, app)
        || activate_details_continue_action(event.column, event.row, app)
        || activate_row(event.column, event.row, app)
    {
        return true;
    }
    false
}

/// Activate one workspace tab using the state-owned view transition.
fn activate_tab(column: u16, row: u16, app: &mut AppState) -> bool {
    let index = app.pi_scan.tab_rects.iter().position(|rect| {
        rect.is_some_and(|(x, y, width, height)| {
            column >= x
                && column < x.saturating_add(width)
                && row >= y
                && row < y.saturating_add(height)
        })
    });
    let Some(index) = index else {
        return false;
    };
    app.pi_scan.set_view(PiScanView::all()[index]);
    true
}

/// Activate the rendered Details continuation action through the shared keyboard transition.
fn activate_details_continue_action(column: u16, row: u16, app: &mut AppState) -> bool {
    app.pi_scan.view == PiScanView::Details
        && app.pi_scan.details_continue_action_hit_test(column, row)
        && keys::open_continuation_confirmation(app)
}

/// Select a rendered target/result row and open Details on a repeated result click.
fn activate_row(column: u16, row: u16, app: &mut AppState) -> bool {
    match app.pi_scan.view {
        PiScanView::Targets => {
            let Some(index) = app.pi_scan.target_hit_test(column, row) else {
                return false;
            };
            app.pi_scan.selected_target = index;
            app.pi_scan.selected = index;
            true
        }
        PiScanView::Results => {
            let Some(index) = app.pi_scan.result_hit_test(column, row) else {
                return false;
            };
            if app.pi_scan.selected_result == index {
                app.pi_scan.set_view(PiScanView::Details);
            } else {
                app.pi_scan.selected_result = index;
                app.pi_scan.selected = index;
            }
            true
        }
        PiScanView::Setup | PiScanView::Overview | PiScanView::Progress | PiScanView::Details => {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::pi_scan::result::{Coverage, ExpectedIdentity, MergedScanResult};
    use crate::state::pi_scan_ui::{PiScanActionHitRect, PiScanDisplayResult};
    use crossterm::event::KeyModifiers;

    /// Build one stale result requiring confirmation for mouse routing coverage.
    fn stale_display_result() -> PiScanDisplayResult {
        PiScanDisplayResult {
            validated: MergedScanResult {
                identity: ExpectedIdentity {
                    scan_id: "scan-alpha".to_string(),
                    package_base: "alpha".to_string(),
                    commit_oid: "commit".to_string(),
                },
                coverage: Coverage::Complete,
                limitations: Vec::new(),
                findings: Vec::new(),
            },
            observed_head_oid: "head".to_string(),
            stale: true,
            mutable_sources: Vec::new(),
        }
    }

    /// Clicking the state-owned Details action uses the same exact confirmation transition as c.
    #[test]
    fn details_continue_action_click_opens_typed_confirmation() {
        let mut app = AppState::default();
        let result = stale_display_result();
        let binding = result.binding();
        app.pi_scan.results.push(result);
        app.pi_scan.set_view(PiScanView::Details);
        app.pi_scan
            .set_details_continue_action_rect(Some(PiScanActionHitRect {
                x: 10,
                y: 20,
                width: 18,
                height: 1,
            }));
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };

        assert!(handle_mouse(event, &mut app));
        assert!(app.pi_scan.pending_action.is_none());
        assert!(matches!(
            &app.modal,
            crate::state::Modal::ConfirmPiScanContinuation { confirmation, .. }
                if confirmation.result_binding == binding
                    && confirmation.package_base == "alpha"
                    && confirmation.stale_acknowledgement_required
        ));
    }
}
