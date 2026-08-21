//! Setup, disclosure, and consent page.

use crate::state::{AppState, PiScanAvailability, PiScanReadiness};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
};

use super::SemanticTone;

/// Render provider/privacy/cost/coverage disclosure and independent consents.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    if app.pi_scan.wizard.is_some() {
        super::wizard::render(f, app, area);
        return;
    }
    render_advanced(f, app, area);
}

/// Preserve advanced setup controls while presenting them in balanced sections.
fn render_advanced(f: &mut Frame, app: &mut AppState, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "▶ [r] {}",
            crate::i18n::t(app, "app.pi_scan.setup.rerun_wizard")
        ),
        super::semantic_style(SemanticTone::Active),
    ))];
    push_runtime_lines(&mut lines, app);
    push_route_cost_lines(&mut lines, app);
    push_safety_lines(&mut lines, app);
    push_permission_lines(&mut lines, app);
    let scroll = super::clamp_line_scroll(app.pi_scan.view_scroll.setup, &lines, area);
    app.pi_scan.view_scroll.setup = scroll;
    super::body_scrolled(f, app, area, "app.pi_scan.tabs.setup", lines, scroll);
}

/// Append runtime availability, readiness, and bounded-configuration rows.
fn push_runtime_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    let pi = &app.pi_scan;
    let setting = &pi.settings;
    let (availability, availability_tone) = availability_state(app, &pi.availability);
    let (readiness, readiness_tone) = readiness_state(app, &pi.readiness);
    let issues = setting.validation_issues();
    let (configuration, configuration_tone) = if issues.is_empty() {
        (
            crate::i18n::t(app, "app.pi_scan.setup.config_within_bounds"),
            SemanticTone::Success,
        )
    } else {
        (
            issues
                .iter()
                .map(|issue| localize_setting_issue(app, issue))
                .collect::<Vec<_>>()
                .join("; "),
            SemanticTone::Error,
        )
    };
    let feature = format!(
        "{} / {}",
        enabled_disabled(app, setting.enabled),
        enabled_disabled(app, setting.background_enabled)
    );
    let feature_tone = if setting.enabled && setting.background_enabled {
        SemanticTone::Success
    } else {
        SemanticTone::Warning
    };

    push_section_gap(lines, app, "app.pi_scan.setup.sections.runtime");
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.availability"),
        availability,
        availability_tone,
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.feature_background"),
        feature,
        feature_tone,
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.readiness"),
        readiness,
        readiness_tone,
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.config"),
        configuration,
        configuration_tone,
    ));
    lines.push(effective_compiled_line(app, setting));
}

/// Append provider route, verified identity, and worst-case cost rows.
fn push_route_cost_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    let pi = &app.pi_scan;
    let setting = &pi.settings;
    push_section_gap(lines, app, "app.pi_scan.setup.sections.route_cost");
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.provider_model"),
        format!(
            "{} / {}",
            display_or_unset(&setting.provider),
            display_or_unset(&setting.model)
        ),
        configured_tone(&setting.provider, &setting.model),
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.fallback"),
        display_or_unset(&setting.fallback_models),
        SemanticTone::Muted,
    ));
    lines.push(super::labeled_line(
        format!(
            "{} / {}",
            crate::i18n::t(app, "app.pi_scan.setup.thinking"),
            crate::i18n::t(app, "app.pi_scan.setup.tool_contract")
        ),
        format!(
            "{} / {}",
            setting.thinking,
            crate::pi_agent::TOOL_CONTRACT_VERSION
        ),
        SemanticTone::Normal,
    ));
    lines.push(super::labeled_line(
        format!(
            "[v] {}",
            crate::i18n::t(app, "app.pi_scan.setup.verified_pi")
        ),
        display_or_unset(&pi.verified_pi_version),
        present_tone(&pi.verified_pi_version),
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.verified_route"),
        format!(
            "{} / {}",
            display_or_unset(&pi.verified_provider),
            display_or_unset(&pi.verified_model)
        ),
        configured_tone(&pi.verified_provider, &pi.verified_model),
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.wizard.route.advertised"),
        pi.verified_available_models.len().to_string(),
        if pi.verified_available_models.is_empty() {
            SemanticTone::Warning
        } else {
            SemanticTone::Success
        },
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.wizard.pricing.worst_case"),
        format!(
            "{} {} · {}",
            super::format_token_count(pi.verified_reservation.tokens),
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.tokens"),
            super::format_microusd(pi.verified_reservation.cost_microusd)
        ),
        SemanticTone::Normal,
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.wizard.pricing.provenance"),
        if pi.verified_pricing_summary.is_empty() {
            "—".to_string()
        } else {
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.provenance_value")
        },
        if pi.verified_pricing_summary.is_empty() {
            SemanticTone::Warning
        } else {
            SemanticTone::Success
        },
    ));
    lines.push(super::labeled_line(
        crate::i18n::t(app, "app.pi_scan.setup.pricing_binding"),
        display_or_unset(&pi.verified_pricing_binding),
        present_tone(&pi.verified_pricing_binding),
    ));
}

