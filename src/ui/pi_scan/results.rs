//! Validated result list page.

use crate::logic::pi_scan::result::{Coverage, Severity};
use crate::state::AppState;
use crate::state::pi_scan_ui::{PiScanDisplayResult, PiScanListHitRect};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
};

use super::SemanticTone;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Number of normal-size non-result lines before the first rendered result row.
const FULL_RESULT_LIST_PREFIX_LINES: u16 = 3;

/// Render only strictly validated typed advisory results.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let prefix_lines = list_prefix_lines(area);
    let capacity = usize::from(area.height.saturating_sub(2).saturating_sub(prefix_lines)).max(1);
    let offset = visible_offset(
        app.pi_scan.view_scroll.results,
        app.pi_scan.selected_result,
        app.pi_scan.results.len(),
        capacity,
    );
    app.pi_scan.view_scroll.results = offset;
    let mut lines = result_prefix(app, prefix_lines);
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
        lines.push(result_line(
            app,
            result,
            index == app.pi_scan.selected_result,
            usize::from(area.width.saturating_sub(2).max(1)),
        ));
        rects.push(PiScanListHitRect {
            index,
            x: area.x.saturating_add(1),
            y: area
                .y
                .saturating_add(1)
                .saturating_add(prefix_lines)
                .saturating_add(u16::try_from(visible_index).unwrap_or(u16::MAX)),
            width: area.width.saturating_sub(2),
            height: 1,
        });
    }
    app.pi_scan.set_result_row_rects(rects);
    super::body(f, app, area, "app.pi_scan.tabs.results", lines);
}

/// Select compact prefix depth while leaving at least one visible list row when possible.
const fn list_prefix_lines(area: Rect) -> u16 {
    let inner_height = area.height.saturating_sub(2);
    if inner_height >= FULL_RESULT_LIST_PREFIX_LINES.saturating_add(1) {
        FULL_RESULT_LIST_PREFIX_LINES
    } else if inner_height >= 2 {
        1
    } else {
        0
    }
}

/// Build the normal advisory/heading prefix or its compact heading-only form.
fn result_prefix(app: &AppState, prefix_lines: u16) -> Vec<Line<'static>> {
    match prefix_lines {
        FULL_RESULT_LIST_PREFIX_LINES => vec![
            Line::from(crate::i18n::t(app, "app.pi_scan.results.advisory")),
            Line::from(String::new()),
            super::section_heading(app, "app.pi_scan.results.list_heading"),
        ],
        1 => vec![super::section_heading(
            app,
            "app.pi_scan.results.list_heading",
        )],
        _ => Vec::new(),
    }
}

/// Build one scan-friendly result row with independent completion, identity, and severity cues.
fn result_line(
    app: &AppState,
    result: &PiScanDisplayResult,
    selected: bool,
    width: usize,
) -> Line<'static> {
    let package_style = if selected {
        super::semantic_style(SemanticTone::Active).add_modifier(Modifier::BOLD)
    } else {
        super::semantic_style(SemanticTone::Normal)
    };
    let identity_key = if result.stale {
        "app.pi_scan.results.stale"
    } else {
        "app.pi_scan.results.current"
    };
    let identity_tone = if result.stale {
        SemanticTone::Error
    } else {
        SemanticTone::Success
    };
    let mut spans = vec![
        Span::styled(if selected { "› " } else { "  " }, package_style),
        Span::styled(
            result.validated.identity.package_base.clone(),
            package_style,
        ),
        Span::raw(" — ["),
        Span::styled(
            localized_coverage(app, result.validated.coverage),
            super::semantic_style(coverage_tone(result.validated.coverage))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("]"),
    ];
    if let Some(severity) = result.validated.highest_severity() {
        spans.extend([
            Span::raw(" ["),
            Span::styled(
                localized_severity(app, severity),
                super::semantic_style(severity_tone(severity)).add_modifier(Modifier::BOLD),
            ),
            Span::raw("]"),
        ]);
    }
    spans.extend([
        Span::raw(" "),
        Span::styled(
            crate::i18n::t(app, identity_key),
            super::semantic_style(identity_tone).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · "),
        Span::styled(
            localized_completion(app, result),
            super::semantic_style(coverage_tone(result.validated.coverage)),
        ),
    ]);
    let commit = format!(
        " · {}: {}",
        crate::i18n::t(app, "app.pi_scan.targets.commit"),
        super::short_identity(&result.validated.identity.commit_oid)
    );
    if spans_width(&spans).saturating_add(commit.width()) <= width {
        spans.push(Span::styled(
            commit,
            super::semantic_style(SemanticTone::Muted),
        ));
    }
    truncate_line(Line::from(spans), width)
}

/// Measure the display width of styled row fragments before optional metadata is appended.
fn spans_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|span| span.content.as_ref().width()).sum()
}

/// Truncate one styled result row to the viewport so it retains exactly one visual line.
fn truncate_line(line: Line<'static>, width: usize) -> Line<'static> {
    let mut remaining = width;
    let mut spans = Vec::new();
    for span in line.spans {
        let mut content = String::new();
        for character in span.content.chars() {
            let character_width = character.width().unwrap_or(0);
            if character_width > remaining {
                break;
            }
            remaining = remaining.saturating_sub(character_width);
            content.push(character);
        }
        let truncated = content.chars().count() < span.content.chars().count();
        spans.push(Span::styled(content, span.style));
        if truncated || remaining == 0 {
            break;
        }
    }
    Line::from(spans)
}

/// Render localized completion wording from validated coverage and finding count.
fn localized_completion(app: &AppState, result: &PiScanDisplayResult) -> String {
    let finding_count = result.validated.findings.len();
    let key = match (result.validated.coverage, finding_count) {
        (Coverage::Complete, 0) => "complete_no_findings",
        (Coverage::Incomplete, 0) => "incomplete_no_findings",
        (_, 1) => "one_finding",
        (_, _) => "many_findings",
    };
    crate::i18n::t_fmt1(
        app,
        &format!("app.pi_scan.results.completion.{key}"),
        finding_count,
    )
}

/// Render typed coverage with an explicit complete or incomplete status word.
fn localized_coverage(app: &AppState, coverage: Coverage) -> String {
    let key = match coverage {
        Coverage::Complete => "complete",
        Coverage::Incomplete => "incomplete",
    };
    crate::i18n::t(app, &format!("app.pi_scan.coverage.{key}"))
}

/// Map validated coverage to complete or incomplete semantic emphasis.
const fn coverage_tone(coverage: Coverage) -> SemanticTone {
    match coverage {
        Coverage::Complete => SemanticTone::Success,
        Coverage::Incomplete => SemanticTone::Warning,
    }
}

/// Map validated finding severity to its accessible semantic emphasis.
const fn severity_tone(severity: Severity) -> SemanticTone {
    match severity {
        Severity::Info => SemanticTone::Muted,
        Severity::Low => SemanticTone::Active,
        Severity::Medium => SemanticTone::Warning,
        Severity::High | Severity::Critical => SemanticTone::Error,
    }
}

/// Render one typed validated severity using the supplied locale key set.
fn localized_severity(app: &AppState, severity: Severity) -> String {
    crate::i18n::t(
        app,
        &format!("app.pi_scan.results.severity.{}", severity.as_str()),
    )
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
