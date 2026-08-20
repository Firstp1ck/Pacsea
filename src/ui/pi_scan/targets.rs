//! Scan target selection page.

use crate::state::pi_scan_ui::PiScanListHitRect;
use crate::state::{AppState, PiScanTargetStatus};
use crate::theme::theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Render keyboard-selectable package-base targets and dry-run affordance.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let capacity = usize::from(area.height.saturating_sub(3)).max(1);
    let offset = visible_offset(
        app.pi_scan.view_scroll.targets,
        app.pi_scan.selected_target,
        app.pi_scan.targets.len(),
        capacity,
    );
    app.pi_scan.view_scroll.targets = offset;
    let mut lines = vec![Line::from(crate::i18n::t(app, "app.pi_scan.targets.hint"))];
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
        let commit = target.commit_oid.as_deref().unwrap_or("—");
        let style = if index == app.pi_scan.selected_target {
            Style::default()
                .fg(th.sapphire)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.text)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "{marker} {} ({}) @ {commit} — {}",
                target.package_name,
                target.package_base,
                target_status(app, target.status),
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
