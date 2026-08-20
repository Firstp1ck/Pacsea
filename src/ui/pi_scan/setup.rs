//! Setup, disclosure, and consent page.

use crate::state::{AppState, PiScanAvailability, PiScanReadiness};
use ratatui::{Frame, layout::Rect, text::Line};

/// Render provider/privacy/cost/coverage disclosure and independent consents.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    if app.pi_scan.wizard.is_some() {
        super::wizard::render(f, app, area);
        return;
    }
    render_advanced(f, app, area);
}

/// Preserve the existing advanced setup/details page outside the guided wizard.
fn render_advanced(f: &mut Frame, app: &mut AppState, area: Rect) {
    let pi = &app.pi_scan;
    let setting = &pi.settings;
    let availability = match pi.availability {
        PiScanAvailability::MissingBinary => crate::i18n::t(app, "app.pi_scan.setup.missing_pi"),
        PiScanAvailability::RuntimeDisconnected => {
            crate::i18n::t(app, "app.pi_scan.setup.integration_pending")
        }
        PiScanAvailability::Disabled => crate::i18n::t(app, "app.pi_scan.setup.enable_hint"),
        PiScanAvailability::Unsupported => crate::i18n::t(app, "app.pi_scan.setup.unsupported"),
        PiScanAvailability::RuntimeConnected => crate::i18n::t(app, "app.pi_scan.setup.connected"),
    };
    let readiness = match &pi.readiness {
        PiScanReadiness::Unchecked => crate::i18n::t(app, "app.pi_scan.setup.readiness_unchecked"),
        PiScanReadiness::Warning(warning) => format!(
            "{}: {warning}",
            crate::i18n::t(app, "app.pi_scan.setup.readiness_warning")
        ),
        PiScanReadiness::Confirmed => crate::i18n::t(app, "app.pi_scan.setup.readiness_confirmed"),
    };
    let issues = setting.validation_issues();
    let issue_line = if issues.is_empty() {
        crate::i18n::t(app, "app.pi_scan.setup.config_within_bounds")
    } else {
        issues.join("; ")
    };
    let lines = vec![
        Line::from(format!(
            "[r] {}",
            crate::i18n::t(app, "app.pi_scan.setup.rerun_wizard")
        )),
        Line::from(availability),
        Line::from(format!(
            "{}: {} / {}",
            crate::i18n::t(app, "app.pi_scan.setup.feature_background"),
            setting.enabled,
            setting.background_enabled,
        )),
        Line::from(format!(
            "{}: {} / {}",
            crate::i18n::t(app, "app.pi_scan.setup.provider_model"),
            display_or_unset(&setting.provider),
            display_or_unset(&setting.model)
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.fallback"),
            display_or_unset(&setting.fallback_models)
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.setup.privacy_cost")),
        Line::from(format!(
            "{}: {} · {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.thinking"),
            setting.thinking,
            crate::i18n::t(app, "app.pi_scan.setup.tool_contract"),
            crate::pi_agent::TOOL_CONTRACT_VERSION
        )),
        Line::from(format!(
            "[v] {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.verified_pi"),
            display_or_unset(&pi.verified_pi_version)
        )),
        Line::from(format!(
            "{}: {}/{}",
            crate::i18n::t(app, "app.pi_scan.setup.verified_route"),
            display_or_unset(&pi.verified_provider),
            display_or_unset(&pi.verified_model)
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.wizard.route.advertised"),
            pi.verified_available_models.len()
        )),
        Line::from(format!(
            "{}: {} {} · {}",
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.worst_case"),
            super::format_token_count(pi.verified_reservation.tokens),
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.tokens"),
            super::format_microusd(pi.verified_reservation.cost_microusd)
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.provenance"),
            if pi.verified_pricing_summary.is_empty() {
                "—".to_string()
            } else {
                crate::i18n::t(app, "app.pi_scan.wizard.pricing.provenance_value")
            }
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.pricing_binding"),
            display_or_unset(&pi.verified_pricing_binding)
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.setup.coverage")),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.readiness"),
            readiness
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.config"),
            issue_line
        )),
        effective_compiled_line(app, setting),
        Line::from(format!(
            "[c] {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.disclosure_consent"),
            yes_no(app, pi.disclosure_confirmed)
        )),
        Line::from(format!(
            "[o] {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.observation_consent"),
            yes_no(app, pi.runtime.consent.background_observation)
        )),
        Line::from(format!(
            "[p] {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.paid_consent"),
            yes_no(app, pi.runtime.consent.paid_execution)
        )),
        Line::from(format!(
            "[b] {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.background_paid_consent"),
            yes_no(app, pi.background_paid_execution_confirmed)
        )),
        Line::from(format!(
            "[f] {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.fallback_consent"),
            yes_no(app, pi.fallback_confirmed)
        )),
        Line::from(format!(
            "[w] {}: {}",
            crate::i18n::t(app, "app.pi_scan.setup.warning_consent"),
            yes_no(app, pi.readiness_warning_confirmed)
        )),
    ];
    let scroll = super::clamp_line_scroll(app.pi_scan.view_scroll.setup, &lines, area);
    app.pi_scan.view_scroll.setup = scroll;
    super::body_scrolled(f, app, area, "app.pi_scan.tabs.setup", lines, scroll);
}

/// Render effective settings beside the immutable compiled maxima.
fn effective_compiled_line(
    app: &AppState,
    setting: &crate::theme::PiScanSettings,
) -> Line<'static> {
    Line::from(format!(
        "{}: {}",
        crate::i18n::t(app, "app.pi_scan.setup.effective_compiled"),
        crate::i18n::t_fmt(
            app,
            "app.pi_scan.setup.effective_values",
            &[
                &setting.head_query_timeout_seconds,
                &setting.observation_deadline_seconds,
                &setting.model_attempt_timeout_seconds,
                &setting.logical_timeout_seconds,
                &setting.background_starts_per_hour,
                &setting.background_token_cap_24h,
            ],
        )
    ))
}

/// Display an unset provider/model field without implying auto-selection.
fn display_or_unset(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

/// Render a localized confirmation state.
fn yes_no(app: &AppState, value: bool) -> String {
    crate::i18n::t(
        app,
        if value {
            "app.pi_scan.common.confirmed"
        } else {
            "app.pi_scan.common.not_confirmed"
        },
    )
}
