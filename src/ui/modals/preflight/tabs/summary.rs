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
use crate::state::{AppState, PackageItem, PreflightAction};
use crate::theme::theme;
use std::fmt::Write;

/// What: Get source badge text and color for a dependency source.
///
/// Inputs:
/// - `source`: The dependency source.
///
/// Output:
/// - Returns a tuple of (`badge_text`, `color`).
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
            (format!(" [{repo}]"), color)
        }
        DependencySource::Aur => (" [AUR]".to_string(), th.yellow),
        DependencySource::Local => (" [local]".to_string(), th.overlay1),
    }
}

/// What: Render summary data section (risk factors).
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
/// - Shows risk factors if available.
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
            let bullet = format!("  • {reason}");
            lines.push(Line::from(Span::styled(
                bullet,
                Style::default().fg(th.subtext1),
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

    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();
    let packages_with_deps: std::collections::HashSet<String> = dependency_info
        .iter()
        .flat_map(|d| d.required_by.iter())
        .cloned()
        .collect();
    let packages_with_deps_count = packages_with_deps.len();

    // Check if deps data is incomplete for current items (when not resolving)
    let deps_resolving = app.preflight_deps_resolving || app.deps_resolving;
    let deps_incomplete = !deps_resolving
        && !item_names.is_empty()
        && packages_with_deps_count < items.len()
        && dependency_info.len() < items.len();

    // Check if we actually need to wait for data for the current items
    // Only show loading indicator if:
    // 1. Preflight-specific resolution is running (preflight_*_resolving), OR
    // 2. Global resolution is running (files_resolving, deps_resolving, etc.)
    // 3. We're missing data for current items (deps_incomplete heuristic)
    let files_resolving = app.preflight_files_resolving || app.files_resolving;
    let sandbox_resolving = app.preflight_sandbox_resolving || app.sandbox_resolving;

    // Show deps indicator when deps are resolving (matches render_deps_tab logic)
    let show_deps_indicator = deps_resolving || (deps_incomplete && dependency_info.is_empty());
    // Show files indicator when files are resolving (matches render_files_tab logic)
    let show_files_indicator = files_resolving;
    // Show sandbox indicator when sandbox is resolving
    let show_sandbox_indicator = sandbox_resolving;

    tracing::debug!(
        "[UI] render_incomplete_data_indicator: preflight_deps_resolving={}, deps_resolving={}, computed_deps_resolving={}, files_resolving={}/{}, sandbox_resolving={}/{}, deps_incomplete={}, items={}, packages_with_deps={}, dependency_info={}, show_deps_indicator={}, show_files_indicator={}, show_sandbox_indicator={}",
        app.preflight_deps_resolving,
        app.deps_resolving,
        deps_resolving,
        app.preflight_files_resolving,
        app.files_resolving,
        app.preflight_sandbox_resolving,
        app.sandbox_resolving,
        deps_incomplete,
        items.len(),
        packages_with_deps_count,
        dependency_info.len(),
        show_deps_indicator,
        show_files_indicator,
        show_sandbox_indicator
    );

    let has_incomplete_data =
        show_deps_indicator || show_files_indicator || show_sandbox_indicator || deps_incomplete;

    if has_incomplete_data {
        let mut resolving_parts = Vec::new();
        if show_deps_indicator {
            resolving_parts.push(i18n::t(app, "app.modals.preflight.summary.resolving_deps"));
        }
        if show_files_indicator {
            resolving_parts.push(i18n::t(app, "app.modals.preflight.summary.resolving_files"));
        }
        if show_sandbox_indicator {
            resolving_parts.push(i18n::t(
                app,
                "app.modals.preflight.summary.resolving_sandbox",
            ));
        }
        if !resolving_parts.is_empty() {
            lines.push(Line::from(""));
            let resolving_text = resolving_parts.join(", ");
            lines.push(Line::from(Span::styled(
                format!("⟳ {resolving_text}"),
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
                format!(" ({reason})"),
                Style::default().fg(th.red),
            ));
        }
        DependencyStatus::ToUpgrade { current, required } => {
            spans.push(Span::styled("↑ ", Style::default().fg(th.yellow)));
            spans.push(Span::styled(dep.name.clone(), Style::default().fg(th.text)));
            if !dep.version.is_empty() {
                let dep_version = &dep.version;
                spans.push(Span::styled(
                    format!(" {dep_version}"),
                    Style::default().fg(th.overlay2),
                ));
            }
            let (source_badge, badge_color) = get_source_badge(&dep.source);
            spans.push(Span::styled(source_badge, Style::default().fg(badge_color)));
            spans.push(Span::styled(
                format!(" ({current} → {required})"),
                Style::default().fg(th.yellow),
            ));
        }
        _ => return None,
    }
    Some(spans)
}

/// What: Filter installed packages from items list.
///
/// Inputs:
/// - `items`: List of packages to check.
///
/// Output:
/// - Vector of references to packages that are already installed.
fn filter_installed_packages(items: &[PackageItem]) -> Vec<&PackageItem> {
    items
        .iter()
        .filter(|item| crate::index::is_installed(&item.name))
        .collect()
}

/// What: Filter important dependencies (conflicts and upgrades).
///
/// Inputs:
/// - `dependency_info`: List of dependency information.
///
/// Output:
/// - Vector of references to dependencies with conflicts or upgrades.
fn filter_important_deps(dependency_info: &[DependencyInfo]) -> Vec<&DependencyInfo> {
    dependency_info
        .iter()
        .filter(|d| {
            matches!(
                d.status,
                DependencyStatus::Conflict { .. } | DependencyStatus::ToUpgrade { .. }
            )
        })
        .collect()
}

/// What: Group dependencies by the packages that require them.
///
/// Inputs:
/// - `important_deps`: List of important dependencies.
///
/// Output:
/// - `HashMap` mapping package names to their dependencies.
fn group_dependencies_by_package<'a>(
    important_deps: &[&'a DependencyInfo],
) -> std::collections::HashMap<String, Vec<&'a DependencyInfo>> {
    use std::collections::HashMap;
    let mut grouped: HashMap<String, Vec<&DependencyInfo>> = HashMap::new();
    for dep in important_deps {
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(*dep);
        }
    }
    grouped
}

/// What: Build summary parts for conflicts, upgrades, and installed packages.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `conflict_count`: Number of conflicts.
/// - `upgrade_count`: Number of upgrades.
/// - `installed_count`: Number of installed packages.
///
/// Output:
/// - Vector of formatted summary strings.
fn build_summary_parts(
    app: &AppState,
    conflict_count: usize,
    upgrade_count: usize,
    installed_count: usize,
) -> Vec<String> {
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
    if installed_count > 0 {
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.summary.installed_singular",
            installed_count,
        ));
    }
    summary_parts
}