/// Append privacy, cost, and advisory coverage disclosures.
fn push_safety_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    push_section_gap(lines, app, "app.pi_scan.setup.sections.safety");
    lines.push(Line::from(Span::styled(
        format!(
            "  ⚠ {}",
            crate::i18n::t(app, "app.pi_scan.setup.privacy_cost")
        ),
        super::semantic_style(SemanticTone::Warning),
    )));
    lines.push(Line::from(Span::styled(
        format!("  ⚠ {}", crate::i18n::t(app, "app.pi_scan.setup.coverage")),
        super::semantic_style(SemanticTone::Warning),
    )));
}

/// Append all independent consent controls with explicit confirmation wording.
fn push_permission_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    let pi = &app.pi_scan;
    push_section_gap(lines, app, "app.pi_scan.setup.sections.permissions");
    let permissions = [
        (
            "c",
            "app.pi_scan.setup.disclosure_consent",
            pi.disclosure_confirmed,
        ),
        (
            "o",
            "app.pi_scan.setup.observation_consent",
            pi.runtime.consent.background_observation,
        ),
        (
            "p",
            "app.pi_scan.setup.paid_consent",
            pi.runtime.consent.paid_execution,
        ),
        (
            "b",
            "app.pi_scan.setup.background_paid_consent",
            pi.background_paid_execution_confirmed,
        ),
        (
            "f",
            "app.pi_scan.setup.fallback_consent",
            pi.fallback_confirmed,
        ),
        (
            "w",
            "app.pi_scan.setup.warning_consent",
            pi.readiness_warning_confirmed,
        ),
    ];
    for (key, label_key, confirmed) in permissions {
        let (value, tone) = confirmation_state(app, confirmed);
        lines.push(super::labeled_line(
            format!("[{key}] {}", crate::i18n::t(app, label_key)),
            value,
            tone,
        ));
    }
}

/// Insert one deliberate section gap followed by a shared heading.
fn push_section_gap(lines: &mut Vec<Line<'static>>, app: &AppState, key: &str) {
    lines.push(Line::from(""));
    lines.push(super::section_heading(app, key));
}

/// Render effective settings beside the immutable compiled maxima.
fn effective_compiled_line(
    app: &AppState,
    setting: &crate::theme::PiScanSettings,
) -> Line<'static> {
    super::labeled_line(
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
        ),
        SemanticTone::Muted,
    )
}

