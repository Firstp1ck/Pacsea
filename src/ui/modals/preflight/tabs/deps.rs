use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::modal::{DependencyInfo, DependencySource, DependencyStatus};
use crate::state::{AppState, PackageItem, PreflightAction};
use crate::theme::theme;
use crate::ui::modals::preflight::helpers::format_count_with_indicator;
use std::collections::{HashMap, HashSet};

/// What: Dependency statistics grouped by status.
///
/// Inputs: None (struct fields).
///
/// Output: None (struct fields).
///
/// Details: Contains counts of dependencies by their status type.
struct DepStats {
    total: usize,
    installed: usize,
    to_install: usize,
    to_upgrade: usize,
    conflict: usize,
    missing: usize,
}

/// What: Calculate dependency statistics from unique dependencies.
///
/// Inputs:
/// - `unique_deps`: Map of unique dependency names to their info.
///
/// Output:
/// - Returns `DepStats` with counts by status.
///
/// Details:
/// - Counts each dependency only once regardless of how many packages require it.
fn calculate_dep_stats(unique_deps: &HashMap<String, &DependencyInfo>) -> DepStats {
    DepStats {
        total: unique_deps.len(),
        installed: unique_deps
            .values()
            .filter(|d| matches!(d.status, DependencyStatus::Installed { .. }))
            .count(),
        to_install: unique_deps
            .values()
            .filter(|d| matches!(d.status, DependencyStatus::ToInstall))
            .count(),
        to_upgrade: unique_deps
            .values()
            .filter(|d| matches!(d.status, DependencyStatus::ToUpgrade { .. }))
            .count(),
        conflict: unique_deps
            .values()
            .filter(|d| matches!(d.status, DependencyStatus::Conflict { .. }))
            .count(),
        missing: unique_deps
            .values()
            .filter(|d| matches!(d.status, DependencyStatus::Missing))
            .count(),
    }
}

/// What: Check if dependency data is incomplete based on resolution state and heuristics.
///
/// Inputs:
/// - `app`: Application state for resolution flags.
/// - `total_deps`: Total number of unique dependencies.
/// - `items`: Packages under review.
/// - `deps`: All dependency info to check package representation.
///
/// Output:
/// - Returns true if data appears incomplete.
///
/// Details:
/// - Data is incomplete if resolving with some data, or if heuristic suggests partial data.
fn has_incomplete_data(
    app: &AppState,
    total_deps: usize,
    items: &[PackageItem],
    deps: &[DependencyInfo],
) -> bool {
    let is_resolving = app.preflight_deps_resolving || app.deps_resolving;
    tracing::debug!(
        "[UI] compute_is_resolving: preflight_deps_resolving={}, deps_resolving={}, is_resolving={}, total_deps={}, items={}, deps={}",
        app.preflight_deps_resolving,
        app.deps_resolving,
        is_resolving,
        total_deps,
        items.len(),
        deps.len()
    );
    if is_resolving && total_deps > 0 {
        return true;
    }
    if total_deps == 0 {
        return false;
    }
    let packages_with_deps: HashSet<String> = deps
        .iter()
        .flat_map(|d| d.required_by.iter())
        .cloned()
        .collect();
    let packages_with_deps_count = packages_with_deps.len();
    packages_with_deps_count < items.len() && total_deps < items.len()
}