/// What: Build display items list for rendering.
///
/// Inputs:
/// - `items`: List of packages.
/// - `grouped`: Dependencies grouped by package.
/// - `installed_packages`: List of installed packages.
///
/// Output:
/// - Vector of (`is_header`, `package_name`, `optional_dependency`) tuples.
fn build_display_items<'a>(
    items: &[PackageItem],
    grouped: &std::collections::HashMap<String, Vec<&'a DependencyInfo>>,
    installed_packages: &[&PackageItem],
) -> Vec<(bool, String, Option<&'a DependencyInfo>)> {
    use std::collections::HashSet;
    let mut display_items: Vec<(bool, String, Option<&DependencyInfo>)> = Vec::new();
    for pkg_name in items.iter().map(|p| &p.name) {
        if let Some(pkg_deps) = grouped.get(pkg_name) {
            display_items.push((true, pkg_name.clone(), None));
            let mut seen_deps = HashSet::new();
            for dep in pkg_deps {
                if seen_deps.insert(dep.name.as_str()) {
                    display_items.push((false, String::new(), Some(*dep)));
                }
            }
        }
    }
    for installed_pkg in installed_packages {
        if !grouped.contains_key(&installed_pkg.name) {
            display_items.push((true, installed_pkg.name.clone(), None));
            display_items.push((false, installed_pkg.name.clone(), None));
        }
    }
    display_items
}

