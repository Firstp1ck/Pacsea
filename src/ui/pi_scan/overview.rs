//! Scanner overview page.

use crate::state::AppState;
use ratatui::{Frame, layout::Rect, text::Line};

/// Render queue, active work, budgets, consent, and coverage cautions.
pub(super) fn render(f: &mut Frame, app: &AppState, area: Rect) {
    let pi = &app.pi_scan;
    let active = pi.runtime.active.as_ref().map_or_else(
        || "—".to_string(),
        |item| {
            format!(
                "{} @ {}",
                item.request.key.package_base, item.request.key.commit_oid
            )
        },
    );
    let pauses = if pi.runtime.pause_reasons.is_empty() {
        "—".to_string()
    } else {
        format!("{:?}", pi.runtime.pause_reasons)
    };
    super::body(
        f,
        app,
        area,
        "app.pi_scan.tabs.overview",
        vec![
            Line::from(format!(
                "{}: {}",
                crate::i18n::t(app, "app.pi_scan.overview.active"),
                active
            )),
            Line::from(format!(
                "{}: {}",
                crate::i18n::t(app, "app.pi_scan.overview.queued"),
                pi.runtime.queue.len()
            )),
            Line::from(format!(
                "{}: {}",
                crate::i18n::t(app, "app.pi_scan.overview.pauses"),
                pauses
            )),
            Line::from(format!(
                "{}: {}/h, {}/24h, ${}/24h",
                crate::i18n::t(app, "app.pi_scan.overview.budgets"),
                pi.settings.background_starts_per_hour,
                pi.settings.background_token_cap_24h,
                pi.settings.background_cost_cap_24h
            )),
            Line::from(format!(
                "{}: {} / {}",
                crate::i18n::t(app, "app.pi_scan.overview.consents"),
                pi.runtime.consent.background_observation,
                pi.runtime.consent.paid_execution
            )),
            Line::from(crate::i18n::t(app, "app.pi_scan.overview.advisory")),
        ],
    );
}
