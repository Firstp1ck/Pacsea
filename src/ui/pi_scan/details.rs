//! Selected validated-result detail page.

use crate::logic::pi_scan::result::{Coverage, MergedScanResult, Severity};
use crate::state::pi_scan_ui::PiScanActionHitRect;
use crate::state::{AppState, PiScanDisplayResult};
use crate::theme::theme;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

/// Height reserved for the fixed continuation action row.
const CONTINUE_ACTION_HEIGHT: u16 = 1;

/// Render every validated result as a package-labeled expandable report.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    app.pi_scan.set_details_continue_action_rect(None);
    app.pi_scan.details_header_rects.clear();
    if app.pi_scan.results.is_empty() {
        app.pi_scan.view_scroll.details = 0;
        super::body(
            f,
            app,
            area,
            "app.pi_scan.tabs.details",
            vec![Line::from(crate::i18n::t(app, "app.pi_scan.results.empty"))],
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(CONTINUE_ACTION_HEIGHT),
        ])
        .split(area);
    let results = app.pi_scan.results.clone();
    let selected = app.pi_scan.selected_result;
    let show_raw = app.pi_scan.show_raw_output;
    let selected_acknowledged = app.pi_scan.selected_result_acknowledged();
    let mut lines = vec![Line::from(crate::i18n::t(
        app,
        "app.pi_scan.details.package_list_hint",
    ))];
    let mut header_positions = Vec::new();
    for (index, result) in results.iter().enumerate() {
        lines.push(Line::from(""));
        header_positions.push((index, lines.len()));
        let expanded = app.pi_scan.is_result_expanded(index);
        push_header(&mut lines, app, result, index == selected, expanded);
        if expanded {
            push_result_content(
                &mut lines,
                app,
                result,
                index == selected,
                selected_acknowledged,
                show_raw,
            );
        }
    }

    let scroll = super::clamp_line_scroll(app.pi_scan.view_scroll.details, &lines, chunks[0]);
    app.pi_scan.view_scroll.details = scroll;
    app.pi_scan.detail_scroll = scroll;
    app.pi_scan.details_header_rects =
        visible_header_rects(&header_positions, &lines, scroll, chunks[0]);
    let title = details_breadcrumb(app);
    super::body_with_title(f, chunks[0], &title, lines, scroll);
    render_continue_action(f, app, chunks[1]);
}

/// What: Build the `Results › Report: package` breadcrumb for the report view.
///
/// Inputs:
/// - `app`: Workspace state with the selected result.
///
/// Output:
/// - Localized breadcrumb naming the parent page and selected package.
///
/// Details:
/// - Tells the user where they are and where Esc returns to.
fn details_breadcrumb(app: &AppState) -> String {
    let parent = crate::i18n::t(app, "app.pi_scan.tabs.results");
    app.pi_scan.selected_result().map_or_else(
        || {
            format!(
                "{parent} › {}",
                crate::i18n::t(app, "app.pi_scan.tabs.details")
            )
        },
        |result| {
            format!(
                "{parent} › {}",
                crate::i18n::t_fmt1(
                    app,
                    "app.pi_scan.details.breadcrumb",
                    &result.validated.identity.package_base
                )
            )
        },
    )
}