/// What: Render install action dependencies (conflicts and upgrades).
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `items`: Packages under review.
/// - `dependency_info`: Dependency information.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows conflicts and upgrades grouped by package.
/// - Displays installed packages separately.
fn render_install_dependencies(
    app: &AppState,
    items: &[PackageItem],
    dependency_info: &[DependencyInfo],
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    let installed_packages = filter_installed_packages(items);
    let important_deps = filter_important_deps(dependency_info);

    if important_deps.is_empty() && installed_packages.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.no_conflicts_or_upgrades"),
            Style::default().fg(th.green),
        )));
        return lines;
    }

    let grouped = group_dependencies_by_package(&important_deps);
    let conflict_count = important_deps
        .iter()
        .filter(|d| matches!(d.status, DependencyStatus::Conflict { .. }))
        .count();
    let upgrade_count = important_deps
        .iter()
        .filter(|d| matches!(d.status, DependencyStatus::ToUpgrade { .. }))
        .count();
    let installed_count = installed_packages.len();

    let summary_parts = build_summary_parts(app, conflict_count, upgrade_count, installed_count);
    let header_text = if conflict_count > 0 || installed_count > 0 {
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
            .fg(if conflict_count > 0 || installed_count > 0 {
                th.red
            } else {
                th.yellow
            })
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let display_items = build_display_items(items, &grouped, &installed_packages);

    // Render all items (no viewport - mouse scrolling handles it)
    for (is_header, pkg_name, dep) in &display_items {
        if *is_header {
            let style = Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD);
            lines.push(Line::from(Span::styled(format!("▶ {pkg_name}"), style)));
        } else if let Some(dep) = dep {
            if let Some(spans) = render_dependency_spans(dep) {
                lines.push(Line::from(spans));
            }
        } else if !pkg_name.is_empty() && installed_packages.iter().any(|p| p.name == *pkg_name) {
            // This is an installed package entry (not a real dependency)
            // Show "installed" status
            let spans = vec![
                Span::styled("  ", Style::default()),
                Span::styled("✓ ", Style::default().fg(th.green)),
                Span::styled(pkg_name.clone(), Style::default().fg(th.text)),
                Span::styled(" (installed)", Style::default().fg(th.subtext1)),
            ];
            lines.push(Line::from(spans));
        }
    }
    lines
}

// Constants for removal action rendering
const CASCADE_PREVIEW_MAX: usize = 8;

/// What: Context data for cascade rendering operations.
///
/// Inputs:
/// - `removal_targets`: Set of package names to be removed (lowercase).
/// - `allows_dependents`: Cached value of `cascade_mode.allows_dependents()`.
///
/// Output:
/// - Returns a `CascadeRenderingContext` struct.
///
/// Details:
/// - Groups related data to reduce parameter passing and variable scope.
struct CascadeRenderingContext {
    removal_targets: std::collections::HashSet<String>,
    allows_dependents: bool,
}

impl CascadeRenderingContext {
    /// What: Create a new cascade rendering context.
    ///
    /// Inputs:
    /// - `items`: Packages to remove.
    /// - `cascade_mode`: Removal cascade mode.
    ///
    /// Output:
    /// - Returns a `CascadeRenderingContext`.
    ///
    /// Details:
    /// - Pre-computes removal targets and `allows_dependents` flag.
    fn new(items: &[PackageItem], cascade_mode: CascadeMode) -> Self {
        let removal_targets: std::collections::HashSet<String> = items
            .iter()
            .map(|pkg| pkg.name.to_ascii_lowercase())
            .collect();
        let allows_dependents = cascade_mode.allows_dependents();
        Self {
            removal_targets,
            allows_dependents,
        }
    }

    /// What: Check if a dependency is directly dependent on removal targets.
    ///
    /// Inputs:
    /// - `dep`: Dependency information.
    ///
    /// Output:
    /// - Returns true if dependency is directly dependent.
    ///
    /// Details:
    /// - Checks if any parent in `depends_on` is in `removal_targets`.
    fn is_direct_dependent(&self, dep: &DependencyInfo) -> bool {
        dep.depends_on
            .iter()
            .any(|parent| self.removal_targets.contains(&parent.to_ascii_lowercase()))
    }
}

/// What: Display information for a cascade candidate dependency.
///
/// Inputs:
/// - `bullet`: Bullet character to display.
/// - `name_color`: Color for the dependency name.
/// - `detail`: Detail text about the dependency status.
/// - `roots`: Formatted string of packages that require this dependency.
///
/// Output:
/// - Returns a `DependencyDisplayInfo` struct.
///
/// Details:
/// - Groups all display-related data for a dependency.
struct DependencyDisplayInfo {
    bullet: &'static str,
    name_color: ratatui::style::Color,
    detail: String,
    roots: String,
}

