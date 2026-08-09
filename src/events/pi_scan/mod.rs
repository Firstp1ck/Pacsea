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
