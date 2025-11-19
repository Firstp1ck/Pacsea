use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::{AppState, PackageItem};
use crate::theme::theme;
use std::collections::{HashMap, HashSet};

/// What: Type alias for sandbox display items.
///
/// Inputs: None
///
/// Output: None
///
/// Details:
/// - Represents a display item in the sandbox tab.
/// - Format: (is_header, package_name, Option<(dep_type, dep_name, dep_info)>)
type SandboxDisplayItem = (
    bool,
    String,
    Option<(
        &'static str, // "depends", "makedepends", "checkdepends", "optdepends"
        String,       // dependency name
        crate::logic::sandbox::DependencyDelta,
    )>,
);

/// What: Render error state for sandbox tab.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `error`: Error message to display.
///
/// Output:
/// - Returns vector of lines for error display.
///
/// Details:
/// - Shows error message and retry hint.
fn render_error_state(app: &AppState, error: &str) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();
    tracing::warn!("[UI] render_sandbox_tab: Displaying error: {}", error);
    lines.push(Line::from(Span::styled(
        i18n::t_fmt1(app, "app.modals.preflight.sandbox.error", error),
        Style::default().fg(th.red),
    )));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.preflight.sandbox.retry_hint"),
        Style::default().fg(th.subtext0),
    )));
    lines.push(Line::from(""));
    lines
}

/// What: Render AUR package headers.
///
/// Inputs:
/// - `items`: Packages to render headers for.
///
/// Output:
/// - Returns vector of lines with package headers.
///
/// Details:
/// - Only renders headers for AUR packages.
fn render_aur_package_headers(items: &[PackageItem]) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();
    for item in items.iter() {
        let is_aur = matches!(item.source, crate::state::Source::Aur);
        if is_aur {
            lines.push(Line::from(Span::styled(
                format!("▶ {} ", item.name),
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
        }
    }
    lines
}

/// What: Render loading state for sandbox tab.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `items`: Packages to show headers for.
///
/// Output:
/// - Returns vector of lines for loading display.
///
/// Details:
/// - Shows AUR package headers and loading message.
fn render_loading_state(app: &AppState, items: &[PackageItem]) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = render_aur_package_headers(items);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.preflight.sandbox.updating"),
        Style::default().fg(th.yellow),
    )));
    lines
}

/// What: Render analyzing state for sandbox tab.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `items`: Packages to show headers for.
///
/// Output:
/// - Returns vector of lines for analyzing display.
///
/// Details:
/// - Shows AUR package headers and analyzing message.
fn render_analyzing_state(app: &AppState, items: &[PackageItem]) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = render_aur_package_headers(items);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.preflight.sandbox.analyzing"),
        Style::default().fg(th.subtext0),
    )));
    lines
}

/// What: Build flat list of display items from packages and sandbox info.
///
/// Inputs:
/// - `items`: Packages under review.
/// - `sandbox_info`: Sandbox information.
/// - `sandbox_tree_expanded`: Set of expanded package names.
///
/// Output:
/// - Returns vector of display items.
///
/// Details:
/// - Creates flat list with package headers and dependencies (only if expanded).
fn build_display_items(
    items: &[PackageItem],
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_tree_expanded: &HashSet<String>,
) -> Vec<SandboxDisplayItem> {
    let mut display_items = Vec::new();

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
            tracing::debug!(
                "[UI] render_sandbox_tab: Expanding package '{}' with {} depends, {} makedepends, {} checkdepends, {} optdepends",
                item.name,
                info.depends.len(),
                info.makedepends.len(),
                info.checkdepends.len(),
                info.optdepends.len()
            );
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
        } else if is_aur && is_expanded {
            // AUR package is expanded but no sandbox info found - log warning
            tracing::warn!(
                "[UI] render_sandbox_tab: Package '{}' is AUR and expanded but no sandbox info found",
                item.name
            );
        }
    }

    display_items
}

/// What: Calculate viewport range for visible items.
///
/// Inputs:
/// - `total_items`: Total number of display items.
/// - `selected_idx`: Currently selected item index.
/// - `available_height`: Available height for rendering.
/// - `sandbox_selected`: Mutable reference to selected index (for clamping).
///
/// Output:
/// - Returns (start_idx, end_idx) viewport range.
///
/// Details:
/// - Ensures selected item is always visible.
/// - Accounts for section headers that add extra lines.
fn calculate_viewport(
    total_items: usize,
    selected_idx: usize,
    available_height: usize,
    sandbox_selected: &mut usize,
) -> (usize, usize) {
    // Validate and clamp selected index to prevent out-of-bounds access
    let sandbox_selected_clamped = if total_items > 0 {
        selected_idx.min(total_items.saturating_sub(1))
    } else {
        0
    };
    if *sandbox_selected != sandbox_selected_clamped {
        tracing::warn!(
            "[UI] render_sandbox_tab: Clamping sandbox_selected from {} to {} (total_items={})",
            *sandbox_selected,
            sandbox_selected_clamped,
            total_items
        );
        *sandbox_selected = sandbox_selected_clamped;
    }

    if total_items <= available_height {
        // All items fit on screen
        return (0, total_items);
    }

    // Ensure selected item is always visible - keep it within [start_idx, end_idx)
    // Try to center it, but adjust if needed to keep it visible
    // Reduce available_height slightly to account for section headers that add extra lines
    let effective_height = available_height.saturating_sub(2); // Reserve space for section headers
    let mut start_idx = sandbox_selected_clamped
        .saturating_sub(effective_height / 2)
        .max(0)
        .min(total_items.saturating_sub(effective_height));
    let mut end_idx = (start_idx + effective_height).min(total_items);

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

    (start_idx, end_idx)
}

