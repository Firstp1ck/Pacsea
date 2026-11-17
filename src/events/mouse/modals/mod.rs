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
/// - `SshAurSetup` modal: Copies the public key when clicking the copy row (when shown).
/// - Preflight modal: Handles tab clicks, package group header toggles, service restart decisions,
///   and scroll navigation for Deps/Files/Services tabs.
/// - News modal: Handles item selection, URL opening, and scroll navigation; closes on outside click.
/// - `SystemUpdate`, `Repositories`, and `OptionalDeps` modals: wheel moves the focused row/cursor
///   when the pointer is inside the modal bounds (plus `OptionalDeps` wizard click).
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
        crate::state::Modal::Announcement { .. } => {
            simple::handle_announcement_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::News { .. } => simple::handle_news_modal(m, mx, my, is_left_down, app),
        crate::state::Modal::Updates { .. } => {
            simple::handle_updates_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::SystemUpdate { .. } => {
            simple::handle_system_update_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::Repositories { .. } => {
            simple::handle_repositories_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::OptionalDeps { .. } => {
            simple::handle_optional_deps_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::SshAurSetup { .. } => {
            simple::handle_ssh_aur_setup_modal(m, mx, my, is_left_down, app)
        }
        crate::state::Modal::ConfirmPiScanContinuation { .. } => {
            if is_left_down {
                let index = app.pi_scan.continuation_buttons.iter().position(|rect| {
                    rect.is_some_and(|(x, y, width, height)| {
                        mx >= x
                            && mx < x.saturating_add(width)
                            && my >= y
                            && my < y.saturating_add(height)
                    })
                });
                if let Some(index) = index {
                    super::super::modals::activate_pi_scan_continuation_button(app, index == 0);
                }
            }
            Some(false)
        }
        crate::state::Modal::ClosingPiScan => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod pi_scan_tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};

    /// Clickable continuation buttons use the same guarded confirmation and cancellation paths.
    #[test]
    fn continuation_buttons_capture_and_dispatch_mouse_clicks() {
        let mut app = AppState::default();
        let confirmation = crate::state::modal::PiScanContinuationConfirmation {
            result_binding: "missing".to_string(),
            package_base: "demo".to_string(),
            finding_acknowledgement_required: false,
            stale_acknowledgement_required: false,
        };
        app.pi_scan.continuation_buttons = [Some((3, 7, 12, 1)), Some((16, 7, 12, 1))];
        let event = |x| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: 7,
            modifiers: KeyModifiers::NONE,
        };
        app.modal = crate::state::Modal::ConfirmPiScanContinuation {
            confirmation: confirmation.clone(),
            scroll: 0,
        };
        assert_eq!(
            handle_modal_mouse(event(18), 18, 7, true, &mut app),
            Some(false)
        );
        assert!(matches!(app.modal, crate::state::Modal::None));
        assert!(app.pi_scan.pending_action.is_none());
        app.modal = crate::state::Modal::ConfirmPiScanContinuation {
            confirmation,
            scroll: 0,
        };
        assert_eq!(
            handle_modal_mouse(event(4), 4, 7, true, &mut app),
            Some(false)
        );
        assert!(matches!(app.modal, crate::state::Modal::None));
        assert!(app.pi_scan.notices.foreground.is_some());
    }

    /// Clicking the rendered Cancel label must not confirm, at wide or narrow widths.
    #[test]
    fn rendered_continuation_cancel_click_never_confirms() {
        use ratatui::{Terminal, backend::TestBackend};
        for width in [110, 20] {
            let mut app = AppState {
                app_mode: crate::state::types::AppMode::PiScan,
                modal: crate::state::Modal::ConfirmPiScanContinuation {
                    confirmation: crate::state::modal::PiScanContinuationConfirmation {
                        result_binding: "missing".to_string(),
                        package_base: "demo".to_string(),
                        finding_acknowledgement_required: false,
                        stale_acknowledgement_required: false,
                    },
                    scroll: 0,
                },
                ..AppState::default()
            };
            let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
            app.translations = crate::i18n::load_locale_file("en-US", &locales).expect("locale");
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).expect("terminal");
            terminal
                .draw(|frame| crate::ui::ui(frame, &mut app))
                .expect("render");
            let buffer = terminal.backend().buffer();
            let needle = if width < 38 {
                "[Esc]"
            } else {
                "[ Esc Cancel ]"
            };
            let (x, y) = (0..24)
                .find_map(|row| {
                    let line = (0..width)
                        .map(|col| buffer[(col, row)].symbol())
                        .collect::<String>();
                    line.find(needle).map(|column| {
                        (
                            u16::try_from(column).expect("x")
                                + u16::try_from(needle.len() / 2).expect("center"),
                            row,
                        )
                    })
                })
                .expect("visible cancel label");
            assert_eq!(
                handle_modal_mouse(
                    MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        column: x,
                        row: y,
                        modifiers: KeyModifiers::NONE,
                    },
                    x,
                    y,
                    true,
                    &mut app
                ),
                Some(false)
            );
            assert!(matches!(app.modal, crate::state::Modal::None));
            assert!(
                app.pi_scan.notices.foreground.is_none(),
                "Cancel must not invoke confirmation at {width}"
            );
        }
    }

    /// Closing overlay never forwards mouse clicks to Pi Scan tabs.
    #[test]
    fn closing_pi_scan_owns_mouse() {
        let mut app = AppState {
            modal: crate::state::Modal::ClosingPiScan,
            ..AppState::default()
        };
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: 7,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(handle_modal_mouse(event, 3, 7, true, &mut app), Some(false));
    }
}
