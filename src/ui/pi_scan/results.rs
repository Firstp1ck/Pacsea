//! Validated result list page.

use crate::state::AppState;
use crate::state::pi_scan_ui::PiScanListHitRect;
use crate::theme::theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Render only strictly validated typed advisory results.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let capacity = usize::from(area.height.saturating_sub(3)).max(1);
    let offset = visible_offset(
        app.pi_scan.view_scroll.results,
        app.pi_scan.selected_result,
        app.pi_scan.results.len(),
        capacity,
    );
    app.pi_scan.view_scroll.results = offset;
    let mut lines = vec![Line::from(crate::i18n::t(
        app,
        "app.pi_scan.results.advisory",
    ))];
    if app.pi_scan.results.is_empty() {
        lines.push(Line::from(crate::i18n::t(app, "app.pi_scan.results.empty")));
    }
    let mut rects = Vec::new();
    for (visible_index, (index, result)) in app
        .pi_scan
        .results
        .iter()
        .enumerate()
        .skip(offset)
        .take(capacity)
        .enumerate()
    {
        let style = if index == app.pi_scan.selected_result {
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
        rects.push(PiScanListHitRect {
            index,
            x: area.x.saturating_add(1),
            y: area
                .y
                .saturating_add(2)
                .saturating_add(u16::try_from(visible_index).unwrap_or(u16::MAX)),
            width: area.width.saturating_sub(2),
            height: 1,
        });
    }
    app.pi_scan.set_result_row_rects(rects);
    super::body(f, app, area, "app.pi_scan.tabs.results", lines);
}

/// Keep a selected result visible while clamping to the current list capacity.
fn visible_offset(offset: usize, selected: usize, len: usize, capacity: usize) -> usize {
    let maximum = len.saturating_sub(capacity);
    let mut visible = offset.min(maximum);
    if selected < visible {
        visible = selected;
    } else if selected >= visible.saturating_add(capacity) {
        visible = selected.saturating_add(1).saturating_sub(capacity);
    }
    visible.min(maximum)
}
