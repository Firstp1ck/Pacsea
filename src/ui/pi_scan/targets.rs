//! Scan target selection page.

use crate::state::pi_scan_ui::PiScanListHitRect;
use crate::state::{AppState, PiScanTargetStatus};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
};

use super::SemanticTone;
use unicode_width::UnicodeWidthChar;

/// Number of normal-size non-target lines before the first rendered target row.
const FULL_TARGET_LIST_PREFIX_LINES: u16 = 3;

/// Render keyboard-selectable package-base targets and dry-run affordance.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let prefix_lines = list_prefix_lines(area);
    let capacity = usize::from(area.height.saturating_sub(2).saturating_sub(prefix_lines)).max(1);
    let offset = visible_offset(
        app.pi_scan.view_scroll.targets,
        app.pi_scan.selected_target,
        app.pi_scan.targets.len(),
        capacity,
    );
    app.pi_scan.view_scroll.targets = offset;
    let mut lines = target_prefix(app, prefix_lines);
    if app.pi_scan.targets.is_empty() {
        lines.push(Line::from(crate::i18n::t(app, "app.pi_scan.targets.empty")));
    }
    let mut rects = Vec::new();
    for (visible_index, (index, target)) in app
        .pi_scan
        .targets
        .iter()
        .enumerate()
        .skip(offset)
        .take(capacity)
        .enumerate()
    {
        let marker = if target.selected { "[x]" } else { "[ ]" };
        let marker_tone = if target.selected {
            SemanticTone::Active
        } else {
            SemanticTone::Muted
        };
        let package_style = if index == app.pi_scan.selected_target {
            super::semantic_style(SemanticTone::Active).add_modifier(Modifier::BOLD)
        } else {
            super::semantic_style(SemanticTone::Normal)
        };
        let mut spans = vec![
            Span::styled(format!("{marker} "), super::semantic_style(marker_tone)),
            Span::styled(target.package_name.clone(), package_style),
            Span::raw(" — "),
            Span::styled(
                target_status(app, target.status),
                super::semantic_style(target_status_tone(target.status))
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if target.package_base != target.package_name {
            spans.push(Span::styled(
                format!(
                    " · {}: {}",
                    crate::i18n::t(app, "app.pi_scan.targets.package_base"),
                    target.package_base
                ),
                super::semantic_style(SemanticTone::Muted),
            ));
        }
        spans.push(Span::styled(
            format!(
                " · {}: {}",
                crate::i18n::t(app, "app.pi_scan.targets.commit"),
                target
                    .commit_oid
                    .as_deref()
                    .map_or_else(|| "—".to_string(), super::short_identity)
            ),
            super::semantic_style(SemanticTone::Muted),
        ));
        lines.push(truncate_line(
            Line::from(spans),
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
    if let Some(preview) = &app.pi_scan.dry_run_preview {
        lines.push(Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.targets.dry_run_preview"),
            preview.process
        )));
        lines.push(Line::from(preview.disclosure.clone()));
    }
    app.pi_scan.set_target_row_rects(rects);
    super::body(f, app, area, "app.pi_scan.tabs.targets", lines);
}

/// Select compact prefix depth while leaving at least one visible list row when possible.
const fn list_prefix_lines(area: Rect) -> u16 {
    let inner_height = area.height.saturating_sub(2);
    if inner_height >= FULL_TARGET_LIST_PREFIX_LINES.saturating_add(1) {
        FULL_TARGET_LIST_PREFIX_LINES
    } else if inner_height >= 2 {
        1
    } else {
        0
    }
}

/// Build the normal hint/heading prefix or its compact heading-only form.
fn target_prefix(app: &AppState, prefix_lines: u16) -> Vec<Line<'static>> {
    match prefix_lines {
        FULL_TARGET_LIST_PREFIX_LINES => vec![
            Line::from(crate::i18n::t(app, "app.pi_scan.targets.hint")),
            Line::from(String::new()),
            super::section_heading(app, "app.pi_scan.targets.list_heading"),
        ],
        1 => vec![super::section_heading(
            app,
            "app.pi_scan.targets.list_heading",
        )],
        _ => Vec::new(),
    }
}

/// Truncate one styled target row to the viewport so it retains exactly one visual line.
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

/// Keep a selected item visible while clamping an item offset to available content.
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

/// Render a localized target status.
fn target_status(app: &AppState, status: PiScanTargetStatus) -> String {
    let key = match status {
        PiScanTargetStatus::Unbaselined => "unbaselined",
        PiScanTargetStatus::Queued => "queued",
        PiScanTargetStatus::Running => "running",
        PiScanTargetStatus::Paused => "paused",
        PiScanTargetStatus::Completed => "completed",
        PiScanTargetStatus::Failed => "failed",
        PiScanTargetStatus::Interrupted => "interrupted",
        PiScanTargetStatus::Cancelled => "cancelled",
    };
    crate::i18n::t(app, &format!("app.pi_scan.target_status.{key}"))
}

/// Map every target status to an accessible shared semantic tone.
const fn target_status_tone(status: PiScanTargetStatus) -> SemanticTone {
    match status {
        PiScanTargetStatus::Running => SemanticTone::Active,
        PiScanTargetStatus::Completed => SemanticTone::Success,
        PiScanTargetStatus::Failed => SemanticTone::Error,
        PiScanTargetStatus::Unbaselined
        | PiScanTargetStatus::Queued
        | PiScanTargetStatus::Paused
        | PiScanTargetStatus::Interrupted
        | PiScanTargetStatus::Cancelled => SemanticTone::Warning,
    }
}