/// Localize one known settings validation issue while preserving unknown text verbatim.
fn localize_setting_issue(app: &AppState, issue: &str) -> String {
    let (key, arguments): (&str, &[&dyn std::fmt::Display]) = match issue {
        "pi_scan_binary must name the Pi executable" => (
            "app.pi_scan.setup.validation_issue.executable",
            &[&"pi_scan_binary"],
        ),
        "pi_scan_observation_interval_seconds must be at least 900" => (
            "app.pi_scan.setup.validation_issue.at_least",
            &[&"pi_scan_observation_interval_seconds", &900],
        ),
        "pi_scan_head_query_timeout_seconds must be between 1 and 15" => (
            "app.pi_scan.setup.validation_issue.between",
            &[&"pi_scan_head_query_timeout_seconds", &15],
        ),
        "pi_scan_observation_deadline_seconds must be between 1 and 90" => (
            "app.pi_scan.setup.validation_issue.between",
            &[&"pi_scan_observation_deadline_seconds", &90],
        ),
        "pi_scan_model_attempt_timeout_seconds must be between 1 and 300" => (
            "app.pi_scan.setup.validation_issue.between",
            &[&"pi_scan_model_attempt_timeout_seconds", &300],
        ),
        "pi_scan_logical_timeout_seconds must be between 1 and 720" => (
            "app.pi_scan.setup.validation_issue.between",
            &[&"pi_scan_logical_timeout_seconds", &720],
        ),
        "pi_scan_background_starts_per_hour cannot exceed 5" => (
            "app.pi_scan.setup.validation_issue.cannot_exceed",
            &[&"pi_scan_background_starts_per_hour", &5],
        ),
        "pi_scan_background_token_cap_24h cannot exceed 500000" => (
            "app.pi_scan.setup.validation_issue.cannot_exceed",
            &[&"pi_scan_background_token_cap_24h", &500_000],
        ),
        "pi_scan_result_retention_days must be at least 1" => (
            "app.pi_scan.setup.validation_issue.at_least",
            &[&"pi_scan_result_retention_days", &1],
        ),
        "pi_scan_background_cost_cap_24h must be a non-negative decimal" => (
            "app.pi_scan.setup.validation_issue.nonnegative_decimal",
            &[&"pi_scan_background_cost_cap_24h"],
        ),
        "pi_scan_https_proxy must be credential-free HTTPS or empty" => (
            "app.pi_scan.setup.validation_issue.https_proxy",
            &[&"pi_scan_https_proxy"],
        ),
        _ => return issue.to_string(),
    };
    crate::i18n::t_fmt(app, key, arguments)
}

/// Return localized availability wording and its accessible semantic tone.
fn availability_state(app: &AppState, availability: &PiScanAvailability) -> (String, SemanticTone) {
    match availability {
        PiScanAvailability::MissingBinary => (
            crate::i18n::t(app, "app.pi_scan.setup.missing_pi"),
            SemanticTone::Error,
        ),
        PiScanAvailability::RuntimeDisconnected => (
            crate::i18n::t(app, "app.pi_scan.setup.integration_pending"),
            SemanticTone::Error,
        ),
        PiScanAvailability::Disabled => (
            crate::i18n::t(app, "app.pi_scan.setup.enable_hint"),
            SemanticTone::Warning,
        ),
        PiScanAvailability::Unsupported => (
            crate::i18n::t(app, "app.pi_scan.setup.unsupported"),
            SemanticTone::Error,
        ),
        PiScanAvailability::RuntimeConnected => (
            crate::i18n::t(app, "app.pi_scan.setup.connected"),
            SemanticTone::Success,
        ),
    }
}

/// Return localized readiness wording and its accessible semantic tone.
fn readiness_state(app: &AppState, readiness: &PiScanReadiness) -> (String, SemanticTone) {
    match readiness {
        PiScanReadiness::Unchecked => (
            crate::i18n::t(app, "app.pi_scan.setup.readiness_unchecked"),
            SemanticTone::Warning,
        ),
        PiScanReadiness::Warning(warning) => (
            format!(
                "{}: {warning}",
                crate::i18n::t(app, "app.pi_scan.setup.readiness_warning")
            ),
            SemanticTone::Warning,
        ),
        PiScanReadiness::Confirmed => (
            crate::i18n::t(app, "app.pi_scan.setup.readiness_confirmed"),
            SemanticTone::Success,
        ),
    }
}

