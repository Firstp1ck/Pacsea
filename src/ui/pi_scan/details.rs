//! Selected validated-result detail page.

use crate::state::AppState;
use ratatui::{Frame, layout::Rect, text::Line};

/// Render coverage, limitations, findings, disagreements, and acknowledgements.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let Some(result) = app.pi_scan.selected_result() else {
        app.pi_scan.view_scroll.details = 0;
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
    if app.pi_scan.settings.show_raw_output || app.pi_scan.show_raw_output {
        lines.push(Line::from(crate::i18n::t(app, "app.pi_scan.details.raw")));
        lines.extend(
            result
                .canonical_raw()
                .lines()
                .map(|line| Line::from(line.to_string())),
        );
    }
    let scroll = super::clamp_line_scroll(app.pi_scan.view_scroll.details, &lines, area);
    app.pi_scan.view_scroll.details = scroll;
    app.pi_scan.detail_scroll = scroll;
    super::body_scrolled(f, app, area, "app.pi_scan.tabs.details", lines, scroll);
}