/// What: Get bullet character and name color for a dependency based on cascade mode.
///
/// Inputs:
/// - `allows_dependents`: Whether cascade mode allows dependents.
/// - `is_direct`: Whether dependency is directly dependent.
/// - `th`: Theme colors.
///
/// Output:
/// - Returns a tuple of (`bullet`, `name_color`).
///
/// Details:
/// - Simplifies conditional logic for bullet and color selection.
const fn get_bullet_and_color(
    allows_dependents: bool,
    is_direct: bool,
    th: &crate::theme::Theme,
) -> (&'static str, ratatui::style::Color) {
    if allows_dependents {
        if is_direct {
            ("● ", th.red)
        } else {
            ("○ ", th.yellow)
        }
    } else if is_direct {
        ("⛔ ", th.red)
    } else {
        ("⚠ ", th.yellow)
    }
}

/// What: Get detail text for a dependency status.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `status`: Dependency status.
///
/// Output:
/// - Returns detail text string.
///
/// Details:
/// - Extracts status-to-text conversion logic.
fn get_dependency_detail(app: &AppState, status: &DependencyStatus) -> String {
    match status {
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
    }
}

/// What: Build dependency display information.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `dep`: Dependency information.
/// - `ctx`: Cascade rendering context.
/// - `th`: Theme colors.
/// - `is_direct`: Pre-computed flag indicating if dependency is directly dependent.
///
/// Output:
/// - Returns `DependencyDisplayInfo`.
///
/// Details:
/// - Prepares all display data for a dependency in one place.
/// - Uses pre-computed `is_direct` to avoid recalculation.
fn build_dependency_display_info(
    app: &AppState,
    dep: &DependencyInfo,
    ctx: &CascadeRenderingContext,
    th: &crate::theme::Theme,
    is_direct: bool,
) -> DependencyDisplayInfo {
    let (bullet, name_color) = get_bullet_and_color(ctx.allows_dependents, is_direct, th);
    let detail = get_dependency_detail(app, &dep.status);
    let roots = if dep.required_by.is_empty() {
        String::new()
    } else {
        i18n::t_fmt1(
            app,
            "app.modals.preflight.summary.targets_label",
            dep.required_by.join(", "),
        )
    };
    DependencyDisplayInfo {
        bullet,
        name_color,
        detail,
        roots,
    }
}

/// What: Build spans for a cascade candidate dependency.
///
/// Inputs:
/// - `dep`: Dependency information.
/// - `display_info`: Display information for the dependency.
/// - `th`: Theme colors.
///
/// Output:
/// - Returns a vector of spans.
///
/// Details:
/// - Uses builder pattern to construct dependency spans.
fn build_cascade_dependency_spans(
    dep: &DependencyInfo,
    display_info: &DependencyDisplayInfo,
    th: &crate::theme::Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    spans.push(Span::styled(
        display_info.bullet,
        Style::default().fg(th.subtext0),
    ));
    spans.push(Span::styled(
        dep.name.clone(),
        Style::default()
            .fg(display_info.name_color)
            .add_modifier(Modifier::BOLD),
    ));
    if !display_info.detail.is_empty() {
        spans.push(Span::styled(" — ", Style::default().fg(th.subtext1)));
        spans.push(Span::styled(
            display_info.detail.clone(),
            Style::default().fg(th.subtext1),
        ));
    }
    if !display_info.roots.is_empty() {
        spans.push(Span::styled(
            display_info.roots.clone(),
            Style::default().fg(th.overlay1),
        ));
    }
    spans
}

/// What: Render cascade mode header.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `cascade_mode`: Removal cascade mode.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Renders the cascade mode information header.
fn render_cascade_mode_header(app: &AppState, cascade_mode: CascadeMode) -> Vec<Line<'static>> {
    let th = theme();
    let mode_line = i18n::t_fmt(
        app,
        "app.modals.preflight.summary.cascade_mode",
        &[&cascade_mode.flag(), &cascade_mode.description()],
    );
    vec![
        Line::from(Span::styled(
            mode_line,
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ]
}

/// What: Render removal plan command.
///
/// Inputs:
/// - `app`: Application state for i18n and `dry_run` flag.
/// - `items`: Packages to remove.
/// - `cascade_mode`: Removal cascade mode.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Renders the removal plan preview with command.
fn render_removal_plan(
    app: &AppState,
    items: &[PackageItem],
    cascade_mode: CascadeMode,
) -> Vec<Line<'static>> {
    let th = theme();
    let removal_names: Vec<&str> = items.iter().map(|pkg| pkg.name.as_str()).collect();
    let plan_header_style = Style::default()
        .fg(th.overlay1)
        .add_modifier(Modifier::BOLD);
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
    vec![
        Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.removal_plan_preview"),
            plan_header_style,
        )),
        Line::from(Span::styled(plan_command, Style::default().fg(th.text))),
    ]
}