/// What: Record visible wrapped report-header cells for click expansion.
///
/// Inputs: Header indices/line offsets, rendered lines, visual scroll, and report area.
///
/// Output: Hit rectangles clipped to the visible report body.
///
/// Details: Uses the same word-wrap row count as Details scrolling, including multi-row headers.
fn visible_header_rects(
    headers: &[(usize, usize)],
    lines: &[Line<'static>],
    scroll: u16,
    area: Rect,
) -> Vec<crate::state::pi_scan_ui::PiScanListHitRect> {
    let width = usize::from(area.width.saturating_sub(2).max(1));
    let bottom = area.y.saturating_add(area.height.saturating_sub(1));
    headers
        .iter()
        .filter_map(|&(index, line)| {
            let visual_top: usize = lines
                .iter()
                .take(line)
                .map(|row| super::wrapped_line_count(row, width))
                .sum();
            let height = super::wrapped_line_count(&lines[line], width);
            let scroll = usize::from(scroll);
            if visual_top.saturating_add(height) <= scroll {
                return None;
            }
            let y = area.y.saturating_add(1).saturating_add(
                u16::try_from(visual_top.saturating_sub(scroll)).unwrap_or(u16::MAX),
            );
            if y >= bottom {
                return None;
            }
            let visible_height = height.saturating_sub(scroll.saturating_sub(visual_top));
            Some(crate::state::pi_scan_ui::PiScanListHitRect {
                index,
                x: area.x.saturating_add(1),
                y,
                width: area.width.saturating_sub(2),
                height: u16::try_from(visible_height)
                    .unwrap_or(u16::MAX)
                    .min(bottom.saturating_sub(y)),
            })
        })
        .collect()
}

/// What: Render and record the fixed Details continuation action.
///
/// Inputs:
/// - `f`: Target terminal frame.
/// - `app`: Localization and state-owned mouse rectangle.
/// - `area`: Fixed row reserved below the scrollable report.
///
/// Output:
/// - Draws the localized keyboard/button action and records its clickable bounds.
///
/// Details:
/// - The row remains outside Details scrolling, and narrow layouts retain the leading `c` key cue.
fn render_continue_action(f: &mut Frame, app: &mut AppState, area: Rect) {
    let label = crate::i18n::t(app, "app.pi_scan.continuation.action");
    let text = format!("[c] {label}");
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[c] ",
                Style::default()
                    .fg(theme().crust)
                    .bg(theme().sapphire)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                label,
                Style::default()
                    .fg(theme().sapphire)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(theme().surface1)),
        area,
    );
    if area.width == 0 || area.height == 0 {
        return;
    }
    let text_width = u16::try_from(UnicodeWidthStr::width(text.as_str())).unwrap_or(u16::MAX);
    app.pi_scan
        .set_details_continue_action_rect(Some(PiScanActionHitRect {
            x: area.x,
            y: area.y,
            width: area.width.min(text_width),
            height: 1,
        }));
}

/// What: Add one compact package header with an expansion marker.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localized labels.
/// - `result`: Validated result supplying the package name.
/// - `selected`: Whether this package owns selected-result actions.
/// - `expanded`: Whether its content is visible.
///
/// Output:
/// - One styled, package-labeled header appended to `lines`.
///
/// Details:
/// - The marker carries expansion state so the label does not repeat technical UI state.
fn push_header(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    result: &PiScanDisplayResult,
    selected: bool,
    expanded: bool,
) {
    let marker = if expanded { "▾" } else { "▸" };
    let selected_label = selected.then(|| {
        format!(
            "  [{}]",
            crate::i18n::t(app, "app.pi_scan.details.selected")
        )
    });
    let mut spans = vec![Span::styled(
        format!("{marker} {}", result.validated.identity.package_base),
        Style::default()
            .fg(if selected {
                theme().sapphire
            } else {
                theme().text
            })
            .add_modifier(Modifier::BOLD),
    )];
    if let Some(label) = selected_label {
        spans.push(Span::styled(label, Style::default().fg(theme().overlay1)));
    }
    lines.push(Line::from(spans));
}

/// What: Add a human-readable report for one expanded package.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localized labels.
/// - `result`: Validated result whose content is being rendered.
/// - `selected`: Whether acknowledgement and continuation belong to this section.
/// - `selected_acknowledged`: Current selected-result acknowledgement status.
/// - `show_raw`: Effective technical-output visibility.
///
/// Output:
/// - Summary, limitations, findings, and optional technical data appended to `lines`.
///
/// Details:
/// - The readable report preserves the validated meaning while exact source messages remain in
///   the technical section.
fn push_result_content(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    result: &PiScanDisplayResult,
    selected: bool,
    selected_acknowledged: bool,
    show_raw: bool,
) {
    push_section_heading(lines, app, "app.pi_scan.details.summary");
    push_summary(lines, app, result, selected, selected_acknowledged);

    let limitations = humanized_limitations(app, &result.validated.limitations);
    if !limitations.is_empty() {
        lines.push(Line::from(""));
        push_counted_heading(
            lines,
            app,
            "app.pi_scan.details.limitations",
            limitations.len(),
        );
        for limitation in limitations {
            lines.push(Line::from(vec![
                Span::styled("    • ", Style::default().fg(theme().yellow)),
                Span::raw(limitation),
            ]));
        }
    }

    if !result.validated.findings.is_empty() {
        lines.push(Line::from(""));
        push_counted_heading(
            lines,
            app,
            "app.pi_scan.details.findings",
            result.validated.findings.len(),
        );
        push_findings(lines, app, &result.validated);
    }

    lines.push(Line::from(""));
    push_technical_details(lines, app, result, show_raw);
}