/// What: Render summary header lines for dependencies.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `action`: Whether install or remove.
/// - `stats`: Dependency statistics.
/// - `items_count`: Number of packages.
/// - `has_incomplete`: Whether data is incomplete.
///
/// Output:
/// - Returns vector of header lines.
///
/// Details:
/// - Shows different headers for install vs remove actions.
fn render_summary_header(
    app: &AppState,
    action: &PreflightAction,
    stats: &DepStats,
    items_count: usize,
    has_incomplete: bool,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    if stats.total == 0 {
        return lines;
    }

    if matches!(*action, PreflightAction::Remove) {
        let count_text = format_count_with_indicator(stats.total, items_count, has_incomplete);
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(
                app,
                "app.modals.preflight.deps.dependents_rely_on",
                count_text,
            ),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    } else {
        let mut summary_parts = Vec::new();
        let total_text = format_count_with_indicator(stats.total, items_count, has_incomplete);
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.deps.total",
            total_text,
        ));
        if stats.installed > 0 {
            let count_text =
                format_count_with_indicator(stats.installed, stats.total, has_incomplete);
            summary_parts.push(i18n::t_fmt1(
                app,
                "app.modals.preflight.deps.installed",
                count_text,
            ));
        }
        if stats.to_install > 0 {
            let count_text =
                format_count_with_indicator(stats.to_install, stats.total, has_incomplete);
            summary_parts.push(i18n::t_fmt1(
                app,
                "app.modals.preflight.deps.to_install",
                count_text,
            ));
        }
        if stats.to_upgrade > 0 {
            let count_text =
                format_count_with_indicator(stats.to_upgrade, stats.total, has_incomplete);
            summary_parts.push(i18n::t_fmt1(
                app,
                "app.modals.preflight.deps.to_upgrade",
                count_text,
            ));
        }
        if stats.conflict > 0 {
            let count_text =
                format_count_with_indicator(stats.conflict, stats.total, has_incomplete);
            summary_parts.push(i18n::t_fmt1(
                app,
                "app.modals.preflight.deps.conflicts",
                count_text,
            ));
        }
        if stats.missing > 0 {
            let count_text =
                format_count_with_indicator(stats.missing, stats.total, has_incomplete);
            summary_parts.push(i18n::t_fmt1(
                app,
                "app.modals.preflight.deps.missing",
                count_text,
            ));
        }
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(
                app,
                "app.modals.preflight.deps.dependencies_label",
                summary_parts.join(", "),
            ),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    lines
}

/// What: Render empty state when no dependencies are found.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `action`: Whether install or remove.
/// - `items`: Packages under review.
/// - `is_resolving`: Whether resolution is in progress.
/// - `deps_error`: Optional error message.
///
/// Output:
/// - Returns vector of empty state lines.
///
/// Details:
/// - Shows different messages for install vs remove, with error handling.
fn render_empty_state(
    app: &AppState,
    action: &PreflightAction,
    items: &[PackageItem],
    is_resolving: bool,
    deps_error: &Option<String>,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    if !matches!(*action, PreflightAction::Install) {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.deps.no_deps_for_removal"),
            Style::default().fg(th.subtext1),
        )));
        return lines;
    }

    if is_resolving {
        for pkg_name in items.iter().map(|p| &p.name) {
            let mut spans = Vec::new();
            spans.push(Span::styled(
                format!("▶ {pkg_name} "),
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("(0 deps)", Style::default().fg(th.subtext1)));
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.deps.resolving"),
            Style::default().fg(th.yellow),
        )));
    } else if let Some(err_msg) = deps_error {
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(app, "app.modals.preflight.deps.error", err_msg),
            Style::default().fg(th.red),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.deps.retry_hint"),
            Style::default().fg(th.subtext1),
        )));
    } else {
        for pkg_name in items.iter().map(|p| &p.name) {
            let mut spans = Vec::new();
            spans.push(Span::styled(
                format!("▶ {pkg_name} "),
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("(0 deps)", Style::default().fg(th.subtext1)));
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.deps.resolving"),
            Style::default().fg(th.subtext1),
        )));
    }

    lines
}

/// What: Build display items list with package headers and dependencies.
///
/// Inputs:
/// - `items`: Packages under review.
/// - `grouped`: Dependencies grouped by package name.
/// - `dep_tree_expanded`: Set of expanded package names.
///
/// Output:
/// - Returns vector of (`is_header`, `header_name`, `optional_dep`) tuples.
///
/// Details:
/// - Includes all packages even if they have no dependencies.
fn build_display_items<'a>(
    items: &[PackageItem],
    grouped: &'a HashMap<String, Vec<&'a DependencyInfo>>,
    dep_tree_expanded: &HashSet<String>,
) -> Vec<(bool, String, Option<&'a DependencyInfo>)> {
    let mut display_items = Vec::new();
    for pkg_name in items.iter().map(|p| &p.name) {
        let is_expanded = dep_tree_expanded.contains(pkg_name);
        display_items.push((true, pkg_name.clone(), None));

        if is_expanded && let Some(pkg_deps) = grouped.get(pkg_name) {
            let mut seen_deps = HashSet::new();
            for dep in pkg_deps.iter() {
                if seen_deps.insert(dep.name.as_str()) {
                    display_items.push((false, String::new(), Some(*dep)));
                }
            }
        }
    }
    display_items
}

