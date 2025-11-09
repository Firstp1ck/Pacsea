use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::modal::{
    CascadeMode, DependencyInfo, DependencySource, DependencyStatus, FileChangeType,
    PackageFileInfo,
};
use crate::state::{AppState, PackageItem, PreflightAction, PreflightTab};
use crate::theme::theme;
use std::collections::HashSet;

#[allow(clippy::too_many_arguments)]
pub fn render_preflight(
    f: &mut Frame,
    area: Rect,
    app: &mut AppState,
    items: &[PackageItem],
    action: &PreflightAction,
    tab: &PreflightTab,
    dependency_info: &mut Vec<DependencyInfo>,
    dep_selected: &mut usize,
    dep_tree_expanded: &HashSet<String>,
    file_info: &mut Vec<PackageFileInfo>,
    file_selected: &mut usize,
    file_tree_expanded: &HashSet<String>,
    cascade_mode: CascadeMode,
) {
    let th = theme();
    // Use cached dependencies if available, otherwise resolve on-demand
    // Note: Cached deps are populated in background when packages are added to install list
    if dependency_info.is_empty() && matches!(*action, PreflightAction::Install) {
        // Check if we have cached dependencies from app state
        // (This would require passing app state, but for now we resolve on-demand as fallback)
        tracing::info!(
            "[UI] Starting dependency resolution for {} packages in Preflight modal",
            items.len()
        );
        let start_time = std::time::Instant::now();
        *dependency_info = crate::logic::deps::resolve_dependencies(items);
        let elapsed = start_time.elapsed();
        tracing::info!(
            "[UI] Dependency resolution completed in {:?}. Found {} dependencies",
            elapsed,
            dependency_info.len()
        );
        *dep_selected = 0;
    }
    // Lazy load file info when Files tab is accessed (use cached files if available)
    // Note: Cached files are populated in background when packages are added to install list
    if file_info.is_empty() && matches!(*tab, PreflightTab::Files) {
        // Check if we have cached files from app state
        // (This would require passing app state, but for now we resolve on-demand as fallback)
        tracing::info!(
            "[UI] Starting file resolution for {} packages in Preflight modal",
            items.len()
        );
        let start_time = std::time::Instant::now();
        *file_info = crate::logic::files::resolve_file_changes(items, *action);
        let elapsed = start_time.elapsed();
        tracing::info!(
            "[UI] File resolution completed in {:?}. Found {} package file infos",
            elapsed,
            file_info.len()
        );
        *file_selected = 0;
    }
    // Removed verbose rendering log
    let w = area.width.saturating_sub(6).min(96);
    let h = area.height.saturating_sub(8).min(22);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    // Split rect into content area and keybinds pane (reserve 4 lines for keybinds to account for borders)
    // With double borders, we need: 1 top border + 2 content lines + 1 bottom border = 4 lines minimum
    let keybinds_height = 4;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(keybinds_height)])
        .split(rect);
    let content_rect = layout[0];
    let keybinds_rect = layout[1];

    let title = match action {
        PreflightAction::Install => " Preflight: Install ",
        PreflightAction::Remove => " Preflight: Remove ",
    };
    let border_color = th.lavender;
    let bg_color = th.crust;

    // Build header tab labels and calculate tab rectangles for mouse clicks
    let tab_labels = ["Summary", "Deps", "Files", "Services", "Sandbox"];
    let mut header = String::new();
    let current_tab = *tab;

    // Calculate tab rectangles for mouse click detection
    // Tab header is on the first line of content_rect (after border)
    let tab_y = content_rect.y + 1; // +1 for top border
    let mut tab_x = content_rect.x + 1; // +1 for left border
    app.preflight_tab_rects = [None; 5];

    for (i, lbl) in tab_labels.iter().enumerate() {
        let is = matches!(
            (i, current_tab),
            (0, PreflightTab::Summary)
                | (1, PreflightTab::Deps)
                | (2, PreflightTab::Files)
                | (3, PreflightTab::Services)
                | (4, PreflightTab::Sandbox)
        );
        if i > 0 {
            header.push_str("  ");
            tab_x += 2; // Account for spacing
        }

        // Calculate tab width (with brackets if active)
        let tab_width = if is {
            lbl.len() + 2 // [label]
        } else {
            lbl.len()
        } as u16;

        // Store rectangle for this tab
        app.preflight_tab_rects[i] = Some((tab_x, tab_y, tab_width, 1));
        tab_x += tab_width;

        if is {
            header.push('[');
            header.push_str(lbl);
            header.push(']');
        } else {
            header.push_str(lbl);
        }
    }

    // Store content area rectangle for package group click detection
    // Content area starts after the header (2 lines: header + empty line)
    app.preflight_content_rect = Some((
        content_rect.x + 1,                    // +1 for left border
        content_rect.y + 3,                    // +1 for top border + 2 for header lines
        content_rect.width.saturating_sub(2),  // -2 for borders
        content_rect.height.saturating_sub(3), // -1 for top border - 2 for header lines
    ));

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    match current_tab {
        PreflightTab::Summary => match *action {
            PreflightAction::Install if !dependency_info.is_empty() => {
                // Filter dependencies to only show conflicts and upgrades
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
                        "No conflicts or upgrades required.",
                        Style::default().fg(th.green),
                    )));
                } else {
                    // Group by packages that require them
                    use std::collections::{HashMap, HashSet};
                    let mut grouped: HashMap<String, Vec<&DependencyInfo>> = HashMap::new();
                    for dep in important_deps.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    // Count conflicts and upgrades
                    let conflict_count = important_deps
                        .iter()
                        .filter(|d| matches!(d.status, DependencyStatus::Conflict { .. }))
                        .count();
                    let upgrade_count = important_deps
                        .iter()
                        .filter(|d| matches!(d.status, DependencyStatus::ToUpgrade { .. }))
                        .count();

                    // Summary header
                    let mut summary_parts = Vec::new();
                    if conflict_count > 0 {
                        summary_parts.push(format!("{} conflict(s)", conflict_count));
                    }
                    if upgrade_count > 0 {
                        summary_parts.push(format!("{} upgrade(s)", upgrade_count));
                    }

                    // Use different header based on what we have
                    let header_text = if conflict_count > 0 {
                        format!("Issues: {}", summary_parts.join(", "))
                    } else if upgrade_count > 0 {
                        format!("Summary: {}", summary_parts.join(", "))
                    } else {
                        "Summary: No conflicts or upgrades required.".to_string()
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

                    // Display grouped dependencies
                    let available_height = (content_rect.height as usize).saturating_sub(6);
                    let mut displayed = 0;
                    for pkg_name in items.iter().map(|p| &p.name) {
                        if let Some(pkg_deps) = grouped.get(pkg_name) {
                            if displayed >= available_height {
                                break;
                            }
                            // Package header
                            lines.push(Line::from(Span::styled(
                                format!("▶ {}", pkg_name),
                                Style::default()
                                    .fg(th.overlay1)
                                    .add_modifier(Modifier::BOLD),
                            )));
                            displayed += 1;

                            // Deduplicate dependencies within this package's group
                            let mut seen_deps = HashSet::new();
                            for dep in pkg_deps.iter() {
                                if seen_deps.insert(dep.name.as_str())
                                    && displayed < available_height
                                {
                                    let mut spans = Vec::new();
                                    spans.push(Span::styled("  ", Style::default())); // Indentation

                                    // Status indicator and dependency info
                                    match &dep.status {
                                        DependencyStatus::Conflict { reason } => {
                                            spans.push(Span::styled(
                                                "⚠ ",
                                                Style::default().fg(th.red),
                                            ));
                                            spans.push(Span::styled(
                                                dep.name.clone(),
                                                Style::default().fg(th.text),
                                            ));
                                            // Version requirement
                                            if !dep.version.is_empty() {
                                                spans.push(Span::styled(
                                                    format!(" {}", dep.version),
                                                    Style::default().fg(th.overlay2),
                                                ));
                                            }
                                            // Source badge
                                            let (source_badge, badge_color) = match &dep.source {
                                                DependencySource::Official { repo } => {
                                                    let repo_lower = repo.to_lowercase();
                                                    let color =
                                                        if crate::index::is_eos_repo(&repo_lower)
                                                            || crate::index::is_cachyos_repo(
                                                                &repo_lower,
                                                            )
                                                        {
                                                            th.sapphire
                                                        } else {
                                                            th.green
                                                        };
                                                    (format!(" [{}]", repo), color)
                                                }
                                                DependencySource::Aur => {
                                                    (" [AUR]".to_string(), th.yellow)
                                                }
                                                DependencySource::Local => {
                                                    (" [local]".to_string(), th.overlay1)
                                                }
                                            };
                                            spans.push(Span::styled(
                                                source_badge,
                                                Style::default().fg(badge_color),
                                            ));
                                            spans.push(Span::styled(
                                                format!(" ({})", reason),
                                                Style::default().fg(th.red),
                                            ));
                                        }
                                        DependencyStatus::ToUpgrade { current, required } => {
                                            spans.push(Span::styled(
                                                "↑ ",
                                                Style::default().fg(th.yellow),
                                            ));
                                            spans.push(Span::styled(
                                                dep.name.clone(),
                                                Style::default().fg(th.text),
                                            ));
                                            // Version requirement
                                            if !dep.version.is_empty() {
                                                spans.push(Span::styled(
                                                    format!(" {}", dep.version),
                                                    Style::default().fg(th.overlay2),
                                                ));
                                            }
                                            // Source badge
                                            let (source_badge, badge_color) = match &dep.source {
                                                DependencySource::Official { repo } => {
                                                    let repo_lower = repo.to_lowercase();
                                                    let color =
                                                        if crate::index::is_eos_repo(&repo_lower)
                                                            || crate::index::is_cachyos_repo(
                                                                &repo_lower,
                                                            )
                                                        {
                                                            th.sapphire
                                                        } else {
                                                            th.green
                                                        };
                                                    (format!(" [{}]", repo), color)
                                                }
                                                DependencySource::Aur => {
                                                    (" [AUR]".to_string(), th.yellow)
                                                }
                                                DependencySource::Local => {
                                                    (" [local]".to_string(), th.overlay1)
                                                }
                                            };
                                            spans.push(Span::styled(
                                                source_badge,
                                                Style::default().fg(badge_color),
                                            ));
                                            spans.push(Span::styled(
                                                format!(" ({} → {})", current, required),
                                                Style::default().fg(th.yellow),
                                            ));
                                        }
                                        _ => continue, // Shouldn't happen due to filter, but be safe
                                    }

                                    displayed += 1;
                                    lines.push(Line::from(spans));
                                }
                            }
                        }
                    }

                    if displayed >= available_height && important_deps.len() > displayed {
                        lines.push(Line::from(""));
                        lines.push(Line::from(Span::styled(
                            format!("... and {} more", important_deps.len() - displayed),
                            Style::default().fg(th.subtext1),
                        )));
                    }
                }
            }
            PreflightAction::Remove => {
                let mode = cascade_mode;
                let mode_line = format!("Cascade mode: {} ({})", mode.flag(), mode.description());
                lines.push(Line::from(Span::styled(
                    mode_line,
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));

                if items.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No removal targets selected.",
                        Style::default().fg(th.subtext1),
                    )));
                } else {
                    let removal_names: Vec<&str> =
                        items.iter().map(|pkg| pkg.name.as_str()).collect();
                    let plan_header_style = Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD);
                    lines.push(Line::from(Span::styled(
                        "Removal plan preview",
                        plan_header_style,
                    )));

                    let mut plan_command = format!(
                        "sudo pacman {} --noconfirm {}",
                        mode.flag(),
                        removal_names.join(" ")
                    );
                    if app.dry_run {
                        plan_command = format!("DRY RUN: {}", plan_command);
                    }
                    lines.push(Line::from(Span::styled(
                        plan_command,
                        Style::default().fg(th.text),
                    )));

                    let dependent_count = dependency_info.len();
                    let (summary_text, summary_style) = if dependent_count == 0 {
                        (
                            "No installed packages depend on the removal list.".to_string(),
                            Style::default().fg(th.green),
                        )
                    } else if mode.allows_dependents() {
                        (
                            format!("Cascade will include {dependent_count} dependent package(s)."),
                            Style::default().fg(th.yellow),
                        )
                    } else {
                        (
                            format!(
                                "{dependent_count} dependent package(s) currently block removal."
                            ),
                            Style::default().fg(th.red),
                        )
                    };
                    lines.push(Line::from(Span::styled(summary_text, summary_style)));
                    lines.push(Line::from(""));

                    if dependent_count > 0 {
                        if app.remove_preflight_summary.is_empty() {
                            lines.push(Line::from(Span::styled(
                                "Calculating reverse dependencies...",
                                Style::default().fg(th.subtext1),
                            )));
                        } else {
                            lines.push(Line::from(Span::styled(
                                "Removal impact overview:",
                                Style::default()
                                    .fg(th.overlay1)
                                    .add_modifier(Modifier::BOLD),
                            )));
                            lines.push(Line::from(""));

                            for summary in &app.remove_preflight_summary {
                                let mut message = format!(
                                    "{} → {} dependent(s)",
                                    summary.package, summary.total_dependents
                                );
                                if summary.direct_dependents > 0 {
                                    message.push_str(&format!(
                                        " ({} direct)",
                                        summary.direct_dependents
                                    ));
                                }
                                if summary.transitive_dependents > 0 {
                                    message.push_str(&format!(
                                        " ({} transitive)",
                                        summary.transitive_dependents
                                    ));
                                }
                                lines.push(Line::from(Span::styled(
                                    message,
                                    Style::default().fg(th.text),
                                )));
                            }
                            lines.push(Line::from(""));
                        }

                        let (impact_header, impact_style) = if mode.allows_dependents() {
                            (
                                "Cascade will also remove these package(s):".to_string(),
                                Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                            )
                        } else {
                            (
                                "Dependents (not removed in current mode):".to_string(),
                                Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                            )
                        };
                        lines.push(Line::from(Span::styled(impact_header, impact_style)));

                        let removal_targets: HashSet<String> = items
                            .iter()
                            .map(|pkg| pkg.name.to_ascii_lowercase())
                            .collect();
                        let mut cascade_candidates: Vec<&DependencyInfo> =
                            dependency_info.iter().collect();
                        cascade_candidates.sort_by(|a, b| {
                            let a_direct = a.depends_on.iter().any(|parent| {
                                removal_targets.contains(&parent.to_ascii_lowercase())
                            });
                            let b_direct = b.depends_on.iter().any(|parent| {
                                removal_targets.contains(&parent.to_ascii_lowercase())
                            });
                            b_direct.cmp(&a_direct).then_with(|| a.name.cmp(&b.name))
                        });

                        const CASCADE_PREVIEW_MAX: usize = 8;
                        for dep in cascade_candidates.iter().take(CASCADE_PREVIEW_MAX) {
                            let is_direct = dep.depends_on.iter().any(|parent| {
                                removal_targets.contains(&parent.to_ascii_lowercase())
                            });
                            let bullet = if mode.allows_dependents() {
                                if is_direct { "● " } else { "○ " }
                            } else if is_direct {
                                "⛔ "
                            } else {
                                "⚠ "
                            };
                            let name_color = if mode.allows_dependents() {
                                if is_direct { th.red } else { th.yellow }
                            } else if is_direct {
                                th.red
                            } else {
                                th.yellow
                            };
                            let name_style =
                                Style::default().fg(name_color).add_modifier(Modifier::BOLD);
                            let detail = match &dep.status {
                                DependencyStatus::Conflict { reason } => reason.clone(),
                                DependencyStatus::ToUpgrade { .. } => {
                                    "requires version change".to_string()
                                }
                                DependencyStatus::Installed { .. } => {
                                    "already satisfied".to_string()
                                }
                                DependencyStatus::ToInstall => {
                                    "not currently installed".to_string()
                                }
                                DependencyStatus::Missing => "missing".to_string(),
                            };
                            let roots = if dep.required_by.is_empty() {
                                String::new()
                            } else {
                                format!(" (targets: {})", dep.required_by.join(", "))
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
                                format!(
                                    "... and {} more impacted package(s)",
                                    cascade_candidates.len() - CASCADE_PREVIEW_MAX
                                ),
                                Style::default().fg(th.subtext1),
                            )));
                        }

                        lines.push(Line::from(""));
                        if mode.allows_dependents() {
                            lines.push(Line::from(Span::styled(
                                "These packages will be removed automatically when the command runs.",
                                Style::default().fg(th.subtext1),
                            )));
                        } else {
                            lines.push(Line::from(Span::styled(
                                "Enable cascade mode (press 'm') to include them automatically.",
                                Style::default().fg(th.subtext1),
                            )));
                        }
                        lines.push(Line::from(Span::styled(
                            "Use the Deps tab to inspect affected packages.",
                            Style::default().fg(th.subtext1),
                        )));
                    }
                }
            }
            _ => {
                if items.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No items selected.",
                        Style::default().fg(th.subtext1),
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        format!("{} package(s) selected", items.len()),
                        Style::default().fg(th.text),
                    )));
                }
            }
        },
        PreflightTab::Deps => {
            // Use already resolved dependencies (resolved above if needed)
            let deps = dependency_info;

            // Group dependencies by the packages that require them
            use std::collections::HashMap;
            let mut grouped: HashMap<String, Vec<&DependencyInfo>> = HashMap::new();
            for dep in deps.iter() {
                for req_by in &dep.required_by {
                    grouped.entry(req_by.clone()).or_default().push(dep);
                }
            }

            // Calculate summary statistics
            let total = deps.len();
            let installed_count = deps
                .iter()
                .filter(|d| matches!(d.status, DependencyStatus::Installed { .. }))
                .count();
            let to_install_count = deps
                .iter()
                .filter(|d| matches!(d.status, DependencyStatus::ToInstall))
                .count();
            let to_upgrade_count = deps
                .iter()
                .filter(|d| matches!(d.status, DependencyStatus::ToUpgrade { .. }))
                .count();
            let conflict_count = deps
                .iter()
                .filter(|d| matches!(d.status, DependencyStatus::Conflict { .. }))
                .count();
            let missing_count = deps
                .iter()
                .filter(|d| matches!(d.status, DependencyStatus::Missing))
                .count();

            // Summary header
            if total > 0 {
                if matches!(*action, PreflightAction::Remove) {
                    lines.push(Line::from(Span::styled(
                        format!("Dependents: {} package(s) rely on the removal list", total),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(""));
                } else {
                    let mut summary_parts = Vec::new();
                    summary_parts.push(format!("{} total", total));
                    if installed_count > 0 {
                        summary_parts.push(format!("{} installed", installed_count));
                    }
                    if to_install_count > 0 {
                        summary_parts.push(format!("{} to install", to_install_count));
                    }
                    if to_upgrade_count > 0 {
                        summary_parts.push(format!("{} to upgrade", to_upgrade_count));
                    }
                    if conflict_count > 0 {
                        summary_parts.push(format!("{} conflicts", conflict_count));
                    }
                    if missing_count > 0 {
                        summary_parts.push(format!("{} missing", missing_count));
                    }
                    lines.push(Line::from(Span::styled(
                        format!("Dependencies: {}", summary_parts.join(", ")),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(""));
                }
            } else if matches!(*action, PreflightAction::Install) {
                lines.push(Line::from(Span::styled(
                    "Resolving dependencies...",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "No dependencies to show for removal operation.",
                    Style::default().fg(th.subtext1),
                )));
            }

            // Build flat list with grouped structure for navigation
            // Format: [package_name, dep1, dep2, ...] for each package
            let mut display_items: Vec<(bool, String, Option<&DependencyInfo>)> = Vec::new();
            for pkg_name in items.iter().map(|p| &p.name) {
                if let Some(pkg_deps) = grouped.get(pkg_name) {
                    // Add package header
                    let is_expanded = dep_tree_expanded.contains(pkg_name);
                    display_items.push((true, pkg_name.clone(), None));
                    // Add its dependencies only if expanded (deduplicate within this package's group)
                    if is_expanded {
                        use std::collections::HashSet;
                        let mut seen_deps = HashSet::new();
                        for dep in pkg_deps.iter() {
                            if seen_deps.insert(dep.name.as_str()) {
                                display_items.push((false, String::new(), Some(dep)));
                            }
                        }
                    }
                }
            }

            // Dependency list with grouping
            let available_height = (content_rect.height as usize).saturating_sub(6);
            let total_items = display_items.len();
            let dep_selected_clamped = (*dep_selected).min(total_items.saturating_sub(1));
            if *dep_selected != dep_selected_clamped {
                *dep_selected = dep_selected_clamped;
            }

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
                                format!(" (installed: {})", version),
                                Style::default().fg(th.subtext1),
                            ));
                        }
                        DependencyStatus::ToUpgrade { current, required } => {
                            spans.push(Span::styled(
                                format!(" ({} → {})", current, required),
                                Style::default().fg(th.yellow),
                            ));
                        }
                        DependencyStatus::Conflict { reason } => {
                            spans.push(Span::styled(
                                format!(" ({})", reason),
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
                    format!(
                        "... showing {}-{} of {}",
                        start_idx + 1,
                        end_idx,
                        display_items.len()
                    ),
                    Style::default().fg(th.subtext1),
                )));
            }
        }
        PreflightTab::Files => {
            if file_info.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Resolving file changes...",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                // Build flat list of display items: package headers + files (only if expanded)
                // Store owned data to avoid lifetime issues
                type FileDisplayItem = (
                    bool,
                    String,
                    Option<(FileChangeType, String, bool, bool, bool)>,
                );
                let mut display_items: Vec<FileDisplayItem> = Vec::new();
                for pkg_info in file_info.iter() {
                    if !pkg_info.files.is_empty() {
                        let is_expanded = file_tree_expanded.contains(&pkg_info.name);
                        display_items.push((true, pkg_info.name.clone(), None)); // Package header
                        // Add files only if package is expanded
                        if is_expanded {
                            for file in pkg_info.files.iter() {
                                display_items.push((
                                    false,
                                    String::new(),
                                    Some((
                                        file.change_type.clone(),
                                        file.path.clone(),
                                        file.is_config,
                                        file.predicted_pacnew,
                                        file.predicted_pacsave,
                                    )),
                                )); // File entry
                            }
                        }
                    }
                }

                let sync_info = crate::logic::files::get_file_db_sync_info();

                if display_items.is_empty() {
                    // Check if we have package entries but they're all empty
                    let has_aur_packages = items
                        .iter()
                        .any(|p| matches!(p.source, crate::state::Source::Aur));
                    let has_official_packages = items
                        .iter()
                        .any(|p| matches!(p.source, crate::state::Source::Official { .. }));

                    // Count packages with empty file lists
                    let mut unresolved_packages = Vec::new();
                    for pkg_info in file_info.iter() {
                        if pkg_info.files.is_empty() {
                            unresolved_packages.push(pkg_info.name.clone());
                        }
                    }

                    if !file_info.is_empty() {
                        // File resolution completed but no files found
                        if !unresolved_packages.is_empty() {
                            lines.push(Line::from(Span::styled(
                                format!(
                                    "No file changes found for {} package(s).",
                                    unresolved_packages.len()
                                ),
                                Style::default().fg(th.subtext1),
                            )));
                            lines.push(Line::from(""));

                            // Show appropriate notes based on package types
                            if has_official_packages {
                                lines.push(Line::from(Span::styled(
                                            "Note: File database may need syncing (pacman -Fy requires root).",
                                            Style::default().fg(th.subtext0),
                                        )));
                                lines.push(Line::from(Span::styled(
                                    "Press 'f' to sync file database in a terminal.",
                                    Style::default().fg(th.subtext0),
                                )));
                            }
                            if has_aur_packages {
                                lines.push(Line::from(Span::styled(
                                    "Note: AUR packages require building to determine file lists.",
                                    Style::default().fg(th.subtext0),
                                )));
                            }
                        } else {
                            lines.push(Line::from(Span::styled(
                                "No file changes to display.",
                                Style::default().fg(th.subtext1),
                            )));
                        }
                    } else {
                        // File resolution hasn't completed or failed
                        lines.push(Line::from(Span::styled(
                            "No file changes to display.",
                            Style::default().fg(th.subtext1),
                        )));
                    }

                    // Show file database sync timestamp
                    if let Some((_age_days, date_str, color_category)) = sync_info.clone() {
                        lines.push(Line::from(""));
                        let (sync_color, sync_text) = match color_category {
                            0 => (th.green, format!("Files updated on {}", date_str)),
                            1 => (th.yellow, format!("Files updated on {}", date_str)),
                            _ => (th.red, format!("Files updated on {}", date_str)),
                        };
                        lines.push(Line::from(Span::styled(
                            sync_text,
                            Style::default().fg(sync_color),
                        )));
                    }
                } else {
                    // Display summary first (needed to calculate available_height accurately)
                    let total_files: usize = file_info.iter().map(|p| p.total_count).sum();
                    let total_new: usize = file_info.iter().map(|p| p.new_count).sum();
                    let total_changed: usize = file_info.iter().map(|p| p.changed_count).sum();
                    let total_removed: usize = file_info.iter().map(|p| p.removed_count).sum();
                    let total_config: usize = file_info.iter().map(|p| p.config_count).sum();
                    let total_pacnew: usize = file_info.iter().map(|p| p.pacnew_candidates).sum();
                    let total_pacsave: usize = file_info.iter().map(|p| p.pacsave_candidates).sum();

                    let mut summary_parts = vec![format!("{} total", total_files)];
                    if total_new > 0 {
                        summary_parts.push(format!("{} new", total_new));
                    }
                    if total_changed > 0 {
                        summary_parts.push(format!("{} changed", total_changed));
                    }
                    if total_removed > 0 {
                        summary_parts.push(format!("{} removed", total_removed));
                    }
                    if total_config > 0 {
                        summary_parts.push(format!("{} config", total_config));
                    }
                    if total_pacnew > 0 {
                        summary_parts.push(format!("{} pacnew", total_pacnew));
                    }
                    if total_pacsave > 0 {
                        summary_parts.push(format!("{} pacsave", total_pacsave));
                    }

                    lines.push(Line::from(Span::styled(
                        format!("Files: {}", summary_parts.join(", ")),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(""));

                    // Show file database sync timestamp
                    let sync_timestamp_lines =
                        if let Some((_age_days, date_str, color_category)) = sync_info.clone() {
                            let (sync_color, sync_text) = match color_category {
                                0 => (th.green, format!("Files updated on {}", date_str)),
                                1 => (th.yellow, format!("Files updated on {}", date_str)),
                                _ => (th.red, format!("Files updated on {}", date_str)),
                            };
                            lines.push(Line::from(Span::styled(
                                sync_text,
                                Style::default().fg(sync_color),
                            )));
                            lines.push(Line::from(""));
                            2 // timestamp line + empty line
                        } else {
                            0
                        };

                    // Calculate available height for file list AFTER adding summary and sync timestamp
                    // Lines used before file list: tab header (1) + empty (1) + summary (1) + empty (1) + sync timestamp (0-2)
                    // Total: 4-6 lines
                    let header_lines = 4 + sync_timestamp_lines;
                    let available_height = (content_rect.height.saturating_sub(1) as usize)
                        .saturating_sub(header_lines)
                        .max(1);

                    // Calculate scroll position
                    let total_items = display_items.len();
                    // Clamp file_selected to valid range
                    let file_selected_clamped = (*file_selected).min(total_items.saturating_sub(1));
                    if *file_selected != file_selected_clamped {
                        *file_selected = file_selected_clamped;
                    }
                    // Only scroll if there are more items than can fit on screen
                    let (start_idx, end_idx) = if total_items <= available_height {
                        // All items fit - show everything starting from 0
                        (0, total_items)
                    } else {
                        // More items than fit - center selected item or scroll to show it
                        let start = file_selected_clamped
                            .saturating_sub(available_height / 2)
                            .min(total_items.saturating_sub(available_height));
                        let end = (start + available_height).min(total_items);
                        (start, end)
                    };

                    // Display files with scrolling
                    for (display_idx, (is_header, pkg_name, file_opt)) in display_items
                        .iter()
                        .enumerate()
                        .skip(start_idx)
                        .take(end_idx - start_idx)
                    {
                        let is_selected = display_idx == *file_selected;
                        if *is_header {
                            // Find package info for this header
                            let pkg_info = file_info.iter().find(|p| p.name == *pkg_name).unwrap();
                            let is_expanded = file_tree_expanded.contains(pkg_name);

                            // Package header with expand/collapse indicator
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

                            let mut spans = vec![
                                Span::styled(
                                    format!("{} {} ", arrow_symbol, pkg_name),
                                    header_style,
                                ),
                                Span::styled(
                                    format!("({} files", pkg_info.total_count),
                                    Style::default().fg(th.subtext1),
                                ),
                            ];

                            if pkg_info.new_count > 0 {
                                spans.push(Span::styled(
                                    format!(", {} new", pkg_info.new_count),
                                    Style::default().fg(th.green),
                                ));
                            }
                            if pkg_info.changed_count > 0 {
                                spans.push(Span::styled(
                                    format!(", {} changed", pkg_info.changed_count),
                                    Style::default().fg(th.yellow),
                                ));
                            }
                            if pkg_info.removed_count > 0 {
                                spans.push(Span::styled(
                                    format!(", {} removed", pkg_info.removed_count),
                                    Style::default().fg(th.red),
                                ));
                            }
                            if pkg_info.config_count > 0 {
                                spans.push(Span::styled(
                                    format!(", {} config", pkg_info.config_count),
                                    Style::default().fg(th.mauve),
                                ));
                            }
                            if pkg_info.pacnew_candidates > 0 {
                                spans.push(Span::styled(
                                    format!(", {} pacnew", pkg_info.pacnew_candidates),
                                    Style::default().fg(th.yellow),
                                ));
                            }
                            if pkg_info.pacsave_candidates > 0 {
                                spans.push(Span::styled(
                                    format!(", {} pacsave", pkg_info.pacsave_candidates),
                                    Style::default().fg(th.red),
                                ));
                            }
                            spans.push(Span::styled(")", Style::default().fg(th.subtext1)));

                            lines.push(Line::from(spans));
                        } else if let Some((
                            change_type,
                            path,
                            is_config,
                            predicted_pacnew,
                            predicted_pacsave,
                        )) = file_opt
                        {
                            // File entry
                            let (icon, color) = match change_type {
                                FileChangeType::New => ("+", th.green),
                                FileChangeType::Changed => ("~", th.yellow),
                                FileChangeType::Removed => ("-", th.red),
                            };

                            let highlight_bg = if is_selected { Some(th.lavender) } else { None };
                            let icon_style = if let Some(bg) = highlight_bg {
                                Style::default().fg(color).bg(bg)
                            } else {
                                Style::default().fg(color)
                            };
                            let mut spans = vec![Span::styled(format!("  {} ", icon), icon_style)];

                            if *is_config {
                                let cfg_style = if let Some(bg) = highlight_bg {
                                    Style::default().fg(th.mauve).bg(bg)
                                } else {
                                    Style::default().fg(th.mauve)
                                };
                                spans.push(Span::styled("⚙ ", cfg_style));
                            }

                            // Add pacnew/pacsave indicators
                            if *predicted_pacnew {
                                let pacnew_style = if let Some(bg) = highlight_bg {
                                    Style::default().fg(th.yellow).bg(bg)
                                } else {
                                    Style::default().fg(th.yellow)
                                };
                                spans.push(Span::styled("⚠ pacnew ", pacnew_style));
                            }
                            if *predicted_pacsave {
                                let pacsave_style = if let Some(bg) = highlight_bg {
                                    Style::default().fg(th.red).bg(bg)
                                } else {
                                    Style::default().fg(th.red)
                                };
                                spans.push(Span::styled("⚠ pacsave ", pacsave_style));
                            }

                            let path_style = if let Some(bg) = highlight_bg {
                                Style::default()
                                    .fg(th.crust)
                                    .bg(bg)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(th.text)
                            };

                            spans.push(Span::styled(path.clone(), path_style));

                            lines.push(Line::from(spans));
                        }
                    }

                    if total_items > available_height {
                        lines.push(Line::from(""));
                        lines.push(Line::from(Span::styled(
                            format!(
                                "... showing {}-{} of {} items (↑↓ to navigate)",
                                start_idx + 1,
                                end_idx,
                                total_items
                            ),
                            Style::default().fg(th.subtext1),
                        )));
                    }
                }
            }
        }
        PreflightTab::Services => {
            lines.push(Line::from(Span::styled(
                "Services (placeholder) — impacted services/restarts will appear here",
                Style::default().fg(th.text),
            )));
        }
        PreflightTab::Sandbox => {
            lines.push(Line::from(Span::styled(
                "Sandbox (placeholder) — AUR preflight build checks will appear here",
                Style::default().fg(th.text),
            )));
        }
    }

    // Render content area (no bottom border - keybinds pane will have top border)
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(bg_color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(bg_color)),
        );
    f.render_widget(boxw, content_rect);

    // Render keybinds pane at the bottom
    // Check if any AUR packages are present for scanning
    let has_aur = items
        .iter()
        .any(|p| matches!(p.source, crate::state::Source::Aur));

    // Build footer hint based on current tab
    let mut scan_hint = match current_tab {
        PreflightTab::Deps => {
            if has_aur {
                "Left/Right: tabs  •  Up/Down: navigate  •  Enter/Space: toggle  •  a: expand/collapse all  •  ?: help  •  s: scan AUR  •  d: dry-run  •  p: proceed  •  q: close"
            } else {
                "Left/Right: tabs  •  Up/Down: navigate  •  Enter/Space: toggle  •  a: expand/collapse all  •  ?: help  •  d: dry-run  •  p: proceed  •  q: close"
            }
        }
        PreflightTab::Files => {
            if has_aur {
                "Left/Right: tabs  •  Up/Down: navigate  •  Enter/Space: expand/collapse  •  a: expand/collapse all  •  f: sync file DB  •  s: scan AUR  •  d: dry-run  •  p: proceed  •  q: close"
            } else {
                "Left/Right: tabs  •  Up/Down: navigate  •  Enter/Space: expand/collapse  •  a: expand/collapse all  •  f: sync file DB  •  d: dry-run  •  p: proceed  •  q: close"
            }
        }
        _ => {
            if has_aur {
                "Left/Right: tabs  •  s: scan AUR  •  d: dry-run  •  p: proceed  •  q: close"
            } else {
                "Left/Right: tabs  •  d: dry-run  •  p: proceed  •  q: close"
            }
        }
    }
    .to_string();

    if matches!(*action, PreflightAction::Remove) {
        scan_hint.push_str("  •  m: cascade mode");
    }

    let keybinds_lines = vec![
        Line::from(""), // Empty line for spacing
        Line::from(Span::styled(scan_hint, Style::default().fg(th.subtext1))),
    ];

    // Adjust keybinds rect to start exactly where content rect ends (no gap)
    let keybinds_rect_adjusted = Rect {
        x: keybinds_rect.x,
        y: content_rect.y + content_rect.height,
        width: keybinds_rect.width,
        height: keybinds_rect.height,
    };

    let keybinds_widget = Paragraph::new(keybinds_lines)
        .style(Style::default().fg(th.text).bg(bg_color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(bg_color)),
        );
    f.render_widget(keybinds_widget, keybinds_rect_adjusted);
}