/// What: Add the compact review summary for one package.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localized labels.
/// - `result`: Result supplying status, coverage, and identity state.
/// - `selected`: Whether this package owns the next-step action.
/// - `acknowledged`: Whether the selected result may continue.
///
/// Output:
/// - Readable labeled summary rows appended to `lines`.
///
/// Details:
/// - Status wording avoids unsupported safety claims and distinguishes incomplete coverage.
fn push_summary(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    result: &PiScanDisplayResult,
    selected: bool,
    acknowledged: bool,
) {
    let result_style = result
        .validated
        .highest_severity()
        .map_or_else(|| Style::default().fg(theme().green), severity_style);
    push_labeled_line(
        lines,
        app,
        "app.pi_scan.details.result",
        result_summary(app, &result.validated),
        result_style,
    );
    let (coverage_key, coverage_style) = match result.validated.coverage {
        Coverage::Complete => (
            "app.pi_scan.details.coverage_complete",
            Style::default().fg(theme().green),
        ),
        Coverage::Incomplete => (
            "app.pi_scan.details.coverage_incomplete",
            Style::default().fg(theme().yellow),
        ),
    };
    push_labeled_line(
        lines,
        app,
        "app.pi_scan.details.coverage",
        crate::i18n::t(app, coverage_key),
        coverage_style,
    );
    let identity_key = if result.stale {
        "app.pi_scan.details.identity_changed"
    } else if super::result_has_newer_version(app, result) {
        "app.pi_scan.details.identity_newer_version"
    } else {
        "app.pi_scan.details.identity_current"
    };
    let identity_style = if result.stale || super::result_has_newer_version(app, result) {
        Style::default().fg(theme().yellow)
    } else {
        Style::default().fg(theme().green)
    };
    push_labeled_line(
        lines,
        app,
        "app.pi_scan.details.identity",
        crate::i18n::t(app, identity_key),
        identity_style,
    );
    if selected {
        let action_key = if acknowledged {
            "app.pi_scan.details.ready_to_continue"
        } else {
            "app.pi_scan.details.ack_required"
        };
        push_labeled_line(
            lines,
            app,
            "app.pi_scan.details.next_step",
            crate::i18n::t(app, action_key),
            Style::default().fg(if acknowledged {
                theme().sapphire
            } else {
                theme().yellow
            }),
        );
    }
}

/// What: Produce plain result wording from validated coverage and finding counts.
///
/// Inputs:
/// - `app`: Application state used for localized labels.
/// - `result`: Validated result supplying coverage and findings.
///
/// Output:
/// - A concise sentence that does not make a package-safety claim.
///
/// Details:
/// - Singular and plural finding wording use separate translation keys.
fn result_summary(app: &AppState, result: &MergedScanResult) -> String {
    match (result.findings.len(), result.coverage) {
        (0, Coverage::Complete) => crate::i18n::t(app, "app.pi_scan.details.no_findings_complete"),
        (0, Coverage::Incomplete) => {
            crate::i18n::t(app, "app.pi_scan.details.no_findings_incomplete")
        }
        (1, _) => format!(
            "1 {}",
            crate::i18n::t(app, "app.pi_scan.details.one_finding")
        ),
        (count, _) => format!(
            "{count} {}",
            crate::i18n::t(app, "app.pi_scan.details.many_findings")
        ),
    }
}

/// What: Append exact technical data or a compact hidden-state hint.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localized labels.
/// - `result`: Result supplying canonical technical data.
/// - `show_raw`: Whether exact data is currently visible.
///
/// Output:
/// - A discoverable technical section appended to `lines`.
///
/// Details:
/// - Exact validated data remains available through the existing `t` toggle.
fn push_technical_details(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    result: &PiScanDisplayResult,
    show_raw: bool,
) {
    let state_key = if show_raw {
        "app.pi_scan.details.technical_visible"
    } else {
        "app.pi_scan.details.technical_hidden"
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}", crate::i18n::t(app, "app.pi_scan.details.technical")),
            Style::default()
                .fg(theme().mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  [{}]", crate::i18n::t(app, state_key)),
            Style::default().fg(theme().overlay1),
        ),
    ]));
    if show_raw {
        lines.extend(result.canonical_raw().lines().map(|line| {
            Line::from(Span::styled(
                format!("    {line}"),
                Style::default().fg(theme().subtext0),
            ))
        }));
    } else {
        lines.push(Line::from(Span::styled(
            format!(
                "    {}",
                crate::i18n::t(app, "app.pi_scan.details.technical_hint")
            ),
            Style::default().fg(theme().subtext0),
        )));
    }
}

