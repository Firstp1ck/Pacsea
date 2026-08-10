//! Responsive rendering for the native seven-step Pi Scan setup wizard.

use crate::state::AppState;
use crate::state::pi_scan_setup::{
    PiScanSetupApplyStatus, PiScanSetupHitRect, PiScanSetupHitTarget, PiScanSetupStep,
    PiScanSetupWizardState,
};
use crate::theme::theme;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

/// Render the complete wizard and refresh its semantic mouse hit rectangles.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let Some(wizard) = app.pi_scan.wizard.clone() else {
        return;
    };
    let compact = area.height < 10;
    let issue_height = if compact || wizard.validation_issues.is_empty() {
        1
    } else {
        2
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if compact { 1 } else { 2 }),
            Constraint::Min(3),
            Constraint::Length(issue_height),
            Constraint::Length(if compact { 1 } else { 2 }),
        ])
        .split(area);
    render_progress(f, app, &wizard, chunks[0]);
    render_step(f, app, &wizard, chunks[1]);
    render_validation(f, app, &wizard, chunks[2]);
    let mut hit_rects = render_controls(f, app, &wizard, chunks[3]);
    append_control_rects(&wizard, chunks[1], &mut hit_rects);
    if let Some(live) = app.pi_scan.wizard.as_mut() {
        live.set_hit_rects(hit_rects);
    }
}

/// Draw fixed seven-step progress with the current page always visible.
fn render_progress(f: &mut Frame, app: &AppState, wizard: &PiScanSetupWizardState, area: Rect) {
    let th = theme();
    let step_number = wizard.step.index() + 1;
    let title = step_title(app, wizard.step);
    let compact = format!(
        "{} {step_number}/7 — {title}",
        crate::i18n::t(app, "app.pi_scan.wizard.progress")
    );
    let markers = PiScanSetupStep::all()
        .iter()
        .enumerate()
        .map(|(index, _)| match index.cmp(&wizard.step.index()) {
            std::cmp::Ordering::Less => "●",
            std::cmp::Ordering::Equal => "◆",
            std::cmp::Ordering::Greater => "○",
        })
        .collect::<Vec<_>>()
        .join(" ");
    let lines = if area.height == 1 {
        vec![Line::from(vec![
            Span::styled("◆ ", Style::default().fg(th.sapphire)),
            Span::styled(
                compact,
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            ),
        ])]
    } else {
        vec![
            Line::from(Span::styled(
                compact,
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(markers, Style::default().fg(th.sapphire))),
        ]
    };
    f.render_widget(Paragraph::new(lines), area);
}

/// Draw one page body with keyboard focus styling.
fn render_step(f: &mut Frame, app: &AppState, wizard: &PiScanSetupWizardState, area: Rect) {
    let title = step_title(app, wizard.step);
    let lines = match wizard.step {
        PiScanSetupStep::Welcome => welcome_lines(app, wizard),
        PiScanSetupStep::PiReadiness => readiness_lines(app, wizard),
        PiScanSetupStep::Route => route_lines(app, wizard),
        PiScanSetupStep::PricingPrivacy => pricing_lines(app, wizard),
        PiScanSetupStep::OptionalBehavior => optional_lines(app, wizard),
        PiScanSetupStep::Review => review_lines(app, wizard),
        PiScanSetupStep::Activate => activate_lines(app, wizard),
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((wizard.body_scroll, 0))
            .block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}

/// Draw inline validation or the current non-destructive status notice.
fn render_validation(f: &mut Frame, app: &AppState, wizard: &PiScanSetupWizardState, area: Rect) {
    let th = theme();
    let (text, color) = if wizard.validation_issues.is_empty() {
        (
            wizard.notice.as_ref().map_or_else(
                || crate::i18n::t(app, "app.pi_scan.wizard.validation.ready"),
                |notice| localize_wizard_message(app, notice),
            ),
            th.green,
        )
    } else {
        (
            wizard
                .validation_issues
                .iter()
                .map(|issue| localize_wizard_message(app, issue))
                .collect::<Vec<_>>()
                .join(" · "),
            th.red,
        )
    };
    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: true }),
        area,
    );
}

