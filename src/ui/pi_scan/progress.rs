//! Queue and active-progress page.

use crate::state::AppState;
use ratatui::{Frame, layout::Rect, text::Line};

/// Render sequential queue and detach/pause/cancel/reopen affordances.
pub(super) fn render(f: &mut Frame, app: &AppState, area: Rect) {
    let pi = &app.pi_scan;
    let mut lines = vec![Line::from(crate::i18n::t(app, "app.pi_scan.progress.keys"))];
    if let Some(active) = &pi.runtime.active {
        lines.push(Line::from(format!(
            "{}: {} @ {} (#{}), {}={}",
            crate::i18n::t(app, "app.pi_scan.progress.active"),
            active.request.key.package_base,
            active.request.key.commit_oid,
            active.correlation_id,
            crate::i18n::t(app, "app.pi_scan.progress.detached"),
            pi.detached
        )));
    } else {
        lines.push(Line::from(crate::i18n::t(
            app,
            "app.pi_scan.progress.no_active",
        )));
    }
    for request in &pi.runtime.queue {
        let priority = crate::i18n::t(
            app,
            match request.priority {
                crate::state::pi_scan::PiScanPriority::Foreground => {
                    "app.pi_scan.priority.foreground"
                }
                crate::state::pi_scan::PiScanPriority::Background => {
                    "app.pi_scan.priority.background"
                }
            },
        );
        lines.push(Line::from(format!(
            "• {} @ {} ({priority})",
            request.key.package_base, request.key.commit_oid
        )));
    }
    super::body(f, app, area, "app.pi_scan.tabs.progress", lines);
}
