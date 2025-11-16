use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::{AppState, PackageItem};
use crate::theme::theme;
use std::collections::{HashMap, HashSet};

/// What: Render the Sandbox tab content for the preflight modal.
///
/// Inputs:
/// - `app`: Application state for i18n and data access.
/// - `items`: Packages under review.
/// - `sandbox_info`: Sandbox information.
/// - `sandbox_selected`: Currently selected sandbox item index (mutable).
/// - `sandbox_tree_expanded`: Set of expanded package names.
/// - `sandbox_loaded`: Whether sandbox is loaded.
/// - `sandbox_error`: Optional error message.
/// - `selected_optdepends`: Map of selected optional dependencies.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows sandbox dependency analysis for AUR packages.
/// - Supports viewport-based rendering for large dependency lists.
#[allow(clippy::too_many_arguments)]
pub fn render_sandbox_tab(
    app: &AppState,
    items: &[PackageItem],
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_selected: &mut usize,
    sandbox_tree_expanded: &HashSet<String>,
    sandbox_loaded: bool,
    sandbox_error: &Option<String>,
    selected_optdepends: &HashMap<String, HashSet<String>>,
    content_rect: Rect,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    // Display error if any
    if let Some(err) = sandbox_error.as_ref() {
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(app, "app.modals.preflight.sandbox.error", err),
            Style::default().fg(th.red),
        )));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.sandbox.retry_hint"),
            Style::default().fg(th.subtext0),
        )));
        lines.push(Line::from(""));
    } else if app.preflight_sandbox_resolving || app.sandbox_resolving {
        // ALWAYS show loading message when resolving, regardless of sandbox_loaded state
        // Show package headers first (only AUR packages), then loading message
        for item in items.iter() {
            let is_aur = matches!(item.source, crate::state::Source::Aur);
            if is_aur {
                let mut spans = Vec::new();
                spans.push(Span::styled(
                    format!("▶ {} ", item.name),
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(spans));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.sandbox.updating"),
            Style::default().fg(th.yellow),
        )));
    } else if !sandbox_loaded || sandbox_info.is_empty() {
        // Show package headers first (only AUR packages), then analyzing/resolving message
        for item in items.iter() {
            let is_aur = matches!(item.source, crate::state::Source::Aur);
            if is_aur {
                let mut spans = Vec::new();
                spans.push(Span::styled(
                    format!("▶ {} ", item.name),
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(spans));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.sandbox.analyzing"),
            Style::default().fg(th.subtext0),
        )));
    } else {
        // Build flat list of display items: package headers + dependencies (only if expanded)
        // Format: (is_header, package_name, Option<(dep_type, dep_name, dep_info)>)
        type SandboxDisplayItem = (
            bool,
            String,
            Option<(
                &'static str, // "depends", "makedepends", "checkdepends", "optdepends"
                String,       // dependency name
                crate::logic::sandbox::DependencyDelta,
            )>,
        );
        let mut display_items: Vec<SandboxDisplayItem> = Vec::new();

        for item in items.iter() {
            let is_aur = matches!(item.source, crate::state::Source::Aur);
            let is_expanded = sandbox_tree_expanded.contains(&item.name);

            // Add package header
            display_items.push((true, item.name.clone(), None));

            // Add dependencies only if expanded and AUR
            if is_expanded
                && is_aur
                && let Some(info) = sandbox_info.iter().find(|s| s.package_name == item.name)
            {
                // Runtime dependencies (depends)
                for dep in &info.depends {
                    display_items.push((
                        false,
                        item.name.clone(),
                        Some(("depends", dep.name.clone(), dep.clone())),
                    ));
                }
                // Build dependencies (makedepends)
                for dep in &info.makedepends {
                    display_items.push((
                        false,
                        item.name.clone(),
                        Some(("makedepends", dep.name.clone(), dep.clone())),
                    ));
                }
                // Test dependencies (checkdepends)
                for dep in &info.checkdepends {
                    display_items.push((
                        false,
                        item.name.clone(),
                        Some(("checkdepends", dep.name.clone(), dep.clone())),
                    ));
                }
                // Optional dependencies (optdepends)
                for dep in &info.optdepends {
                    display_items.push((
                        false,
                        item.name.clone(),
                        Some(("optdepends", dep.name.clone(), dep.clone())),
                    ));
                }
            }
        }

        // Calculate viewport based on selected index (like Deps/Files tabs)
        // Performance optimization: Only render visible items (viewport-based rendering)
        // This prevents performance issues with large dependency lists
        let available_height = (content_rect.height as usize).saturating_sub(6);
        let total_items = display_items.len();
        tracing::debug!(
            "[UI] Sandbox tab: total_items={}, sandbox_selected={}, items={}, sandbox_info={}, expanded_count={}",
            total_items,
            *sandbox_selected,
            items.len(),
            sandbox_info.len(),
            sandbox_tree_expanded.len()
        );
        let sandbox_selected_clamped = (*sandbox_selected).min(total_items.saturating_sub(1));
        if *sandbox_selected != sandbox_selected_clamped {
            tracing::debug!(
                "[UI] Sandbox tab: clamping sandbox_selected from {} to {} (total_items={})",
                *sandbox_selected,
                sandbox_selected_clamped,
                total_items
            );
            *sandbox_selected = sandbox_selected_clamped;
        }

        // Calculate viewport range: only render items visible on screen
        // Account for section headers which add extra lines but aren't in display_items
        // Simple approach: ensure selected item is always within viewport bounds
        let mut start_idx;
        let mut end_idx;

        if total_items <= available_height {
            // All items fit on screen
            start_idx = 0;
            end_idx = total_items;
        } else {
            // Ensure selected item is always visible - keep it within [start_idx, end_idx)
            // Try to center it, but adjust if needed to keep it visible
            // Reduce available_height slightly to account for section headers that add extra lines
            let effective_height = available_height.saturating_sub(2); // Reserve space for section headers
            start_idx = sandbox_selected_clamped
                .saturating_sub(effective_height / 2)
                .max(0)
                .min(total_items.saturating_sub(effective_height));
            end_idx = (start_idx + effective_height).min(total_items);

            // Ensure selected item is within bounds - adjust if necessary
            if sandbox_selected_clamped < start_idx {
                // Selected item is before start - move start to include it
                start_idx = sandbox_selected_clamped;
                end_idx = (start_idx + effective_height).min(total_items);
            } else if sandbox_selected_clamped >= end_idx {
                // Selected item is at or beyond end - position it at bottom of viewport
                // Make sure to include it even if section headers take up space
                end_idx = (sandbox_selected_clamped + 1).min(total_items);
                start_idx = end_idx.saturating_sub(effective_height).max(0);
                end_idx = (start_idx + effective_height).min(total_items);
                // Final check: ensure selected item is visible
                if sandbox_selected_clamped >= end_idx {
                    end_idx = sandbox_selected_clamped + 1;
                    start_idx = end_idx.saturating_sub(effective_height).max(0);
                }
            }
        }

        // Track which packages we've seen to group dependencies properly
        let mut last_dep_type: Option<&str> = None;

        // Render visible items
        for (idx, (is_header, pkg_name, dep_opt)) in display_items
            .iter()
            .enumerate()
            .skip(start_idx)
            .take(end_idx - start_idx)
        {
            let is_selected = idx == *sandbox_selected;

            if *is_header {
                // Package header
                let item = items.iter().find(|p| p.name == *pkg_name).unwrap();
                let is_aur = matches!(item.source, crate::state::Source::Aur);
                let is_expanded = sandbox_tree_expanded.contains(pkg_name);
                let arrow_symbol = if is_aur && is_expanded {
                    "▼"
                } else if is_aur {
                    "▶"
                } else {
                    ""
                };

                let header_style = if is_selected {
                    Style::default()
                        .fg(th.crust)
                        .bg(th.sapphire)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(th.sapphire)
                        .add_modifier(Modifier::BOLD)
                };

                let source_str = match &item.source {
                    crate::state::Source::Aur => "AUR".to_string(),
                    crate::state::Source::Official { repo, .. } => repo.clone(),
                };
                let mut header_text = i18n::t_fmt(
                    app,
                    "app.modals.preflight.sandbox.package_label",
                    &[pkg_name, &source_str],
                );
                if !arrow_symbol.is_empty() {
                    header_text = format!("{} {}", arrow_symbol, header_text);
                }

                lines.push(Line::from(Span::styled(header_text, header_style)));

                last_dep_type = None;

                // Show message for official packages or collapsed AUR packages
                if !is_aur {
                    lines.push(Line::from(Span::styled(
                        format!(
                            "  {}",
                            i18n::t(
                                app,
                                "app.modals.preflight.sandbox.official_packages_prebuilt"
                            )
                        ),
                        Style::default().fg(th.subtext0),
                    )));
                } else if !is_expanded {
                    // Show dependency count for collapsed AUR packages
                    if let Some(info) = sandbox_info.iter().find(|s| s.package_name == *pkg_name) {
                        let dep_count = info.depends.len()
                            + info.makedepends.len()
                            + info.checkdepends.len()
                            + info.optdepends.len();
                        if dep_count > 0 {
                            lines.push(Line::from(Span::styled(
                                format!(
                                    "  {}",
                                    i18n::t_fmt1(
                                        app,
                                        "app.modals.preflight.sandbox.dependencies_expand_hint",
                                        dep_count.to_string()
                                    )
                                ),
                                Style::default().fg(th.subtext1),
                            )));
                        } else {
                            lines.push(Line::from(Span::styled(
                                format!(
                                    "  {}",
                                    i18n::t(
                                        app,
                                        "app.modals.preflight.sandbox.no_build_dependencies"
                                    )
                                ),
                                Style::default().fg(th.green),
                            )));
                        }
                    }
                }
            } else if let Some((dep_type, dep_name, dep)) = dep_opt {
                // Dependency item (indented)
                // Show section header when dep_type changes
                if last_dep_type != Some(dep_type) {
                    let section_name = match *dep_type {
                        "depends" => {
                            i18n::t(app, "app.modals.preflight.sandbox.runtime_dependencies")
                        }
                        "makedepends" => {
                            i18n::t(app, "app.modals.preflight.sandbox.build_dependencies")
                        }
                        "checkdepends" => {
                            i18n::t(app, "app.modals.preflight.sandbox.test_dependencies")
                        }
                        "optdepends" => {
                            i18n::t(app, "app.modals.preflight.sandbox.optional_dependencies")
                        }
                        _ => String::new(),
                    };
                    if !section_name.is_empty() {
                        lines.push(Line::from(Span::styled(
                            section_name,
                            Style::default()
                                .fg(th.sapphire)
                                .add_modifier(Modifier::BOLD),
                        )));
                    }
                    last_dep_type = Some(dep_type);
                }

                // Dependency line with selection highlight
                // Check if this is a selected optional dependency
                let is_optdep_selected = if *dep_type == "optdepends" {
                    selected_optdepends
                        .get(pkg_name)
                        .map(|set| {
                            // Extract package name from dependency spec (may include version or description)
                            let pkg_name_from_dep =
                                crate::logic::sandbox::extract_package_name(dep_name);
                            set.contains(dep_name) || set.contains(&pkg_name_from_dep)
                        })
                        .unwrap_or(false)
                } else {
                    false
                };

                let status_icon = if dep.is_installed {
                    if dep.version_satisfied { "✓" } else { "⚠" }
                } else {
                    match *dep_type {
                        "optdepends" => {
                            if is_optdep_selected {
                                "☑" // Checkbox checked
                            } else {
                                "☐" // Checkbox unchecked
                            }
                        }
                        "checkdepends" => "○",
                        _ => "✗",
                    }
                };
                let status_color = if dep.is_installed {
                    if dep.version_satisfied {
                        th.green
                    } else {
                        th.yellow
                    }
                } else {
                    match *dep_type {
                        "optdepends" => {
                            if is_optdep_selected {
                                th.sapphire // Highlight selected optdepends
                            } else {
                                th.subtext0
                            }
                        }
                        "checkdepends" => th.subtext0,
                        _ => th.red,
                    }
                };

                let dep_style = if is_selected {
                    Style::default()
                        .fg(th.crust)
                        .bg(th.sapphire)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(status_color)
                };

                let mut dep_line = format!("  {} {}", status_icon, dep_name);
                if let Some(ref version) = dep.installed_version {
                    dep_line.push_str(&format!(" (installed: {})", version));
                }
                if *dep_type == "optdepends" && is_optdep_selected {
                    dep_line.push_str(" [selected]");
                }
                lines.push(Line::from(Span::styled(dep_line, dep_style)));
            }
        }

        // Show indicator if there are more items below
        if end_idx < total_items {
            lines.push(Line::from(Span::styled(
                format!(
                    "… {} more item{}",
                    total_items - end_idx,
                    if total_items - end_idx == 1 { "" } else { "s" }
                ),
                Style::default().fg(th.subtext1),
            )));
        }

        // If no packages at all
        if items.is_empty() {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.preflight.sandbox.no_packages"),
                Style::default().fg(th.subtext0),
            )));
        }
    }

    lines
}
