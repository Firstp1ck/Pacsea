//! Modal mouse event handling (Help, `VirusTotalSetup`, `Preflight`, News).

use crate::state::AppState;
use crossterm::event::MouseEvent;

mod preflight;
mod preflight_helpers;
mod preflight_tabs;
mod simple;

/// Handle mouse events for modals.
///
/// What: Process mouse interactions within modal dialogs (Help, `VirusTotalSetup`, `Preflight`, News).
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `is_left_down`: Whether the left mouse button is pressed
/// - `app`: Mutable application state containing modal state and UI rectangles
///
/// Output:
/// - `Some(bool)` if the event was handled (consumed by a modal), `None` if not handled.
///   The boolean value indicates whether the application should exit (always `false` here).
///
/// Details:
/// - Help modal: Supports scrolling within content area and closes on outside click.
/// - `VirusTotalSetup` modal: Opens URL when clicking the link area; consumes all other events.
/// - Preflight modal: Handles tab clicks, package group header toggles, service restart decisions,
///   and scroll navigation for Deps/Files/Services tabs.
/// - News modal: Handles item selection, URL opening, and scroll navigation; closes on outside click.
pub(super) fn handle_modal_mouse(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    app: &mut AppState,
) -> Option<bool> {
    match &mut app.modal {
        crate::state::Modal::Help => Some(simple::handle_help_modal(m, mx, my, is_left_down, app)),
        crate::state::Modal::VirusTotalSetup { .. } => Some(simple::handle_virustotal_modal(
            m,
            mx,
            my,
            is_left_down,
            app,
        )),
        crate::state::Modal::Preflight { .. } => {
            preflight::handle_preflight_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::News { .. } => simple::handle_news_modal(m, mx, my, is_left_down, app),
        crate::state::Modal::Updates { .. } => {
            simple::handle_updates_modal(m, mx, my, is_left_down, app)
        }
        _ => None,
    }
}