/// Draw contextual navigation buttons and return exact click rectangles.
fn render_controls(
    f: &mut Frame,
    app: &AppState,
    wizard: &PiScanSetupWizardState,
    area: Rect,
) -> Vec<PiScanSetupHitRect> {
    let th = theme();
    let controls = footer_controls(wizard);
    let mut spans = Vec::new();
    let hit_rects = control_hit_rects(app, area, &controls);
    for (target, key) in controls {
        let label = control_label(app, target, key);
        spans.push(Span::styled(
            label,
            Style::default()
                .fg(th.base)
                .bg(if matches!(target, PiScanSetupHitTarget::Apply) {
                    th.green
                } else {
                    th.sapphire
                })
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }
    let controls = Paragraph::new(Line::from(spans));
    if area.height > 1 {
        f.render_widget(controls.block(Block::default().borders(Borders::TOP)), area);
    } else {
        f.render_widget(controls, area);
    }
    hit_rects
}

/// Build semantic footer controls for the current step and apply outcome.
fn footer_controls(wizard: &PiScanSetupWizardState) -> Vec<(PiScanSetupHitTarget, &'static str)> {
    let mut controls = vec![(PiScanSetupHitTarget::Cancel, "Esc")];
    if wizard.step.previous().is_some()
        && !matches!(wizard.apply_status, PiScanSetupApplyStatus::Complete)
    {
        controls.push((PiScanSetupHitTarget::Back, "b"));
    }
    match wizard.step {
        PiScanSetupStep::Review => controls.push((PiScanSetupHitTarget::Apply, "a")),
        PiScanSetupStep::Activate
            if matches!(wizard.apply_status, PiScanSetupApplyStatus::Failed(_)) =>
        {
            controls.push((PiScanSetupHitTarget::Retry, "r"));
        }
        PiScanSetupStep::Activate => {}
        _ => controls.push((PiScanSetupHitTarget::Next, "n")),
    }
    controls
}

/// Convert footer controls into half-open terminal rectangles.
fn control_hit_rects(
    app: &AppState,
    area: Rect,
    controls: &[(PiScanSetupHitTarget, &'static str)],
) -> Vec<PiScanSetupHitRect> {
    let mut x = area.x;
    controls
        .iter()
        .map(|(target, key)| {
            let label = control_label(app, *target, key);
            let width = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
            let rect = PiScanSetupHitRect {
                target: *target,
                x,
                y: area
                    .y
                    .saturating_add(1)
                    .min(area.bottom().saturating_sub(1)),
                width,
                height: 1,
            };
            x = x.saturating_add(width).saturating_add(1);
            rect
        })
        .collect()
}

/// Add click seams for visible page-local control rows.
fn append_control_rects(
    wizard: &PiScanSetupWizardState,
    body_area: Rect,
    hit_rects: &mut Vec<PiScanSetupHitRect>,
) {
    let first_line = match wizard.step {
        PiScanSetupStep::PiReadiness | PiScanSetupStep::Route => 1,
        PiScanSetupStep::PricingPrivacy => 5,
        PiScanSetupStep::OptionalBehavior => 0,
        PiScanSetupStep::Welcome | PiScanSetupStep::Review | PiScanSetupStep::Activate => return,
    };
    for index in 0..wizard.focus_count() {
        let line = u16::try_from(first_line + index).unwrap_or(u16::MAX);
        let row = body_area.y.saturating_add(1).saturating_add(line);
        if row >= body_area.bottom().saturating_sub(1) {
            break;
        }
        hit_rects.push(PiScanSetupHitRect {
            target: PiScanSetupHitTarget::Control(index),
            x: body_area.x.saturating_add(1),
            y: row,
            width: body_area.width.saturating_sub(2),
            height: 1,
        });
    }
}

/// Render one footer control label.
fn control_label(app: &AppState, target: PiScanSetupHitTarget, key: &str) -> String {
    let label_key = match target {
        PiScanSetupHitTarget::Back => "back",
        PiScanSetupHitTarget::Next => "next",
        PiScanSetupHitTarget::Cancel => {
            if app.pi_scan.wizard.as_ref().is_some_and(|wizard| {
                matches!(wizard.apply_status, PiScanSetupApplyStatus::Complete)
            }) {
                "close"
            } else {
                "cancel"
            }
        }
        PiScanSetupHitTarget::Retry => "retry",
        PiScanSetupHitTarget::Apply => "apply",
        PiScanSetupHitTarget::Control(_) => "select",
    };
    format!(
        " {key} {} ",
        crate::i18n::t(app, &format!("app.pi_scan.wizard.controls.{label_key}"))
    )
}

/// Build advisory-scope Welcome content.
fn welcome_lines(app: &AppState, wizard: &PiScanSetupWizardState) -> Vec<Line<'static>> {
    vec![
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.welcome.summary")),
        Line::from(""),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.welcome.advisory")),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.welcome.data_flow")),
        Line::from(crate::i18n::t(
            app,
            "app.pi_scan.wizard.welcome.no_execution",
        )),
        Line::from(crate::i18n::t(
            app,
            "app.pi_scan.wizard.welcome.credentials",
        )),
        Line::from(""),
        Line::from(if wizard.first_run {
            crate::i18n::t(app, "app.pi_scan.wizard.welcome.first_run")
        } else {
            crate::i18n::t(app, "app.pi_scan.wizard.welcome.reconfigure")
        }),
    ]
}

/// Build Pi binary and no-model readiness controls.
fn readiness_lines(app: &AppState, wizard: &PiScanSetupWizardState) -> Vec<Line<'static>> {
    let version = wizard
        .verified
        .as_ref()
        .map_or("—", |facts| facts.pi_version.as_str());
    vec![
        Line::from(crate::i18n::t(
            app,
            "app.pi_scan.wizard.readiness.explanation",
        )),
        focused_line(
            app,
            wizard,
            0,
            format!(
                "{}: {}_",
                crate::i18n::t(app, "app.pi_scan.wizard.readiness.binary"),
                wizard.candidate.binary
            ),
        ),
        focused_line(
            app,
            wizard,
            1,
            format!(
                "[Enter] {}",
                crate::i18n::t(app, "app.pi_scan.wizard.readiness.verify")
            ),
        ),
        Line::from(format!(
            "{}: {version}",
            crate::i18n::t(app, "app.pi_scan.wizard.readiness.version")
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.readiness.no_model")),
        Line::from(crate::i18n::t(
            app,
            "app.pi_scan.wizard.readiness.credentials",
        )),
    ]
}

/// Build exact route and thinking selectors.
fn route_lines(app: &AppState, wizard: &PiScanSetupWizardState) -> Vec<Line<'static>> {
    let route = format!("{}/{}", wizard.candidate.provider, wizard.candidate.model);
    let count = wizard
        .verified
        .as_ref()
        .map_or(0, |facts| facts.routes.len());
    vec![
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.route.explanation")),
        focused_line(
            app,
            wizard,
            0,
            format!(
                "← {}: {route} →",
                crate::i18n::t(app, "app.pi_scan.wizard.route.primary")
            ),
        ),
        focused_line(
            app,
            wizard,
            1,
            format!(
                "← {}: {} →",
                crate::i18n::t(app, "app.pi_scan.wizard.route.thinking"),
                wizard.candidate.thinking
            ),
        ),
        Line::from(format!(
            "{}: {count}",
            crate::i18n::t(app, "app.pi_scan.wizard.route.advertised")
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.route.exact_only")),
    ]
}

/// Build selected-route pricing, disclosure, and foreground-paid confirmation content.
fn pricing_lines(app: &AppState, wizard: &PiScanSetupWizardState) -> Vec<Line<'static>> {
    let reservation = wizard.reviewed_reservation();
    let route = format!(
        "{}/{}",
        display_or_dash(&wizard.candidate.provider),
        display_or_dash(&wizard.candidate.model)
    );
    vec![
        Line::from(format!(
            "{}: {route}",
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.selected_route")
        )),
        Line::from(format!(
            "{}: {} {} · {}",
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.worst_case"),
            super::format_token_count(reservation.tokens),
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.tokens"),
            super::format_microusd(reservation.cost_microusd)
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.provenance"),
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.provenance_value")
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.pricing.disclosure")),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.pricing.coverage")),
        focused_toggle(
            app,
            wizard,
            0,
            "app.pi_scan.wizard.pricing.confirm_disclosure",
            wizard.confirmations.disclosure_confirmed,
        ),
        focused_toggle(
            app,
            wizard,
            1,
            "app.pi_scan.wizard.pricing.confirm_foreground",
            wizard.confirmations.foreground_paid_confirmed,
        ),
        focused_toggle(
            app,
            wizard,
            2,
            "app.pi_scan.wizard.pricing.confirm_readiness",
            wizard.confirmations.readiness_warning_confirmed,
        ),
    ]
}

/// Build conservative optional settings and independent background decisions.
fn optional_lines(app: &AppState, wizard: &PiScanSetupWizardState) -> Vec<Line<'static>> {
    let fallback = if wizard.candidate.fallback_models.is_empty() {
        crate::i18n::t(app, "app.pi_scan.wizard.common.off")
    } else {
        wizard.candidate.fallback_models.clone()
    };
    vec![
        focused_toggle(
            app,
            wizard,
            0,
            "app.pi_scan.wizard.optional.observation",
            wizard.candidate_consent.background_observation,
        ),
        focused_toggle(
            app,
            wizard,
            1,
            "app.pi_scan.wizard.optional.background_paid",
            wizard.candidate.background_enabled && wizard.candidate_consent.paid_execution,
        ),
        focused_line(
            app,
            wizard,
            2,
            format!(
                "[Space] {}: {fallback}",
                crate::i18n::t(app, "app.pi_scan.wizard.optional.fallback")
            ),
        ),
        focused_line(
            app,
            wizard,
            3,
            format!(
                "← {}: {} →",
                crate::i18n::t(app, "app.pi_scan.wizard.optional.starts"),
                wizard.candidate.background_starts_per_hour
            ),
        ),
        focused_line(
            app,
            wizard,
            4,
            format!(
                "← {}: {} →",
                crate::i18n::t(app, "app.pi_scan.wizard.optional.tokens"),
                wizard.candidate.background_token_cap_24h
            ),
        ),
        focused_line(
            app,
            wizard,
            5,
            format!(
                "← {}: {} →",
                crate::i18n::t(app, "app.pi_scan.wizard.optional.cost"),
                wizard.candidate.background_cost_cap_24h
            ),
        ),
        focused_line(
            app,
            wizard,
            6,
            format!(
                "← {}: {} →",
                crate::i18n::t(app, "app.pi_scan.wizard.optional.retention"),
                wizard.candidate.result_retention_days
            ),
        ),
        focused_line(
            app,
            wizard,
            7,
            format!(
                "{}: {}_",
                crate::i18n::t(app, "app.pi_scan.wizard.optional.proxy"),
                display_or_dash(&wizard.candidate.https_proxy)
            ),
        ),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.optional.defaults")),
    ]
}

/// Build full effective-value and material-binding review content.
fn review_lines(app: &AppState, wizard: &PiScanSetupWizardState) -> Vec<Line<'static>> {
    let settings = &wizard.candidate;
    let facts = wizard.verified.as_ref();
    let binding = facts.map_or("—", |facts| facts.pricing_binding.as_str());
    let pi_version = facts.map_or("—", |facts| facts.pi_version.as_str());
    let pricing_observed = facts.map_or(0, |facts| facts.pricing_observed_at_unix_seconds);
    let pricing_age = facts.map_or(0, |facts| facts.maximum_pricing_age_seconds);
    let reservation = wizard.reviewed_reservation();
    vec![
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.review.explanation")),
        Line::from(format!(
            "{}: {} · {}/{} · {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.route"),
            settings.binary,
            settings.provider,
            settings.model,
            settings.thinking
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.pi_version"),
            pi_version
        )),
        Line::from(format!(
            "{}: {} / {} / {} / {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.pricing"),
            reservation.tokens,
            reservation.cost_microusd,
            pricing_observed,
            pricing_age
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.fallback"),
            display_or_dash(&settings.fallback_models)
        )),
        Line::from(format!(
            "{}: {} / {} / {} / {} / {} / {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.confirmations"),
            yes_no(app, wizard.confirmations.disclosure_confirmed),
            yes_no(app, wizard.confirmations.foreground_paid_confirmed),
            yes_no(app, wizard.candidate_consent.background_observation),
            yes_no(app, wizard.candidate_consent.paid_execution),
            yes_no(app, wizard.confirmations.fallback_confirmed),
            yes_no(app, wizard.confirmations.readiness_warning_confirmed)
        )),
        Line::from(format!(
            "{}: {} / {} / {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.background"),
            yes_no(app, settings.background_enabled),
            settings.background_starts_per_hour,
            settings.background_cost_cap_24h
        )),
        Line::from(format!(
            "{}: {} / {} / {} / {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.timeouts"),
            settings.head_query_timeout_seconds,
            settings.observation_deadline_seconds,
            settings.model_attempt_timeout_seconds,
            settings.logical_timeout_seconds
        )),
        Line::from(format!(
            "{}: {} / {} / {} / {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.limits"),
            settings.background_token_cap_24h,
            settings.result_retention_days,
            yes_no(app, settings.show_raw_output),
            display_or_dash(&settings.https_proxy)
        )),
        Line::from(format!(
            "{}: head≤15 / observe≤90 / model≤300 / logical≤720 / starts≤5 / tokens≤500000",
            crate::i18n::t(app, "app.pi_scan.wizard.review.compiled")
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.wizard.review.binding"),
            binding
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.review.transaction")),
        Line::from(crate::i18n::t(app, "app.pi_scan.wizard.review.advisory")),
    ]
}