/// What: Get dependency status icon based on dependency state.
///
/// Inputs:
/// - `dep`: Dependency delta information.
/// - `dep_type`: Type of dependency.
/// - `is_optdep_selected`: Whether optional dependency is selected.
///
/// Output:
/// - Returns status icon string.
///
/// Details:
/// - Returns appropriate icon based on installation and version status.
fn get_dependency_status_icon(
    dep: &crate::logic::sandbox::DependencyDelta,
    dep_type: &str,
    is_optdep_selected: bool,
) -> &'static str {
    if dep.is_installed {
        if dep.version_satisfied { "✓" } else { "⚠" }
    } else {
        match dep_type {
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
    }
}

/// What: Get dependency status color based on dependency state.
///
/// Inputs:
/// - `dep`: Dependency delta information.
/// - `dep_type`: Type of dependency.
/// - `is_optdep_selected`: Whether optional dependency is selected.
///
/// Output:
/// - Returns color from theme.
///
/// Details:
/// - Returns appropriate color based on installation and version status.
fn get_dependency_status_color(
    dep: &crate::logic::sandbox::DependencyDelta,
    dep_type: &str,
    is_optdep_selected: bool,
) -> ratatui::style::Color {
    let th = theme();
    if dep.is_installed {
        if dep.version_satisfied {
            th.green
        } else {
            th.yellow
        }
    } else {
        match dep_type {
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
    }
}

/// What: Render package header line.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `item`: Package item to render.
/// - `is_expanded`: Whether package is expanded.
/// - `is_selected`: Whether package is selected.
///
/// Output:
/// - Returns line for package header.
///
/// Details:
/// - Formats package header with source and expansion indicator.
fn render_package_header(
    app: &AppState,
    item: &PackageItem,
    is_expanded: bool,
    is_selected: bool,
) -> Line<'static> {
    let th = theme();
    let is_aur = matches!(item.source, crate::state::Source::Aur);
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
        &[&item.name, &source_str],
    );
    if !arrow_symbol.is_empty() {
        header_text = format!("{} {}", arrow_symbol, header_text);
    }

    Line::from(Span::styled(header_text, header_style))
}

/// What: Render package header details (messages for official/collapsed packages).
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `pkg_name`: Package name.
/// - `is_aur`: Whether package is from AUR.
/// - `is_expanded`: Whether package is expanded.
/// - `sandbox_info`: Sandbox information to get dependency counts.
///
/// Output:
/// - Returns vector of lines for package details.
///
/// Details:
/// - Shows message for official packages or dependency count for collapsed AUR packages.
fn render_package_header_details(
    app: &AppState,
    pkg_name: &str,
    is_aur: bool,
    is_expanded: bool,
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

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
        if let Some(info) = sandbox_info.iter().find(|s| s.package_name == pkg_name) {
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
                        i18n::t(app, "app.modals.preflight.sandbox.no_build_dependencies")
                    ),
                    Style::default().fg(th.green),
                )));
            }
        } else {
            // AUR package but no sandbox info - this shouldn't happen but handle gracefully
            tracing::debug!(
                "[UI] render_sandbox_tab: AUR package '{}' collapsed but no sandbox info found",
                pkg_name
            );
        }
    }

    lines
}

/// What: Render dependency section header when dependency type changes.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `dep_type`: Type of dependency.
///
/// Output:
/// - Returns optional line for section header.
///
/// Details:
/// - Returns section header line if dep_type is valid.
fn render_dependency_section_header(app: &AppState, dep_type: &str) -> Option<Line<'static>> {
    let th = theme();
    let section_name = match dep_type {
        "depends" => i18n::t(app, "app.modals.preflight.sandbox.runtime_dependencies"),
        "makedepends" => i18n::t(app, "app.modals.preflight.sandbox.build_dependencies"),
        "checkdepends" => i18n::t(app, "app.modals.preflight.sandbox.test_dependencies"),
        "optdepends" => i18n::t(app, "app.modals.preflight.sandbox.optional_dependencies"),
        _ => return None,
    };
    Some(Line::from(Span::styled(
        section_name,
        Style::default()
            .fg(th.sapphire)
            .add_modifier(Modifier::BOLD),
    )))
}

