//! Keyboard and mouse handling for the native Pi Scan workspace.

mod keys;

use crate::state::pi_scan_ui::PiScanNoticeSeverity;
use crate::state::{AppState, PiScanView, Source, types::AppMode};
use crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};

/// Handle one workspace key and localize legacy state-owned notice text at the event boundary.
pub fn handle_key(key: KeyEvent, app: &mut AppState) -> bool {
    keys::handle_key(key, app)
}

/// Open Pi Scan from Search and explain when the current context is not an AUR package.
pub fn open_from_search(app: &mut AppState) {
    let has_aur_context = app
        .results
        .get(app.selected)
        .is_some_and(|item| matches!(item.source, Source::Aur));
    keys::open_from_search(app);
    if !has_aur_context {
        let chord = search_entry_label(app);
        let notice = crate::i18n::t_fmt(app, "app.pi_scan.notices.non_aur_entry", &[&chord]);
        app.pi_scan
            .set_foreground_notice(notice, PiScanNoticeSeverity::Info);
    }
}

/// What: Format the configured Search-mode entry key for context guidance.
///
/// Inputs: Effective Search keymap and locale.
///
/// Output: The configured first chord or a localized fallback when unbound.
///
/// Details: Avoids claiming Shift+A when the user customized the entry key.
pub fn search_entry_label(app: &AppState) -> String {
    app.keymap.search_normal_pi_scan.first().map_or_else(
        || crate::i18n::t(app, "app.pi_scan.common.search_shortcut"),
        crate::theme::KeyChord::label,
    )
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
    app.pi_scan.origin = app.app_mode;
    app.pi_scan.open_context(None, false);
    if !app.pi_scan.setup_complete() {
        app.pi_scan.begin_setup_wizard(true);
    }
    app.app_mode = AppMode::PiScan;
}

/// Handle workspace tabs, list rows, and wheel scrolling.
pub fn handle_mouse(event: MouseEvent, app: &mut AppState) -> bool {
    if matches!(event.kind, MouseEventKind::Down(_)) {
        app.pi_scan.cancel_armed = None;
    }
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
    if app.pi_scan.budget_dialog.is_some() {
        if app.pi_scan.budget_body_overflows {
            match event.kind {
                MouseEventKind::ScrollUp => {
                    app.pi_scan.budget_body_scroll =
                        app.pi_scan.budget_body_scroll.saturating_sub(1);
                }
                MouseEventKind::ScrollDown => {
                    app.pi_scan.budget_body_scroll =
                        app.pi_scan.budget_body_scroll.saturating_add(1);
                }
                _ => {}
            }
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left)
            && let Some((x, y, width, height)) = app.pi_scan.budget_dialog_rect
        {
            let choice_row = y.saturating_add(height.saturating_sub(5));
            if event.column > x
                && event.column < x.saturating_add(width.saturating_sub(1))
                && (event.row == choice_row || event.row == choice_row.saturating_add(1))
            {
                let selected = app.pi_scan.budget_dialog.as_ref().is_some_and(|dialog| {
                    (dialog.selection == crate::state::pi_scan::PiScanBudgetAdjustment::Double)
                        == (event.row == choice_row)
                });
                if selected {
                    app.pi_scan.submit_budget_dialog();
                } else if let Some(dialog) = app.pi_scan.budget_dialog.as_mut() {
                    dialog.toggle_selection();
                }
            }
        }
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
    app.pi_scan.set_view(PiScanView::tabs()[index]);
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
        PiScanView::Setup => {
            let Some(index) = app.pi_scan.setup_row_rects.iter().position(|rect| {
                rect.is_some_and(|(x, y, width, height)| {
                    column >= x
                        && column < x.saturating_add(width)
                        && row >= y
                        && row < y.saturating_add(height)
                })
            }) else {
                return false;
            };
            if index == app.pi_scan.selected_setup {
                keys::activate_setup_row(app);
            } else {
                app.pi_scan.selected_setup = index;
            }
            true
        }
        PiScanView::Targets => {
            let Some(index) = app.pi_scan.target_hit_test(column, row) else {
                return false;
            };
            let checkbox = app
                .pi_scan
                .target_row_rects
                .iter()
                .find(|rect| rect.index == index)
                .is_some_and(|rect| column < rect.x.saturating_add(3));
            if (checkbox || app.pi_scan.selected_target == index)
                && let Some(target) = app.pi_scan.targets.get_mut(index)
            {
                target.selected = !target.selected;
            }
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
        PiScanView::Details => {
            let Some(index) = app
                .pi_scan
                .details_header_rects
                .iter()
                .find(|rect| rect.contains(column, row))
                .map(|rect| rect.index)
            else {
                return false;
            };
            app.pi_scan.selected_result = index;
            app.pi_scan.selected = index;
            app.pi_scan.toggle_result_expansion(index);
            true
        }
        PiScanView::Overview | PiScanView::Progress => false,
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
