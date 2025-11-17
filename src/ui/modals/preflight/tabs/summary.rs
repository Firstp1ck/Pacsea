use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::modal::{
    CascadeMode, DependencyInfo, DependencySource, DependencyStatus, PreflightHeaderChips,
    PreflightSummaryData,
};
use crate::state::{AppState, PackageItem, PreflightAction, Source};
use crate::theme::theme;

use super::super::helpers::{format_bytes, format_signed_bytes};

/// What: Get source badge text and color for a dependency source.
///
/// Inputs:
/// - `source`: The dependency source.
///
/// Output:
/// - Returns a tuple of (badge_text, color).
///
/// Details:
/// - Formats repository names, AUR, and local sources with appropriate colors.
fn get_source_badge(source: &DependencySource) -> (String, ratatui::style::Color) {
    let th = theme();
    match source {
        DependencySource::Official { repo } => {
            let repo_lower = repo.to_lowercase();
            let color = if crate::index::is_eos_repo(&repo_lower)
                || crate::index::is_cachyos_repo(&repo_lower)
            {
                th.sapphire
            } else {
                th.green
            };
            (format!(" [{}]", repo), color)
        }
        DependencySource::Aur => (" [AUR]".to_string(), th.yellow),
        DependencySource::Local => (" [local]".to_string(), th.overlay1),
    }
}

/// What: Render summary data section (risk factors, notes, per-package overview).
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `summary_data`: Summary data to render.
/// - `header_chips`: Header chip data for risk level.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows risk factors, notes, and per-package details if available.
fn render_summary_data(
    app: &AppState,
    summary_data: &PreflightSummaryData,
    header_chips: &PreflightHeaderChips,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    let risk_color = match header_chips.risk_level {
        crate::state::modal::RiskLevel::Low => th.green,
        crate::state::modal::RiskLevel::Medium => th.yellow,
        crate::state::modal::RiskLevel::High => th.red,
    };

    if !summary_data.risk_reasons.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.risk_factors"),
            Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
        )));
        for reason in &summary_data.risk_reasons {
            let bullet = format!("  • {}", reason);
            lines.push(Line::from(Span::styled(
                bullet,
                Style::default().fg(th.subtext1),
            )));
        }
    }
    if !summary_data.summary_notes.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.notes"),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )));
        for note in &summary_data.summary_notes {
            let bullet = format!("  • {}", note);
            lines.push(Line::from(Span::styled(
                bullet,
                Style::default().fg(th.subtext1),
            )));
        }
    }
    if !summary_data.packages.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.per_package_overview"),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )));
        for pkg in &summary_data.packages {
            let mut entry = format!("  • {}", pkg.name);
            match &pkg.source {
                Source::Aur => entry.push_str(" [AUR]"),
                Source::Official { repo, .. } => entry.push_str(&format!(" [{}]", repo)),
            }
            if let Some(installed) = &pkg.installed_version {
                entry.push_str(&format!(" {} → {}", installed, pkg.target_version));
            } else {
                entry.push_str(&format!(" {}", pkg.target_version));
            }
            if pkg.is_major_bump {
                entry.push_str(&format!(
                    " ({})",
                    i18n::t(app, "app.modals.preflight.summary.major_bump")
                ));
            }
            if pkg.is_downgrade {
                entry.push_str(&format!(
                    " ({})",
                    i18n::t(app, "app.modals.preflight.summary.downgrade")
                ));
            }
            if let Some(bytes) = pkg.download_bytes {
                entry.push_str(&format!(
                    " {}",
                    i18n::t_fmt1(
                        app,
                        "app.modals.preflight.summary.download",
                        format_bytes(bytes)
                    )
                ));
            }
            if let Some(delta) = pkg.install_delta_bytes {
                entry.push_str(&format!(
                    " {}",
                    i18n::t_fmt1(
                        app,
                        "app.modals.preflight.summary.size",
                        format_signed_bytes(delta)
                    )
                ));
            }
            if !pkg.notes.is_empty() {
                entry.push_str(&format!(" • {}", pkg.notes.join("; ")));
            }
            lines.push(Line::from(Span::styled(
                entry,
                Style::default().fg(th.subtext0),
            )));
        }
    }
    lines.push(Line::from(""));
    lines
}