/// What: Render dependency line.
///
/// Inputs:
/// - `dep_name`: Dependency name.
/// - `dep`: Dependency delta information.
/// - `dep_type`: Type of dependency.
/// - `is_optdep_selected`: Whether optional dependency is selected.
/// - `is_selected`: Whether dependency is selected.
///
/// Output:
/// - Returns line for dependency.
///
/// Details:
/// - Formats dependency line with status icon and version info.
fn render_dependency_line(
    dep_name: &str,
    dep: &crate::logic::sandbox::DependencyDelta,
    dep_type: &str,
    is_optdep_selected: bool,
    is_selected: bool,
) -> Line<'static> {
    let th = theme();
    let status_icon = get_dependency_status_icon(dep, dep_type, is_optdep_selected);
    let status_color = get_dependency_status_color(dep, dep_type, is_optdep_selected);

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
    if dep_type == "optdepends" && is_optdep_selected {
        dep_line.push_str(" [selected]");
    }
    Line::from(Span::styled(dep_line, dep_style))
}

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

    // Log render state for debugging
    tracing::debug!(
        "[UI] render_sandbox_tab: items={}, sandbox_info={}, sandbox_loaded={}, sandbox_selected={}, expanded={}, resolving={}/{}",
        items.len(),
        sandbox_info.len(),
        sandbox_loaded,
        sandbox_selected,
        sandbox_tree_expanded.len(),
        app.preflight_sandbox_resolving,
        app.sandbox_resolving
    );

    // Log detailed dependency information only at DEBUG level (called on every render)
    // Detailed package info is already logged in sync_sandbox when data changes
    if !sandbox_info.is_empty() {
        tracing::debug!(
            "[UI] render_sandbox_tab: Rendering {} sandbox info entries",
            sandbox_info.len()
        );
    }

    // Handle error/loading/analyzing states
    if let Some(err) = sandbox_error.as_ref() {
        return render_error_state(app, err);
    } else if app.preflight_sandbox_resolving || app.sandbox_resolving {
        tracing::debug!(
            "[UI] render_sandbox_tab: Showing loading state (resolving={}/{})",
            app.preflight_sandbox_resolving,
            app.sandbox_resolving
        );
        let aur_count = items
            .iter()
            .filter(|i| matches!(i.source, crate::state::Source::Aur))
            .count();
        tracing::debug!(
            "[UI] render_sandbox_tab: Showing {} AUR package headers",
            aur_count
        );
        return render_loading_state(app, items);
    } else if !sandbox_loaded || sandbox_info.is_empty() {
        tracing::debug!(
            "[UI] render_sandbox_tab: Not loaded or empty (loaded={}, info_len={}), showing analyzing message",
            sandbox_loaded,
            sandbox_info.len()
        );
        return render_analyzing_state(app, items);
    }

    // Build flat list of display items: package headers + dependencies (only if expanded)
    let display_items = build_display_items(items, sandbox_info, sandbox_tree_expanded);

    // Calculate viewport based on selected index (like Deps/Files tabs)
    // Performance optimization: Only render visible items (viewport-based rendering)
    // This prevents performance issues with large dependency lists
    let available_height = (content_rect.height as usize).saturating_sub(6);
    let total_items = display_items.len();
    tracing::debug!(
        "[UI] render_sandbox_tab: Rendering data - total_items={}, sandbox_selected={}, items={}, sandbox_info={}, expanded_count={}, available_height={}",
        total_items,
        *sandbox_selected,
        items.len(),
        sandbox_info.len(),
        sandbox_tree_expanded.len(),
        available_height
    );

    let (start_idx, end_idx) = calculate_viewport(
        total_items,
        *sandbox_selected,
        available_height,
        sandbox_selected,
    );

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
            let Some(item) = items.iter().find(|p| p.name == *pkg_name) else {
                tracing::warn!(
                    "[UI] render_sandbox_tab: Package '{}' not found in items list, skipping",
                    pkg_name
                );
                continue;
            };
            let is_aur = matches!(item.source, crate::state::Source::Aur);
            let is_expanded = sandbox_tree_expanded.contains(pkg_name);

            lines.push(render_package_header(app, item, is_expanded, is_selected));
            last_dep_type = None;

            // Show message for official packages or collapsed AUR packages
            lines.extend(render_package_header_details(
                app,
                pkg_name,
                is_aur,
                is_expanded,
                sandbox_info,
            ));
        } else if let Some((dep_type, dep_name, dep)) = dep_opt {
            // Dependency item (indented)
            // Show section header when dep_type changes
            if last_dep_type != Some(dep_type) {
                if let Some(header_line) = render_dependency_section_header(app, dep_type) {
                    lines.push(header_line);
                }
                last_dep_type = Some(dep_type);
            }

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

            lines.push(render_dependency_line(
                dep_name,
                dep,
                dep_type,
                is_optdep_selected,
                is_selected,
            ));
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

    lines
}
