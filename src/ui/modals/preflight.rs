use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::modal::{
    CascadeMode, DependencyInfo, DependencySource, DependencyStatus, FileChangeType,
    PackageFileInfo, PreflightHeaderChips, PreflightSummaryData, ServiceImpact,
    ServiceRestartDecision,
};
use crate::state::{AppState, PackageItem, PreflightAction, PreflightTab, Source};
use crate::theme::theme;
use std::collections::HashSet;

fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut size = value as f64;
    let mut unit_index = 0usize;
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{} {}", value, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

fn format_signed_bytes(value: i64) -> String {
    if value == 0 {
        return "0 B".to_string();
    }
    let magnitude = value.unsigned_abs();
    if value > 0 {
        format!("+{}", format_bytes(magnitude))
    } else {
        format!("-{}", format_bytes(magnitude))
    }
}

/// What: Render header chips as a compact horizontal line of metrics.
///
/// Inputs:
/// - `chips`: Header chip data containing counts and sizes.
///
/// Output:
/// - Returns a `Line` containing styled chip spans separated by spaces.
///
/// Details:
/// - Formats package count, download size, install delta, AUR count, and risk score
///   as compact chips. Risk score uses color coding (green/yellow/red) based on level.
fn render_header_chips(app: &AppState, chips: &PreflightHeaderChips) -> Line<'static> {
    let th = theme();
    let mut spans = Vec::new();

    // Package count chip
    let pkg_text = if chips.aur_count > 0 {
        format!(
            "{}{}",
            chips.package_count,
            i18n::t_fmt1(
                app,
                "app.modals.preflight.header_chips.aur_packages",
                chips.aur_count
            )
        )
    } else {
        format!("{}", chips.package_count)
    };
    spans.push(Span::styled(
        format!("[{}]", pkg_text),
        Style::default()
            .fg(th.sapphire)
            .add_modifier(Modifier::BOLD),
    ));

    // Download size chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    spans.push(Span::styled(
        i18n::t_fmt1(
            app,
            "app.modals.preflight.header_chips.download_label",
            format_bytes(chips.download_bytes),
        ),
        Style::default().fg(th.sapphire),
    ));

    // Install delta chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    let delta_color = if chips.install_delta_bytes > 0 {
        th.green
    } else if chips.install_delta_bytes < 0 {
        th.red
    } else {
        th.overlay1 // Neutral color for zero
    };
    spans.push(Span::styled(
        i18n::t_fmt1(
            app,
            "app.modals.preflight.header_chips.size_label",
            format_signed_bytes(chips.install_delta_bytes),
        ),
        Style::default().fg(delta_color),
    ));

    // Risk score chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    let risk_label = match chips.risk_level {
        crate::state::modal::RiskLevel::Low => {
            i18n::t(app, "app.modals.preflight.header_chips.risk_low")
        }
        crate::state::modal::RiskLevel::Medium => {
            i18n::t(app, "app.modals.preflight.header_chips.risk_medium")
        }
        crate::state::modal::RiskLevel::High => {
            i18n::t(app, "app.modals.preflight.header_chips.risk_high")
        }
    };
    let risk_color = match chips.risk_level {
        crate::state::modal::RiskLevel::Low => th.green,
        crate::state::modal::RiskLevel::Medium => th.yellow,
        crate::state::modal::RiskLevel::High => th.red,
    };
    spans.push(Span::styled(
        format!("[Risk: {} ({})]", risk_label, chips.risk_score),
        Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
    ));

    Line::from(spans)
}