/// What: Convert validated limitation messages into concise user-facing explanations.
///
/// Inputs:
/// - `app`: Application state used for localized explanations.
/// - `limitations`: Exact validated limitation messages.
///
/// Output:
/// - Deduplicated readable explanations in source order.
///
/// Details:
/// - Unrecognized messages remain unchanged, and every original remains available in technical data.
fn humanized_limitations(app: &AppState, limitations: &[String]) -> Vec<String> {
    let mut readable = Vec::new();
    for limitation in limitations {
        let explanation = humanize_limitation(app, limitation);
        if !readable.contains(&explanation) {
            readable.push(explanation);
        }
    }
    readable
}

/// What: Rewrite one known scanner limitation without dropping its useful subject.
///
/// Inputs:
/// - `app`: Application state used for localized explanations.
/// - `limitation`: Exact validated limitation text.
///
/// Output:
/// - A plain-language explanation, or the original text for an unknown pattern.
///
/// Details:
/// - Pattern matching is deliberately narrow so new validation messages are never misrepresented.
fn humanize_limitation(app: &AppState, limitation: &str) -> String {
    if limitation.contains("snapshot tools were unavailable") {
        return crate::i18n::t(app, "app.pi_scan.details.limit_tools_unavailable");
    }
    if limitation.contains("source snapshot is empty") {
        return crate::i18n::t(app, "app.pi_scan.details.limit_no_sources");
    }
    if limitation.contains("relative URL without a base") {
        return limitation_subject(limitation).map_or_else(
            || crate::i18n::t(app, "app.pi_scan.details.limit_incomplete_address"),
            |subject| {
                format!(
                    "`{subject}` {}",
                    crate::i18n::t(app, "app.pi_scan.details.limit_incomplete_address_subject")
                )
            },
        );
    }
    if limitation.contains("archive entry-count limit exceeded")
        || limitation.contains("exceeded the archive entry-count limit")
    {
        return limitation_subject(limitation).map_or_else(
            || crate::i18n::t(app, "app.pi_scan.details.limit_archive_too_large"),
            |subject| {
                format!(
                    "`{subject}` {}",
                    crate::i18n::t(app, "app.pi_scan.details.limit_archive_too_large_subject")
                )
            },
        );
    }
    if limitation.contains("incomplete archive entry")
        || limitation.contains("archive special or unknown entry")
    {
        return archive_entry_name(limitation).map_or_else(
            || crate::i18n::t(app, "app.pi_scan.details.limit_unsupported_entry"),
            |entry| {
                format!(
                    "{} `{entry}`.",
                    crate::i18n::t(app, "app.pi_scan.details.limit_unsupported_entry_named")
                )
            },
        );
    }
    limitation.to_string()
}

