use crate::state::AppState;

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