/// What: Calculate viewport range for visible items.
///
/// Inputs:
/// - `available_height`: Available screen height.
/// - `total_items`: Total number of display items.
/// - `dep_selected`: Currently selected index (mutable).
///
/// Output:
/// - Returns (`start_idx`, `end_idx`) tuple for viewport range.
///
/// Details:
/// - Clamps selected index and calculates centered viewport.
fn calculate_viewport(
    available_height: usize,
    total_items: usize,
    dep_selected: &mut usize,
) -> (usize, usize) {
    let dep_selected_clamped = (*dep_selected).min(total_items.saturating_sub(1));
    if *dep_selected != dep_selected_clamped {
        tracing::debug!(
            "[UI] Deps tab: clamping dep_selected from {} to {} (total_items={})",
            *dep_selected,
            dep_selected_clamped,
            total_items
        );
        *dep_selected = dep_selected_clamped;
    }

    let start_idx = dep_selected_clamped
        .saturating_sub(available_height / 2)
        .min(total_items.saturating_sub(available_height));
    let end_idx = (start_idx + available_height).min(total_items);
    (start_idx, end_idx)
}

/// What: Render a package header line.
///
/// Inputs:
/// - `header_name`: Package name.
/// - `is_expanded`: Whether package tree is expanded.
/// - `is_selected`: Whether this item is selected.
/// - `grouped`: Dependencies grouped by package name.
/// - `th`: Theme colors.
///
/// Output:
/// - Returns vector of spans for the header line.
///
/// Details:
/// - Shows arrow symbol, package name, and dependency count.
fn render_package_header(
    header_name: &str,
    is_expanded: bool,
    is_selected: bool,
    grouped: &HashMap<String, Vec<&DependencyInfo>>,
    th: &crate::theme::Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let arrow_symbol = if is_expanded { "▼" } else { "▶" };
    let header_style = if is_selected {
        Style::default()
            .fg(th.crust)
            .bg(th.lavender)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD)
    };
    spans.push(Span::styled(
        format!("{arrow_symbol} {header_name} "),
        header_style,
    ));

    if let Some(pkg_deps) = grouped.get(header_name) {
        let mut seen_deps = HashSet::new();
        let dep_count = pkg_deps
            .iter()
            .filter(|dep| seen_deps.insert(dep.name.as_str()))
            .count();
        spans.push(Span::styled(
            format!("({dep_count} deps)"),
            Style::default().fg(th.subtext1),
        ));
    } else {
        spans.push(Span::styled("(0 deps)", Style::default().fg(th.subtext1)));
    }

    spans
}

/// What: Render a dependency item line.
///
/// Inputs:
/// - `dep`: Dependency information.
/// - `is_selected`: Whether this item is selected.
/// - `app`: Application state for i18n.
/// - `th`: Theme colors.
///
/// Output:
/// - Returns vector of spans for the dependency line.
///
/// Details:
/// - Shows status icon, name, version, source badge, and additional status info.
fn render_dependency_item(
    dep: &DependencyInfo,
    is_selected: bool,
    app: &AppState,
    th: &crate::theme::Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    spans.push(Span::styled("  ", Style::default()));

    let (status_icon, status_color) = match &dep.status {
        DependencyStatus::Installed { .. } => ("✓", th.green),
        DependencyStatus::ToInstall => ("+", th.yellow),
        DependencyStatus::ToUpgrade { .. } => ("↑", th.yellow),
        DependencyStatus::Conflict { .. } => ("⚠", th.red),
        DependencyStatus::Missing => ("?", th.red),
    };
    spans.push(Span::styled(
        format!("{status_icon} "),
        Style::default().fg(status_color),
    ));

    let name_style = if is_selected {
        Style::default()
            .fg(th.crust)
            .bg(th.lavender)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    };
    spans.push(Span::styled(dep.name.clone(), name_style));

    if !dep.version.is_empty() {
        spans.push(Span::styled(
            format!(" {}", dep.version),
            Style::default().fg(th.overlay2),
        ));
    }

    let (source_badge, badge_color) = match &dep.source {
        DependencySource::Official { repo } => {
            let repo_lower = repo.to_lowercase();
            let color = if crate::index::is_eos_repo(&repo_lower)
                || crate::index::is_cachyos_repo(&repo_lower)
            {
                th.sapphire
            } else {
                th.green
            };
            (format!(" [{repo}]"), color)
        }
        DependencySource::Aur => (" [AUR]".to_string(), th.yellow),
        DependencySource::Local => (" [local]".to_string(), th.overlay1),
    };
    spans.push(Span::styled(source_badge, Style::default().fg(badge_color)));

    if dep.is_core {
        spans.push(Span::styled(
            " [CORE]",
            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
        ));
    } else if dep.is_system {
        spans.push(Span::styled(
            " [SYSTEM]",
            Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
        ));
    }

    match &dep.status {
        DependencyStatus::Installed { version } => {
            spans.push(Span::styled(
                i18n::t_fmt1(app, "app.modals.preflight.deps.installed_version", version),
                Style::default().fg(th.subtext1),
            ));
        }
        DependencyStatus::ToUpgrade { current, required } => {
            spans.push(Span::styled(
                format!(" ({current} → {required})"),
                Style::default().fg(th.yellow),
            ));
        }
        DependencyStatus::Conflict { reason } => {
            spans.push(Span::styled(
                format!(" ({reason})"),
                Style::default().fg(th.red),
            ));
        }
        _ => {}
    }

    spans
}

