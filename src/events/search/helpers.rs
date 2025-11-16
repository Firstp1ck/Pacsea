use crossterm::event::KeyEvent;

use crate::state::AppState;

/// What: Check if a key event matches any chord in a list, handling Shift+char edge cases.
///
/// Inputs:
/// - `ke`: Key event from terminal
/// - `list`: List of configured key chords to match against
///
/// Output:
/// - `true` if the key event matches any chord in the list, `false` otherwise
///
/// Details:
/// - Treats Shift+<char> from config as equivalent to uppercase char without Shift from terminal.
/// - Handles cases where terminals report Shift inconsistently.
pub fn matches_any(ke: &KeyEvent, list: &[crate::theme::KeyChord]) -> bool {
    list.iter().any(|c| {
        if (c.code, c.mods) == (ke.code, ke.modifiers) {
            return true;
        }
        match (c.code, ke.code) {
            (crossterm::event::KeyCode::Char(cfg_ch), crossterm::event::KeyCode::Char(ev_ch)) => {
                let cfg_has_shift = c.mods.contains(crossterm::event::KeyModifiers::SHIFT);
                if !cfg_has_shift {
                    return false;
                }
                // Accept uppercase event regardless of SHIFT flag
                if ev_ch == cfg_ch.to_ascii_uppercase() {
                    return true;
                }
                // Accept lowercase char if terminal reports SHIFT in modifiers
                if ke.modifiers.contains(crossterm::event::KeyModifiers::SHIFT)
                    && ev_ch.to_ascii_lowercase() == cfg_ch
                {
                    return true;
                }
                false
            }
            _ => false,
        }
    })
}

/// What: Handle pane navigation from Search pane to adjacent panes.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `direction`: Direction to navigate ("right" for Install, "left" for Recent)
/// - `details_tx`: Channel to request details for the focused item
/// - `preview_tx`: Channel to request preview details when moving focus
///
/// Output:
/// - None (modifies app state directly)
///
/// Details:
/// - In installed-only mode, right navigation targets Downgrade first.
/// - Otherwise, right navigation targets Install list.
/// - Left navigation always targets Recent pane.
pub fn navigate_pane(
    app: &mut AppState,
    direction: &str,
    details_tx: &tokio::sync::mpsc::UnboundedSender<crate::state::PackageItem>,
    preview_tx: &tokio::sync::mpsc::UnboundedSender<crate::state::PackageItem>,
) {
    match direction {
        "right" => {
            if app.installed_only_mode {
                // Target Downgrade first in installed-only mode
                app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                if app.downgrade_state.selected().is_none() && !app.downgrade_list.is_empty() {
                    app.downgrade_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::super::utils::refresh_downgrade_details(app, details_tx);
            } else {
                if app.install_state.selected().is_none() && !app.install_list.is_empty() {
                    app.install_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Install;
                super::super::utils::refresh_install_details(app, details_tx);
            }
        }
        "left" => {
            if app.history_state.selected().is_none() && !app.recent.is_empty() {
                app.history_state.select(Some(0));
            }
            app.focus = crate::state::Focus::Recent;
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        _ => {}
    }
}
