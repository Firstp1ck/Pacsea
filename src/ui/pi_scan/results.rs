//! Validated result list page.

use crate::state::AppState;
use crate::theme::theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Render only strictly validated typed advisory results.
pub(super) fn render(f: &mut Frame, app: &AppState, area: Rect) {
    let th = theme();
    let mut lines = vec![Line::from(crate::i18n::t(
        app,
        "app.pi_scan.results.advisory",
    ))];
    if app.pi_scan.results.is_empty() {
        lines.push(Line::from(crate::i18n::t(app, "app.pi_scan.results.empty")));
    }
    for (index, result) in app.pi_scan.results.iter().enumerate() {
        let style = if index == app.pi_scan.selected {
            Style::default()
                .fg(th.sapphire)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.text)
        };
        let stale_label = if result.stale {
            crate::i18n::t(app, "app.pi_scan.results.stale")
        } else {
            String::new()
        };
        lines.push(Line::from(Span::styled(
            format!(
                "{} @ {} — {} {stale_label}",
                result.validated.identity.package_base,
                result.validated.identity.commit_oid,
                result.completion_wording()
            ),
            style,
        )));
    }
    super::body(f, app, area, "app.pi_scan.tabs.results", lines);
}
