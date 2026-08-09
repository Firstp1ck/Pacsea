//! Selected validated-result detail page.

use crate::state::AppState;
use ratatui::{Frame, layout::Rect, text::Line};

/// Render coverage, limitations, findings, disagreements, and acknowledgements.
pub(super) fn render(f: &mut Frame, app: &AppState, area: Rect) {
    let Some(result) = app.pi_scan.selected_result() else {
        super::body(
            f,
            app,
            area,
            "app.pi_scan.tabs.details",
            vec![Line::from(crate::i18n::t(app, "app.pi_scan.results.empty"))],
        );
        return;
    };
    let mut lines = vec![
        Line::from(result.completion_wording()),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.details.coverage"),
            crate::i18n::t(
                app,
                match result.validated.coverage {
                    crate::logic::pi_scan::result::Coverage::Complete =>
                        "app.pi_scan.coverage.complete",
                    crate::logic::pi_scan::result::Coverage::Incomplete =>
                        "app.pi_scan.coverage.incomplete",
                },
            ),
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.details.stale"),
            result.stale
        )),
        Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.details.acknowledged"),
            app.pi_scan.selected_result_acknowledged()
        )),
        Line::from(crate::i18n::t(app, "app.pi_scan.details.ack_keys")),
    ];
    for limitation in &result.validated.limitations {
        lines.push(Line::from(format!("! {limitation}")));
    }
    for finding in &result.validated.findings {
        lines.push(Line::from(format!(
            "[{}] {}:{}",
            finding.severity.as_str(),
            finding.snapshot,
            finding.path
        )));
        lines.push(Line::from(format!("  {}", finding.evidence)));
        if finding.disagreement {
            lines.push(Line::from(crate::i18n::t(
                app,
                "app.pi_scan.details.disagreement",
            )));
        }
    }
    if app.pi_scan.settings.show_raw_output {
        lines.push(Line::from(crate::i18n::t(app, "app.pi_scan.details.raw")));
        lines.extend(
            result
                .canonical_raw()
                .lines()
                .map(|line| Line::from(line.to_string())),
        );
    }
    super::body(f, app, area, "app.pi_scan.tabs.details", lines);
}