/// Display an unset provider/model field without implying auto-selection.
fn display_or_unset(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

/// Select success only when both route components are configured.
fn configured_tone(first: &str, second: &str) -> SemanticTone {
    if first.trim().is_empty() || second.trim().is_empty() {
        SemanticTone::Warning
    } else {
        SemanticTone::Success
    }
}

/// Select success only when one verified value is present.
fn present_tone(value: &str) -> SemanticTone {
    if value.trim().is_empty() {
        SemanticTone::Warning
    } else {
        SemanticTone::Success
    }
}

/// Render a localized enabled/disabled state.
fn enabled_disabled(app: &AppState, value: bool) -> String {
    crate::i18n::t(
        app,
        if value {
            "app.pi_scan.common.enabled"
        } else {
            "app.pi_scan.common.disabled"
        },
    )
}

/// Render a localized confirmation state with its semantic tone.
fn confirmation_state(app: &AppState, value: bool) -> (String, SemanticTone) {
    (
        crate::i18n::t(
            app,
            if value {
                "app.pi_scan.common.confirmed"
            } else {
                "app.pi_scan.common.not_confirmed"
            },
        ),
        if value {
            SemanticTone::Success
        } else {
            SemanticTone::Warning
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{localize_setting_issue, render};
    use crate::state::AppState;
    use ratatui::{Terminal, backend::TestBackend};

    /// Advanced Setup exposes all balanced sections at a normal terminal size.
    #[test]
    fn advanced_setup_renders_balanced_sections() {
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::default();
        load_english(&mut app);
        terminal
            .draw(|frame| render(frame, &mut app, frame.area()))
            .expect("advanced setup render");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        for heading in [
            "Runtime",
            "Route and cost",
            "Safety and coverage",
            "Permissions",
        ] {
            assert!(
                rendered.contains(heading),
                "missing {heading:?}: {rendered:?}"
            );
        }
    }

    /// German Advanced Setup localizes finite settings validation issues.
    #[test]
    fn advanced_setup_localizes_german_validation_issue() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::default();
        load_locale(&mut app, "de-DE");
        app.pi_scan.settings.head_query_timeout_seconds = 16;
        terminal
            .draw(|frame| render(frame, &mut app, frame.area()))
            .expect("German advanced setup render");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert!(
            rendered.contains("pi_scan_head_query_timeout_seconds muss zwischen 1 und 15 liegen"),
            "missing localized validation issue: {rendered:?}"
        );
        assert!(!rendered.contains("must be between"));
    }

    /// Every finite settings validation issue has localized German UI wording.
    #[test]
    fn all_known_validation_issues_have_german_ui_wording() {
        let mut app = AppState::default();
        load_locale(&mut app, "de-DE");
        for issue in [
            "pi_scan_binary must name the Pi executable",
            "pi_scan_observation_interval_seconds must be at least 900",
            "pi_scan_head_query_timeout_seconds must be between 1 and 15",
            "pi_scan_observation_deadline_seconds must be between 1 and 90",
            "pi_scan_model_attempt_timeout_seconds must be between 1 and 300",
            "pi_scan_logical_timeout_seconds must be between 1 and 720",
            "pi_scan_background_starts_per_hour cannot exceed 5",
            "pi_scan_background_token_cap_24h cannot exceed 500000",
            "pi_scan_result_retention_days must be at least 1",
            "pi_scan_background_cost_cap_24h must be a non-negative decimal",
            "pi_scan_https_proxy must be credential-free HTTPS or empty",
        ] {
            assert_ne!(localize_setting_issue(&app, issue), issue);
        }
    }

    /// Unknown settings validation text remains visible verbatim.
    #[test]
    fn unknown_validation_issue_remains_verbatim() {
        let mut app = AppState::default();
        load_locale(&mut app, "de-DE");
        let issue = "future validation issue with runtime detail";
        assert_eq!(localize_setting_issue(&app, issue), issue);
    }

    /// Advanced Setup remains renderable at the compact supported dimensions.
    #[test]
    fn advanced_setup_renders_at_narrow_dimensions() {
        let backend = TestBackend::new(20, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::default();
        load_english(&mut app);
        terminal
            .draw(|frame| render(frame, &mut app, frame.area()))
            .expect("narrow advanced setup render");
    }

    /// Load one shipped locale for human-facing render assertions.
    fn load_locale(app: &mut AppState, locale: &str) {
        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
        app.translations =
            crate::i18n::load_locale_file(locale, &locales).expect("requested locale");
        app.translations_fallback =
            crate::i18n::load_locale_file("en-US", &locales).expect("English fallback locale");
    }

    /// Load the shipped English locale for human-facing render assertions.
    fn load_english(app: &mut AppState) {
        load_locale(app, "en-US");
    }
}