/// Build transactional activation progress, success, or actionable failure content.
fn activate_lines(app: &AppState, wizard: &PiScanSetupWizardState) -> Vec<Line<'static>> {
    let (status_key, detail, color) = match &wizard.apply_status {
        PiScanSetupApplyStatus::Idle => ("idle", String::new(), theme().text),
        PiScanSetupApplyStatus::Validating => ("validating", String::new(), theme().yellow),
        PiScanSetupApplyStatus::Activating => ("activating", String::new(), theme().yellow),
        PiScanSetupApplyStatus::Persisting => ("persisting", String::new(), theme().yellow),
        PiScanSetupApplyStatus::Complete => ("complete", String::new(), theme().green),
        PiScanSetupApplyStatus::Failed(reason) => ("failed", reason.clone(), theme().red),
    };
    let mut lines = vec![Line::from(Span::styled(
        crate::i18n::t(app, &format!("app.pi_scan.wizard.activate.{status_key}")),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ))];
    if !detail.is_empty() {
        lines.push(Line::from(detail));
        lines.push(Line::from(crate::i18n::t(
            app,
            "app.pi_scan.wizard.activate.retry_guidance",
        )));
    }
    lines.push(Line::from(crate::i18n::t(
        app,
        "app.pi_scan.wizard.activate.previous_authoritative",
    )));
    lines.push(Line::from(crate::i18n::t(
        app,
        "app.pi_scan.wizard.activate.no_restart",
    )));
    lines.push(Line::from(crate::i18n::t(
        app,
        "app.pi_scan.wizard.activate.advisory",
    )));
    lines
}