/// What: Render the preflight modal summarizing dependency/file checks before install/remove.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `app`: Mutable application state (stores tab rects/content rects)
/// - `items`: Packages under review
/// - `action`: Whether the preflight is for install or removal
/// - `tab`: Active tab (Summary/Deps/Files/Services/Sandbox)
/// - `dependency_info`, `dep_selected`, `dep_tree_expanded`: Mutable dependency state/cache
/// - `file_info`, `file_selected`, `file_tree_expanded`: Mutable file analysis state/cache
/// - `cascade_mode`: Removal cascade mode when uninstalling
///
/// Output:
/// - Draws the modal content for the chosen tab and updates cached data along with clickable rects.
///
/// Details:
/// - Lazily resolves dependencies/files when first accessed, lays out tab headers, records tab
///   rectangles for mouse navigation, and tailors summaries per tab with theming cues.
#[allow(clippy::too_many_arguments)]
pub fn render_preflight(
    f: &mut Frame,
    area: Rect,
    app: &mut AppState,
    items: &[PackageItem],
    action: &PreflightAction,
    tab: &PreflightTab,
    summary: &mut Option<Box<PreflightSummaryData>>,
    header_chips: &mut PreflightHeaderChips,
    dependency_info: &mut Vec<DependencyInfo>,
    dep_selected: &mut usize,
    dep_tree_expanded: &HashSet<String>,
    deps_error: &mut Option<String>,
    file_info: &mut Vec<PackageFileInfo>,
    file_selected: &mut usize,
    file_tree_expanded: &HashSet<String>,
    files_error: &mut Option<String>,
    service_info: &mut Vec<ServiceImpact>,
    service_selected: &mut usize,
    services_loaded: &mut bool,
    services_error: &mut Option<String>,
    sandbox_info: &mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_selected: &mut usize,
    sandbox_tree_expanded: &HashSet<String>,
    sandbox_loaded: &mut bool,
    sandbox_error: &mut Option<String>,
    selected_optdepends: &mut std::collections::HashMap<String, std::collections::HashSet<String>>,
    cascade_mode: CascadeMode,
) {
    let render_start = std::time::Instant::now();
    let th = theme();
    tracing::info!(
        "[UI] render_preflight START: tab={:?}, items={}, deps={}, files={}, services={}, sandbox={}",
        tab,
        items.len(),
        dependency_info.len(),
        file_info.len(),
        service_info.len(),
        sandbox_info.len()
    );
    // Load dependencies from cache - SIMPLIFIED: Always load when on Deps tab or when empty
    // Auto-resolve if cache is empty and we're on Deps tab
    if matches!(*action, PreflightAction::Install) {
        let should_load = dependency_info.is_empty() || matches!(*tab, PreflightTab::Deps);

        if should_load && matches!(*tab, PreflightTab::Deps) {
            if !app.install_list_deps.is_empty() {
                // Get set of current package names for filtering
                let item_names: std::collections::HashSet<String> =
                    items.iter().map(|i| i.name.clone()).collect();

                // Filter to only show dependencies required by current items
                let filtered: Vec<DependencyInfo> = app
                    .install_list_deps
                    .iter()
                    .filter(|dep| {
                        // Show dependency if any current item requires it
                        dep.required_by
                            .iter()
                            .any(|req_by| item_names.contains(req_by))
                    })
                    .cloned()
                    .collect();

                tracing::debug!(
                    "[UI] Deps tab: cache={}, filtered={}, items={:?}, resolving={}, current={}",
                    app.install_list_deps.len(),
                    filtered.len(),
                    item_names,
                    app.deps_resolving,
                    dependency_info.len()
                );

                // Always update when on Deps tab, but only reset selection if dependencies were empty (first load)
                // Don't reset on every render - that would break navigation
                let was_empty = dependency_info.is_empty();
                if !filtered.is_empty() || dependency_info.is_empty() {
                    *dependency_info = filtered;
                    // Only reset selection if this is the first load (was empty), not on every render
                    if was_empty {
                        *dep_selected = 0;
                    }
                }
            } else if dependency_info.is_empty() {
                // Check if background resolution is in progress
                if app.preflight_deps_resolving || app.deps_resolving {
                    // Background resolution in progress - UI will show loading state
                    tracing::debug!(
                        "[UI] Deps tab: background resolution in progress, items={:?}",
                        items.iter().map(|i| &i.name).collect::<Vec<_>>()
                    );
                } else {
                    // Cache is empty and no resolution in progress - trigger background resolution
                    // This will be handled by the event handler when switching to Deps tab
                    tracing::debug!(
                        "[UI] Deps tab: cache is empty, will auto-resolve, items={:?}",
                        items.iter().map(|i| &i.name).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    // Use cached file info if available
    // Note: Cached files are populated in background when packages are added to install list
    // Note: File resolution is triggered asynchronously in event handlers, not during rendering
    if matches!(*tab, PreflightTab::Files) {
        // Check if we have cached files from app state that match the current items
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_files: Vec<PackageFileInfo> = app
            .install_list_files
            .iter()
            .filter(|file_info| item_names.contains(&file_info.name))
            .cloned()
            .collect();
        // Sync results from background resolution if available
        if !cached_files.is_empty()
            && (file_info.is_empty() || cached_files.len() != file_info.len())
        {
            tracing::debug!(
                "[UI] Syncing {} file infos from background resolution to Preflight modal",
                cached_files.len()
            );
            *file_info = cached_files;
            if *file_selected >= file_info.len() {
                *file_selected = 0;
            }
        } else if file_info.is_empty() {
            // Check if background resolution is in progress
            if app.preflight_files_resolving || app.files_resolving {
                // Background resolution in progress - UI will show loading state
                tracing::debug!(
                    "[UI] Files tab: background resolution in progress, items={:?}",
                    items.iter().map(|i| &i.name).collect::<Vec<_>>()
                );
            }
            // If no cached files available, resolution will be triggered by event handlers when user navigates to Files tab
        }
    }
    // Use cached service info if available
    // Note: Cached services are pre-populated when modal opens, so this only runs if cache was empty
    // Note: Service resolution is triggered asynchronously in event handlers, not during rendering
    if service_info.is_empty() && matches!(*tab, PreflightTab::Services) && !*services_loaded {
        // Check if we have cached services from app state (for install actions)
        // Note: Empty cache is still valid - it means "no services found"
        if matches!(*action, PreflightAction::Install) && !app.services_resolving {
            // Check if cache file exists with matching signature (even if empty)
            let cache_check_start = std::time::Instant::now();
            let cache_exists = if !items.is_empty() {
                let signature = crate::app::services_cache::compute_signature(items);
                let result =
                    crate::app::services_cache::load_cache(&app.services_cache_path, &signature)
                        .is_some();
                let cache_duration = cache_check_start.elapsed();
                if cache_duration.as_millis() > 10 {
                    tracing::warn!(
                        "[UI] Services cache check took {:?} (slow!)",
                        cache_duration
                    );
                }
                result
            } else {
                false
            };

            if cache_exists {
                // Use cached services (may be empty, which is valid)
                if !app.install_list_services.is_empty() {
                    tracing::debug!(
                        "[UI] Using cached service impacts for {} packages",
                        app.install_list_services.len()
                    );
                    *service_info = app.install_list_services.clone();
                } else {
                    // Cache exists but is empty - this is valid, means no services found
                    tracing::debug!(
                        "[UI] Using cached service impacts (empty - no services found)"
                    );
                }
                *service_selected = 0;
                *services_loaded = true;
            } else {
                // No cache found - mark as loaded so we don't check again
                *services_loaded = true;
            }
            // If no cached services available, resolution will be triggered by event handlers when user navigates to Services tab
        }
        // For remove actions or when services are resolving, resolution will be triggered by event handlers
    }
    if !service_info.is_empty() && *service_selected >= service_info.len() {
        *service_selected = service_info.len().saturating_sub(1);
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
        PreflightAction::Install => i18n::t(app, "app.modals.preflight.title_install"),
        PreflightAction::Remove => i18n::t(app, "app.modals.preflight.title_remove"),
    };
    let border_color = th.lavender;
    let bg_color = th.crust;

    // Build header tab labels and calculate tab rectangles for mouse clicks
    let tab_labels = [
        i18n::t(app, "app.modals.preflight.tabs.summary"),
        i18n::t(app, "app.modals.preflight.tabs.deps"),
        i18n::t(app, "app.modals.preflight.tabs.files"),
        i18n::t(app, "app.modals.preflight.tabs.services"),
        i18n::t(app, "app.modals.preflight.tabs.sandbox"),
    ];
    let mut header = String::new();
    let current_tab = *tab;

    // Calculate tab rectangles for mouse click detection
    // Tab header is on the second line of content_rect (after border + chips line)
    let tab_y = content_rect.y + 2; // +1 for top border + 1 for chips line
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
    // Content area starts after the header (3 lines: chips + tabs + empty line)
    app.preflight_content_rect = Some((
        content_rect.x + 1,                    // +1 for left border
        content_rect.y + 4,                    // +1 for top border + 3 for header lines
        content_rect.width.saturating_sub(2),  // -2 for borders
        content_rect.height.saturating_sub(4), // -1 for top border - 3 for header lines
    ));

    let mut lines: Vec<Line<'static>> = Vec::new();
    // Header chips line
    lines.push(render_header_chips(app, header_chips));
    // Tab header line
    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    match current_tab {
        PreflightTab::Summary => {
            if let Some(summary_data) = summary.as_ref() {
                // Header chips already display package count, download size, install delta, and risk score
                // So we skip those here and focus on detailed information
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
                            Source::Official { repo, .. } => {
                                entry.push_str(&format!(" [{}]", repo))
                            }
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
            }
            match *action {
                PreflightAction::Install if !dependency_info.is_empty() => {
                    // Filter dependencies to only show conflicts and upgrades
                    let important_deps: Vec<&DependencyInfo> = dependency_info
                        .iter()
                        .filter(|d| {
                            matches!(
                                d.status,
                                DependencyStatus::Conflict { .. }
                                    | DependencyStatus::ToUpgrade { .. }
                            )
                        })
                        .collect();

                    if important_deps.is_empty() {
                        lines.push(Line::from(Span::styled(
                            i18n::t(app, "app.modals.preflight.summary.no_conflicts_or_upgrades"),
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

                        // Use different header based on what we have
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
                                                let (source_badge, badge_color) = match &dep.source
                                                {
                                                    DependencySource::Official { repo } => {
                                                        let repo_lower = repo.to_lowercase();
                                                        let color = if crate::index::is_eos_repo(
                                                            &repo_lower,
                                                        )
                                                            || crate::index::is_cachyos_repo(
                                                                &repo_lower,
                                                            ) {
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
                                                let (source_badge, badge_color) = match &dep.source
                                                {
                                                    DependencySource::Official { repo } => {
                                                        let repo_lower = repo.to_lowercase();
                                                        let color = if crate::index::is_eos_repo(
                                                            &repo_lower,
                                                        )
                                                            || crate::index::is_cachyos_repo(
                                                                &repo_lower,
                                                            ) {
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
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.summary.and_more",
                                    important_deps.len() - displayed,
                                ),
                                Style::default().fg(th.subtext1),
                            )));
                        }
                    }
                }
                PreflightAction::Remove => {
                    let mode = cascade_mode;
                    let mode_line = i18n::t_fmt(
                        app,
                        "app.modals.preflight.summary.cascade_mode",
                        &[&mode.flag(), &mode.description()],
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
                    } else {
                        let removal_names: Vec<&str> =
                            items.iter().map(|pkg| pkg.name.as_str()).collect();
                        let plan_header_style = Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD);
                        lines.push(Line::from(Span::styled(
                            i18n::t(app, "app.modals.preflight.summary.removal_plan_preview"),
                            plan_header_style,
                        )));

                        let mut plan_command = format!(
                            "sudo pacman {} --noconfirm {}",
                            mode.flag(),
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
                        } else if mode.allows_dependents() {
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
                                    i18n::t(
                                        app,
                                        "app.modals.preflight.summary.calculating_reverse_deps",
                                    ),
                                    Style::default().fg(th.subtext1),
                                )));
                            } else {
                                lines.push(Line::from(Span::styled(
                                    i18n::t(
                                        app,
                                        "app.modals.preflight.summary.removal_impact_overview",
                                    ),
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

                            let (impact_header, impact_style) = if mode.allows_dependents() {
                                (
                                    i18n::t(
                                        app,
                                        "app.modals.preflight.summary.cascade_will_remove",
                                    ),
                                    Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                                )
                            } else {
                                (
                                    i18n::t(
                                        app,
                                        "app.modals.preflight.summary.dependents_not_removed",
                                    ),
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
                                    DependencyStatus::ToUpgrade { .. } => i18n::t(
                                        app,
                                        "app.modals.preflight.summary.requires_version_change",
                                    ),
                                    DependencyStatus::Installed { .. } => i18n::t(
                                        app,
                                        "app.modals.preflight.summary.already_satisfied",
                                    ),
                                    DependencyStatus::ToInstall => i18n::t(
                                        app,
                                        "app.modals.preflight.summary.not_currently_installed",
                                    ),
                                    DependencyStatus::Missing => {
                                        i18n::t(app, "app.modals.preflight.summary.missing")
                                    }
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
                                    spans.push(Span::styled(
                                        " — ",
                                        Style::default().fg(th.subtext1),
                                    ));
                                    spans.push(Span::styled(
                                        detail,
                                        Style::default().fg(th.subtext1),
                                    ));
                                }
                                if !roots.is_empty() {
                                    spans.push(Span::styled(
                                        roots,
                                        Style::default().fg(th.overlay1),
                                    ));
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
                            if mode.allows_dependents() {
                                lines.push(Line::from(Span::styled(
                                    i18n::t(
                                        app,
                                        "app.modals.preflight.summary.will_be_removed_auto",
                                    ),
                                    Style::default().fg(th.subtext1),
                                )));
                            } else {
                                lines.push(Line::from(Span::styled(
                                    i18n::t(
                                        app,
                                        "app.modals.preflight.summary.enable_cascade_mode",
                                    ),
                                    Style::default().fg(th.subtext1),
                                )));
                            }
                            lines.push(Line::from(Span::styled(
                                i18n::t(app, "app.modals.preflight.summary.use_deps_tab"),
                                Style::default().fg(th.subtext1),
                            )));
                        }
                    }
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
        }
        PreflightTab::Deps => {
            // Use already resolved dependencies (resolved above if needed)
            let deps_empty = dependency_info.is_empty();
            let deps_count = dependency_info.len();
            let deps = dependency_info;

            // Group dependencies by the packages that require them
            // Deduplicate dependencies by name (a dependency can be required by multiple packages)
            use std::collections::HashMap;
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
                    use std::collections::HashSet;
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
                        use std::collections::HashSet;
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
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.deps.installed_version",
                                    version,
                                ),
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
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.deps.conflict_reason",
                                    reason,
                                ),
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
        }
        PreflightTab::Files => {
            let is_resolving = app.preflight_files_resolving || app.files_resolving;

            if is_resolving {
                // Show package headers first, then loading message
                for item in items.iter() {
                    let mut spans = Vec::new();
                    spans.push(Span::styled(
                        format!("▶ {} ", item.name),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled("(0 files)", Style::default().fg(th.subtext1)));
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.files.updating"),
                    Style::default().fg(th.yellow),
                )));
            } else if let Some(err_msg) = files_error {
                // Display error with retry hint
                lines.push(Line::from(Span::styled(
                    i18n::t_fmt1(app, "app.modals.preflight.files.error", err_msg),
                    Style::default().fg(th.red),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.files.retry_hint"),
                    Style::default().fg(th.subtext1),
                )));
            } else if file_info.is_empty() {
                // Show package headers first, then resolving message
                for item in items.iter() {
                    let mut spans = Vec::new();
                    spans.push(Span::styled(
                        format!("▶ {} ", item.name),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled("(0 files)", Style::default().fg(th.subtext1)));
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.files.resolving"),
                    Style::default().fg(th.subtext1),
                )));
            } else {
                // Build flat list of display items: package headers + files (only if expanded)
                // Store owned data to avoid lifetime issues
                // Performance: This builds the full display list, but only visible items are rendered
                // below. For very large file lists (thousands of files), consider lazy building or caching.
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
                // Check if file database is stale (older than 7 days)
                const STALE_THRESHOLD_DAYS: u64 = 7;
                let is_stale = crate::logic::files::is_file_db_stale(STALE_THRESHOLD_DAYS);

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
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.files.no_file_changes",
                                    unresolved_packages.len(),
                                ),
                                Style::default().fg(th.subtext1),
                            )));
                            lines.push(Line::from(""));

                            // Show appropriate notes based on package types
                            if has_official_packages {
                                lines.push(Line::from(Span::styled(
                                    i18n::t(app, "app.modals.preflight.files.file_db_sync_note"),
                                    Style::default().fg(th.subtext0),
                                )));
                                lines.push(Line::from(Span::styled(
                                    i18n::t(app, "app.modals.preflight.files.sync_file_db_hint"),
                                    Style::default().fg(th.subtext0),
                                )));
                            }
                            if has_aur_packages {
                                lines.push(Line::from(Span::styled(
                                    i18n::t(app, "app.modals.preflight.files.aur_file_note"),
                                    Style::default().fg(th.subtext0),
                                )));
                            }
                        } else {
                            lines.push(Line::from(Span::styled(
                                i18n::t(app, "app.modals.preflight.files.no_file_changes_display"),
                                Style::default().fg(th.subtext1),
                            )));
                        }
                    } else {
                        // File resolution hasn't completed or failed
                        lines.push(Line::from(Span::styled(
                            i18n::t(app, "app.modals.preflight.files.file_resolution_progress"),
                            Style::default().fg(th.subtext1),
                        )));
                    }

                    // Show stale file database warning if applicable
                    if let Some(true) = is_stale {
                        lines.push(Line::from(""));
                        lines.push(Line::from(Span::styled(
                            i18n::t(app, "app.modals.preflight.files.file_db_stale"),
                            Style::default().fg(th.yellow),
                        )));
                        lines.push(Line::from(Span::styled(
                            i18n::t(app, "app.modals.preflight.files.sync_file_db_root"),
                            Style::default().fg(th.subtext0),
                        )));
                    }

                    // Show file database sync timestamp
                    if let Some((_age_days, date_str, color_category)) = sync_info.clone() {
                        lines.push(Line::from(""));
                        let (sync_color, sync_text) = match color_category {
                            0 => (
                                th.green,
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.files.files_updated_on",
                                    date_str,
                                ),
                            ),
                            1 => (
                                th.yellow,
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.files.files_updated_on",
                                    date_str,
                                ),
                            ),
                            _ => (
                                th.red,
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.files.files_updated_on",
                                    date_str,
                                ),
                            ),
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

                    let mut summary_parts = vec![i18n::t_fmt1(
                        app,
                        "app.modals.preflight.files.total",
                        total_files,
                    )];
                    if total_new > 0 {
                        summary_parts.push(i18n::t_fmt1(
                            app,
                            "app.modals.preflight.files.new",
                            total_new,
                        ));
                    }
                    if total_changed > 0 {
                        summary_parts.push(i18n::t_fmt1(
                            app,
                            "app.modals.preflight.files.changed",
                            total_changed,
                        ));
                    }
                    if total_removed > 0 {
                        summary_parts.push(i18n::t_fmt1(
                            app,
                            "app.modals.preflight.files.removed",
                            total_removed,
                        ));
                    }
                    if total_config > 0 {
                        summary_parts.push(i18n::t_fmt1(
                            app,
                            "app.modals.preflight.files.config",
                            total_config,
                        ));
                    }
                    if total_pacnew > 0 {
                        summary_parts.push(i18n::t_fmt1(
                            app,
                            "app.modals.preflight.files.pacnew",
                            total_pacnew,
                        ));
                    }
                    if total_pacsave > 0 {
                        summary_parts.push(i18n::t_fmt1(
                            app,
                            "app.modals.preflight.files.pacsave",
                            total_pacsave,
                        ));
                    }

                    lines.push(Line::from(Span::styled(
                        i18n::t_fmt1(
                            app,
                            "app.modals.preflight.files.files_label",
                            summary_parts.join(", "),
                        ),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(""));

                    // Show file database sync timestamp
                    let sync_timestamp_lines =
                        if let Some((_age_days, date_str, color_category)) = sync_info.clone() {
                            let (sync_color, sync_text) = match color_category {
                                0 => (
                                    th.green,
                                    i18n::t_fmt1(
                                        app,
                                        "app.modals.preflight.files.files_updated_on",
                                        date_str,
                                    ),
                                ),
                                1 => (
                                    th.yellow,
                                    i18n::t_fmt1(
                                        app,
                                        "app.modals.preflight.files.files_updated_on",
                                        date_str,
                                    ),
                                ),
                                _ => (
                                    th.red,
                                    i18n::t_fmt1(
                                        app,
                                        "app.modals.preflight.files.files_updated_on",
                                        date_str,
                                    ),
                                ),
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
                    // Performance optimization: Only render visible items (viewport-based rendering)
                    // This prevents performance issues with large file lists
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

                    // Display files with scrolling (only render visible items)
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
                                    format!(
                                        ", {}",
                                        i18n::t_fmt1(
                                            app,
                                            "app.modals.preflight.files.new",
                                            pkg_info.new_count
                                        )
                                    ),
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
                            i18n::t_fmt(
                                app,
                                "app.modals.preflight.files.showing_range_items",
                                &[
                                    &(start_idx + 1).to_string(),
                                    &end_idx.to_string(),
                                    &total_items.to_string(),
                                ],
                            ),
                            Style::default().fg(th.subtext1),
                        )));
                    }
                }
            }
        }
        PreflightTab::Services => {
            if app.services_resolving {
                lines.push(Line::from(Span::styled(
                    "Updating service impact data…",
                    Style::default().fg(th.yellow),
                )));
            } else if let Some(err_msg) = services_error {
                // Display error with retry hint
                lines.push(Line::from(Span::styled(
                    format!("⚠ Error: {}", err_msg),
                    Style::default().fg(th.red),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press 'r' to retry service resolution",
                    Style::default().fg(th.subtext1),
                )));
            } else if !*services_loaded {
                lines.push(Line::from(Span::styled(
                    "Gathering service impact data…",
                    Style::default().fg(th.subtext1),
                )));
            } else if service_info.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No systemd services require attention.",
                    Style::default().fg(th.green),
                )));
            } else {
                // Performance optimization: Only render visible items (viewport-based rendering)
                // This prevents performance issues with large service lists
                let available_height = content_rect.height.saturating_sub(6) as usize;
                let visible = available_height.max(1);
                let selected = (*service_selected).min(service_info.len().saturating_sub(1));
                if *service_selected != selected {
                    *service_selected = selected;
                }
                let start = if service_info.len() <= visible {
                    0
                } else {
                    selected
                        .saturating_sub(visible / 2)
                        .min(service_info.len() - visible)
                };
                let end = (start + visible).min(service_info.len());
                // Render only visible services (viewport-based rendering)
                for (idx, svc) in service_info
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take(end - start)
                {
                    let is_selected = idx == selected;
                    let mut spans = Vec::new();
                    let name_style = if is_selected {
                        Style::default()
                            .fg(th.crust)
                            .bg(th.sapphire)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(th.text)
                    };
                    spans.push(Span::styled(svc.unit_name.clone(), name_style));
                    spans.push(Span::raw(" "));
                    let status_span = if svc.is_active {
                        if svc.needs_restart {
                            Span::styled(
                                i18n::t(
                                    app,
                                    "app.modals.preflight.services.active_restart_recommended",
                                ),
                                Style::default().fg(th.yellow),
                            )
                        } else {
                            Span::styled(
                                i18n::t(app, "app.modals.preflight.services.active"),
                                Style::default().fg(th.green),
                            )
                        }
                    } else {
                        Span::styled(
                            i18n::t(app, "app.modals.preflight.services.inactive"),
                            Style::default().fg(th.subtext1),
                        )
                    };
                    spans.push(status_span);
                    spans.push(Span::raw(" "));
                    let decision_span = match svc.restart_decision {
                        ServiceRestartDecision::Restart => Span::styled(
                            i18n::t(app, "app.modals.preflight.services.restart"),
                            Style::default().fg(th.green),
                        ),
                        ServiceRestartDecision::Defer => Span::styled(
                            i18n::t(app, "app.modals.preflight.services.defer"),
                            Style::default().fg(th.yellow),
                        ),
                    };
                    spans.push(decision_span);
                    if !svc.providers.is_empty() {
                        spans.push(Span::raw(" • "));
                        spans.push(Span::styled(
                            svc.providers.join(", "),
                            Style::default().fg(th.overlay1),
                        ));
                    }
                    lines.push(Line::from(spans));
                }
                if end < service_info.len() {
                    lines.push(Line::from(Span::styled(
                        i18n::t_fmt1(
                            app,
                            "app.modals.preflight.services.more_services",
                            service_info.len() - end,
                        ),
                        Style::default().fg(th.subtext1),
                    )));
                }
            }
        }
        PreflightTab::Sandbox => {
            // Show all packages, but only analyze AUR packages
            let aur_items: Vec<_> = items
                .iter()
                .filter(|p| matches!(p.source, crate::state::Source::Aur))
                .collect();

            // Use cached sandbox info if available
            // Note: Cached sandbox info is pre-populated when modal opens, so this only runs if cache was empty
            // Note: Sandbox resolution is triggered asynchronously in event handlers, not during rendering
            if matches!(*action, PreflightAction::Install) {
                // Check if we have cached sandbox info from app state that matches current items
                let item_names: std::collections::HashSet<String> =
                    items.iter().map(|i| i.name.clone()).collect();
                let cached_sandbox: Vec<_> = app
                    .install_list_sandbox
                    .iter()
                    .filter(|s| item_names.contains(&s.package_name))
                    .cloned()
                    .collect();
                // Sync results from background resolution if available (always sync when on Sandbox tab)
                // Always sync cached data to sandbox_info when available
                if !cached_sandbox.is_empty() {
                    // Always update if sandbox_info is empty, or if content differs
                    let needs_update = sandbox_info.is_empty()
                        || cached_sandbox.len() != sandbox_info.len()
                        || cached_sandbox.iter().any(|cached| {
                            !sandbox_info
                                .iter()
                                .any(|existing| existing.package_name == cached.package_name)
                        });
                    if needs_update {
                        tracing::debug!(
                            "[UI] Syncing {} sandbox info entries from background resolution to Preflight modal",
                            cached_sandbox.len()
                        );
                        *sandbox_info = cached_sandbox;
                        *sandbox_loaded = true;
                    }
                }
                // If sandbox_info is empty and we haven't loaded yet, check cache or trigger resolution
                if sandbox_info.is_empty()
                    && !*sandbox_loaded
                    && !app.preflight_sandbox_resolving
                    && !app.sandbox_resolving
                {
                    // Check if cache file exists with matching signature (even if empty)
                    let sandbox_cache_start = std::time::Instant::now();
                    let signature = crate::app::sandbox_cache::compute_signature(items);
                    let sandbox_cache_exists =
                        crate::app::sandbox_cache::load_cache(&app.sandbox_cache_path, &signature)
                            .is_some();
                    let sandbox_cache_duration = sandbox_cache_start.elapsed();
                    if sandbox_cache_duration.as_millis() > 10 {
                        tracing::warn!(
                            "[UI] Sandbox cache check took {:?} (slow!)",
                            sandbox_cache_duration
                        );
                    }
                    if sandbox_cache_exists {
                        // Cache exists but is empty - this is valid, means no sandbox info found
                        // But don't mark as loaded if resolution is still in progress
                        if !app.preflight_sandbox_resolving && !app.sandbox_resolving {
                            tracing::debug!(
                                "[UI] Using cached sandbox info (empty - no sandbox info found)"
                            );
                            *sandbox_loaded = true;
                        }
                    } else if aur_items.is_empty() {
                        // No AUR packages, mark as loaded
                        *sandbox_loaded = true;
                    } else {
                        // Check if background resolution is in progress
                        if app.preflight_sandbox_resolving || app.sandbox_resolving {
                            // Background resolution in progress - UI will show loading state
                            tracing::debug!(
                                "[UI] Sandbox tab: background resolution in progress, items={:?}",
                                items.iter().map(|i| &i.name).collect::<Vec<_>>()
                            );
                            // Don't mark as loaded - keep showing loading state
                        }
                        // If no cached sandbox info available, resolution will be triggered by event handlers when user navigates to Sandbox tab
                        // Don't mark as loaded yet - wait for resolution to complete
                    }
                }
                // Also check if we have sandbox_info already populated (from previous sync or initial load)
                // This ensures we show data even if cached_sandbox is empty but sandbox_info has data
                // But don't mark as loaded if resolution is still in progress
                if !sandbox_info.is_empty()
                    && !*sandbox_loaded
                    && !app.preflight_sandbox_resolving
                    && !app.sandbox_resolving
                {
                    *sandbox_loaded = true;
                }
            } else if aur_items.is_empty() {
                // No AUR packages, mark as loaded
                *sandbox_loaded = true;
            }
            // For remove actions or when sandbox is resolving, resolution will be triggered by event handlers

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
            } else if !*sandbox_loaded || sandbox_info.is_empty() {
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
                        && let Some(info) =
                            sandbox_info.iter().find(|s| s.package_name == item.name)
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
                let sandbox_selected_clamped =
                    (*sandbox_selected).min(total_items.saturating_sub(1));
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

                        let mut header_text = format!(
                            "Package: {} ({})",
                            pkg_name,
                            match &item.source {
                                crate::state::Source::Aur => "AUR",
                                crate::state::Source::Official { repo, .. } => repo,
                            }
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
                            if let Some(info) =
                                sandbox_info.iter().find(|s| s.package_name == *pkg_name)
                            {
                                let dep_count = info.depends.len()
                                    + info.makedepends.len()
                                    + info.checkdepends.len()
                                    + info.optdepends.len();
                                if dep_count > 0 {
                                    lines.push(Line::from(Span::styled(
                                        format!(
                                            "  {}",
                                            i18n::t_fmt1(app, "app.modals.preflight.sandbox.dependencies_expand_hint", dep_count.to_string())
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
                                "depends" => i18n::t(
                                    app,
                                    "app.modals.preflight.sandbox.runtime_dependencies",
                                ),
                                "makedepends" => {
                                    i18n::t(app, "app.modals.preflight.sandbox.build_dependencies")
                                }
                                "checkdepends" => {
                                    i18n::t(app, "app.modals.preflight.sandbox.test_dependencies")
                                }
                                "optdepends" => i18n::t(
                                    app,
                                    "app.modals.preflight.sandbox.optional_dependencies",
                                ),
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
                        "No packages in this transaction.",
                        Style::default().fg(th.subtext0),
                    )));
                }
            }
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
                i18n::t(app, "app.modals.preflight.footer_hints.deps_with_aur")
            } else {
                i18n::t(app, "app.modals.preflight.footer_hints.deps_without_aur")
            }
        }
        PreflightTab::Files => {
            if has_aur {
                i18n::t(app, "app.modals.preflight.footer_hints.files_with_aur")
            } else {
                i18n::t(app, "app.modals.preflight.footer_hints.files_without_aur")
            }
        }
        PreflightTab::Services => {
            if has_aur {
                i18n::t(app, "app.modals.preflight.footer_hints.services_with_aur")
            } else {
                i18n::t(
                    app,
                    "app.modals.preflight.footer_hints.services_without_aur",
                )
            }
        }
        _ => {
            if has_aur {
                i18n::t(app, "app.modals.preflight.footer_hints.default_with_aur")
            } else {
                i18n::t(app, "app.modals.preflight.footer_hints.default_without_aur")
            }
        }
    };

    if matches!(*action, PreflightAction::Remove) {
        scan_hint.push_str(&i18n::t(
            app,
            "app.modals.preflight.footer_hints.cascade_mode",
        ));
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
    let render_duration = render_start.elapsed();
    if render_duration.as_millis() > 50 {
        tracing::warn!("[UI] render_preflight took {:?} (slow!)", render_duration);
    } else {
        tracing::debug!("[UI] render_preflight completed in {:?}", render_duration);
    }
}