/// What: Render incomplete data indicator if data is still resolving.
///
/// Inputs:
/// - `app`: Application state for i18n and resolution flags.
/// - `items`: Packages under review.
/// - `dependency_info`: Dependency information.
///
/// Output:
/// - Returns a vector of lines to render, or empty if no incomplete data.
///
/// Details:
/// - Checks for resolving dependencies, files, and sandbox data.
fn render_incomplete_data_indicator(
    app: &AppState,
    items: &[PackageItem],
    dependency_info: &[DependencyInfo],
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    let deps_resolving = app.preflight_deps_resolving || app.deps_resolving;
    let files_resolving = app.preflight_files_resolving || app.files_resolving;
    let sandbox_resolving = app.preflight_sandbox_resolving || app.sandbox_resolving;

    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();
    let packages_with_deps: std::collections::HashSet<String> = dependency_info
        .iter()
        .flat_map(|d| d.required_by.iter())
        .cloned()
        .collect();
    let packages_with_deps_count = packages_with_deps.len();
    let deps_incomplete = !deps_resolving
        && !item_names.is_empty()
        && packages_with_deps_count < items.len()
        && dependency_info.len() < items.len();

    let has_incomplete_data =
        deps_resolving || files_resolving || sandbox_resolving || deps_incomplete;

    if has_incomplete_data {
        let mut resolving_parts = Vec::new();
        if deps_resolving {
            resolving_parts.push(i18n::t(app, "app.modals.preflight.summary.resolving_deps"));
        }
        if files_resolving {
            resolving_parts.push(i18n::t(app, "app.modals.preflight.summary.resolving_files"));
        }
        if sandbox_resolving {
            resolving_parts.push(i18n::t(
                app,
                "app.modals.preflight.summary.resolving_sandbox",
            ));
        }
        if !resolving_parts.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("⟳ {}", resolving_parts.join(", ")),
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.preflight.summary.data_will_update"),
                Style::default().fg(th.subtext1),
            )));
        }
    }
    lines
}

/// What: Render dependency spans for a conflict or upgrade status.
///
/// Inputs:
/// - `dep`: Dependency information.
///
/// Output:
/// - Returns a vector of spans to render.
///
/// Details:
/// - Formats conflict or upgrade status with appropriate styling.
fn render_dependency_spans(dep: &DependencyInfo) -> Option<Vec<Span<'static>>> {
    let th = theme();
    let mut spans = Vec::new();
    spans.push(Span::styled("  ", Style::default())); // Indentation

    match &dep.status {
        DependencyStatus::Conflict { reason } => {
            spans.push(Span::styled("⚠ ", Style::default().fg(th.red)));
            spans.push(Span::styled(dep.name.clone(), Style::default().fg(th.text)));
            if !dep.version.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", dep.version),
                    Style::default().fg(th.overlay2),
                ));
            }
            let (source_badge, badge_color) = get_source_badge(&dep.source);
            spans.push(Span::styled(source_badge, Style::default().fg(badge_color)));
            spans.push(Span::styled(
                format!(" ({})", reason),
                Style::default().fg(th.red),
            ));
        }
        DependencyStatus::ToUpgrade { current, required } => {
            spans.push(Span::styled("↑ ", Style::default().fg(th.yellow)));
            spans.push(Span::styled(dep.name.clone(), Style::default().fg(th.text)));
            if !dep.version.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", dep.version),
                    Style::default().fg(th.overlay2),
                ));
            }
            let (source_badge, badge_color) = get_source_badge(&dep.source);
            spans.push(Span::styled(source_badge, Style::default().fg(badge_color)));
            spans.push(Span::styled(
                format!(" ({} → {})", current, required),
                Style::default().fg(th.yellow),
            ));
        }
        _ => return None,
    }
    Some(spans)
}