/// What: Render the Dependencies tab content for the preflight modal.
///
/// Inputs:
/// - `app`: Application state for i18n and data access.
/// - `items`: Packages under review.
/// - `action`: Whether install or remove.
/// - `dependency_info`: Dependency information.
/// - `dep_selected`: Currently selected dependency index (mutable).
/// - `dep_tree_expanded`: Set of expanded package names.
/// - `deps_error`: Optional error message.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows dependency statistics and grouped dependency tree.
/// - Supports viewport-based rendering for large lists.
#[allow(clippy::too_many_arguments)]
pub fn render_deps_tab(
    app: &AppState,
    items: &[PackageItem],
    action: &PreflightAction,
    dependency_info: &[DependencyInfo],
    dep_selected: &mut usize,
    dep_tree_expanded: &HashSet<String>,
    deps_error: &Option<String>,
    content_rect: Rect,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    // Group dependencies by the packages that require them and deduplicate
    let mut grouped: HashMap<String, Vec<&DependencyInfo>> = HashMap::new();
    let mut unique_deps: HashMap<String, &DependencyInfo> = HashMap::new();
    for dep in dependency_info.iter() {
        unique_deps.entry(dep.name.clone()).or_insert(dep);
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    // Calculate statistics and check for incomplete data
    let stats = calculate_dep_stats(&unique_deps);
    let has_incomplete = has_incomplete_data(app, stats.total, items, dependency_info);

    // Render summary header or empty state
    if stats.total > 0 {
        lines.extend(render_summary_header(
            app,
            action,
            &stats,
            items.len(),
            has_incomplete,
        ));
    } else if dependency_info.is_empty() {
        let is_resolving = app.preflight_deps_resolving || app.deps_resolving;
        lines.extend(render_empty_state(
            app,
            action,
            items,
            is_resolving,
            deps_error,
        ));
        // Return early - empty state already shows packages, don't render them again
        return lines;
    }

    // Build display items and render viewport
    let display_items = build_display_items(items, &grouped, dep_tree_expanded);
    let available_height = (content_rect.height as usize).saturating_sub(6);
    let total_items = display_items.len();
    tracing::debug!(
        "[UI] Deps tab: total_items={}, dep_selected={}, items={}, deps={}, expanded_count={}",
        total_items,
        *dep_selected,
        items.len(),
        dependency_info.len(),
        dep_tree_expanded.len()
    );

    let (start_idx, end_idx) = calculate_viewport(available_height, total_items, dep_selected);

    // Render visible items
    for (idx, (is_header, header_name, dep)) in display_items
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(end_idx - start_idx)
    {
        let is_selected = idx == *dep_selected;
        let spans = if *is_header {
            let is_expanded = dep_tree_expanded.contains(header_name);
            render_package_header(header_name, is_expanded, is_selected, &grouped, &th)
        } else if let Some(dep) = dep {
            render_dependency_item(dep, is_selected, app, &th)
        } else {
            continue;
        };
        lines.push(Line::from(spans));
    }

    // Show range indicator if needed
    if total_items > available_height {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t_fmt(
                app,
                "app.modals.preflight.deps.showing_range",
                &[
                    &(start_idx + 1).to_string(),
                    &end_idx.to_string(),
                    &total_items.to_string(),
                ],
            ),
            Style::default().fg(th.subtext1),
        )));
    }

    lines
}
