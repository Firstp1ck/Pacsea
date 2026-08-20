//! Scanner overview page.

use crate::state::AppState;
use crate::state::pi_scan::PiScanAccountingClass;
use ratatui::{Frame, layout::Rect, text::Line};

/// Render queue, active work, budgets, consent, notices, and coverage cautions.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
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
    let (consumed_tokens, consumed_cost) = pi
        .runtime
        .budget
        .records
        .iter()
        .filter(|record| record.class == PiScanAccountingClass::Background)
        .fold((0u64, 0u64), |(tokens, cost), record| {
            (
                tokens.saturating_add(record.consumed_tokens.unwrap_or(0)),
                cost.saturating_add(record.consumed_cost_microusd.unwrap_or(0)),
            )
        });
    let mut lines = vec![
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
            "{}: {}/h · {} {}/{} · {} {}/{}",
            crate::i18n::t(app, "app.pi_scan.overview.budgets"),
            pi.settings.background_starts_per_hour,
            crate::i18n::t(app, "app.pi_scan.overview.tokens_used"),
            super::format_token_count(consumed_tokens),
            super::format_token_count(pi.settings.background_token_cap_24h),
            crate::i18n::t(app, "app.pi_scan.overview.cost_used"),
            super::format_microusd(consumed_cost),
            pi.settings.background_cost_cap_24h
        )),
        Line::from(format!(
            "{}: {} / {}",
            crate::i18n::t(app, "app.pi_scan.overview.consents"),
            pi.runtime.consent.background_observation,
            pi.runtime.consent.paid_execution
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.overview.advisory")),
    ];
    if let Some(background) = pi.notices.background.as_ref() {
        lines.push(Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.overview.background_notice"),
            super::localized_notice(app, background)
        )));
    }
    let scroll = super::clamp_line_scroll(pi.view_scroll.overview, &lines, area);
    app.pi_scan.view_scroll.overview = scroll;
    super::body_scrolled(f, app, area, "app.pi_scan.tabs.overview", lines, scroll);
}