/// What: Render install action dependencies (conflicts and upgrades).
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `items`: Packages under review.
/// - `dependency_info`: Dependency information.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows conflicts and upgrades grouped by package.
fn render_install_dependencies(
    app: &AppState,
    items: &[PackageItem],
    dependency_info: &[DependencyInfo],
    content_rect: Rect,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    let important_deps: Vec<&DependencyInfo> = dependency_info
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                DependencyStatus::Conflict { .. } | DependencyStatus::ToUpgrade { .. }
            )
        })
        .collect();

    if important_deps.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.no_conflicts_or_upgrades"),
            Style::default().fg(th.green),
        )));
        return lines;
    }

    use std::collections::{HashMap, HashSet};
    let mut grouped: HashMap<String, Vec<&DependencyInfo>> = HashMap::new();
    for dep in important_deps.iter() {
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    let conflict_count = important_deps
        .iter()
        .filter(|d| matches!(d.status, DependencyStatus::Conflict { .. }))
        .count();
    let upgrade_count = important_deps
        .iter()
        .filter(|d| matches!(d.status, DependencyStatus::ToUpgrade { .. }))
        .count();

    let mut summary_parts = Vec::new();
    if conflict_count > 0 {
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.summary.conflict_singular",
            conflict_count,
        ));
    }
    if upgrade_count > 0 {
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.summary.upgrade_singular",
            upgrade_count,
        ));
    }

    let header_text = if conflict_count > 0 {
        i18n::t_fmt1(
            app,
            "app.modals.preflight.summary.issues",
            summary_parts.join(", "),
        )
    } else if upgrade_count > 0 {
        i18n::t_fmt1(
            app,
            "app.modals.preflight.summary.summary_label",
            summary_parts.join(", "),
        )
    } else {
        i18n::t(app, "app.modals.preflight.summary.summary_no_conflicts")
    };

    lines.push(Line::from(Span::styled(
        header_text,
        Style::default()
            .fg(if conflict_count > 0 {
                th.red
            } else {
                th.yellow
            })
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let available_height = (content_rect.height as usize).saturating_sub(6);
    let mut displayed = 0;
    for pkg_name in items.iter().map(|p| &p.name) {
        if let Some(pkg_deps) = grouped.get(pkg_name) {
            if displayed >= available_height {
                break;
            }
            lines.push(Line::from(Span::styled(
                format!("▶ {}", pkg_name),
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            displayed += 1;

            let mut seen_deps = HashSet::new();
            for dep in pkg_deps.iter() {
                if seen_deps.insert(dep.name.as_str())
                    && displayed < available_height
                    && let Some(spans) = render_dependency_spans(dep)
                {
                    displayed += 1;
                    lines.push(Line::from(spans));
                }
            }
        }
    }

    if displayed >= available_height && important_deps.len() > displayed {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(
                app,
                "app.modals.preflight.summary.and_more",
                important_deps.len() - displayed,
            ),
            Style::default().fg(th.subtext1),
        )));
    }
    lines
}

/// What: Render remove action content (removal plan and cascade impact).
///
/// Inputs:
/// - `app`: Application state for i18n and data access.
/// - `items`: Packages to remove.
/// - `dependency_info`: Dependency information.
/// - `cascade_mode`: Removal cascade mode.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows removal plan, dependent count, and cascade impact preview.
fn render_remove_action(
    app: &AppState,
    items: &[PackageItem],
    dependency_info: &[DependencyInfo],
    cascade_mode: CascadeMode,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    let mode_line = i18n::t_fmt(
        app,
        "app.modals.preflight.summary.cascade_mode",
        &[&cascade_mode.flag(), &cascade_mode.description()],
    );
    lines.push(Line::from(Span::styled(
        mode_line,
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.no_removal_targets"),
            Style::default().fg(th.subtext1),
        )));
        return lines;
    }

    let removal_names: Vec<&str> = items.iter().map(|pkg| pkg.name.as_str()).collect();
    let plan_header_style = Style::default()
        .fg(th.overlay1)
        .add_modifier(Modifier::BOLD);
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.preflight.summary.removal_plan_preview"),
        plan_header_style,
    )));

    let mut plan_command = format!(
        "sudo pacman {} --noconfirm {}",
        cascade_mode.flag(),
        removal_names.join(" ")
    );
    if app.dry_run {
        plan_command = i18n::t_fmt1(
            app,
            "app.modals.preflight.summary.dry_run_prefix",
            plan_command,
        );
    }
    lines.push(Line::from(Span::styled(
        plan_command,
        Style::default().fg(th.text),
    )));

    let dependent_count = dependency_info.len();
    let (summary_text, summary_style) = if dependent_count == 0 {
        (
            i18n::t(app, "app.modals.preflight.summary.no_dependents"),
            Style::default().fg(th.green),
        )
    } else if cascade_mode.allows_dependents() {
        (
            i18n::t_fmt1(
                app,
                "app.modals.preflight.summary.cascade_will_include",
                dependent_count,
            ),
            Style::default().fg(th.yellow),
        )
    } else {
        (
            i18n::t_fmt1(
                app,
                "app.modals.preflight.summary.dependents_block_removal",
                dependent_count,
            ),
            Style::default().fg(th.red),
        )
    };
    lines.push(Line::from(Span::styled(summary_text, summary_style)));
    lines.push(Line::from(""));

    if dependent_count > 0 {
        if app.remove_preflight_summary.is_empty() {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.preflight.summary.calculating_reverse_deps"),
                Style::default().fg(th.subtext1),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.preflight.summary.removal_impact_overview"),
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            for summary in &app.remove_preflight_summary {
                let mut message = i18n::t_fmt(
                    app,
                    "app.modals.preflight.summary.dependent_singular",
                    &[&summary.package, &summary.total_dependents],
                );
                if summary.direct_dependents > 0 {
                    message.push_str(&format!(
                        " {}",
                        i18n::t_fmt1(
                            app,
                            "app.modals.preflight.summary.direct_singular",
                            summary.direct_dependents
                        )
                    ));
                }
                if summary.transitive_dependents > 0 {
                    message.push_str(&format!(
                        " {}",
                        i18n::t_fmt1(
                            app,
                            "app.modals.preflight.summary.transitive_singular",
                            summary.transitive_dependents
                        )
                    ));
                }
                lines.push(Line::from(Span::styled(
                    message,
                    Style::default().fg(th.text),
                )));
            }
            lines.push(Line::from(""));
        }

        let (impact_header, impact_style) = if cascade_mode.allows_dependents() {
            (
                i18n::t(app, "app.modals.preflight.summary.cascade_will_remove"),
                Style::default().fg(th.red).add_modifier(Modifier::BOLD),
            )
        } else {
            (
                i18n::t(app, "app.modals.preflight.summary.dependents_not_removed"),
                Style::default().fg(th.red).add_modifier(Modifier::BOLD),
            )
        };
        lines.push(Line::from(Span::styled(impact_header, impact_style)));

        let removal_targets: std::collections::HashSet<String> = items
            .iter()
            .map(|pkg| pkg.name.to_ascii_lowercase())
            .collect();
        let mut cascade_candidates: Vec<&DependencyInfo> = dependency_info.iter().collect();
        cascade_candidates.sort_by(|a, b| {
            let a_direct = a
                .depends_on
                .iter()
                .any(|parent| removal_targets.contains(&parent.to_ascii_lowercase()));
            let b_direct = b
                .depends_on
                .iter()
                .any(|parent| removal_targets.contains(&parent.to_ascii_lowercase()));
            b_direct.cmp(&a_direct).then_with(|| a.name.cmp(&b.name))
        });

        const CASCADE_PREVIEW_MAX: usize = 8;
        for dep in cascade_candidates.iter().take(CASCADE_PREVIEW_MAX) {
            let is_direct = dep
                .depends_on
                .iter()
                .any(|parent| removal_targets.contains(&parent.to_ascii_lowercase()));
            let bullet = if cascade_mode.allows_dependents() {
                if is_direct { "● " } else { "○ " }
            } else if is_direct {
                "⛔ "
            } else {
                "⚠ "
            };
            let name_color = if cascade_mode.allows_dependents() {
                if is_direct { th.red } else { th.yellow }
            } else if is_direct {
                th.red
            } else {
                th.yellow
            };
            let name_style = Style::default().fg(name_color).add_modifier(Modifier::BOLD);
            let detail = match &dep.status {
                DependencyStatus::Conflict { reason } => reason.clone(),
                DependencyStatus::ToUpgrade { .. } => {
                    i18n::t(app, "app.modals.preflight.summary.requires_version_change")
                }
                DependencyStatus::Installed { .. } => {
                    i18n::t(app, "app.modals.preflight.summary.already_satisfied")
                }
                DependencyStatus::ToInstall => {
                    i18n::t(app, "app.modals.preflight.summary.not_currently_installed")
                }
                DependencyStatus::Missing => i18n::t(app, "app.modals.preflight.summary.missing"),
            };
            let roots = if dep.required_by.is_empty() {
                String::new()
            } else {
                i18n::t_fmt1(
                    app,
                    "app.modals.preflight.summary.targets_label",
                    dep.required_by.join(", "),
                )
            };

            let mut spans = Vec::new();
            spans.push(Span::styled(bullet, Style::default().fg(th.subtext0)));
            spans.push(Span::styled(dep.name.clone(), name_style));
            if !detail.is_empty() {
                spans.push(Span::styled(" — ", Style::default().fg(th.subtext1)));
                spans.push(Span::styled(detail, Style::default().fg(th.subtext1)));
            }
            if !roots.is_empty() {
                spans.push(Span::styled(roots, Style::default().fg(th.overlay1)));
            }
            lines.push(Line::from(spans));
        }

        if cascade_candidates.len() > CASCADE_PREVIEW_MAX {
            lines.push(Line::from(Span::styled(
                i18n::t_fmt1(
                    app,
                    "app.modals.preflight.summary.and_more_impacted",
                    cascade_candidates.len() - CASCADE_PREVIEW_MAX,
                ),
                Style::default().fg(th.subtext1),
            )));
        }

        lines.push(Line::from(""));
        if cascade_mode.allows_dependents() {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.preflight.summary.will_be_removed_auto"),
                Style::default().fg(th.subtext1),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.preflight.summary.enable_cascade_mode"),
                Style::default().fg(th.subtext1),
            )));
        }
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.use_deps_tab"),
            Style::default().fg(th.subtext1),
        )));
    }
    lines
}