/// What: Get dependent summary text and style.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `dependent_count`: Number of dependent packages.
/// - `allows_dependents`: Whether cascade mode allows dependents.
///
/// Output:
/// - Returns a tuple of (text, style).
///
/// Details:
/// - Determines summary message based on dependent count and cascade mode.
fn get_dependent_summary(
    app: &AppState,
    dependent_count: usize,
    allows_dependents: bool,
) -> (String, Style) {
    let th = theme();
    if dependent_count == 0 {
        (
            i18n::t(app, "app.modals.preflight.summary.no_dependents"),
            Style::default().fg(th.green),
        )
    } else if allows_dependents {
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
    }
}

/// What: Render dependent summary section.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `dependent_count`: Number of dependent packages.
/// - `allows_dependents`: Whether cascade mode allows dependents.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Renders the summary of dependent packages.
fn render_dependent_summary(
    app: &AppState,
    dependent_count: usize,
    allows_dependents: bool,
) -> Vec<Line<'static>> {
    let (summary_text, summary_style) =
        get_dependent_summary(app, dependent_count, allows_dependents);
    vec![
        Line::from(Span::styled(summary_text, summary_style)),
        Line::from(""),
    ]
}

/// What: Render removal impact overview.
///
/// Inputs:
/// - `app`: Application state for i18n and `remove_preflight_summary`.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Renders the impact overview or calculating message.
fn render_impact_overview(app: &AppState) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();
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
                let _ = write!(
                    message,
                    " {}",
                    i18n::t_fmt1(
                        app,
                        "app.modals.preflight.summary.direct_singular",
                        summary.direct_dependents
                    )
                );
            }
            if summary.transitive_dependents > 0 {
                let _ = write!(
                    message,
                    " {}",
                    i18n::t_fmt1(
                        app,
                        "app.modals.preflight.summary.transitive_singular",
                        summary.transitive_dependents
                    )
                );
            }
            lines.push(Line::from(Span::styled(
                message,
                Style::default().fg(th.text),
            )));
        }
        lines.push(Line::from(""));
    }
    lines
}

/// What: Get impact header text and style.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `allows_dependents`: Whether cascade mode allows dependents.
///
/// Output:
/// - Returns a tuple of (text, style).
///
/// Details:
/// - Determines impact header based on cascade mode.
fn get_impact_header(app: &AppState, allows_dependents: bool) -> (String, Style) {
    let th = theme();
    let text = if allows_dependents {
        i18n::t(app, "app.modals.preflight.summary.cascade_will_remove")
    } else {
        i18n::t(app, "app.modals.preflight.summary.dependents_not_removed")
    };
    (
        text,
        Style::default().fg(th.red).add_modifier(Modifier::BOLD),
    )
}