/// What: Extract a source or archive subject from a validated limitation.
///
/// Inputs:
/// - `limitation`: Exact limitation text.
///
/// Output:
/// - The first backticked value or source token when present.
///
/// Details:
/// - Returned values borrow from the validated input and are used only for display.
fn limitation_subject(limitation: &str) -> Option<&str> {
    first_backticked(limitation).or_else(|| {
        limitation
            .strip_prefix("source ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(|value| value.trim_matches('`'))
    })
}

/// What: Extract the unsupported archive entry named by a known limitation.
///
/// Inputs:
/// - `limitation`: Exact limitation text.
///
/// Output:
/// - A bounded entry name when the known message includes one.
///
/// Details:
/// - Handles both backticked validator messages and the older colon-delimited wording.
fn archive_entry_name(limitation: &str) -> Option<&str> {
    first_backticked(limitation).or_else(|| {
        limitation
            .strip_prefix("The recipe tree contains an incomplete archive entry: ")
            .map(|entry| entry.trim_end_matches('.'))
    })
}

/// What: Return the first value enclosed in backticks.
///
/// Inputs:
/// - `value`: Text that may contain a backticked subject.
///
/// Output:
/// - The enclosed value, or `None` when delimiters are incomplete.
///
/// Details:
/// - Empty values are rejected.
fn first_backticked(value: &str) -> Option<&str> {
    let start = value.find('`')?.saturating_add(1);
    let end = start.saturating_add(value.get(start..)?.find('`')?);
    let subject = value.get(start..end)?;
    (!subject.is_empty()).then_some(subject)
}

/// What: Add a localized section heading.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localization.
/// - `key`: Translation key for the heading.
///
/// Output:
/// - One indented, styled heading appended to `lines`.
///
/// Details:
/// - Shared styling keeps every report section visually consistent.
fn push_section_heading(lines: &mut Vec<Line<'static>>, app: &AppState, key: &str) {
    lines.push(Line::from(Span::styled(
        format!("  {}", crate::i18n::t(app, key)),
        Style::default()
            .fg(theme().mauve)
            .add_modifier(Modifier::BOLD),
    )));
}

/// What: Add a localized section heading with its item count.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localization.
/// - `key`: Translation key for the heading.
/// - `count`: Number of items shown below the heading.
///
/// Output:
/// - One counted, styled heading appended to `lines`.
///
/// Details:
/// - The count lets users assess the report before reading every entry.
fn push_counted_heading(lines: &mut Vec<Line<'static>>, app: &AppState, key: &str, count: usize) {
    lines.push(Line::from(Span::styled(
        format!("  {} ({count})", crate::i18n::t(app, key)),
        Style::default()
            .fg(theme().mauve)
            .add_modifier(Modifier::BOLD),
    )));
}

/// What: Add one summary row with separate label and value styling.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localization.
/// - `label_key`: Translation key for the row label.
/// - `value`: Human-readable row value.
/// - `value_style`: Semantic status style for the value.
///
/// Output:
/// - One indented summary row appended to `lines`.
///
/// Details:
/// - A fixed indentation and subdued labels keep the summary easy to scan.
fn push_labeled_line(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    label_key: &str,
    value: String,
    value_style: Style,
) {
    lines.push(Line::from(vec![
        Span::styled(
            format!("    {}: ", crate::i18n::t(app, label_key)),
            Style::default().fg(theme().overlay1),
        ),
        Span::styled(value, value_style),
    ]));
}

/// What: Append each validated finding as a numbered, labeled report item.
///
/// Inputs:
/// - `lines`: Destination detail lines.
/// - `app`: Application state used for localized labels.
/// - `result`: Validated result containing findings.
///
/// Output:
/// - Finding title, location, evidence, and disagreement notes appended to `lines`.
///
/// Details:
/// - Only validated typed fields are rendered; original model output is never displayed.
fn push_findings(lines: &mut Vec<Line<'static>>, app: &AppState, result: &MergedScanResult) {
    for (index, finding) in result.findings.iter().enumerate() {
        let title = finding
            .assessments
            .first()
            .map(|assessment| assessment.title.trim())
            .filter(|title| !title.is_empty())
            .map_or_else(
                || crate::i18n::t(app, "app.pi_scan.details.untitled_finding"),
                str::to_string,
            );
        lines.push(Line::from(vec![
            Span::raw(format!("    {}. ", index.saturating_add(1))),
            Span::styled(
                format!("[{}] ", finding.severity.as_str().to_uppercase()),
                severity_style(finding.severity).add_modifier(Modifier::BOLD),
            ),
            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        ]));
        push_labeled_line(
            lines,
            app,
            "app.pi_scan.details.location",
            format!("{} / {}", finding.snapshot, finding.path),
            Style::default().fg(theme().text),
        );
        push_labeled_line(
            lines,
            app,
            "app.pi_scan.details.evidence",
            finding.evidence.clone(),
            Style::default().fg(theme().text),
        );
        if finding.disagreement {
            push_labeled_line(
                lines,
                app,
                "app.pi_scan.details.note",
                crate::i18n::t(app, "app.pi_scan.details.disagreement"),
                Style::default().fg(theme().yellow),
            );
        }
    }
}

/// What: Select the semantic color used for a validated severity.
///
/// Inputs:
/// - `severity`: Typed validated finding severity.
///
/// Output:
/// - Theme-aware foreground style.
///
/// Details:
/// - High and critical findings share the strongest warning color.
fn severity_style(severity: Severity) -> Style {
    let color = match severity {
        Severity::Info => theme().subtext0,
        Severity::Low => theme().sapphire,
        Severity::Medium => theme().yellow,
        Severity::High | Severity::Critical => theme().red,
    };
    Style::default().fg(color)
}