/// What: Render the Summary tab content for the preflight modal.
///
/// Inputs:
/// - `app`: Application state for i18n and data access.
/// - `items`: Packages under review.
/// - `action`: Whether install or remove.
/// - `summary`: Summary data (optional).
/// - `header_chips`: Header chip data for risk level.
/// - `dependency_info`: Dependency information.
/// - `cascade_mode`: Removal cascade mode.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows risk factors, notes, per-package overview, and dependency conflicts/upgrades for install.
/// - Shows removal plan and cascade impact for remove actions.
#[allow(clippy::too_many_arguments)]
pub fn render_summary_tab(
    app: &AppState,
    items: &[PackageItem],
    action: &PreflightAction,
    summary: &Option<Box<PreflightSummaryData>>,
    header_chips: &PreflightHeaderChips,
    dependency_info: &[DependencyInfo],
    cascade_mode: CascadeMode,
    content_rect: Rect,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    if let Some(summary_data) = summary.as_ref() {
        lines.extend(render_summary_data(app, summary_data, header_chips));
    } else {
        lines.push(Line::from(Span::styled(
            "Computing summary...",
            Style::default().fg(th.overlay1),
        )));
        lines.push(Line::from(""));
    }

    lines.extend(render_incomplete_data_indicator(
        app,
        items,
        dependency_info,
    ));

    match *action {
        PreflightAction::Install if !dependency_info.is_empty() => {
            lines.extend(render_install_dependencies(
                app,
                items,
                dependency_info,
                content_rect,
            ));
        }
        PreflightAction::Remove => {
            lines.extend(render_remove_action(
                app,
                items,
                dependency_info,
                cascade_mode,
            ));
        }
        _ => {
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.summary.no_items_selected"),
                    Style::default().fg(th.subtext1),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    i18n::t_fmt1(
                        app,
                        "app.modals.preflight.summary.packages_selected",
                        items.len(),
                    ),
                    Style::default().fg(th.text),
                )));
            }
        }
    }

    lines
}