/// What: Prepare and sort cascade candidates.
///
/// Inputs:
/// - `dependency_info`: Dependency information.
/// - `ctx`: Cascade rendering context.
///
/// Output:
/// - Returns sorted vector of cascade candidates with `is_direct` flag.
///
/// Details:
/// - Prepares cascade candidates with pre-computed `is_direct` flag to avoid recomputation.
fn prepare_cascade_candidates<'a>(
    dependency_info: &'a [DependencyInfo],
    ctx: &CascadeRenderingContext,
) -> Vec<(&'a DependencyInfo, bool)> {
    let mut candidates: Vec<(&'a DependencyInfo, bool)> = dependency_info
        .iter()
        .map(|dep| (dep, ctx.is_direct_dependent(dep)))
        .collect();
    candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
    candidates
}

/// What: Render cascade candidates preview.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `candidates`: Prepared cascade candidates with `is_direct` flags.
/// - `ctx`: Cascade rendering context.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Renders the cascade candidates list with preview limit.
/// - Uses pre-computed `is_direct` flags to avoid recalculation.
fn render_cascade_candidates(
    app: &AppState,
    candidates: &[(&DependencyInfo, bool)],
    ctx: &CascadeRenderingContext,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();
    for (dep, is_direct) in candidates.iter().take(CASCADE_PREVIEW_MAX) {
        let display_info = build_dependency_display_info(app, dep, ctx, &th, *is_direct);
        let spans = build_cascade_dependency_spans(dep, &display_info, &th);
        lines.push(Line::from(spans));
    }
    if candidates.len() > CASCADE_PREVIEW_MAX {
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(
                app,
                "app.modals.preflight.summary.and_more_impacted",
                candidates.len() - CASCADE_PREVIEW_MAX,
            ),
            Style::default().fg(th.subtext1),
        )));
    }
    lines
}

/// What: Render cascade footer messages.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `allows_dependents`: Whether cascade mode allows dependents.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Renders footer messages based on cascade mode.
fn render_cascade_footer(app: &AppState, allows_dependents: bool) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = vec![Line::from("")];
    let footer_text = if allows_dependents {
        i18n::t(app, "app.modals.preflight.summary.will_be_removed_auto")
    } else {
        i18n::t(app, "app.modals.preflight.summary.enable_cascade_mode")
    };
    lines.push(Line::from(Span::styled(
        footer_text,
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.preflight.summary.use_deps_tab"),
        Style::default().fg(th.subtext1),
    )));
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
/// - Uses helper functions and data structures to reduce complexity.
fn render_remove_action(
    app: &AppState,
    items: &[PackageItem],
    dependency_info: &[DependencyInfo],
    cascade_mode: CascadeMode,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let th = theme();

    // Render cascade mode header
    lines.extend(render_cascade_mode_header(app, cascade_mode));

    // Early return for empty items
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.summary.no_removal_targets"),
            Style::default().fg(th.subtext1),
        )));
        return lines;
    }

    // Render removal plan
    lines.extend(render_removal_plan(app, items, cascade_mode));

    // Prepare context (caches repeated computations)
    let ctx = CascadeRenderingContext::new(items, cascade_mode);
    let dependent_count = dependency_info.len();

    // Render dependent summary
    lines.extend(render_dependent_summary(
        app,
        dependent_count,
        ctx.allows_dependents,
    ));

    // Render dependent impact section if there are dependents
    if dependent_count > 0 {
        lines.extend(render_impact_overview(app));

        // Render impact header
        let (impact_header, impact_style) = get_impact_header(app, ctx.allows_dependents);
        lines.push(Line::from(Span::styled(impact_header, impact_style)));

        // Prepare and render cascade candidates
        let candidates = prepare_cascade_candidates(dependency_info, &ctx);
        lines.extend(render_cascade_candidates(app, &candidates, &ctx));

        // Render footer
        lines.extend(render_cascade_footer(app, ctx.allows_dependents));
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
/// - `summary_selected`: Selected index in the package list (for navigation).
/// - `conflicts_selected`: Selected index in the conflicts list (for navigation).
/// - `header_chips`: Header chip data for risk level.
/// - `dependency_info`: Dependency information.
/// - `cascade_mode`: Removal cascade mode.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows risk factors and dependency conflicts/upgrades for install.
/// - Shows removal plan and cascade impact for remove actions.
/// - Supports scrolling through the conflicts list with viewport-based rendering.
/// - Only highlights the active section (conflicts) to avoid dual cursors.
#[allow(clippy::too_many_arguments)]
pub fn render_summary_tab(
    app: &AppState,
    items: &[PackageItem],
    action: PreflightAction,
    summary: Option<&Box<PreflightSummaryData>>,
    header_chips: &PreflightHeaderChips,
    dependency_info: &[DependencyInfo],
    cascade_mode: CascadeMode,
    _content_rect: Rect,
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

    match action {
        PreflightAction::Install if !dependency_info.is_empty() => {
            lines.extend(render_install_dependencies(app, items, dependency_info));
        }
        PreflightAction::Remove => {
            lines.extend(render_remove_action(
                app,
                items,
                dependency_info,
                cascade_mode,
            ));
        }
        PreflightAction::Install => {
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
