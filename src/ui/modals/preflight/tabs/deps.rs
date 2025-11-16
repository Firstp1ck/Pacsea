use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::modal::{DependencyInfo, DependencySource, DependencyStatus};
use crate::state::{AppState, PackageItem, PreflightAction};
use crate::theme::theme;
use std::collections::{HashMap, HashSet};

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

    // Use already resolved dependencies (resolved above if needed)
    let deps_empty = dependency_info.is_empty();
    let deps_count = dependency_info.len();
    let deps = dependency_info;

    // Group dependencies by the packages that require them
    // Deduplicate dependencies by name (a dependency can be required by multiple packages)
    let mut grouped: HashMap<String, Vec<&DependencyInfo>> = HashMap::new();
    let mut unique_deps: HashMap<String, &DependencyInfo> = HashMap::new();

    for dep in deps.iter() {
        // Track unique dependencies for statistics (use first occurrence)
        unique_deps.entry(dep.name.clone()).or_insert(dep);

        // Group by required_by for display
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    // Calculate summary statistics using unique dependencies
    // This ensures each dependency is counted only once, regardless of how many packages require it
    let total = unique_deps.len();
    let installed_count = unique_deps
        .values()
        .filter(|d| matches!(d.status, DependencyStatus::Installed { .. }))
        .count();
    let to_install_count = unique_deps
        .values()
        .filter(|d| matches!(d.status, DependencyStatus::ToInstall))
        .count();
    let to_upgrade_count = unique_deps
        .values()
        .filter(|d| matches!(d.status, DependencyStatus::ToUpgrade { .. }))
        .count();
    let conflict_count = unique_deps
        .values()
        .filter(|d| matches!(d.status, DependencyStatus::Conflict { .. }))
        .count();
    let missing_count = unique_deps
        .values()
        .filter(|d| matches!(d.status, DependencyStatus::Missing))
        .count();

    // Summary header
    if total > 0 {
        if matches!(*action, PreflightAction::Remove) {
            lines.push(Line::from(Span::styled(
                i18n::t_fmt1(app, "app.modals.preflight.deps.dependents_rely_on", total),
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
        } else {
            let mut summary_parts = Vec::new();
            summary_parts.push(i18n::t_fmt1(app, "app.modals.preflight.deps.total", total));
            if installed_count > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.deps.installed",
                    installed_count,
                ));
            }
            if to_install_count > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.deps.to_install",
                    to_install_count,
                ));
            }
            if to_upgrade_count > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.deps.to_upgrade",
                    to_upgrade_count,
                ));
            }
            if conflict_count > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.deps.conflicts",
                    conflict_count,
                ));
            }
            if missing_count > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.deps.missing",
                    missing_count,
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
    } else if matches!(*action, PreflightAction::Install) {
        // Check if we're currently resolving (including preflight-specific resolution)
        let is_resolving = app.preflight_deps_resolving || app.deps_resolving;

        // Always show install list (package headers) even when resolving
        // Show loading message below the list
        if deps_empty {
            if is_resolving {
                // Show package headers first, then loading message
                for pkg_name in items.iter().map(|p| &p.name) {
                    let mut spans = Vec::new();
                    spans.push(Span::styled(
                        format!("▶ {} ", pkg_name),
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
                // Display error with retry hint
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
                // No dependencies found and not resolving - show package headers
                for pkg_name in items.iter().map(|p| &p.name) {
                    let mut spans = Vec::new();
                    spans.push(Span::styled(
                        format!("▶ {} ", pkg_name),
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
        }
    } else {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.deps.no_deps_for_removal"),
            Style::default().fg(th.subtext1),
        )));
    }

    // Build flat list with grouped structure for navigation
    // Format: [package_name, dep1, dep2, ...] for each package
    // Performance: This builds the full display list, but only visible items are rendered
    // below. For very large lists (thousands of items), consider lazy building or caching.
    // IMPORTANT: Show ALL packages, even if they have no dependencies
    let mut display_items: Vec<(bool, String, Option<&DependencyInfo>)> = Vec::new();
    for pkg_name in items.iter().map(|p| &p.name) {
        // Always add package header (even if no dependencies)
        let is_expanded = dep_tree_expanded.contains(pkg_name);
        display_items.push((true, pkg_name.clone(), None));

        // Add its dependencies only if expanded AND package has dependencies
        if is_expanded && let Some(pkg_deps) = grouped.get(pkg_name) {
            let mut seen_deps = HashSet::new();
            for dep in pkg_deps.iter() {
                if seen_deps.insert(dep.name.as_str()) {
                    display_items.push((false, String::new(), Some(dep)));
                }
            }
        }
    }

    // Dependency list with grouping
    // Performance optimization: Only render visible items (viewport-based rendering)
    // This prevents performance issues with large dependency lists
    let available_height = (content_rect.height as usize).saturating_sub(6);
    let total_items = display_items.len();
    tracing::debug!(
        "[UI] Deps tab: total_items={}, dep_selected={}, items={}, deps={}, expanded_count={}",
        total_items,
        *dep_selected,
        items.len(),
        deps_count,
        dep_tree_expanded.len()
    );
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

    // Calculate viewport range: only render items visible on screen
    let start_idx = dep_selected_clamped
        .saturating_sub(available_height / 2)
        .min(total_items.saturating_sub(available_height));
    let end_idx = (start_idx + available_height).min(total_items);

    for (idx, (is_header, header_name, dep)) in display_items
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(end_idx - start_idx)
    {
        let is_selected = idx == *dep_selected;
        let mut spans = Vec::new();

        if *is_header {
            // Package header
            let is_expanded = dep_tree_expanded.contains(header_name);
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
                format!("{} {} ", arrow_symbol, header_name),
                header_style,
            ));

            // Add dependency count in brackets (similar to Files tab)
            if let Some(pkg_deps) = grouped.get(header_name) {
                let mut seen_deps = HashSet::new();
                let dep_count = pkg_deps
                    .iter()
                    .filter(|dep| seen_deps.insert(dep.name.as_str()))
                    .count();
                spans.push(Span::styled(
                    format!("({} deps)", dep_count),
                    Style::default().fg(th.subtext1),
                ));
            } else {
                // Package has no dependencies
                spans.push(Span::styled("(0 deps)", Style::default().fg(th.subtext1)));
            }
        } else if let Some(dep) = dep {
            // Dependency item (indented)
            spans.push(Span::styled("  ", Style::default())); // Indentation

            // Status indicator
            let (status_icon, status_color) = match &dep.status {
                DependencyStatus::Installed { .. } => ("✓", th.green),
                DependencyStatus::ToInstall => ("+", th.yellow),
                DependencyStatus::ToUpgrade { .. } => ("↑", th.yellow),
                DependencyStatus::Conflict { .. } => ("⚠", th.red),
                DependencyStatus::Missing => ("?", th.red),
            };
            spans.push(Span::styled(
                format!("{} ", status_icon),
                Style::default().fg(status_color),
            ));

            // Package name
            let name_style = if is_selected {
                Style::default()
                    .fg(th.crust)
                    .bg(th.lavender)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(th.text)
            };
            spans.push(Span::styled(dep.name.clone(), name_style));

            // Version requirement
            if !dep.version.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", dep.version),
                    Style::default().fg(th.overlay2),
                ));
            }

            // Source badge with color coding
            let (source_badge, badge_color) = match &dep.source {
                DependencySource::Official { repo } => {
                    let repo_lower = repo.to_lowercase();
                    let color = if crate::index::is_eos_repo(&repo_lower)
                        || crate::index::is_cachyos_repo(&repo_lower)
                    {
                        th.sapphire // Blueish for EOS/Cachy
                    } else {
                        th.green // Green for core/extra and other official repos
                    };
                    (format!(" [{}]", repo), color)
                }
                DependencySource::Aur => (" [AUR]".to_string(), th.yellow),
                DependencySource::Local => (" [local]".to_string(), th.overlay1),
            };
            spans.push(Span::styled(source_badge, Style::default().fg(badge_color)));

            // Core/System markers
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

            // Additional status info
            match &dep.status {
                DependencyStatus::Installed { version } => {
                    spans.push(Span::styled(
                        i18n::t_fmt1(app, "app.modals.preflight.deps.installed_version", version),
                        Style::default().fg(th.subtext1),
                    ));
                }
                DependencyStatus::ToUpgrade { current, required } => {
                    spans.push(Span::styled(
                        i18n::t_fmt(
                            app,
                            "app.modals.preflight.deps.version_upgrade",
                            &[current, required],
                        ),
                        Style::default().fg(th.yellow),
                    ));
                }
                DependencyStatus::Conflict { reason } => {
                    spans.push(Span::styled(
                        i18n::t_fmt1(app, "app.modals.preflight.deps.conflict_reason", reason),
                        Style::default().fg(th.red),
                    ));
                }
                _ => {}
            }
        }

        lines.push(Line::from(spans));
    }

    if display_items.len() > available_height {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t_fmt(
                app,
                "app.modals.preflight.deps.showing_range",
                &[
                    &(start_idx + 1).to_string(),
                    &end_idx.to_string(),
                    &display_items.len().to_string(),
                ],
            ),
            Style::default().fg(th.subtext1),
        )));
    }

    lines
}
