use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::AppState;
use crate::theme::{KeyChord, theme};

/// What: Render modal overlays (Alert, ConfirmInstall, ConfirmRemove, SystemUpdate, Help, News).
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (modal state, rects)
/// - `area`: Full available area; modals are centered within it
///
/// Output:
/// - Draws the active modal overlay and updates any modal-specific rects for hit-testing.
///
/// Details:
/// - Clears the area behind the modal; draws a styled centered box; content varies by modal.
/// - Help dynamically reflects keymap; News draws a selectable list and records list rect.
pub fn render_modals(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    // Draw a full-screen scrim behind any active modal to avoid underlying text bleed/concatenation
    if !matches!(app.modal, crate::state::Modal::None) {
        let scrim = Block::default().style(Style::default().bg(th.mantle));
        f.render_widget(scrim, area);
    }

    match &mut app.modal {
        crate::state::Modal::Alert { message } => {
            // Detect help messages and make them larger
            let is_help = message.contains("Help") || message.contains("Tab Help");
            let w = area
                .width
                .saturating_sub(10)
                .min(if is_help { 90 } else { 80 });
            let h = if is_help {
                area.height.saturating_sub(6).min(28)
            } else {
                7
            };
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            // Choose labels depending on error type (config vs network/other)
            let is_config = message.contains("Unknown key")
                || message.contains("Missing required keys")
                || message.contains("Missing '='")
                || message.contains("Missing key before '='")
                || message.contains("Duplicate key")
                || message.contains("Invalid color")
                || message.to_lowercase().contains("theme configuration");
            let clippy_block = {
                let ml = message.to_lowercase();
                ml.contains("clipboard")
                    || ml.contains("wl-copy")
                    || ml.contains("xclip")
                    || ml.contains("wl-clipboard")
            };
            let header_text = if is_help {
                "Help"
            } else if is_config {
                "Configuration error"
            } else if clippy_block {
                "Clipboard Copy"
            } else {
                "Connection issue"
            };
            let is_clipboard = {
                let ml = message.to_lowercase();
                ml.contains("clipboard")
                    || ml.contains("wl-copy")
                    || ml.contains("xclip")
                    || ml.contains("wl-clipboard")
            };
            let box_title = if is_help {
                " Help "
            } else if is_config {
                " Configuration Error "
            } else if is_clipboard {
                " Clipboard Copy "
            } else {
                " Connection issue "
            };
            let header_color = if is_help || is_config {
                th.mauve
            } else {
                th.red
            };

            // Parse message into lines for help messages
            let mut lines: Vec<Line<'static>> = Vec::new();
            if is_help {
                for line in message.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(th.text),
                    )));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    header_text,
                    Style::default()
                        .fg(header_color)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    message.clone(),
                    Style::default().fg(th.text),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter or Esc to close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .scroll((if is_help { app.help_scroll } else { 0 }, 0))
                .block(
                    Block::default()
                        .title(Span::styled(
                            box_title,
                            Style::default()
                                .fg(header_color)
                                .add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(header_color))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::ConfirmInstall { items } => {
            let w = area.width.saturating_sub(6).min(90);
            let h = area.height.saturating_sub(6).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Confirm installation",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Nothing to install",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for p in items.iter().take((h as usize).saturating_sub(6)) {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", p.name),
                        Style::default().fg(th.text),
                    )));
                }
                if items.len() + 6 > h as usize {
                    lines.push(Line::from(Span::styled(
                        "…",
                        Style::default().fg(th.subtext1),
                    )));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter to confirm or Esc to cancel",
                Style::default().fg(th.subtext1),
            )));
            lines.push(Line::from(Span::styled(
                "Press S to scan AUR package(s) before install",
                Style::default().fg(th.overlay1),
            )));
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Confirm Install ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::Preflight {
            items,
            action,
            tab,
            dependency_info,
            dep_selected,
            dep_tree_expanded,
            file_info,
            file_selected,
        } => {
            // Use cached dependencies if available, otherwise resolve on-demand
            // Note: Cached deps are populated in background when packages are added to install list
            if dependency_info.is_empty()
                && matches!(*action, crate::state::PreflightAction::Install)
            {
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
            // Lazy load file info when Files tab is accessed
            if file_info.is_empty() && *tab == crate::state::PreflightTab::Files {
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

            let title = match action {
                crate::state::PreflightAction::Install => " Preflight: Install ",
                crate::state::PreflightAction::Remove => " Preflight: Remove ",
            };
            let border_color = th.lavender;
            let bg_color = th.crust;

            // Build header tab labels
            let tab_labels = ["Summary", "Deps", "Files", "Services", "Sandbox"];
            let mut header = String::new();
            let current_tab = *tab;
            for (i, lbl) in tab_labels.iter().enumerate() {
                let is = matches!(
                    (i, current_tab),
                    (0, crate::state::PreflightTab::Summary)
                        | (1, crate::state::PreflightTab::Deps)
                        | (2, crate::state::PreflightTab::Files)
                        | (3, crate::state::PreflightTab::Services)
                        | (4, crate::state::PreflightTab::Sandbox)
                );
                if i > 0 {
                    header.push_str("  ");
                }
                if is {
                    header.push('[');
                    header.push_str(lbl);
                    header.push(']');
                } else {
                    header.push_str(lbl);
                }
            }

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                header,
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            match current_tab {
                crate::state::PreflightTab::Summary => {
                    if matches!(*action, crate::state::PreflightAction::Install)
                        && !dependency_info.is_empty()
                    {
                        // Filter dependencies to only show conflicts and upgrades
                        let important_deps: Vec<&crate::state::modal::DependencyInfo> = dependency_info
                            .iter()
                            .filter(|d| {
                                matches!(d.status, crate::state::modal::DependencyStatus::Conflict { .. } |
                                         crate::state::modal::DependencyStatus::ToUpgrade { .. })
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
                            let mut grouped: HashMap<
                                String,
                                Vec<&crate::state::modal::DependencyInfo>,
                            > = HashMap::new();
                            for dep in important_deps.iter() {
                                for req_by in &dep.required_by {
                                    grouped.entry(req_by.clone()).or_default().push(dep);
                                }
                            }

                            // Count conflicts and upgrades
                            let conflict_count = important_deps
                                .iter()
                                .filter(|d| {
                                    matches!(
                                        d.status,
                                        crate::state::modal::DependencyStatus::Conflict { .. }
                                    )
                                })
                                .count();
                            let upgrade_count = important_deps
                                .iter()
                                .filter(|d| {
                                    matches!(
                                        d.status,
                                        crate::state::modal::DependencyStatus::ToUpgrade { .. }
                                    )
                                })
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
                            let available_height = (h as usize).saturating_sub(8);
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
                                                crate::state::modal::DependencyStatus::Conflict { reason } => {
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
                                                        crate::state::modal::DependencySource::Official { repo } => {
                                                            let repo_lower = repo.to_lowercase();
                                                            let color = if crate::index::is_eos_repo(&repo_lower) || crate::index::is_cachyos_repo(&repo_lower) {
                                                                th.sapphire
                                                            } else {
                                                                th.green
                                                            };
                                                            (format!(" [{}]", repo), color)
                                                        }
                                                        crate::state::modal::DependencySource::Aur => (" [AUR]".to_string(), th.yellow),
                                                        crate::state::modal::DependencySource::Local => (" [local]".to_string(), th.overlay1),
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
                                                crate::state::modal::DependencyStatus::ToUpgrade { current, required } => {
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
                                                        crate::state::modal::DependencySource::Official { repo } => {
                                                            let repo_lower = repo.to_lowercase();
                                                            let color = if crate::index::is_eos_repo(&repo_lower) || crate::index::is_cachyos_repo(&repo_lower) {
                                                                th.sapphire
                                                            } else {
                                                                th.green
                                                            };
                                                            (format!(" [{}]", repo), color)
                                                        }
                                                        crate::state::modal::DependencySource::Aur => (" [AUR]".to_string(), th.yellow),
                                                        crate::state::modal::DependencySource::Local => (" [local]".to_string(), th.overlay1),
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
                    } else {
                        // Fallback for remove action or no dependencies
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
                }
                crate::state::PreflightTab::Deps => {
                    // Use already resolved dependencies (resolved above if needed)
                    let deps = dependency_info;

                    // Group dependencies by the packages that require them
                    use std::collections::HashMap;
                    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                        HashMap::new();
                    for dep in deps.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    // Calculate summary statistics
                    let total = deps.len();
                    let installed_count = deps
                        .iter()
                        .filter(|d| {
                            matches!(
                                d.status,
                                crate::state::modal::DependencyStatus::Installed { .. }
                            )
                        })
                        .count();
                    let to_install_count = deps
                        .iter()
                        .filter(|d| {
                            matches!(d.status, crate::state::modal::DependencyStatus::ToInstall)
                        })
                        .count();
                    let to_upgrade_count = deps
                        .iter()
                        .filter(|d| {
                            matches!(
                                d.status,
                                crate::state::modal::DependencyStatus::ToUpgrade { .. }
                            )
                        })
                        .count();
                    let conflict_count = deps
                        .iter()
                        .filter(|d| {
                            matches!(
                                d.status,
                                crate::state::modal::DependencyStatus::Conflict { .. }
                            )
                        })
                        .count();
                    let missing_count = deps
                        .iter()
                        .filter(|d| {
                            matches!(d.status, crate::state::modal::DependencyStatus::Missing)
                        })
                        .count();

                    // Summary header
                    if total > 0 {
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
                    } else if matches!(*action, crate::state::PreflightAction::Install) {
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
                    let mut display_items: Vec<(
                        bool,
                        String,
                        Option<&crate::state::modal::DependencyInfo>,
                    )> = Vec::new();
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
                    let available_height = (h as usize).saturating_sub(8);
                    let start_idx = (*dep_selected)
                        .saturating_sub(available_height / 2)
                        .min(display_items.len().saturating_sub(available_height));
                    let end_idx = (start_idx + available_height).min(display_items.len());

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
                                crate::state::modal::DependencyStatus::Installed { .. } => {
                                    ("✓", th.green)
                                }
                                crate::state::modal::DependencyStatus::ToInstall => {
                                    ("+", th.yellow)
                                }
                                crate::state::modal::DependencyStatus::ToUpgrade { .. } => {
                                    ("↑", th.yellow)
                                }
                                crate::state::modal::DependencyStatus::Conflict { .. } => {
                                    ("⚠", th.red)
                                }
                                crate::state::modal::DependencyStatus::Missing => ("?", th.red),
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
                                crate::state::modal::DependencySource::Official { repo } => {
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
                                crate::state::modal::DependencySource::Aur => {
                                    (" [AUR]".to_string(), th.yellow)
                                }
                                crate::state::modal::DependencySource::Local => {
                                    (" [local]".to_string(), th.overlay1)
                                }
                            };
                            spans
                                .push(Span::styled(source_badge, Style::default().fg(badge_color)));

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
                                crate::state::modal::DependencyStatus::Installed { version } => {
                                    spans.push(Span::styled(
                                        format!(" (installed: {})", version),
                                        Style::default().fg(th.subtext1),
                                    ));
                                }
                                crate::state::modal::DependencyStatus::ToUpgrade {
                                    current,
                                    required,
                                } => {
                                    spans.push(Span::styled(
                                        format!(" ({} → {})", current, required),
                                        Style::default().fg(th.yellow),
                                    ));
                                }
                                crate::state::modal::DependencyStatus::Conflict { reason } => {
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
                crate::state::PreflightTab::Files => {
                    if file_info.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "Resolving file changes...",
                            Style::default().fg(th.subtext1),
                        )));
                    } else {
                        // Calculate available height for file list (accounting for header, footer, etc.)
                        let available_height = rect.height.saturating_sub(6).max(1);

                        // Build flat list of display items: package headers + files
                        // Store owned data to avoid lifetime issues
                        type FileDisplayItem = (
                            bool,
                            String,
                            Option<(
                                crate::state::modal::FileChangeType,
                                String,
                                bool,
                                bool,
                                bool,
                            )>,
                        );
                        let mut display_items: Vec<FileDisplayItem> = Vec::new();
                        for pkg_info in file_info.iter() {
                            if !pkg_info.files.is_empty() {
                                display_items.push((true, pkg_info.name.clone(), None)); // Package header
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
                            if let Some((_age_days, date_str, color_category)) =
                                crate::logic::files::get_file_db_sync_info()
                            {
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
                            // Calculate scroll position
                            let total_items = display_items.len();
                            let start_idx = (*file_selected).min(total_items.saturating_sub(1));
                            let end_idx = (start_idx + available_height as usize).min(total_items);

                            // Display summary
                            let total_files: usize = file_info.iter().map(|p| p.total_count).sum();
                            let total_new: usize = file_info.iter().map(|p| p.new_count).sum();
                            let total_changed: usize =
                                file_info.iter().map(|p| p.changed_count).sum();
                            let total_removed: usize =
                                file_info.iter().map(|p| p.removed_count).sum();
                            let total_config: usize =
                                file_info.iter().map(|p| p.config_count).sum();
                            let total_pacnew: usize =
                                file_info.iter().map(|p| p.pacnew_candidates).sum();
                            let total_pacsave: usize =
                                file_info.iter().map(|p| p.pacsave_candidates).sum();

                            let mut summary_parts = vec![
                                format!("Total: {} files", total_files),
                                format!("{} new", total_new),
                                format!("{} changed", total_changed),
                                format!("{} removed", total_removed),
                                format!("{} config", total_config),
                            ];

                            if total_pacnew > 0 {
                                summary_parts.push(format!("{} pacnew", total_pacnew));
                            }
                            if total_pacsave > 0 {
                                summary_parts.push(format!("{} pacsave", total_pacsave));
                            }

                            lines.push(Line::from(Span::styled(
                                summary_parts.join(", "),
                                Style::default().fg(th.subtext1),
                            )));
                            lines.push(Line::from(""));

                            // Show file database sync timestamp
                            if let Some((_age_days, date_str, color_category)) =
                                crate::logic::files::get_file_db_sync_info()
                            {
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
                            }

                            // Display files with scrolling
                            for (display_idx, (is_header, pkg_name, file_opt)) in display_items
                                .iter()
                                .enumerate()
                                .skip(start_idx)
                                .take(end_idx - start_idx)
                            {
                                if *is_header {
                                    // Find package info for this header
                                    let pkg_info =
                                        file_info.iter().find(|p| p.name == *pkg_name).unwrap();

                                    // Package header
                                    let mut spans = vec![
                                        Span::styled(
                                            format!("📦 {} ", pkg_name),
                                            Style::default()
                                                .fg(th.sapphire)
                                                .add_modifier(Modifier::BOLD),
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
                                        crate::state::modal::FileChangeType::New => ("+", th.green),
                                        crate::state::modal::FileChangeType::Changed => {
                                            ("~", th.yellow)
                                        }
                                        crate::state::modal::FileChangeType::Removed => {
                                            ("-", th.red)
                                        }
                                    };

                                    let mut spans = vec![Span::styled(
                                        format!("  {} ", icon),
                                        Style::default().fg(color),
                                    )];

                                    if *is_config {
                                        spans.push(Span::styled(
                                            "⚙ ",
                                            Style::default().fg(th.mauve),
                                        ));
                                    }

                                    // Add pacnew/pacsave indicators
                                    if *predicted_pacnew {
                                        spans.push(Span::styled(
                                            "⚠ pacnew ",
                                            Style::default().fg(th.yellow),
                                        ));
                                    }
                                    if *predicted_pacsave {
                                        spans.push(Span::styled(
                                            "⚠ pacsave ",
                                            Style::default().fg(th.red),
                                        ));
                                    }

                                    spans.push(Span::styled(
                                        path.clone(),
                                        Style::default().fg(if display_idx == *file_selected {
                                            th.surface1
                                        } else {
                                            th.text
                                        }),
                                    ));

                                    lines.push(Line::from(spans));
                                }
                            }

                            if total_items > available_height as usize {
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
                crate::state::PreflightTab::Services => {
                    lines.push(Line::from(Span::styled(
                        "Services (placeholder) — impacted services/restarts will appear here",
                        Style::default().fg(th.text),
                    )));
                }
                crate::state::PreflightTab::Sandbox => {
                    lines.push(Line::from(Span::styled(
                        "Sandbox (placeholder) — AUR preflight build checks will appear here",
                        Style::default().fg(th.text),
                    )));
                }
            }

            lines.push(Line::from(""));
            // Check if any AUR packages are present for scanning
            let has_aur = items
                .iter()
                .any(|p| matches!(p.source, crate::state::Source::Aur));

            // Build footer hint based on current tab
            let scan_hint = match current_tab {
                crate::state::PreflightTab::Deps => {
                    if has_aur {
                        "Left/Right: tabs  •  Up/Down: navigate  •  Enter/Space: toggle  •  a: expand/collapse all  •  ?: help  •  s: scan AUR  •  d: dry-run  •  p: proceed  •  q: close"
                    } else {
                        "Left/Right: tabs  •  Up/Down: navigate  •  Enter/Space: toggle  •  a: expand/collapse all  •  ?: help  •  d: dry-run  •  p: proceed  •  q: close"
                    }
                }
                crate::state::PreflightTab::Files => {
                    if has_aur {
                        "Left/Right: tabs  •  Up/Down: navigate  •  f: sync file DB  •  s: scan AUR  •  d: dry-run  •  p: proceed  •  q: close"
                    } else {
                        "Left/Right: tabs  •  Up/Down: navigate  •  f: sync file DB  •  d: dry-run  •  p: proceed  •  q: close"
                    }
                }
                _ => {
                    if has_aur {
                        "Left/Right: tabs  •  s: scan AUR  •  d: dry-run  •  p: proceed  •  q: close"
                    } else {
                        "Left/Right: tabs  •  d: dry-run  •  p: proceed  •  q: close"
                    }
                }
            };
            lines.push(Line::from(Span::styled(
                scan_hint,
                Style::default().fg(th.subtext1),
            )));

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
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(bg_color)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::PreflightExec {
            items,
            action,
            tab,
            verbose,
            log_lines,
            abortable,
        } => {
            let th = theme();
            let w = area.width.saturating_sub(4).min(110);
            let h = area.height.saturating_sub(4).min(area.height);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            let border_color = th.lavender;
            let bg_color = th.crust;
            let title = match action {
                crate::state::PreflightAction::Install => " Execute: Install ",
                crate::state::PreflightAction::Remove => " Execute: Remove ",
            };

            // Split inner content: left (sidebar) 30%, right (log) 70%
            let inner = ratatui::prelude::Rect {
                x: rect.x + 1,
                y: rect.y + 1,
                width: rect.width.saturating_sub(2),
                height: rect.height.saturating_sub(2),
            };
            let cols = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Percentage(30),
                    ratatui::layout::Constraint::Percentage(70),
                ])
                .split(inner);

            // Sidebar: show selected tab header and items
            let mut s_lines: Vec<Line<'static>> = Vec::new();
            let tab_labels = ["Summary", "Deps", "Files", "Services", "Sandbox"];
            let mut header = String::new();
            let current_tab = *tab;
            for (i, lbl) in tab_labels.iter().enumerate() {
                let is = matches!(
                    (i, current_tab),
                    (0, crate::state::PreflightTab::Summary)
                        | (1, crate::state::PreflightTab::Deps)
                        | (2, crate::state::PreflightTab::Files)
                        | (3, crate::state::PreflightTab::Services)
                        | (4, crate::state::PreflightTab::Sandbox)
                );
                if i > 0 {
                    header.push_str("  ");
                }
                if is {
                    header.push('[');
                    header.push_str(lbl);
                    header.push(']');
                } else {
                    header.push_str(lbl);
                }
            }
            s_lines.push(Line::from(Span::styled(
                header,
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            s_lines.push(Line::from(""));
            for p in items.iter().take(12) {
                s_lines.push(Line::from(Span::styled(
                    format!("- {}", p.name),
                    Style::default().fg(th.text),
                )));
            }
            let sidebar = Paragraph::new(s_lines)
                .style(Style::default().fg(th.text).bg(bg_color))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Plan ",
                            Style::default()
                                .fg(border_color)
                                .add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(bg_color)),
                );
            f.render_widget(sidebar, cols[0]);

            // Log panel
            let mut log_text: Vec<Line<'static>> = Vec::new();
            if log_lines.is_empty() {
                log_text.push(Line::from(Span::styled(
                    "Starting… (placeholder; real logs will stream here)",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for l in log_lines
                    .iter()
                    .rev()
                    .take(cols[1].height as usize - 2)
                    .rev()
                {
                    log_text.push(Line::from(Span::styled(
                        l.clone(),
                        Style::default().fg(th.text),
                    )));
                }
            }
            log_text.push(Line::from(""));
            let footer = format!(
                "l: verbose={}  •  x: abort{}  •  q/Esc/Enter: close",
                if *verbose { "ON" } else { "OFF" },
                if *abortable { " (available)" } else { "" }
            );
            log_text.push(Line::from(Span::styled(
                footer,
                Style::default().fg(th.subtext1),
            )));

            let logw = Paragraph::new(log_text)
                .style(Style::default().fg(th.text).bg(th.base))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(Span::styled(
                            title,
                            Style::default()
                                .fg(border_color)
                                .add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(th.base)),
                );
            f.render_widget(logw, cols[1]);
        }
        crate::state::Modal::PostSummary {
            success,
            changed_files,
            pacnew_count,
            pacsave_count,
            services_pending,
            snapshot_label,
        } => {
            let th = theme();
            let w = area.width.saturating_sub(8).min(96);
            let h = area.height.saturating_sub(6).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            let border_color = if *success { th.green } else { th.red };
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                if *success { "Success" } else { "Failed" },
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "Changed files: {} (pacnew: {}, pacsave: {})",
                    changed_files, pacnew_count, pacsave_count
                ),
                Style::default().fg(th.text),
            )));
            if let Some(label) = snapshot_label {
                lines.push(Line::from(Span::styled(
                    format!("Snapshot: {}", label),
                    Style::default().fg(th.text),
                )));
            }
            if !services_pending.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Services pending restart:",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                )));
                for s in services_pending
                    .iter()
                    .take((h as usize).saturating_sub(10))
                {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", s),
                        Style::default().fg(th.text),
                    )));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "r: rollback  •  s: restart services  •  Enter/Esc: close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Post-Transaction Summary ",
                            Style::default()
                                .fg(border_color)
                                .add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::ConfirmRemove { items } => {
            let w = area.width.saturating_sub(6).min(90);
            let h = area.height.saturating_sub(6).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Confirm removal",
                Style::default().fg(th.red).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            // Warn explicitly if any core packages are present
            let has_core = items.iter().any(|p| match &p.source {
                crate::state::Source::Official { repo, .. } => repo.eq_ignore_ascii_case("core"),
                _ => false,
            });
            if has_core {
                lines.push(Line::from(Span::styled(
                    "WARNING: core packages selected. Removing core packages may break your system.",
                    Style::default()
                        .fg(th.red)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
            }
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Nothing to remove",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for p in items.iter().take((h as usize).saturating_sub(6)) {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", p.name),
                        Style::default().fg(th.text),
                    )));
                }
                if items.len() + 6 > h as usize {
                    lines.push(Line::from(Span::styled(
                        "…",
                        Style::default().fg(th.subtext1),
                    )));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter to confirm or Esc to cancel",
                Style::default().fg(th.subtext1),
            )));
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Confirm Remove ",
                            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.red))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            country_idx,
            countries,
            mirror_count,
            cursor,
        } => {
            let w = area.width.saturating_sub(8).min(80);
            let h = 14;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "System Update",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            let mark = |b: bool| if b { "[x]" } else { "[ ]" };

            let entries: [(&str, bool); 4] = [
                ("Update Arch Mirrors", *do_mirrors),
                ("Update Pacman (sudo pacman -Syyu)", *do_pacman),
                ("Update AUR (paru/yay)", *do_aur),
                ("Remove Cache (pacman/yay)", *do_cache),
            ];

            for (i, (label, on)) in entries.iter().enumerate() {
                let style = if *cursor == i {
                    Style::default()
                        .fg(th.crust)
                        .bg(th.lavender)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(th.text)
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", mark(*on)), Style::default().fg(th.overlay1)),
                    Span::styled((*label).to_string(), style),
                ]));
            }

            // Country selector (mirrors)
            lines.push(Line::from(""));
            let country_label = if *country_idx < countries.len() {
                &countries[*country_idx]
            } else {
                "Worldwide"
            };
            // Read configured countries and mirror count from settings for display
            let prefs = crate::theme::settings();
            let conf_countries = if prefs.selected_countries.trim().is_empty() {
                "Worldwide".to_string()
            } else {
                prefs.selected_countries.clone()
            };
            // If Worldwide is selected, show the configured countries
            let shown_countries = if country_label == "Worldwide" {
                conf_countries.as_str()
            } else {
                country_label
            };
            let style = if *cursor == entries.len() {
                Style::default()
                    .fg(th.crust)
                    .bg(th.lavender)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(th.text)
            };
            lines.push(Line::from(vec![
                Span::styled("Country (Mirrors): ", Style::default().fg(th.overlay1)),
                Span::styled(shown_countries.to_string(), style),
                Span::raw("  •  "),
                Span::styled(
                    format!("Count: {}", mirror_count),
                    Style::default().fg(th.overlay1),
                ),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Space: toggle  •  Left/Right: change country  •  -/+ change count  •  Enter: run  •  Esc: cancel",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Update System ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::Help => {
            // Full-screen translucent help overlay
            let w = area.width.saturating_sub(6).min(96);
            let h = area.height.saturating_sub(4).min(28);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            // Record inner content rect (exclude borders) for mouse hit-testing
            app.help_rect = Some((
                rect.x + 1,
                rect.y + 1,
                rect.width.saturating_sub(2),
                rect.height.saturating_sub(2),
            ));
            let km = &app.keymap;

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Pacsea Help",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            // Utility to format a binding line
            let fmt = |label: &str, chord: KeyChord| -> Line<'static> {
                Line::from(vec![
                    Span::styled(
                        format!("{label:18}"),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", chord.label()),
                        Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                    ),
                ])
            };

            if let Some(k) = km.help_overlay.first().copied() {
                lines.push(fmt("Help overlay", k));
            }
            if let Some(k) = km.exit.first().copied() {
                lines.push(fmt("Exit", k));
            }
            if let Some(k) = km.reload_theme.first().copied() {
                lines.push(fmt("Reload theme", k));
            }
            // Move menu toggles into Normal Mode section; omit here
            if let Some(k) = km.pane_next.first().copied() {
                lines.push(fmt("Next pane", k));
            }
            if let Some(k) = km.pane_left.first().copied() {
                lines.push(fmt("Focus left", k));
            }
            if let Some(k) = km.pane_right.first().copied() {
                lines.push(fmt("Focus right", k));
            }
            if let Some(k) = km.show_pkgbuild.first().copied() {
                lines.push(fmt("Show PKGBUILD", k));
            }
            // Show configured key for change sorting
            if let Some(k) = km.change_sort.first().copied() {
                lines.push(fmt("Change sorting", k));
            }
            lines.push(Line::from(""));

            // Dynamic section for per-pane actions based on keymap
            lines.push(Line::from(Span::styled(
                "Search:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.search_move_up.first(), km.search_move_down.first()) {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
                lines.push(fmt(
                    "  Page",
                    KeyChord {
                        code: pu.code,
                        mods: pu.mods,
                    },
                ));
                lines.push(fmt(
                    "  Page",
                    KeyChord {
                        code: pd.code,
                        mods: pd.mods,
                    },
                ));
            }
            if let Some(k) = km.search_add.first().copied() {
                lines.push(fmt("  Add", k));
            }
            if let Some(k) = km.search_install.first().copied() {
                lines.push(fmt("  Install", k));
            }
            if let Some(k) = km.search_backspace.first().copied() {
                lines.push(fmt("  Delete", k));
            }

            // Search normal mode
            if km
                .search_normal_toggle
                .first()
                .or(km.search_normal_insert.first())
                .or(km.search_normal_select_left.first())
                .or(km.search_normal_select_right.first())
                .or(km.search_normal_delete.first())
                .or(km.search_normal_open_status.first())
                .or(km.config_menu_toggle.first())
                .or(km.options_menu_toggle.first())
                .or(km.panels_menu_toggle.first())
                .is_some()
            {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Search (Normal mode):",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                )));
                if let Some(k) = km.search_normal_toggle.first().copied() {
                    lines.push(fmt("  Toggle normal", k));
                }
                if let Some(k) = km.search_normal_insert.first().copied() {
                    lines.push(fmt("  Insert Mode", k));
                }
                if let Some(k) = km.search_normal_select_left.first().copied() {
                    lines.push(fmt("  Select left", k));
                }
                if let Some(k) = km.search_normal_select_right.first().copied() {
                    lines.push(fmt("  Select right", k));
                }
                if let Some(k) = km.search_normal_delete.first().copied() {
                    lines.push(fmt("  Delete", k));
                }
                if let Some(k) = km.search_normal_open_status.first().copied() {
                    lines.push(fmt("  Open Arch status", k));
                }
                if let Some(k) = km.config_menu_toggle.first().copied() {
                    lines.push(fmt("  Config/Lists menu", k));
                }
                if let Some(k) = km.options_menu_toggle.first().copied() {
                    lines.push(fmt("  Options menu", k));
                }
                if let Some(k) = km.panels_menu_toggle.first().copied() {
                    lines.push(fmt("  Panels menu", k));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Install:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first())
            {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let Some(k) = km.install_confirm.first().copied() {
                lines.push(fmt("  Confirm", k));
            }
            if let Some(k) = km.install_remove.first().copied() {
                lines.push(fmt("  Remove", k));
            }
            if let Some(k) = km.install_clear.first().copied() {
                lines.push(fmt("  Clear", k));
            }
            if let Some(k) = km.install_find.first().copied() {
                lines.push(fmt("  Find", k));
            }
            if let Some(k) = km.install_to_search.first().copied() {
                lines.push(fmt("  To Search", k));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Recent:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let Some(k) = km.recent_use.first().copied() {
                lines.push(fmt("  Use", k));
            }
            if let Some(k) = km.recent_add.first().copied() {
                lines.push(fmt("  Add", k));
            }
            if let Some(k) = km.recent_find.first().copied() {
                lines.push(fmt("  Find", k));
            }
            if let Some(k) = km.recent_to_search.first().copied() {
                lines.push(fmt("  To Search", k));
            }
            if let Some(k) = km.recent_remove.first().copied() {
                lines.push(fmt("  Remove", k));
            }
            // Explicit: Shift+Del clears Recent (display only)
            lines.push(fmt(
                "  Clear",
                crate::theme::KeyChord {
                    code: crossterm::event::KeyCode::Delete,
                    mods: crossterm::event::KeyModifiers::SHIFT,
                },
            ));

            // Mouse and UI controls
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Mouse:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw(
                "  • Scroll lists (Results/Recent/Install) and PKGBUILD with mouse wheel",
            )));
            lines.push(Line::from(Span::raw(
                "  • Toggle PKGBUILD: click 'Show PKGBUILD' in details",
            )));
            lines.push(Line::from(Span::raw(
                "  • Copy PKGBUILD: click the title button (copies with suffix from settings.conf)",
            )));
            lines.push(Line::from(Span::raw(
                "  • Open details URL: Ctrl+Shift+Left click on the URL",
            )));
            lines.push(Line::from(Span::raw(
                "  • Results title bar: click Sort/Options/Panels/Config to open menus",
            )));
            lines.push(Line::from(Span::raw(
                "  • Toggle filters (AUR/core/extra/multilib/EOS/cachyos): click their labels",
            )));
            lines.push(Line::from(Span::raw(
                "  • Arch Status (top-right): click to open status.archlinux.org",
            )));

            // Dialogs
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "System Update dialog:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw(
                "  • Open via Options → Update System",
            )));
            lines.push(Line::from(Span::raw(
                "  • Up/Down: move • Space: toggle • Left/Right: change country • Enter: run • Esc: close",
            )));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "News dialog:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw(
                "  • Open via Options → News • Up/Down: select • Enter: open • Esc: close",
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter or Esc to close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .scroll((app.help_scroll, 0))
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Help ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::News { items, selected } => {
            let w = (area.width * 2) / 3;
            let h = area.height.saturating_sub(8).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            // Record outer and inner rects for mouse hit-testing
            app.news_rect = Some((rect.x, rect.y, rect.width, rect.height));

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Arch Linux News",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No news items available.",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for (i, it) in items.iter().enumerate() {
                    let tl = it.title.to_lowercase();
                    let is_critical = tl.contains("critical")
                        || tl.contains("require manual intervention")
                        || tl.contains("requires manual intervention");
                    let style = if *selected == i {
                        let fg = if is_critical { th.red } else { th.text };
                        Style::default().fg(fg).bg(th.surface1)
                    } else {
                        let fg = if is_critical { th.red } else { th.text };
                        Style::default().fg(fg)
                    };
                    let prefs = crate::theme::settings();
                    let line = format!(
                        "{} {}  {}",
                        if app.news_read_urls.contains(&it.url) {
                            &prefs.news_read_symbol
                        } else {
                            &prefs.news_unread_symbol
                        },
                        it.date,
                        it.title
                    );
                    lines.push(Line::from(Span::styled(line, style)));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "Up/Down: select  •  Enter: open  •  {}: mark read  •  {}: mark all read  •  Esc: close",
                    app.keymap
                        .news_mark_read
                        .first()
                        .map(|k| k.label())
                        .unwrap_or_else(|| "R".to_string()),
                    app.keymap
                        .news_mark_all_read
                        .first()
                        .map(|k| k.label())
                        .unwrap_or_else(|| "Ctrl+R".to_string())
                ),
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " News ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);

            // The list content starts two lines after title and blank line, and ends before footer hint lines.
            // Approximate inner list area (exclude 1-char borders):
            let list_inner_x = rect.x + 1;
            let list_inner_y = rect.y + 1 + 2; // header + blank line
            let list_inner_w = rect.width.saturating_sub(2);
            // Compute visible rows budget: total height minus borders, header (2 lines), footer (2 lines)
            let inner_h = rect.height.saturating_sub(2);
            let list_rows = inner_h.saturating_sub(4);
            app.news_list_rect = Some((list_inner_x, list_inner_y, list_inner_w, list_rows));
        }
        crate::state::Modal::OptionalDeps { rows, selected } => {
            // Build content lines with selection and install status markers
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "TUI Optional Deps",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            for (i, row) in rows.iter().enumerate() {
                let is_sel = *selected == i;
                let (mark, color) = if row.installed {
                    ("✔ installed", th.green)
                } else {
                    ("⏺ not installed", th.overlay1)
                };
                let style = if is_sel {
                    Style::default()
                        .fg(th.crust)
                        .bg(th.lavender)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(th.text)
                };
                let mut segs: Vec<Span> = Vec::new();
                segs.push(Span::styled(format!("{}  ", row.label), style));
                segs.push(Span::styled(
                    format!("[{}]", row.package),
                    Style::default().fg(th.overlay1),
                ));
                segs.push(Span::raw("  "));
                segs.push(Span::styled(mark.to_string(), Style::default().fg(color)));
                if let Some(note) = &row.note {
                    segs.push(Span::raw("  "));
                    segs.push(Span::styled(
                        format!("({})", note),
                        Style::default().fg(th.overlay2),
                    ));
                }
                lines.push(Line::from(segs));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Up/Down: select  •  Enter: install  •  Esc: close",
                Style::default().fg(th.subtext1),
            )));

            render_simple_list_modal(f, area, "Optional Deps", lines);
        }
        crate::state::Modal::ScanConfig {
            do_clamav,
            do_trivy,
            do_semgrep,
            do_shellcheck,
            do_virustotal,
            do_custom,
            do_sleuth,
            cursor,
        } => {
            let th = crate::theme::theme();
            let mut lines: Vec<Line<'static>> = Vec::new();

            let items: [(&str, bool); 7] = [
                ("ClamAV (antivirus)", *do_clamav),
                ("Trivy (filesystem)", *do_trivy),
                ("Semgrep (static analysis)", *do_semgrep),
                ("ShellCheck (PKGBUILD/.install)", *do_shellcheck),
                ("VirusTotal (hash lookups)", *do_virustotal),
                ("Custom scan for Suspicious patterns", *do_custom),
                ("aur-sleuth (LLM audit)", *do_sleuth),
            ];

            for (i, (label, checked)) in items.iter().enumerate() {
                let mark = if *checked { "[x]" } else { "[ ]" };
                let mut spans: Vec<Span> = Vec::new();
                spans.push(Span::styled(
                    format!("{} ", mark),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ));
                let style = if i == *cursor {
                    Style::default()
                        .fg(th.text)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(th.subtext1)
                };
                spans.push(Span::styled((*label).to_string(), style));
                lines.push(Line::from(spans));
            }

            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled(
                "Up/Down: select  •  Space: toggle  •  Enter: run  •  Esc: cancel",
                Style::default().fg(th.overlay1),
            )));

            render_simple_list_modal(f, area, "Scan Configuration", lines);
        }
        crate::state::Modal::GnomeTerminalPrompt => {
            // Centered confirmation dialog for installing GNOME Terminal
            let w = area.width.saturating_sub(10).min(90);
            let h = 9;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            let lines: Vec<Line<'static>> = vec![
                Line::from(Span::styled(
                    "GNOME Terminal or Console recommended",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "GNOME was detected, but no GNOME terminal (gnome-terminal or gnome-console/kgx) is installed.",
                    Style::default().fg(th.text),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter to install gnome-terminal  •  Esc to cancel",
                    Style::default().fg(th.subtext1),
                )),
                Line::from(Span::styled(
                    "Cancel may lead to unexpected behavior.",
                    Style::default().fg(th.yellow),
                )),
            ];

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Install a GNOME Terminal ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::VirusTotalSetup { input, cursor: _ } => {
            // Centered dialog for VirusTotal API key setup with clickable URL and input field
            let w = area.width.saturating_sub(10).min(90);
            let h = 11;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            // Build content
            let vt_url = "https://www.virustotal.com/gui/my-apikey";
            // Show input buffer (not masked)
            let shown = if input.is_empty() {
                "<empty>".to_string()
            } else {
                input.clone()
            };
            let lines: Vec<Line<'static>> = vec![
                Line::from(Span::styled(
                    "VirusTotal API Setup",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Open the link to view your API key:",
                    Style::default().fg(th.text),
                )),
                Line::from(vec![
                    // Surround with spaces to avoid visual concatenation with underlying content
                    Span::styled(" ", Style::default().fg(th.text)),
                    Span::styled(
                        vt_url.to_string(),
                        Style::default()
                            .fg(th.lavender)
                            .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter/paste your API key below and press Enter to save (Esc to cancel):",
                    Style::default().fg(th.subtext1),
                )),
                Line::from(Span::styled(
                    format!("API key: {}", shown),
                    Style::default().fg(th.text),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Tip: After saving, scans will auto-query VirusTotal by file hash.",
                    Style::default().fg(th.overlay1),
                )),
            ];

            let inner_x = rect.x + 1;
            let inner_y = rect.y + 1;
            let url_line_y = inner_y + 3;
            let url_x = inner_x + 1;
            let url_w = vt_url.len() as u16;
            app.vt_url_rect = Some((url_x, url_line_y, url_w, 1));
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " VirusTotal ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::ImportHelp => {
            let w = area.width.saturating_sub(10).min(85);
            let h = 19;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            let lines: Vec<Line<'static>> = vec![
                Line::from(Span::styled(
                    "Import File Format",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "The import file should contain one package name per line.",
                    Style::default().fg(th.text),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Format:",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::raw("  • One package name per line")),
                Line::from(Span::raw("  • Blank lines are ignored")),
                Line::from(Span::raw(
                    "  • Lines starting with '#' are treated as comments",
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Example:",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::raw("  firefox")),
                Line::from(Span::raw("  # This is a comment")),
                Line::from(Span::raw("  vim")),
                Line::from(Span::raw("  paru")),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[Enter]",
                        Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" confirm", Style::default().fg(th.overlay1)),
                    Span::raw("  •  "),
                    Span::styled(
                        "[Esc]",
                        Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" cancel", Style::default().fg(th.overlay1)),
                ]),
            ];

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Import Help ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::None => {}
    }
}

/// Render a centered, simple list modal with a title and provided content lines.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `box_title`: Title shown in the modal border
/// - `lines`: Pre-built content lines
fn render_simple_list_modal(f: &mut Frame, area: Rect, box_title: &str, lines: Vec<Line<'static>>) {
    let th = theme();
    let w = area.width.saturating_sub(8).min(80);
    let h = area.height.saturating_sub(8).min(20);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" {} ", box_title),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

#[cfg(test)]
mod tests {
    /// What: Render all modal variants and record expected rects
    ///
    /// - Input: Cycle Alert, ConfirmInstall, ConfirmRemove(core), Help, News
    /// - Output: No panic; Help/news rects populated where applicable
    #[test]
    fn modals_set_rects_and_render_variants() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(100, 28);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = crate::state::AppState {
            ..Default::default()
        };

        // Alert
        app.modal = crate::state::Modal::Alert {
            message: "Test".into(),
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // ConfirmInstall
        app.modal = crate::state::Modal::ConfirmInstall { items: vec![] };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // ConfirmRemove with core warn
        app.modal = crate::state::Modal::ConfirmRemove {
            items: vec![crate::state::PackageItem {
                name: "glibc".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Official {
                    repo: "core".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            }],
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // Help
        app.modal = crate::state::Modal::Help;
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();
        assert!(app.help_rect.is_some());

        // News
        app.modal = crate::state::Modal::News {
            items: vec![crate::state::NewsItem {
                date: "2025-10-11".into(),
                title: "Test".into(),
                url: "".into(),
            }],
            selected: 0,
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();
        assert!(app.news_rect.is_some());
        assert!(app.news_list_rect.is_some());
    }
}
