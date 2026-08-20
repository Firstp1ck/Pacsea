//! Queue and active-progress page.

use crate::state::AppState;
use ratatui::{Frame, layout::Rect, text::Line};
use std::time::{SystemTime, UNIX_EPOCH};

/// Render sequential queue and pause/cancel/retry affordances.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let pi = &app.pi_scan;
    let mut lines = Vec::new();
    if let Some(active) = &pi.runtime.active {
        let now = unix_now();
        let elapsed = now.saturating_sub(active.started_at_unix);
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            [usize::try_from(now % 10).unwrap_or(0)];
        lines.push(Line::from(format!(
            "{spinner} {}: {} @ {} (#{})",
            crate::i18n::t(app, "app.pi_scan.progress.active"),
            active.request.key.package_base,
            active.request.key.commit_oid,
            active.correlation_id,
        )));
        lines.push(Line::from(format!(
            "{} {:02}:{:02} · {} {} {} / {}",
            crate::i18n::t(app, "app.pi_scan.progress.running_for"),
            elapsed / 60,
            elapsed % 60,
            crate::i18n::t(app, "app.pi_scan.progress.reservation"),
            super::format_token_count(active.request.reservation.tokens),
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.tokens"),
            super::format_microusd(active.request.reservation.cost_microusd),
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
            "• {} @ {} ({priority}) · {} {} / {}",
            request.key.package_base,
            request.key.commit_oid,
            super::format_token_count(request.reservation.tokens),
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.tokens"),
            super::format_microusd(request.reservation.cost_microusd),
        )));
    }
    let scroll = super::clamp_line_scroll(pi.view_scroll.progress, &lines, area);
    app.pi_scan.view_scroll.progress = scroll;
    super::body_scrolled(f, app, area, "app.pi_scan.tabs.progress", lines, scroll);
}

/// Return the current Unix second for elapsed display and redraw-driven animation.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
