//! Keyboard and mouse handling for the native Pi Scan workspace.

mod keys;

use crate::state::pi_scan_ui::PiScanNoticeSeverity;
use crate::state::{AppState, PiScanView, Source};
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
    if activate_tab(event.column, event.row, app) || activate_row(event.column, event.row, app) {
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
