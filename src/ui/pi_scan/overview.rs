//! Scanner overview page.

use crate::state::AppState;
use crate::state::pi_scan::{PiScanAccountingClass, PiScanPauseReason};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
};

use super::SemanticTone;

/// Render grouped current activity, budgets, permissions, and notices.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let mut lines = Vec::new();
    push_activity_lines(&mut lines, app);
    push_budget_lines(&mut lines, app);
    push_permission_lines(&mut lines, app);
    push_notice_lines(&mut lines, app);
    let scroll = super::clamp_line_scroll(app.pi_scan.view_scroll.overview, &lines, area);
    app.pi_scan.view_scroll.overview = scroll;
    super::body_scrolled(f, app, area, "app.pi_scan.tabs.overview", lines, scroll);
}

/// Append active identity, queue depth, and localized pause reasons.
fn push_activity_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    let pi = &app.pi_scan;
    lines.push(super::section_heading(
        app,
        "app.pi_scan.overview.sections.activity",
    ));
    let (active, active_tone) = pi.runtime.active.as_ref().map_or_else(
        || {
            (
                crate::i18n::t(app, "app.pi_scan.overview.no_active"),
                SemanticTone::Muted,
            )
        },
        |item| {
            (
                format!(
                    "{} @ {}",
                    item.request.key.package_base,
                    super::short_identity(&item.request.key.commit_oid.to_string())
                ),
                SemanticTone::Active,
            )
        },
    );
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.overview.active"),
        active,
        active_tone,
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.overview.queued"),
        pi.runtime.queue.len().to_string(),
        if pi.runtime.queue.is_empty() {
            SemanticTone::Muted
        } else {
            SemanticTone::Active
        },
    ));
    let pauses = if pi.runtime.pause_reasons.is_empty() {
        crate::i18n::t(app, "app.pi_scan.common.none")
    } else {
        pi.runtime
            .pause_reasons
            .iter()
            .map(|reason| crate::i18n::t(app, pause_reason_key(*reason)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.overview.pauses"),
        pauses,
        if pi.runtime.pause_reasons.is_empty() {
            SemanticTone::Success
        } else {
            SemanticTone::Warning
        },
    ));
}

/// Append rolling unattended usage and configured limits.
fn push_budget_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    let pi = &app.pi_scan;
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
    push_section_gap(lines, app, "app.pi_scan.overview.sections.budget");
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.overview.starts_limit"),
        format!("{}/h", pi.settings.background_starts_per_hour),
        SemanticTone::Normal,
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.overview.tokens_used"),
        format!(
            "{} / {}",
            super::format_token_count(consumed_tokens),
            super::format_token_count(pi.settings.background_token_cap_24h)
        ),
        SemanticTone::Normal,
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.overview.cost_used"),
        format!(
            "{} / ${} USD",
            super::format_microusd(consumed_cost),
            pi.settings.background_cost_cap_24h
        ),
        SemanticTone::Normal,
    ));
}

/// Append independent observation and paid-execution permissions.
fn push_permission_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    let consent = app.pi_scan.runtime.consent;
    push_section_gap(lines, app, "app.pi_scan.overview.sections.permissions");
    let permissions = [
        (
            "app.pi_scan.overview.observation_permission",
            consent.background_observation,
        ),
        (
            "app.pi_scan.overview.paid_permission",
            consent.paid_execution,
        ),
    ];
    for (key, allowed) in permissions {
        lines.push(super::labeled_line(
            crate::i18n::t(app, key),
            crate::i18n::t(
                app,
                if allowed {
                    "app.pi_scan.common.allowed"
                } else {
                    "app.pi_scan.common.not_allowed"
                },
            ),
            if allowed {
                SemanticTone::Success
            } else {
                SemanticTone::Warning
            },
        ));
    }
}

/// Append the advisory notice and optional runtime background feedback.
fn push_notice_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    push_section_gap(lines, app, "app.pi_scan.overview.sections.notices");
    lines.push(Line::from(Span::styled(
        format!("⚠ {}", crate::i18n::t(app, "app.pi_scan.overview.advisory")),
        super::semantic_style(SemanticTone::Warning),
    )));
    if let Some(background) = app.pi_scan.notices.background.as_ref() {
        lines.push(super::labeled_line(
            crate::i18n::t(app, "app.pi_scan.overview.background_notice"),
            super::localized_notice(app, background),
            notice_tone(background.severity),
        ));
    }
}

/// Insert one deliberate section gap followed by a shared heading.
fn push_section_gap(lines: &mut Vec<Line<'static>>, app: &AppState, key: &str) {
    lines.push(Line::from(""));
    lines.push(super::section_heading(app, key));
}

/// Map one sticky pause reason to its existing localized wording.
const fn pause_reason_key(reason: PiScanPauseReason) -> &'static str {
    match reason {
        PiScanPauseReason::User => "app.pi_scan.progress.pause.user",
        PiScanPauseReason::Service => "app.pi_scan.progress.pause.service",
        PiScanPauseReason::Budget => "app.pi_scan.progress.pause.budget",
    }
}

/// Map one notice severity to the shared semantic emphasis categories.
const fn notice_tone(severity: crate::state::pi_scan_ui::PiScanNoticeSeverity) -> SemanticTone {
    match severity {
        crate::state::pi_scan_ui::PiScanNoticeSeverity::Info => SemanticTone::Active,
        crate::state::pi_scan_ui::PiScanNoticeSeverity::Success => SemanticTone::Success,
        crate::state::pi_scan_ui::PiScanNoticeSeverity::Warning => SemanticTone::Warning,
        crate::state::pi_scan_ui::PiScanNoticeSeverity::Error => SemanticTone::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::state::AppState;
    use ratatui::{Terminal, backend::TestBackend};

    /// Overview exposes activity, budget, permission, and notice sections.
    #[test]
    fn overview_renders_balanced_sections() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::default();
        load_english(&mut app);
        terminal
            .draw(|frame| render(frame, &mut app, frame.area()))
            .expect("overview render");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        for heading in [
            "Current activity",
            "Unattended budget",
            "Permissions",
            "Notices",
        ] {
            assert!(
                rendered.contains(heading),
                "missing {heading:?}: {rendered:?}"
            );
        }
    }

    /// Overview remains renderable at the compact supported dimensions.
    #[test]
    fn overview_renders_at_narrow_dimensions() {
        let backend = TestBackend::new(20, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::default();
        load_english(&mut app);
        terminal
            .draw(|frame| render(frame, &mut app, frame.area()))
            .expect("narrow overview render");
    }

    /// Load the shipped English locale for human-facing render assertions.
    fn load_english(app: &mut AppState) {
        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
        app.translations =
            crate::i18n::load_locale_file("en-US", &locales).expect("English locale");
    }
}
