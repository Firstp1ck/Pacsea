//! Scan target selection page.

use crate::state::{AppState, PiScanTargetStatus};
use crate::theme::theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

/// Render keyboard-selectable package-base targets and dry-run affordance.
pub(super) fn render(f: &mut Frame, app: &AppState, area: Rect) {
    let th = theme();
    let mut lines = vec![Line::from(crate::i18n::t(app, "app.pi_scan.targets.hint"))];
    if app.pi_scan.targets.is_empty() {
        lines.push(Line::from(crate::i18n::t(app, "app.pi_scan.targets.empty")));
    }
    for (index, target) in app.pi_scan.targets.iter().enumerate() {
        let marker = if target.selected { "[x]" } else { "[ ]" };
        let commit = target.commit_oid.as_deref().unwrap_or("unresolved");
        let style = if index == app.pi_scan.selected {
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
    }
    if let Some(preview) = &app.pi_scan.dry_run_preview {
        lines.push(Line::from(format!(
            "{}: {}",
            crate::i18n::t(app, "app.pi_scan.targets.dry_run_preview"),
            preview.process
        )));
        lines.push(Line::from(preview.disclosure.clone()));
    }
    super::body(f, app, area, "app.pi_scan.tabs.targets", lines);
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