/// Render one focused page-local line.
fn focused_line(
    _app: &AppState,
    wizard: &PiScanSetupWizardState,
    index: usize,
    text: String,
) -> Line<'static> {
    let th = theme();
    let style = if wizard.focus == index {
        Style::default()
            .fg(th.base)
            .bg(th.sapphire)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    };
    Line::from(Span::styled(text, style))
}

/// Localize state-owned message keys while preserving bounded dynamic controller details.
fn localize_wizard_message(app: &AppState, message: &str) -> String {
    if message.starts_with("app.pi_scan.wizard.") {
        crate::i18n::t(app, message)
    } else {
        message.to_string()
    }
}

/// Render one independent focused yes/no control.
fn focused_toggle(
    app: &AppState,
    wizard: &PiScanSetupWizardState,
    index: usize,
    key: &str,
    value: bool,
) -> Line<'static> {
    focused_line(
        app,
        wizard,
        index,
        format!(
            "[Space] {}: {}",
            crate::i18n::t(app, key),
            yes_no(app, value)
        ),
    )
}

/// Return the localized title for one wizard step.
fn step_title(app: &AppState, step: PiScanSetupStep) -> String {
    let key = match step {
        PiScanSetupStep::Welcome => "welcome",
        PiScanSetupStep::PiReadiness => "readiness",
        PiScanSetupStep::Route => "route",
        PiScanSetupStep::PricingPrivacy => "pricing",
        PiScanSetupStep::OptionalBehavior => "optional",
        PiScanSetupStep::Review => "review",
        PiScanSetupStep::Activate => "activate",
    };
    crate::i18n::t(app, &format!("app.pi_scan.wizard.steps.{key}"))
}

