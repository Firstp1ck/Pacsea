//! Keyboard and mouse handling for the native Pi Scan workspace.

mod keys;

use crate::state::{AppState, PiScanView};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

pub use keys::{handle_key, open_from_search};

/// Handle tab clicks in the Pi Scan workspace.
pub fn handle_mouse(event: MouseEvent, app: &mut AppState) -> bool {
    if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
        return false;
    }
    if let Some(wizard) = app.pi_scan.wizard.as_ref() {
        if let Some(target) = wizard.hit_test(event.column, event.row) {
            keys::activate_wizard_target(target, app);
        }
        // While the modal-like wizard is active, swallow all other left clicks
        // so workspace tabs cannot bypass Cancel or the validated step order.
        return true;
    }
    for (index, rect) in app.pi_scan.tab_rects.iter().enumerate() {
        let Some((x, y, width, height)) = rect else {
            continue;
        };
        if event.column >= *x
            && event.column < x.saturating_add(*width)
            && event.row >= *y
            && event.row < y.saturating_add(*height)
        {
            app.pi_scan.view = PiScanView::all()[index];
            app.pi_scan.selected = 0;
            return true;
        }
    }
    false
}