/// Render a localized boolean decision.
fn yes_no(app: &AppState, value: bool) -> String {
    crate::i18n::t(
        app,
        if value {
            "app.pi_scan.wizard.common.on"
        } else {
            "app.pi_scan.wizard.common.off"
        },
    )
}

/// Display an empty optional value without suggesting an implicit choice.
fn display_or_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::pi_scan::PiScanConsentState;
    use ratatui::{Terminal, backend::TestBackend};

    /// Render each wizard page at narrow supported dimensions without panic.
    #[test]
    fn all_wizard_steps_render_at_narrow_dimensions() {
        for step in PiScanSetupStep::all() {
            for (width, height) in [(36, 12), (20, 7)] {
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                let mut app = AppState::default();
                let mut wizard = PiScanSetupWizardState::open(
                    app.pi_scan.settings.clone(),
                    PiScanConsentState::default(),
                    true,
                );
                wizard.step = step;
                app.pi_scan.wizard = Some(wizard);
                terminal
                    .draw(|frame| render(frame, &mut app, frame.area()))
                    .expect("wizard render");
                let buffer = terminal.backend().buffer();
                let rendered = buffer
                    .content
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>();
                assert!(
                    rendered.contains('◆'),
                    "step {step:?} at {width}x{height} omitted current-step progress: {rendered:?}"
                );
            }
        }
    }

    /// Rendered footer and body controls must expose deterministic mouse seams.
    #[test]
    fn render_records_footer_and_body_hit_targets() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::default();
        let mut wizard = PiScanSetupWizardState::open(
            app.pi_scan.settings.clone(),
            PiScanConsentState::default(),
            true,
        );
        wizard.step = PiScanSetupStep::OptionalBehavior;
        app.pi_scan.wizard = Some(wizard);
        terminal
            .draw(|frame| render(frame, &mut app, frame.area()))
            .expect("wizard render");
        let hit_rects = &app.pi_scan.wizard.as_ref().expect("wizard").hit_rects;
        let first_control = hit_rects
            .iter()
            .find(|rect| matches!(rect.target, PiScanSetupHitTarget::Control(0)))
            .expect("first optional control hit seam");
        let last_control = hit_rects
            .iter()
            .find(|rect| matches!(rect.target, PiScanSetupHitTarget::Control(7)))
            .expect("last optional control hit seam");
        assert_eq!(first_control.y, 3);
        assert_eq!(last_control.y, 10);
        assert!(
            hit_rects
                .iter()
                .any(|rect| { matches!(rect.target, PiScanSetupHitTarget::Cancel) })
        );
        assert!(
            hit_rects
                .iter()
                .any(|rect| { matches!(rect.target, PiScanSetupHitTarget::Next) })
        );
    }
}
